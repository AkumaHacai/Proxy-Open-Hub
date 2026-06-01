using System.Runtime.InteropServices;
using System.Text;

namespace TrustTunnel.Core.Platform;

public sealed record NativeTrustTunnelProbe(
    string BaseDirectory,
    string VpnEasyDllPath,
    bool VpnEasyDllFound,
    string CliExePath,
    bool CliExeFound,
    string WintunDllPath,
    bool WintunDllFound,
    string ServiceExePath,
    bool ServiceExeFound)
{
    public bool DirectCoreReady => VpnEasyDllFound;
    public bool ServiceCoreReady => VpnEasyDllFound && ServiceExeFound;
}

public static class NativeTrustTunnelRuntimeInfo
{
    public const string VpnEasyDllName = "vpn_easy.dll";
    public const string CliExeName = "trusttunnel_client.exe";
    public const string WintunDllName = "wintun.dll";
    public const string ServiceExeName = "vpn_easy_service.exe";

    public static NativeTrustTunnelProbe Probe(string? baseDirectory = null)
    {
        var directory = Path.GetFullPath(baseDirectory ?? AppContext.BaseDirectory);
        var vpnEasyPath = Path.Combine(directory, VpnEasyDllName);
        var cliPath = Path.Combine(directory, CliExeName);
        var wintunPath = Path.Combine(directory, WintunDllName);
        var servicePath = Path.Combine(directory, ServiceExeName);

        return new NativeTrustTunnelProbe(
            directory,
            vpnEasyPath,
            File.Exists(vpnEasyPath),
            cliPath,
            File.Exists(cliPath),
            wintunPath,
            File.Exists(wintunPath),
            servicePath,
            File.Exists(servicePath));
    }
}

internal enum NativeVpnSessionState
{
    Disconnected = 0,
    Connecting = 1,
    Connected = 2,
    WaitingRecovery = 3,
    Recovering = 4,
    WaitingForNetwork = 5
}

internal enum NativeVpnServiceError
{
    None = 0,
    AccessDenied = 1,
    ServiceExists = 2,
    NoSuchService = 3,
    TimedOut = 4,
    Other = 5
}

internal sealed class NativeTrustTunnelRuntime : IDisposable
{
    private readonly IntPtr _libraryHandle;
    private readonly StateChangedCallback _stateChangedCallback;
    private readonly Action<int> _onStateChanged;
    private readonly VpnEasyStartDelegate _start;
    private readonly VpnEasyStopDelegate _stop;
    private readonly VpnEasyStartExDelegate? _startEx;
    private readonly VpnEasyStopExDelegate? _stopEx;
    private readonly ServiceInstallDelegate? _serviceInstall;
    private readonly ServiceStartDelegate? _serviceStart;
    private readonly ServiceStopDelegate? _serviceStop;
    private IntPtr _vpnHandle;
    private Utf8NativeString? _activeConfig;
    private bool _disposed;

    private NativeTrustTunnelRuntime(IntPtr libraryHandle, Action<int> onStateChanged)
    {
        _libraryHandle = libraryHandle;
        _onStateChanged = onStateChanged;
        _stateChangedCallback = NativeStateChanged;
        _start = GetRequiredExport<VpnEasyStartDelegate>(libraryHandle, "vpn_easy_start");
        _stop = GetRequiredExport<VpnEasyStopDelegate>(libraryHandle, "vpn_easy_stop");
        _startEx = TryGetExport<VpnEasyStartExDelegate>(libraryHandle, "vpn_easy_start_ex");
        _stopEx = TryGetExport<VpnEasyStopExDelegate>(libraryHandle, "vpn_easy_stop_ex");
        _serviceInstall = TryGetExport<ServiceInstallDelegate>(libraryHandle, "vpn_easy_service_install");
        _serviceStart = TryGetExport<ServiceStartDelegate>(libraryHandle, "vpn_easy_service_start");
        _serviceStop = TryGetExport<ServiceStopDelegate>(libraryHandle, "vpn_easy_service_stop");
    }

    public static NativeTrustTunnelRuntime Load(string baseDirectory, Action<int> onStateChanged)
    {
        var probe = NativeTrustTunnelRuntimeInfo.Probe(baseDirectory);
        if (!probe.VpnEasyDllFound)
        {
            throw new VpnException(VpnErrorCode.CoreUnavailableError, $"Native TrustTunnel core was not found: {probe.VpnEasyDllPath}");
        }

        try
        {
            var handle = NativeLibrary.Load(probe.VpnEasyDllPath);
            try
            {
                return new NativeTrustTunnelRuntime(handle, onStateChanged);
            }
            catch
            {
                NativeLibrary.Free(handle);
                throw;
            }
        }
        catch (Exception ex) when (ex is DllNotFoundException or BadImageFormatException or EntryPointNotFoundException)
        {
            throw new VpnException(VpnErrorCode.CoreUnavailableError, $"Cannot load TrustTunnel native core from {probe.VpnEasyDllPath}: {ex.Message}");
        }
    }

    public void StartDirect(string tomlConfig)
    {
        ThrowIfDisposed();
        _activeConfig?.Dispose();
        _activeConfig = new Utf8NativeString(tomlConfig);

        try
        {
            if (_startEx is not null && _stopEx is not null)
            {
                _vpnHandle = _startEx(_activeConfig.Pointer, _stateChangedCallback, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero);
                if (_vpnHandle == IntPtr.Zero)
                {
                    ClearActiveConfig();
                    throw new VpnException(VpnErrorCode.NativeStartFailed, "TrustTunnel native core rejected the generated TOML config.");
                }

                return;
            }

            _start(_activeConfig.Pointer, _stateChangedCallback, IntPtr.Zero);
        }
        catch
        {
            ClearActiveConfig();
            throw;
        }
    }

    public void StopDirect()
    {
        ThrowIfDisposed();
        if (_vpnHandle != IntPtr.Zero && _stopEx is not null)
        {
            _stopEx(_vpnHandle);
            _vpnHandle = IntPtr.Zero;
            ClearActiveConfig();
            return;
        }

        _stop();
        ClearActiveConfig();
    }

    public NativeVpnServiceError InstallService(
        string imagePath,
        string logFilePath,
        string pipeName,
        string serviceName,
        string displayName,
        string description)
    {
        ThrowIfDisposed();
        if (_serviceInstall is null)
        {
            return NativeVpnServiceError.Other;
        }

        return (NativeVpnServiceError)_serviceInstall(imagePath, logFilePath, pipeName, serviceName, displayName, description);
    }

    public NativeVpnServiceError StartService(string serviceName, string pipeName, string tomlConfig)
    {
        ThrowIfDisposed();
        if (_serviceStart is null)
        {
            return NativeVpnServiceError.Other;
        }

        using var config = new Utf8NativeString(tomlConfig);
        return (NativeVpnServiceError)_serviceStart(serviceName, pipeName, config.Pointer, _stateChangedCallback, IntPtr.Zero);
    }

    public NativeVpnServiceError StopService(string serviceName, string pipeName)
    {
        ThrowIfDisposed();
        if (_serviceStop is null)
        {
            return NativeVpnServiceError.Other;
        }

        return (NativeVpnServiceError)_serviceStop(serviceName, pipeName);
    }

    public void Dispose()
    {
        if (_disposed)
        {
            return;
        }

        if (_vpnHandle != IntPtr.Zero && _stopEx is not null)
        {
            _stopEx(_vpnHandle);
            _vpnHandle = IntPtr.Zero;
        }

        ClearActiveConfig();
        NativeLibrary.Free(_libraryHandle);
        _disposed = true;
    }

    private void ClearActiveConfig()
    {
        _activeConfig?.Dispose();
        _activeConfig = null;
    }

    private void NativeStateChanged(IntPtr arg, int state)
    {
        _onStateChanged(state);
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(_disposed, this);
    }

    private static T GetRequiredExport<T>(IntPtr libraryHandle, string name) where T : Delegate
    {
        if (!NativeLibrary.TryGetExport(libraryHandle, name, out var symbol))
        {
            throw new EntryPointNotFoundException($"Export '{name}' was not found.");
        }

        return Marshal.GetDelegateForFunctionPointer<T>(symbol);
    }

    private static T? TryGetExport<T>(IntPtr libraryHandle, string name) where T : Delegate
    {
        return NativeLibrary.TryGetExport(libraryHandle, name, out var symbol)
            ? Marshal.GetDelegateForFunctionPointer<T>(symbol)
            : null;
    }

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate void StateChangedCallback(IntPtr arg, int state);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate void VpnEasyStartDelegate(IntPtr tomlConfig, StateChangedCallback callback, IntPtr callbackArg);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate void VpnEasyStopDelegate();

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate IntPtr VpnEasyStartExDelegate(
        IntPtr tomlConfig,
        StateChangedCallback stateChangedCallback,
        IntPtr stateChangedCallbackArg,
        IntPtr connectionInfoCallback,
        IntPtr connectionInfoCallbackArg);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate void VpnEasyStopExDelegate(IntPtr vpn);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl, CharSet = CharSet.Unicode)]
    private delegate int ServiceInstallDelegate(
        [MarshalAs(UnmanagedType.LPWStr)] string imagePath,
        [MarshalAs(UnmanagedType.LPWStr)] string logFilePath,
        [MarshalAs(UnmanagedType.LPWStr)] string pipeName,
        [MarshalAs(UnmanagedType.LPWStr)] string serviceName,
        [MarshalAs(UnmanagedType.LPWStr)] string displayName,
        [MarshalAs(UnmanagedType.LPWStr)] string description);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl, CharSet = CharSet.Unicode)]
    private delegate int ServiceStartDelegate(
        [MarshalAs(UnmanagedType.LPWStr)] string serviceName,
        [MarshalAs(UnmanagedType.LPWStr)] string pipeName,
        IntPtr tomlConfig,
        StateChangedCallback callback,
        IntPtr callbackArg);

    [UnmanagedFunctionPointer(CallingConvention.Cdecl, CharSet = CharSet.Unicode)]
    private delegate int ServiceStopDelegate(
        [MarshalAs(UnmanagedType.LPWStr)] string serviceName,
        [MarshalAs(UnmanagedType.LPWStr)] string pipeName);

    private sealed class Utf8NativeString : IDisposable
    {
        public Utf8NativeString(string value)
        {
            var bytes = Encoding.UTF8.GetBytes(value);
            Pointer = Marshal.AllocHGlobal(bytes.Length + 1);
            Marshal.Copy(bytes, 0, Pointer, bytes.Length);
            Marshal.WriteByte(Pointer, bytes.Length, 0);
        }

        public IntPtr Pointer { get; }

        public void Dispose()
        {
            Marshal.FreeHGlobal(Pointer);
        }
    }
}
