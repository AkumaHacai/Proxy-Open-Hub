using System.Diagnostics;
using System.Text;
using TrustTunnel.Core.Diagnostics;
using TrustTunnel.Core.Models;

namespace TrustTunnel.Core.Platform;

internal sealed class TrustTunnelCliRuntime : IDisposable
{
    private readonly string _executablePath;
    private readonly RedactingLog _log;
    private readonly Action<int?> _onExited;
    private readonly Action<string> _onCoreLine;
    private readonly object _gate = new();
    private Process? _process;
    private string? _configPath;
    private bool _disposed;

    public TrustTunnelCliRuntime(string executablePath, RedactingLog log, Action<int?> onExited, Action<string> onCoreLine)
    {
        _executablePath = executablePath;
        _log = log;
        _onExited = onExited;
        _onCoreLine = onCoreLine;
    }

    public void Start(string tomlConfig, LogLevel logLevel)
    {
        ThrowIfDisposed();
        var runtimeDirectory = GetRuntimeDirectory();
        Directory.CreateDirectory(runtimeDirectory);
        _configPath = Path.Combine(runtimeDirectory, $"trusttunnel-{Guid.NewGuid():n}.toml");
        File.WriteAllText(_configPath, tomlConfig, new UTF8Encoding(false));

        var process = new Process
        {
            StartInfo = new ProcessStartInfo
            {
                FileName = _executablePath,
                Arguments = $"--config {Quote(_configPath)} --loglevel {ToCliLogLevel(logLevel)}",
                WorkingDirectory = Path.GetDirectoryName(_executablePath) ?? AppContext.BaseDirectory,
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true
            },
            EnableRaisingEvents = true
        };

        process.OutputDataReceived += (_, args) => LogCoreLine("core", args.Data);
        process.ErrorDataReceived += (_, args) => LogCoreLine("core-error", args.Data);
        process.Exited += (_, _) =>
        {
            CleanupConfig();
            _onExited(GetExitCode(process));
        };

        if (!process.Start())
        {
            CleanupConfig();
            throw new VpnException(VpnErrorCode.NativeStartFailed, "TrustTunnel CLI process did not start.");
        }

        process.BeginOutputReadLine();
        process.BeginErrorReadLine();

        lock (_gate)
        {
            _process = process;
        }

        if (process.WaitForExit(700))
        {
            var exitCode = GetExitCode(process);
            CleanupConfig();
            throw new VpnException(VpnErrorCode.NativeStartFailed, $"TrustTunnel CLI process exited during startup with code {exitCode}.");
        }
    }

    public void Stop()
    {
        Process? process;
        lock (_gate)
        {
            process = _process;
            _process = null;
        }

        if (process is not null)
        {
            try
            {
                if (!process.HasExited)
                {
                    process.Kill(entireProcessTree: true);
                    process.WaitForExit(5000);
                }
            }
            finally
            {
                process.Dispose();
            }
        }

        CleanupConfig();
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        Stop();
        _disposed = true;
    }

    private static string GetRuntimeDirectory()
    {
        var baseDirectory = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        if (string.IsNullOrWhiteSpace(baseDirectory))
        {
            baseDirectory = Path.GetTempPath();
        }

        return Path.Combine(baseDirectory, "TrustTunnel", "runtime");
    }

    private static string Quote(string value)
    {
        return "\"" + value.Replace("\"", "\\\"", StringComparison.Ordinal) + "\"";
    }

    private static string ToCliLogLevel(LogLevel logLevel)
    {
        return logLevel.ToString().ToLowerInvariant();
    }

    private static int? GetExitCode(Process process)
    {
        try
        {
            return process.HasExited ? process.ExitCode : null;
        }
        catch (InvalidOperationException)
        {
            return null;
        }
    }

    private void LogCoreLine(string level, string? line)
    {
        if (!string.IsNullOrWhiteSpace(line))
        {
            _log.Info($"{level}: {line}");
            _onCoreLine(line);
        }
    }

    private void CleanupConfig()
    {
        var path = Interlocked.Exchange(ref _configPath, null);
        if (string.IsNullOrWhiteSpace(path))
        {
            return;
        }

        try
        {
            File.Delete(path);
        }
        catch (IOException)
        {
        }
        catch (UnauthorizedAccessException)
        {
        }
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
    }
}
