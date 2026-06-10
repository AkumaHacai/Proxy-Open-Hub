using System.Security.Principal;
using System.Net;
using System.Net.Sockets;
using TrustTunnel.Core.Diagnostics;
using TrustTunnel.Core.Models;
using TrustTunnel.Core.Security;
using TrustTunnel.Core.State;
using TrustTunnel.Core.Toml;
using TrustTunnel.Core.Validation;

namespace TrustTunnel.Core.Platform;

public enum VpnErrorCode
{
    ConfigValidationError,
    TunPermissionError,
    WintunMissingError,
    CoreUnavailableError,
    NativeStartFailed,
    NativeServiceError
}

public sealed class VpnException : Exception
{
    public VpnException(VpnErrorCode code, string message) : base(message)
    {
        Code = code;
    }

    public VpnErrorCode Code { get; }
}

public interface IVpnController
{
    Task ConnectAsync(ServerProfile profile, CancellationToken cancellationToken = default);
    Task DisconnectAsync(CancellationToken cancellationToken = default);
}

public sealed class NativeBridgeVpnController : IVpnController, IDisposable
{
    private const string ServiceName = "TrustTunnelEasyService";
    private const string ServiceDisplayName = "TrustTunnel VPN Service";
    private const string ServicePipeName = @"\\.\pipe\trusttunnel-easy";
    private static readonly TimeSpan NativeStartTimeout = TimeSpan.FromSeconds(30);

    private readonly ConnectionStateStore _stateStore;
    private readonly RedactingLog _log;
    private readonly ISecretStore _secretStore;
    private readonly TrustTunnelTomlBuilder _tomlBuilder = new();
    private readonly object _stateLock = new();
    private NativeTrustTunnelRuntime? _runtime;
    private TrustTunnelCliRuntime? _cliRuntime;
    private CancellationTokenSource? _nativeWatchdogCts;
    private bool _serviceModeActive;
    private bool _disposed;
    private DateTimeOffset _lastNativeStateAt = DateTimeOffset.MinValue;
    private string _lastCliError = "";

    public NativeBridgeVpnController(ConnectionStateStore stateStore, RedactingLog log, ISecretStore secretStore)
    {
        _stateStore = stateStore;
        _log = log;
        _secretStore = secretStore;
    }

    public async Task ConnectAsync(ServerProfile profile, CancellationToken cancellationToken = default)
    {
        ObjectDisposedException.ThrowIf(_disposed, this);

        if (IsConnectionBusy(CurrentPhase))
        {
            throw new VpnException(VpnErrorCode.NativeStartFailed, "TrustTunnel is already connecting or connected.");
        }

        TryTransitionTo(ConnectionPhase.Preparing, "Validating profile", profile.DisplayName);
        var report = TrustTunnelValidators.Validate(profile);
        if (!report.IsValid)
        {
            TryTransitionTo(ConnectionPhase.Error, report.Issues.First(i => i.Severity == ValidationSeverity.Error).Message);
            throw new VpnException(VpnErrorCode.ConfigValidationError, "Profile validation failed.");
        }

        EnsureTunPrerequisites(profile);
        EnsureLocalListenerAvailable(profile);

        var toml = await BuildTomlAsync(profile, cancellationToken);
        var probe = NativeTrustTunnelRuntimeInfo.Probe(AppContext.BaseDirectory);

        TryTransitionTo(ConnectionPhase.Connecting, "Starting TrustTunnel native adapter", profile.DisplayName);
        _log.Info($"Connecting to {profile.Endpoint.Hostname} via {profile.Endpoint.UpstreamProtocol}");

        if (probe.VpnEasyDllFound)
        {
            await ConnectNativeAsync(profile, toml, cancellationToken);
            return;
        }

        if (probe.CliExeFound && !profile.ServiceModeEnabled)
        {
            await ConnectCliAsync(profile, toml, cancellationToken);
            return;
        }

        var message = profile.ServiceModeEnabled && !probe.VpnEasyDllFound
            ? $"Service mode requires {NativeTrustTunnelRuntimeInfo.VpnEasyDllName}; CLI fallback cannot control the service."
            : $"Native TrustTunnel core was not found. Expected {NativeTrustTunnelRuntimeInfo.VpnEasyDllName} or {NativeTrustTunnelRuntimeInfo.CliExeName} in {probe.BaseDirectory}.";
        TryTransitionTo(ConnectionPhase.Error, message, profile.DisplayName);
        throw new VpnException(VpnErrorCode.CoreUnavailableError, message);
    }

    private async Task ConnectNativeAsync(ServerProfile profile, string toml, CancellationToken cancellationToken)
    {
        NativeTrustTunnelRuntime runtime;
        try
        {
            runtime = NativeTrustTunnelRuntime.Load(AppContext.BaseDirectory, HandleNativeStateChanged);
        }
        catch (VpnException ex)
        {
            TryTransitionTo(ConnectionPhase.Error, ex.Message, profile.DisplayName);
            throw;
        }

        var startRequestedAt = DateTimeOffset.UtcNow;

        lock (_stateLock)
        {
            _runtime = runtime;
            _cliRuntime = null;
            _serviceModeActive = profile.ServiceModeEnabled;
            _lastNativeStateAt = DateTimeOffset.MinValue;
        }

        try
        {
            if (profile.ServiceModeEnabled)
            {
                await Task.Run(() => StartServiceMode(runtime, toml), cancellationToken);
            }
            else
            {
                await Task.Run(() => runtime.StartDirect(toml), cancellationToken);
            }

            StartNativeWatchdog(startRequestedAt, profile.DisplayName);
        }
        catch
        {
            ClearNativeRuntime(runtime);
            runtime.Dispose();
            TryTransitionTo(ConnectionPhase.Error, "TrustTunnel native adapter failed to start.", profile.DisplayName);
            throw;
        }
    }

    private async Task ConnectCliAsync(ServerProfile profile, string toml, CancellationToken cancellationToken)
    {
        var probe = NativeTrustTunnelRuntimeInfo.Probe(AppContext.BaseDirectory);
        var runtime = new TrustTunnelCliRuntime(probe.CliExePath, _log, HandleCliExited, HandleCliCoreLine);

        lock (_stateLock)
        {
            _runtime = null;
            _cliRuntime = runtime;
            _serviceModeActive = false;
            _lastCliError = "";
        }

        try
        {
            await Task.Run(() => runtime.Start(toml, LogLevel.Info), cancellationToken);
            TryTransitionTo(ConnectionPhase.Connecting, "TrustTunnel CLI core is running", profile.DisplayName);
            _log.Info("TrustTunnel CLI core started.");
        }
        catch
        {
            ClearCliRuntime(runtime);
            runtime.Dispose();
            TryTransitionTo(ConnectionPhase.Error, "TrustTunnel CLI core failed to start.", profile.DisplayName);
            throw;
        }
    }

    public async Task DisconnectAsync(CancellationToken cancellationToken = default)
    {
        if (_disposed)
        {
            return;
        }

        NativeTrustTunnelRuntime? runtime;
        TrustTunnelCliRuntime? cliRuntime;
        bool serviceMode;
        lock (_stateLock)
        {
            runtime = _runtime;
            cliRuntime = _cliRuntime;
            serviceMode = _serviceModeActive;
        }

        if (runtime is null && cliRuntime is null)
        {
            TryTransitionTo(ConnectionPhase.Disconnected, "Disconnected");
            _log.Info("Disconnected");
            return;
        }

        if (CurrentPhase != ConnectionPhase.Error)
        {
            TryTransitionTo(ConnectionPhase.Disconnecting, "Stopping tunnel");
        }

        try
        {
            await Task.Run(() =>
            {
                if (cliRuntime is not null)
                {
                    cliRuntime.Stop();
                }
                else if (runtime is not null && serviceMode)
                {
                    var error = runtime.StopService(ServiceName, ServicePipeName);
                    if (error != NativeVpnServiceError.None)
                    {
                        throw new VpnException(VpnErrorCode.NativeServiceError, $"TrustTunnel service stop failed: {DescribeServiceError(error)}");
                    }
                }
                else if (runtime is not null)
                {
                    runtime.StopDirect();
                }
            }, cancellationToken);

            TryTransitionTo(ConnectionPhase.Disconnected, "Disconnected");
            _log.Info("Disconnected");
        }
        finally
        {
            if (runtime is not null)
            {
                ClearNativeRuntime(runtime);
                runtime.Dispose();
            }

            if (cliRuntime is not null)
            {
                ClearCliRuntime(cliRuntime);
                cliRuntime.Dispose();
            }
        }
    }

    private async Task<string> BuildTomlAsync(ServerProfile profile, CancellationToken cancellationToken)
    {
        var password = await _secretStore.ReadAsync(profile.Endpoint.PasswordSecretRef, cancellationToken);
        if (string.IsNullOrWhiteSpace(password))
        {
            TryTransitionTo(ConnectionPhase.Error, "Endpoint password is missing.");
            throw new VpnException(VpnErrorCode.ConfigValidationError, "Endpoint password is missing.");
        }

        var clientRandom = string.IsNullOrWhiteSpace(profile.Endpoint.ClientRandomSecretRef)
            ? ""
            : await _secretStore.ReadAsync(profile.Endpoint.ClientRandomSecretRef, cancellationToken) ?? "";
        var socksPassword = string.IsNullOrWhiteSpace(profile.Listener.Socks.PasswordSecretRef)
            ? ""
            : await _secretStore.ReadAsync(profile.Listener.Socks.PasswordSecretRef, cancellationToken) ?? "";

        return _tomlBuilder.Build(new TrustTunnelConfig
        {
            Endpoint = profile.Endpoint,
            Listener = profile.Listener,
            Routing = profile.Routing
        }, password, clientRandom, socksPassword);
    }

    private static bool IsConnectionBusy(ConnectionPhase phase)
    {
        return phase is ConnectionPhase.Preparing
            or ConnectionPhase.Connecting
            or ConnectionPhase.Authenticating
            or ConnectionPhase.Connected
            or ConnectionPhase.Reconnecting
            or ConnectionPhase.Disconnecting;
    }

    private void EnsureTunPrerequisites(ServerProfile profile)
    {
        if (!OperatingSystem.IsWindows() || profile.Listener.Mode != ListenerMode.Tun)
        {
            return;
        }

        var probe = NativeTrustTunnelRuntimeInfo.Probe(AppContext.BaseDirectory);
        if (!probe.WintunDllFound)
        {
            TryTransitionTo(ConnectionPhase.Error, "wintun.dll was not found next to the executable.");
            throw new VpnException(VpnErrorCode.WintunMissingError, "wintun.dll is required for Windows TUN mode.");
        }

        if (!profile.ServiceModeEnabled && !IsAdministrator())
        {
            TryTransitionTo(ConnectionPhase.PermissionRequired, "Administrator rights are required for direct TUN mode.");
            throw new VpnException(VpnErrorCode.TunPermissionError, "Direct TUN mode requires Administrator rights. Enable service mode to run through the Windows service.");
        }
    }

    private void EnsureLocalListenerAvailable(ServerProfile profile)
    {
        if (profile.Listener.Mode != ListenerMode.Socks)
        {
            return;
        }

        EnsureSocketCanBind(profile.Listener.Socks.Address, "SOCKS5", profile.DisplayName);
        if (!string.IsNullOrWhiteSpace(profile.Listener.Socks.HttpProxyAddress))
        {
            EnsureSocketCanBind(profile.Listener.Socks.HttpProxyAddress, "HTTP proxy", profile.DisplayName);
        }
    }

    private void EnsureSocketCanBind(string address, string listenerName, string serverName)
    {
        if (!TryParseListenAddress(address, out var ipAddress, out var port))
        {
            return;
        }

        TcpListener? listener = null;
        try
        {
            listener = new TcpListener(ipAddress, port);
            listener.Start();
        }
        catch (SocketException ex) when (ex.SocketErrorCode == SocketError.AddressAlreadyInUse)
        {
            var message = $"{listenerName} address {address} is already in use. Change the port in profile settings or stop the other process.";
            TryTransitionTo(ConnectionPhase.Error, message, serverName);
            throw new VpnException(VpnErrorCode.ConfigValidationError, message);
        }
        finally
        {
            listener?.Stop();
        }
    }

    private static bool TryParseListenAddress(string value, out IPAddress ipAddress, out int port)
    {
        ipAddress = IPAddress.Loopback;
        port = 0;

        var separator = value.LastIndexOf(':');
        if (separator <= 0 || !int.TryParse(value[(separator + 1)..], out port))
        {
            return false;
        }

        var host = value[..separator].Trim('[', ']');
        if (host.Length == 0)
        {
            ipAddress = IPAddress.Loopback;
            return true;
        }

        if (!IPAddress.TryParse(host, out var parsed))
        {
            return false;
        }

        ipAddress = parsed;
        return true;
    }

    private void StartServiceMode(NativeTrustTunnelRuntime runtime, string toml)
    {
        var error = runtime.StartService(ServiceName, ServicePipeName, toml);
        if (error == NativeVpnServiceError.None)
        {
            return;
        }

        if (error != NativeVpnServiceError.NoSuchService)
        {
            throw new VpnException(VpnErrorCode.NativeServiceError, $"TrustTunnel service start failed: {DescribeServiceError(error)}");
        }

        var probe = NativeTrustTunnelRuntimeInfo.Probe(AppContext.BaseDirectory);
        if (!probe.ServiceExeFound)
        {
            throw new VpnException(VpnErrorCode.CoreUnavailableError, $"TrustTunnel service is not installed and {NativeTrustTunnelRuntimeInfo.ServiceExeName} was not found: {probe.ServiceExePath}");
        }

        var logPath = GetServiceLogPath();
        Directory.CreateDirectory(Path.GetDirectoryName(logPath)!);
        var installError = runtime.InstallService(
            probe.ServiceExePath,
            logPath,
            ServicePipeName,
            ServiceName,
            ServiceDisplayName,
            "Runs the TrustTunnel native VPN client for the desktop UI.");

        if (installError != NativeVpnServiceError.None && installError != NativeVpnServiceError.ServiceExists)
        {
            throw new VpnException(VpnErrorCode.NativeServiceError, $"TrustTunnel service install failed: {DescribeServiceError(installError)}");
        }

        error = runtime.StartService(ServiceName, ServicePipeName, toml);
        if (error != NativeVpnServiceError.None)
        {
            throw new VpnException(VpnErrorCode.NativeServiceError, $"TrustTunnel service start failed: {DescribeServiceError(error)}");
        }
    }

    private static string GetServiceLogPath()
    {
        var baseDirectory = Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData);
        if (string.IsNullOrWhiteSpace(baseDirectory))
        {
            baseDirectory = AppContext.BaseDirectory;
        }

        return Path.Combine(baseDirectory, "TrustTunnel", "vpn_easy_service.log");
    }

    private void HandleNativeStateChanged(int stateValue)
    {
        lock (_stateLock)
        {
            _lastNativeStateAt = DateTimeOffset.UtcNow;
        }
        CancelNativeWatchdog();

        var stateName = Enum.IsDefined(typeof(NativeVpnSessionState), stateValue)
            ? ((NativeVpnSessionState)stateValue).ToString()
            : $"Unknown({stateValue})";
        _log.Info($"Native TrustTunnel state: {stateName}");

        switch ((NativeVpnSessionState)stateValue)
        {
            case NativeVpnSessionState.Disconnected:
                if (CurrentPhase == ConnectionPhase.Disconnecting || CurrentPhase == ConnectionPhase.Disconnected || CurrentPhase == ConnectionPhase.Idle)
                {
                    TryTransitionTo(ConnectionPhase.Disconnected, "Native adapter disconnected");
                }
                else
                {
                    TryTransitionTo(ConnectionPhase.Error, "Native adapter disconnected before connection was established.");
                }
                break;

            case NativeVpnSessionState.Connecting:
                TryTransitionTo(ConnectionPhase.Connecting, "Native adapter is connecting");
                break;

            case NativeVpnSessionState.Connected:
                TryTransitionTo(ConnectionPhase.Connected, "Connected");
                break;

            case NativeVpnSessionState.WaitingRecovery:
            case NativeVpnSessionState.Recovering:
            case NativeVpnSessionState.WaitingForNetwork:
                if (!TryTransitionTo(ConnectionPhase.Reconnecting, "Native adapter is recovering"))
                {
                    TryTransitionTo(ConnectionPhase.Authenticating, "Native adapter is recovering");
                }
                break;

            default:
                _log.Error($"Unknown native TrustTunnel state: {stateValue}");
                break;
        }
    }

    private void HandleCliExited(int? exitCode)
    {
        var message = $"TrustTunnel CLI core exited with code {(exitCode?.ToString() ?? "unknown")}.";
        if (CurrentPhase == ConnectionPhase.Disconnecting || CurrentPhase == ConnectionPhase.Disconnected || CurrentPhase == ConnectionPhase.Idle)
        {
            _log.Info(message);
            TryTransitionTo(ConnectionPhase.Disconnected, "TrustTunnel CLI core stopped");
        }
        else if (CurrentPhase == ConnectionPhase.Error)
        {
            _log.Info(message);
        }
        else
        {
            _log.Error(message);
            TryTransitionTo(ConnectionPhase.Error, message);
        }
    }

    private void HandleCliCoreLine(string line)
    {
        if (line.Contains("Authorization Required", StringComparison.OrdinalIgnoreCase))
        {
            lock (_stateLock)
            {
                _lastCliError = "Authorization required. Check username/password.";
            }

            TryTransitionTo(ConnectionPhase.Error, "Authorization required. Check username/password.");
            return;
        }

        if (line.Contains("address in use", StringComparison.OrdinalIgnoreCase)
            || line.Contains("Failed to bind socket", StringComparison.OrdinalIgnoreCase)
            || line.Contains("10048", StringComparison.OrdinalIgnoreCase))
        {
            lock (_stateLock)
            {
                _lastCliError = "Local proxy port is already in use. Change the SOCKS5/HTTP proxy port or stop the other process.";
            }

            TryTransitionTo(ConnectionPhase.Error, "Local proxy port is already in use.");
            return;
        }

        if (line.Contains("VPN_SS_CONNECTING", StringComparison.Ordinal))
        {
            TryTransitionTo(ConnectionPhase.Connecting, "Native core is connecting");
            return;
        }

        if (line.Contains("VPN_SS_CONNECTED", StringComparison.Ordinal))
        {
            TryTransitionTo(ConnectionPhase.Connected, "Connected");
            return;
        }

        if (line.Contains("VPN_SS_DISCONNECTED", StringComparison.Ordinal))
        {
            string lastError;
            lock (_stateLock)
            {
                lastError = _lastCliError;
            }

            if (CurrentPhase == ConnectionPhase.Disconnecting || CurrentPhase == ConnectionPhase.Disconnected || CurrentPhase == ConnectionPhase.Idle)
            {
                TryTransitionTo(ConnectionPhase.Disconnected, "Disconnected");
            }
            else if (!string.IsNullOrWhiteSpace(lastError))
            {
                TryTransitionTo(ConnectionPhase.Error, lastError);
            }
            else
            {
                TryTransitionTo(ConnectionPhase.Error, "Native core disconnected before a stable connection was established.");
            }
        }
    }

    private void StartNativeWatchdog(DateTimeOffset startRequestedAt, string serverName)
    {
        CancelNativeWatchdog();
        var watchdogCts = new CancellationTokenSource();
        lock (_stateLock)
        {
            _nativeWatchdogCts = watchdogCts;
        }

        _ = WatchNativeStartAsync(startRequestedAt, serverName, watchdogCts.Token);
    }

    private async Task WatchNativeStartAsync(DateTimeOffset startRequestedAt, string serverName, CancellationToken cancellationToken)
    {
        try
        {
            await Task.Delay(NativeStartTimeout, cancellationToken);
        }
        catch (OperationCanceledException)
        {
            return;
        }

        DateTimeOffset lastStateAt;
        lock (_stateLock)
        {
            lastStateAt = _lastNativeStateAt;
        }

        if (lastStateAt < startRequestedAt && CurrentPhase == ConnectionPhase.Connecting)
        {
            TryTransitionTo(ConnectionPhase.Error, "Native adapter did not report a state within 30 seconds.", serverName);
            _log.Error("Native adapter did not report a state within 30 seconds.");
        }
    }

    private ConnectionPhase CurrentPhase
    {
        get
        {
            lock (_stateLock)
            {
                return _stateStore.Current.Phase;
            }
        }
    }

    private bool TryTransitionTo(ConnectionPhase next, string message = "", string serverName = "")
    {
        lock (_stateLock)
        {
            if (_stateStore.Current.Phase == next)
            {
                return true;
            }

            try
            {
                _stateStore.TransitionTo(next, message, serverName);
                return true;
            }
            catch (InvalidOperationException)
            {
                return false;
            }
        }
    }

    private void ClearNativeRuntime(NativeTrustTunnelRuntime runtime)
    {
        var cleared = false;
        lock (_stateLock)
        {
            if (ReferenceEquals(_runtime, runtime))
            {
                _runtime = null;
                _serviceModeActive = false;
                cleared = true;
            }
        }

        if (cleared)
        {
            CancelNativeWatchdog();
        }
    }

    private void ClearCliRuntime(TrustTunnelCliRuntime runtime)
    {
        lock (_stateLock)
        {
            if (ReferenceEquals(_cliRuntime, runtime))
            {
                _cliRuntime = null;
                _serviceModeActive = false;
            }
        }
    }

    private static string DescribeServiceError(NativeVpnServiceError error)
    {
        return error switch
        {
            NativeVpnServiceError.AccessDenied => "access denied; run once as Administrator to install or control the service",
            NativeVpnServiceError.ServiceExists => "service already exists",
            NativeVpnServiceError.NoSuchService => "service is not installed",
            NativeVpnServiceError.TimedOut => "operation timed out",
            NativeVpnServiceError.Other => "unexpected service error",
            _ => error.ToString()
        };
    }

    private static bool IsAdministrator()
    {
        if (!OperatingSystem.IsWindows())
        {
            return Environment.UserName == "root";
        }

        using var identity = WindowsIdentity.GetCurrent();
        return new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator);
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        _disposed = true;
        CancelNativeWatchdog();

        NativeTrustTunnelRuntime? runtime;
        TrustTunnelCliRuntime? cliRuntime;
        bool serviceMode;
        lock (_stateLock)
        {
            runtime = _runtime;
            cliRuntime = _cliRuntime;
            serviceMode = _serviceModeActive;
            _runtime = null;
            _cliRuntime = null;
            _serviceModeActive = false;
        }

        try
        {
            cliRuntime?.Dispose();
        }
        catch (Exception ex) when (ex is InvalidOperationException or System.ComponentModel.Win32Exception)
        {
            _log.Error($"TrustTunnel CLI dispose failed: {ex.Message}");
        }

        if (runtime is null)
        {
            return;
        }

        try
        {
            if (serviceMode)
            {
                _ = runtime.StopService(ServiceName, ServicePipeName);
            }
            else
            {
                runtime.StopDirect();
            }
        }
        catch (Exception ex) when (ex is VpnException or InvalidOperationException)
        {
            _log.Error($"TrustTunnel native dispose failed: {ex.Message}");
        }
        finally
        {
            runtime.Dispose();
        }
    }

    private void CancelNativeWatchdog()
    {
        CancellationTokenSource? watchdogCts;
        lock (_stateLock)
        {
            watchdogCts = _nativeWatchdogCts;
            _nativeWatchdogCts = null;
        }

        if (watchdogCts is null)
        {
            return;
        }

        watchdogCts.Cancel();
        watchdogCts.Dispose();
    }
}
