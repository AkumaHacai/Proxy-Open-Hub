using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Globalization;
using System.IO;
using System.Net.Http;
using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Animation;
using TrustTunnel.Core.Application;
using TrustTunnel.Core.Diagnostics;
using TrustTunnel.Core.Models;
using TrustTunnel.Core.Platform;
using TrustTunnel.Core.Security;
using TrustTunnel.Core.State;
using TrustTunnel.Core.Toml;
using TrustTunnel.Core.Validation;
using Drawing = System.Drawing;
using WinForms = System.Windows.Forms;

namespace TrustTunnel.Desktop;

public partial class MainWindow : Window
{
    private enum WindowMode
    {
        Expanded,
        Compact
    }

    private const double ExpandedWidth = 920;
    private const double CompactWidth = 336;
    private const double SidebarExpandedWidth = 316;
    private const double WindowHeight = 600;
    private const int DwmWindowCornerPreference = 33;
    private const int DwmRoundCorners = 2;

    private readonly ObservableCollection<ServerProfile> _profiles = new();
    private readonly ObservableCollection<RoutingProfile> _routingProfiles = new();
    private readonly ObservableCollection<DiagnosticResultView> _diagnostics = new();
    private readonly InMemorySecretStore _secretStore = new();
    private readonly TrustTunnelAppService _appService;
    private readonly ConnectionStateStore _stateStore = new();
    private readonly RedactingLog _log = new();
    private readonly TrustTunnelTomlBuilder _tomlBuilder = new();
    private readonly DesktopStateStore _storage = new();
    private readonly IVpnController _vpnController;
    private readonly GeoLookupService _geoLookup = new();
    private readonly TrafficMetricsService _trafficMetrics = new();
    private readonly WindowsSystemProxyService _systemProxy = new();
    private readonly EventHandler<ConnectionSnapshot> _stateChangedHandler;
    private readonly CancellationTokenSource _lifetimeCts = new();
    private WinForms.NotifyIcon? _trayIcon;
    private Drawing.Icon? _trayDrawingIcon;
    private AppSettings _settings = new();
    private bool _loadingUi;
    private string _diagnosticsProfileId = "";
    private ServerProfile? _contextProfile;
    private UIElement? _corePickerTarget;
    private TrafficMetricsSnapshot _currentTraffic = new(0, 0);
    private ServerProfile? _activeProfile;
    private WindowMode _currentMode = WindowMode.Expanded;
    private bool _connected;

    public sealed record DiagnosticResultView(string Name, string Status, string Message, string Elapsed);

    public MainWindow()
    {
        var persisted = _storage.Load();
        _settings = persisted.Settings;
        LocalizationManager.Instance.Apply(_settings.Language);

        InitializeComponent();
        InitializeTrayIcon();
        SourceInitialized += (_, _) => ApplyRoundedWindowCorners();
        PreviewMouseDown += MainWindow_PreviewMouseDown;
        Deactivated += (_, _) => CloseCorePicker();
        Width = ExpandedWidth;
        Height = WindowHeight;
        MinHeight = WindowHeight;
        MaxHeight = WindowHeight;
        MinWidth = CompactWidth;
        MaxWidth = ExpandedWidth;
        ResizeMode = ResizeMode.NoResize;

        _secretStore.Load(persisted.Secrets);
        DesktopTheme.Apply(_settings.Appearance);

        _appService = new TrustTunnelAppService(_secretStore);
        _vpnController = new NativeBridgeVpnController(_stateStore, _log, _secretStore);
        _stateChangedHandler = HandleStateChanged;
        _stateStore.Changed += _stateChangedHandler;
        _trafficMetrics.Updated += HandleTrafficMetricsUpdated;
        ProfilesList.ItemsSource = _profiles;
        RoutingProfileComboBox.DisplayMemberPath = nameof(RoutingProfile.Name);
        RoutingProfileComboBox.ItemsSource = _routingProfiles;
        DiagnosticsResultsList.ItemsSource = _diagnostics;
        LoadPersistedState(persisted);
        _ = EnsureLoadedSocksProfileDefaultsAsync();
        ApplyWindowMode(ParseWindowMode(_settings.MainWindowMode), animate: false);
        RefreshThemeToggleIcon(animate: false);
    }

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int pvAttribute, int cbAttribute);

    private void ApplyRoundedWindowCorners()
    {
        try
        {
            var handle = new WindowInteropHelper(this).Handle;
            if (handle == IntPtr.Zero)
            {
                return;
            }

            var preference = DwmRoundCorners;
            _ = DwmSetWindowAttribute(handle, DwmWindowCornerPreference, ref preference, sizeof(int));
        }
        catch (DllNotFoundException)
        {
        }
        catch (EntryPointNotFoundException)
        {
        }
    }

    private ServerProfile? SelectedProfile => ProfilesList.SelectedItem as ServerProfile;

    private void InitializeTrayIcon()
    {
        var iconPath = Path.Combine(AppContext.BaseDirectory, AppBrand.IconPath);
        _trayDrawingIcon = File.Exists(iconPath)
            ? new Drawing.Icon(iconPath)
            : Drawing.Icon.ExtractAssociatedIcon(Environment.ProcessPath ?? "");

        if (_trayDrawingIcon is null)
        {
            return;
        }

        var openItem = new WinForms.ToolStripMenuItem($"Open {AppBrand.DisplayName}");
        openItem.Click += (_, _) => Dispatcher.Invoke(ShowFromTray);

        var exitItem = new WinForms.ToolStripMenuItem("Exit");
        exitItem.Click += (_, _) => Dispatcher.Invoke(Close);

        var menu = new WinForms.ContextMenuStrip();
        menu.Items.Add(openItem);
        menu.Items.Add(new WinForms.ToolStripSeparator());
        menu.Items.Add(exitItem);

        _trayIcon = new WinForms.NotifyIcon
        {
            ContextMenuStrip = menu,
            Icon = _trayDrawingIcon,
            Text = AppBrand.DisplayName,
            Visible = true
        };
        _trayIcon.DoubleClick += (_, _) => Dispatcher.Invoke(ShowFromTray);
        _trayIcon.MouseClick += (_, args) =>
        {
            if (args.Button == WinForms.MouseButtons.Left)
            {
                Dispatcher.Invoke(ShowFromTray);
            }
        };
    }

    private void ShowFromTray()
    {
        Show();
        WindowState = WindowState.Normal;
        Activate();
    }

    private void HideToTray()
    {
        Hide();
    }

    private void HandleStateChanged(object? sender, ConnectionSnapshot snapshot)
    {
        if (Dispatcher.CheckAccess())
        {
            RenderState(snapshot);
            return;
        }

        Dispatcher.Invoke(() => RenderState(snapshot));
    }

    private async void ImportButton_Click(object sender, RoutedEventArgs e)
    {
        var window = new ImportProfileWindow(_appService, _secretStore) { Owner = this };
        if (window.ShowDialog() == true && window.ResultProfile is { } profile)
        {
            await AddProfileAsync(profile);
            _log.Info($"Profile imported for {profile.Endpoint.Hostname}");
        }
    }

    private async void ServerOptionsButton_Click(object sender, RoutedEventArgs e)
    {
        if (SelectedProfile is not { } profile)
        {
            ShowSelectServerMessage();
            return;
        }

        var window = new ServerOptionsWindow(profile, _secretStore, _routingProfiles, _settings) { Owner = this };
        if (window.ShowDialog() == true && window.ResultProfile is { } updated)
        {
            await ReplaceSelectedProfileAsync(updated);
            _log.Info($"Profile updated for {updated.Endpoint.Hostname}");
        }
    }

    private void RoutingProfilesButton_Click(object sender, RoutedEventArgs e)
    {
        var window = new RoutingProfilesWindow(_routingProfiles) { Owner = this };
        if (window.ShowDialog() == true)
        {
            _routingProfiles.Clear();
            foreach (var profile in window.ResultProfiles)
            {
                _routingProfiles.Add(profile);
            }

            RenderSelectedProfile();
            SaveState();
        }
    }

    private async void SettingsButton_Click(object sender, RoutedEventArgs e)
    {
        CloseCorePicker();
        var window = new SettingsWindow(_settings) { Owner = this };
        if (window.ShowDialog() == true)
        {
            var previousTheme = _settings.Appearance.Theme;
            _settings = window.ResultSettings;
            LocalizationManager.Instance.Apply(_settings.Language);
            DesktopTheme.Apply(_settings.Appearance);
            await EnsureLoadedSocksProfileDefaultsAsync();
            RenderState(_stateStore.Current);
            RefreshThemeToggleIcon(previousTheme != _settings.Appearance.Theme);
            SaveState();
        }
    }

    private void ThemeToggleButton_Click(object sender, RoutedEventArgs e)
    {
        var nextTheme = _settings.Appearance.Theme == AppThemeMode.Dark
            ? AppThemeMode.Light
            : AppThemeMode.Dark;
        _settings = _settings with
        {
            Appearance = _settings.Appearance with { Theme = nextTheme }
        };
        DesktopTheme.Apply(_settings.Appearance);
        RenderState(_stateStore.Current);
        RefreshThemeToggleIcon(animate: true);
        SaveState();
    }

    private void RefreshThemeToggleIcon(bool animate)
    {
        var showSun = _settings.Appearance.Theme == AppThemeMode.Dark;
        var visibleIcon = showSun ? ThemeSunIcon : ThemeMoonIcon;
        var hiddenIcon = showSun ? ThemeMoonIcon : ThemeSunIcon;

        ThemeToggleButton.ToolTip = showSun
            ? "Переключить на светлую тему"
            : "Переключить на темную тему";

        ThemeMoonIcon.BeginAnimation(UIElement.OpacityProperty, null);
        ThemeSunIcon.BeginAnimation(UIElement.OpacityProperty, null);
        ThemeIconRotate.BeginAnimation(RotateTransform.AngleProperty, null);

        if (!animate)
        {
            visibleIcon.Opacity = 1;
            hiddenIcon.Opacity = 0;
            ThemeIconRotate.Angle = 0;
            return;
        }

        visibleIcon.Opacity = 0;
        hiddenIcon.Opacity = 1;
        ThemeIconRotate.Angle = showSun ? -75 : 75;

        var fadeIn = new DoubleAnimation(0, 1, new Duration(TimeSpan.FromMilliseconds(180)))
        {
            BeginTime = TimeSpan.FromMilliseconds(45),
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }
        };
        var fadeOut = new DoubleAnimation(1, 0, new Duration(TimeSpan.FromMilliseconds(120)))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn }
        };
        var rotate = new DoubleAnimation(ThemeIconRotate.Angle, 0, new Duration(TimeSpan.FromMilliseconds(220)))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }
        };

        visibleIcon.BeginAnimation(UIElement.OpacityProperty, fadeIn);
        hiddenIcon.BeginAnimation(UIElement.OpacityProperty, fadeOut);
        ThemeIconRotate.BeginAnimation(RotateTransform.AngleProperty, rotate);
    }

    private async void TomlToolsButton_Click(object sender, RoutedEventArgs e)
    {
        if (SelectedProfile is not { } profile)
        {
            ShowSelectServerMessage();
            return;
        }

        var toml = await BuildTomlAsync(profile);
        var window = new TomlToolsWindow(toml) { Owner = this };
        if (window.ShowDialog() == true && !string.IsNullOrWhiteSpace(window.ResultToml))
        {
            await SaveEditedTomlAsync(profile, window.ResultToml);
        }
    }

    private void LogsButton_Click(object sender, RoutedEventArgs e)
    {
        new TextViewerWindow("Логи", _log.Export(), true) { Owner = this }.ShowDialog();
    }

    private void CoreAddButton_Click(object sender, RoutedEventArgs e)
    {
        if (sender is not UIElement placementTarget)
        {
            return;
        }

        if (CorePickerPopup.IsOpen && ReferenceEquals(_corePickerTarget, placementTarget))
        {
            CloseCorePicker();
            return;
        }

        CorePickerPopup.IsOpen = false;
        _corePickerTarget = placementTarget;
        CorePickerPopup.PlacementTarget = placementTarget;
        CorePickerPopup.IsOpen = true;
    }

    private void CorePickerPopup_Closed(object? sender, EventArgs e)
    {
        _corePickerTarget = null;
    }

    private void CloseCorePicker()
    {
        CorePickerPopup.IsOpen = false;
        _corePickerTarget = null;
    }

    private void MainWindow_PreviewMouseDown(object sender, MouseButtonEventArgs e)
    {
        if (!CorePickerPopup.IsOpen || _corePickerTarget is null)
        {
            return;
        }

        if (e.OriginalSource is DependencyObject source && IsInVisualTree(source, _corePickerTarget))
        {
            return;
        }

        CloseCorePicker();
    }

    private static bool IsInVisualTree(DependencyObject source, DependencyObject target)
    {
        for (var current = source; current is not null; current = GetParent(current))
        {
            if (ReferenceEquals(current, target))
            {
                return true;
            }
        }

        return false;
    }

    private static DependencyObject? GetParent(DependencyObject current)
    {
        try
        {
            return VisualTreeHelper.GetParent(current) ?? LogicalTreeHelper.GetParent(current);
        }
        catch (InvalidOperationException)
        {
            return LogicalTreeHelper.GetParent(current);
        }
    }

    private async void ServerCardTestButton_Click(object sender, RoutedEventArgs e)
    {
        if (GetContextProfile(sender) is { } profile)
        {
            ProfilesList.SelectedItem = profile;
            await RunServerTestsAsync(profile);
        }
    }

    private async void ServerCardRetestButton_Click(object sender, RoutedEventArgs e)
    {
        if (GetContextProfile(sender) is { } profile)
        {
            ProfilesList.SelectedItem = profile;
            await RunServerTestsAsync(profile);
        }
    }

    private async void ContextEditServer_Click(object sender, RoutedEventArgs e)
    {
        if ((GetContextProfile(sender) ?? SelectedProfile) is { } profile)
        {
            ProfilesList.SelectedItem = profile;
            ServerOptionsButton_Click(sender, e);
        }
    }

    private async void ContextPingServer_Click(object sender, RoutedEventArgs e)
    {
        if ((GetContextProfile(sender) ?? SelectedProfile) is { } profile)
        {
            ProfilesList.SelectedItem = profile;
            await RunServerTestsAsync(profile);
        }
    }

    private async void ContextTestServer_Click(object sender, RoutedEventArgs e)
    {
        if ((GetContextProfile(sender) ?? SelectedProfile) is { } profile)
        {
            ProfilesList.SelectedItem = profile;
            await RunServerTestsAsync(profile);
        }
    }

    private async void ContextDuplicateServer_Click(object sender, RoutedEventArgs e)
    {
        if ((GetContextProfile(sender) ?? SelectedProfile) is { } profile)
        {
            ProfilesList.SelectedItem = profile;
            await DuplicateProfileAsync(profile);
        }
    }

    private async void ContextDeleteServer_Click(object sender, RoutedEventArgs e)
    {
        if ((GetContextProfile(sender) ?? SelectedProfile) is { } profile)
        {
            ProfilesList.SelectedItem = profile;
            await DeleteProfileAsync(profile);
        }
    }

    private async Task DuplicateProfileAsync(ServerProfile profile)
    {
        var id = Guid.NewGuid().ToString("n");
        var scope = $"profile/{id}";
        var copy = profile with
        {
            Id = id,
            DisplayName = $"{profile.DisplayName} copy",
            Endpoint = profile.Endpoint with
            {
                PasswordSecretRef = await CopySecretAsync(profile.Endpoint.PasswordSecretRef, scope, "password"),
                ClientRandomSecretRef = await CopySecretAsync(profile.Endpoint.ClientRandomSecretRef, scope, "client_random")
            },
            Listener = profile.Listener with
            {
                Socks = profile.Listener.Socks with
                {
                    PasswordSecretRef = await CopySecretAsync(profile.Listener.Socks.PasswordSecretRef, scope, "socks_password")
                }
            },
            CreatedAt = DateTimeOffset.UtcNow,
            UpdatedAt = DateTimeOffset.UtcNow
        };

        await AddProfileAsync(copy);
        _log.Info($"Profile duplicated for {copy.Endpoint.Hostname}");
    }

    private async Task DeleteProfileAsync(ServerProfile profile)
    {
        var confirmed = AppDialog.Confirm(
            this,
            LocalizationManager.Instance.Translate("Main.DeleteServerTitle"),
            LocalizationManager.Instance.Format("Main.DeleteServerMessage", profile.DisplayName),
            LocalizationManager.Instance.Translate("Common.Delete"),
            LocalizationManager.Instance.Translate("Common.Cancel"),
            AppDialogTone.Danger);
        if (!confirmed)
        {
            return;
        }

        await DeleteSecretsAsync(profile);
        _profiles.Remove(profile);
        RefreshProfileChrome();
        _log.Info($"Profile deleted for {profile.Endpoint.Hostname}");
        RenderSelectedProfile();
        SaveState();
    }

    private async void ConnectionActionButton_Click(object sender, RoutedEventArgs e)
    {
        AnimateConnectTap();
        await ToggleConnectionAsync();
    }

    private void AnimateConnectTap()
    {
        if (RingDiscHost.RenderTransform is not ScaleTransform scale)
        {
            scale = new ScaleTransform(1, 1);
            RingDiscHost.RenderTransform = scale;
        }

        scale.BeginAnimation(ScaleTransform.ScaleXProperty, null);
        scale.BeginAnimation(ScaleTransform.ScaleYProperty, null);
        var down = new DoubleAnimation(0.96, new Duration(TimeSpan.FromMilliseconds(80)))
        {
            AutoReverse = true,
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }
        };
        scale.BeginAnimation(ScaleTransform.ScaleXProperty, down);
        scale.BeginAnimation(ScaleTransform.ScaleYProperty, down);
    }

    private async void SidebarConnectButton_Click(object sender, RoutedEventArgs e)
    {
        await ToggleConnectionAsync();
    }

    private async Task ToggleConnectionAsync()
    {
        if (_stateStore.Current.Phase == ConnectionPhase.Connected)
        {
            await DisconnectAsync();
            return;
        }

        await ConnectAsync();
    }

    private async Task ConnectAsync()
    {
        if (SelectedProfile is not { } profile)
        {
            ShowSelectServerMessage();
            return;
        }

        try
        {
            _activeProfile = profile;
            await _vpnController.ConnectAsync(profile);
        }
        catch (VpnException ex)
        {
            _activeProfile = null;
            _log.Error($"{ex.Code}: {ex.Message}");
        }
    }

    private async Task DisconnectAsync()
    {
        try
        {
            await _vpnController.DisconnectAsync();
        }
        catch (Exception ex)
        {
            _log.Error(ex.Message);
        }
    }

    private void ProfilesList_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        RenderSelectedProfile();
    }

    private void ProfilesList_PreviewMouseRightButtonDown(object sender, MouseButtonEventArgs e)
    {
        if (ItemsControl.ContainerFromElement(ProfilesList, e.OriginalSource as DependencyObject) is ListBoxItem item)
        {
            item.IsSelected = true;
            item.Focus();
            _contextProfile = item.DataContext as ServerProfile;
            OpenProfilesContextMenu(e);
            return;
        }

        _contextProfile = null;
        ProfilesList.Focus();
        OpenProfilesContextMenu(e);
    }

    private void OpenProfilesContextMenu(MouseButtonEventArgs e)
    {
        if (ProfilesList.ContextMenu is { } menu)
        {
            var position = e.GetPosition(ProfilesList);
            menu.IsOpen = false;
            menu.PlacementTarget = ProfilesList;
            menu.Placement = PlacementMode.RelativePoint;
            menu.HorizontalOffset = position.X;
            menu.VerticalOffset = position.Y;
            menu.IsOpen = true;
            e.Handled = true;
        }
    }

    private void ProfilesContextMenu_Opened(object sender, RoutedEventArgs e)
    {
        var hasProfile = _contextProfile is not null;
        ContextEditServerMenuItem.Visibility = hasProfile ? Visibility.Visible : Visibility.Collapsed;
        ContextPingServerMenuItem.Visibility = hasProfile ? Visibility.Visible : Visibility.Collapsed;
        ContextDuplicateServerMenuItem.Visibility = hasProfile ? Visibility.Visible : Visibility.Collapsed;
        ContextDeleteServerMenuItem.Visibility = hasProfile ? Visibility.Visible : Visibility.Collapsed;
        ContextAddSeparator.Visibility = Visibility.Collapsed;
        ContextImportClipboardMenuItem.Visibility = hasProfile ? Visibility.Collapsed : Visibility.Visible;
        ContextImportTomlMenuItem.Visibility = hasProfile ? Visibility.Collapsed : Visibility.Visible;
        ContextImportClipboardMenuItem.IsEnabled = true;
    }

    private async void ContextImportClipboard_Click(object sender, RoutedEventArgs e)
    {
        if (!Clipboard.ContainsText())
        {
            AppDialog.Show(
                this,
                LocalizationManager.Instance.Translate("Import.FromClipboard"),
                LocalizationManager.Instance.Translate("Import.ClipboardNoText"),
                AppDialogTone.Info);
            return;
        }

        try
        {
            var text = Clipboard.GetText();
            var profile = text.TrimStart().StartsWith("tt://", StringComparison.OrdinalIgnoreCase)
                ? await _appService.ImportDeeplinkAsync(text)
                : await _appService.ImportTomlAsync(text);
            await AddProfileAsync(profile);
            _log.Info($"Profile imported from clipboard for {profile.Endpoint.Hostname}");
        }
        catch (Exception ex)
        {
            AppDialog.Show(this, LocalizationManager.Instance.Translate("Import.Failed"), ex.Message, AppDialogTone.Warning);
        }
    }

    private async void ContextImportToml_Click(object sender, RoutedEventArgs e)
    {
        var window = new TomlImportWindow(_appService) { Owner = this };
        if (window.ShowDialog() == true && window.ResultProfile is { } profile)
        {
            await AddProfileAsync(profile);
            _log.Info($"TOML profile imported for {profile.Endpoint.Hostname}");
        }
    }

    private void RoutingProfileComboBox_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_loadingUi || SelectedProfile is not { } profile || RoutingProfileComboBox.SelectedItem is not RoutingProfile routing)
        {
            return;
        }

        ReplaceSelectedProfile(profile with
        {
            RoutingProfileId = routing.Id,
            Routing = routing
        });
    }

    private async Task AddProfileAsync(ServerProfile profile)
    {
        var withRouting = await EnsureSocksProfileDefaultsAsync(EnsureRoutingProfile(profile));
        _profiles.Add(withRouting);
        RefreshProfileChrome();
        ProfilesList.SelectedItem = withRouting;
        RenderSelectedProfile();
        SaveState();
        _ = ResolveGeoAsync(withRouting, _lifetimeCts.Token);
    }

    private async Task ReplaceSelectedProfileAsync(ServerProfile profile)
    {
        ReplaceSelectedProfile(await EnsureSocksProfileDefaultsAsync(profile));
    }

    private void ReplaceSelectedProfile(ServerProfile profile)
    {
        var index = ProfilesList.SelectedIndex;
        if (index < 0)
        {
            return;
        }

        var updated = profile with { UpdatedAt = DateTimeOffset.UtcNow };
        _profiles[index] = updated;
        ProfilesList.SelectedItem = updated;
        RenderSelectedProfile();
        SaveState();
    }

    private async Task<ServerProfile> EnsureSocksProfileDefaultsAsync(ServerProfile profile)
    {
        if (profile.Listener.Mode != ListenerMode.Socks)
        {
            return profile;
        }

        var socks = profile.Listener.Socks;
        var username = string.IsNullOrWhiteSpace(socks.Username)
            ? SocksCredentialGenerator.Username()
            : socks.Username.Trim();
        var passwordRef = socks.PasswordSecretRef;
        var password = string.IsNullOrWhiteSpace(passwordRef)
            ? ""
            : await _secretStore.ReadAsync(passwordRef) ?? "";

        if (string.IsNullOrWhiteSpace(password))
        {
            passwordRef = await _secretStore.SaveAsync($"profile/{profile.Id}", "socks_password", SocksCredentialGenerator.Password());
        }

        return profile with
        {
            Listener = profile.Listener with
            {
                Socks = socks with
                {
                    Address = _settings.DefaultSocksAddress,
                    Username = username,
                    PasswordSecretRef = passwordRef,
                    AllowLanAccess = _settings.DefaultSocksAllowLan,
                    HttpProxyAddress = "",
                    HttpProxyAllowLanAccess = false
                }
            }
        };
    }

    private async Task EnsureLoadedSocksProfileDefaultsAsync()
    {
        var changed = false;
        for (var i = 0; i < _profiles.Count; i++)
        {
            var profile = _profiles[i];
            var updated = await EnsureSocksProfileDefaultsAsync(profile);
            if (updated != profile)
            {
                _profiles[i] = updated;
                changed = true;
            }
        }

        if (!changed)
        {
            return;
        }

        RenderSelectedProfile();
        SaveState();
    }

    private void RenderSelectedProfile()
    {
        _loadingUi = true;
        if (SelectedProfile is not { } profile)
        {
            var i18n = LocalizationManager.Instance;
            SelectedCountryText.Text = "TT";
            SelectedServerText.Text = i18n.Translate("Main.NoServer");
            CompactCountryText.Text = "TT";
            CompactServerText.Text = i18n.Translate("Main.NoServer");
            CompactModeText.Text = i18n.Translate("Main.ImportConfig");
            CompactPingText.Text = "-";
            StatusText.Text = i18n.Translate("Main.Disconnected");
            ModeText.Text = i18n.Translate("Main.ImportOrAdd");
            TransportPillText.Text = "-";
            ListenerPillText.Text = "-";
            DnsPillText.Text = "-";
            _currentTraffic = new TrafficMetricsSnapshot(0, 0);
            RenderTrafficMetrics(_currentTraffic);
            RoutingProfileComboBox.SelectedIndex = -1;
            ConnectButton.IsEnabled = false;
            ConnectButton.ToolTip = i18n.Translate("Main.Connect");
            RingActionText.Text = i18n.Translate("Main.Connect");
            SidebarConnectButton.IsEnabled = false;
            SidebarConnectButton.Content = i18n.Translate("Main.Connect");
            UpdateRing(ConnectionPhase.Idle);
            ClearDiagnostics();
            _loadingUi = false;
            return;
        }

        ClearDiagnosticsIfProfileChanged(profile);
        SelectedCountryText.Text = ToCountryCodeText(profile.CountryCode);
        SelectedServerText.Text = profile.DisplayName;
        ModeText.Text = $"{profile.Endpoint.Hostname} - {profile.Endpoint.UpstreamProtocol} - {profile.Listener.Mode}{(profile.ServiceModeEnabled ? " - Service" : "")}";
        CompactCountryText.Text = ToCountryCodeText(profile.CountryCode);
        CompactServerText.Text = profile.DisplayName;
        CompactModeText.Text = $"{profile.Endpoint.Hostname} - {profile.Endpoint.UpstreamProtocol}";
        CompactPingText.Text = profile.TestResult is null ? "-" : $"{profile.TestResult.PingMs} ms";
        TransportPillText.Text = profile.Endpoint.UpstreamProtocol.ToString();
        ListenerPillText.Text = profile.TestResult is null ? "-" : $"{profile.TestResult.PingMs} ms";
        DnsPillText.Text = "-";
        RenderTrafficMetrics(_currentTraffic);
        ConnectButton.IsEnabled = true;
        SidebarConnectButton.IsEnabled = true;
        RoutingProfileComboBox.SelectedItem = _routingProfiles.FirstOrDefault(routing => routing.Id == profile.RoutingProfileId)
            ?? _routingProfiles.FirstOrDefault(routing => routing.Name == profile.Routing.Name);
        RenderState(_stateStore.Current);
        _loadingUi = false;
    }

    private void RenderState(ConnectionSnapshot snapshot)
    {
        var i18n = LocalizationManager.Instance;
        var statusText = snapshot.Phase switch
        {
            ConnectionPhase.Connected => i18n.Translate("Main.Connected"),
            ConnectionPhase.Disconnecting => i18n.Translate("Main.Disconnecting"),
            ConnectionPhase.Preparing or ConnectionPhase.Connecting or ConnectionPhase.Authenticating => i18n.Translate("Main.Connecting"),
            ConnectionPhase.Reconnecting => i18n.Translate("Main.Reconnecting"),
            ConnectionPhase.Error => i18n.Translate("Main.ConnectionError"),
            _ => i18n.Translate("Main.Disconnected")
        };
        StatusText.Text = statusText;
        HeaderStatusDot.ToolTip = string.IsNullOrWhiteSpace(snapshot.Message)
            ? statusText
            : $"{statusText}: {snapshot.Message}";
        var isActionPhase = snapshot.Phase is ConnectionPhase.Idle
            or ConnectionPhase.Disconnected
            or ConnectionPhase.Connected
            or ConnectionPhase.Error
            or ConnectionPhase.PermissionRequired;
        var actionEnabled = SelectedProfile is not null && isActionPhase;
        ConnectButton.IsEnabled = actionEnabled;
        SidebarConnectButton.IsEnabled = actionEnabled;
        var actionText = snapshot.Phase switch
        {
            ConnectionPhase.Connected => i18n.Translate("Main.Disconnect"),
            ConnectionPhase.Disconnecting => i18n.Translate("Main.Disconnecting"),
            ConnectionPhase.Preparing or ConnectionPhase.Connecting or ConnectionPhase.Authenticating => i18n.Translate("Main.Connecting"),
            ConnectionPhase.Reconnecting => i18n.Translate("Main.Reconnecting"),
            _ => i18n.Translate("Main.Connect")
        };
        ConnectButton.ToolTip = actionText;
        RingActionText.Text = actionText;
        SidebarConnectButton.Content = actionText;
        CompactStatusText.Text = statusText;
        DnsPillText.Text = SelectedProfile is null ? "-" : ToSessionLabel(snapshot.Phase);
        UpdateRing(snapshot.Phase);
        UpdateStatusIndicators(snapshot.Phase == ConnectionPhase.Connected);
        UpdateTrafficSampling(snapshot);
        UpdateSystemProxy(snapshot);
    }

    private void HandleTrafficMetricsUpdated(object? sender, TrafficMetricsSnapshot snapshot)
    {
        if (!Dispatcher.CheckAccess())
        {
            Dispatcher.Invoke(() => HandleTrafficMetricsUpdated(sender, snapshot));
            return;
        }

        _currentTraffic = snapshot;
        RenderTrafficMetrics(snapshot);
    }

    private void RenderTrafficMetrics(TrafficMetricsSnapshot snapshot)
    {
        DownloadMetricText.Text = FormatLiveMbps(snapshot.DownloadMbps);
        UploadMetricText.Text = FormatLiveMbps(snapshot.UploadMbps);
    }

    private static string FormatLiveMbps(double mbps)
    {
        if (mbps <= 0.005)
        {
            return "0.0";
        }

        return mbps < 10
            ? mbps.ToString("0.00", CultureInfo.InvariantCulture)
            : mbps.ToString("0.0", CultureInfo.InvariantCulture);
    }

    private void UpdateTrafficSampling(ConnectionSnapshot snapshot)
    {
        if (snapshot.Phase == ConnectionPhase.Connected && SelectedProfile is { } profile)
        {
            _trafficMetrics.Start(profile);
            return;
        }

        if (snapshot.Phase is ConnectionPhase.Idle
            or ConnectionPhase.Disconnected
            or ConnectionPhase.Error
            or ConnectionPhase.PermissionRequired)
        {
            _trafficMetrics.Stop();
        }
    }

    private void UpdateSystemProxy(ConnectionSnapshot snapshot)
    {
        try
        {
            if (snapshot.Phase == ConnectionPhase.Connected && (_activeProfile ?? SelectedProfile) is { } profile)
            {
                _systemProxy.Apply(_settings.SystemProxyMode, profile);
                return;
            }

            if (snapshot.Phase is ConnectionPhase.Idle
                or ConnectionPhase.Disconnected
                or ConnectionPhase.Error
                or ConnectionPhase.PermissionRequired)
            {
                _systemProxy.Clear();
                _activeProfile = null;
            }
        }
        catch (Exception ex) when (ex is InvalidOperationException or IOException or UnauthorizedAccessException)
        {
            _log.Error($"System proxy update failed: {ex.Message}");
        }
    }

    private static string ToSessionLabel(ConnectionPhase phase)
    {
        var i18n = LocalizationManager.Instance;
        return phase switch
        {
            ConnectionPhase.Connected => i18n.Translate("Main.SessionActive"),
            ConnectionPhase.Preparing or ConnectionPhase.Connecting or ConnectionPhase.Authenticating => i18n.Translate("Main.SessionStart"),
            ConnectionPhase.Reconnecting => i18n.Translate("Main.SessionRetry"),
            ConnectionPhase.Disconnecting => i18n.Translate("Main.SessionStop"),
            ConnectionPhase.Error => i18n.Translate("Main.SessionError"),
            _ => "-"
        };
    }

    private async Task<string> BuildTomlAsync(ServerProfile profile)
    {
        var password = await _secretStore.ReadAsync(profile.Endpoint.PasswordSecretRef) ?? "<missing-secret>";
        var clientRandom = string.IsNullOrWhiteSpace(profile.Endpoint.ClientRandomSecretRef)
            ? ""
            : await _secretStore.ReadAsync(profile.Endpoint.ClientRandomSecretRef) ?? "";
        var socksPassword = string.IsNullOrWhiteSpace(profile.Listener.Socks.PasswordSecretRef)
            ? ""
            : await _secretStore.ReadAsync(profile.Listener.Socks.PasswordSecretRef) ?? "";

        return _tomlBuilder.Build(new TrustTunnelConfig
        {
            Endpoint = profile.Endpoint,
            Listener = profile.Listener,
            Routing = profile.Routing
        }, password, clientRandom, socksPassword);
    }

    private bool ValidateForSave(ServerProfile profile, out string message)
    {
        var report = TrustTunnelValidators.Validate(profile, skipVerificationConfirmed: true);
        var errors = report.Issues.Where(issue => issue.Severity == ValidationSeverity.Error).ToArray();
        message = errors.Length == 0
            ? "OK"
            : string.Join(Environment.NewLine, errors.Select(issue => $"{issue.Code}: {issue.Message}"));
        return errors.Length == 0;
    }

    private ServerProfile EnsureRoutingProfile(ServerProfile profile)
    {
        var routing = _routingProfiles.FirstOrDefault(item => item.Id == profile.RoutingProfileId)
            ?? _routingProfiles.FirstOrDefault();
        return routing is null ? profile : profile with { RoutingProfileId = routing.Id, Routing = routing };
    }

    private async Task<string> CopySecretAsync(string oldSecretRef, string scope, string name)
    {
        if (string.IsNullOrWhiteSpace(oldSecretRef))
        {
            return "";
        }

        var value = await _secretStore.ReadAsync(oldSecretRef);
        return string.IsNullOrEmpty(value) ? "" : await _secretStore.SaveAsync(scope, name, value);
    }

    private async Task SaveEditedTomlAsync(ServerProfile currentProfile, string toml)
    {
        ServerProfile? imported = null;
        try
        {
            imported = await _appService.ImportTomlAsync(toml);
            var sameEndpoint = string.Equals(imported.Endpoint.Hostname, currentProfile.Endpoint.Hostname, StringComparison.OrdinalIgnoreCase);
            var updated = imported with
            {
                Id = currentProfile.Id,
                AutoConnect = currentProfile.AutoConnect,
                ServiceModeEnabled = currentProfile.ServiceModeEnabled,
                CountryCode = sameEndpoint ? currentProfile.CountryCode : "",
                CountryName = sameEndpoint ? currentProfile.CountryName : "",
                TestResult = sameEndpoint ? currentProfile.TestResult : null,
                CreatedAt = currentProfile.CreatedAt
            };

            if (!ValidateForSave(updated, out var message))
            {
                await DeleteSecretsAsync(imported);
                AppDialog.Show(this, "TOML не сохранен", message, AppDialogTone.Warning);
                return;
            }

            var rehomed = await RehomeTomlSecretsAsync(updated, currentProfile.Id);
            await DeleteReplacedSecretsAsync(currentProfile, rehomed);
            await DeleteSecretsAsync(imported);
            await ReplaceSelectedProfileAsync(rehomed);
            _log.Info($"TOML profile saved for {rehomed.Endpoint.Hostname}");
        }
        catch (Exception ex)
        {
            if (imported is not null)
            {
                await DeleteSecretsAsync(imported);
            }

            AppDialog.Show(this, "TOML не сохранен", ex.Message, AppDialogTone.Warning);
        }
    }

    private async Task<ServerProfile> RehomeTomlSecretsAsync(ServerProfile profile, string targetProfileId)
    {
        var scope = $"profile/{targetProfileId}";
        var passwordRef = await CopySecretAsync(profile.Endpoint.PasswordSecretRef, scope, "password");
        var clientRandomRef = await CopySecretAsync(profile.Endpoint.ClientRandomSecretRef, scope, "client_random");
        var socksPasswordRef = await CopySecretAsync(profile.Listener.Socks.PasswordSecretRef, scope, "socks_password");

        return profile with
        {
            Endpoint = profile.Endpoint with
            {
                PasswordSecretRef = passwordRef,
                ClientRandomSecretRef = clientRandomRef
            },
            Listener = profile.Listener with
            {
                Socks = profile.Listener.Socks with
                {
                    PasswordSecretRef = socksPasswordRef
                }
            }
        };
    }

    private async Task DeleteReplacedSecretsAsync(ServerProfile oldProfile, ServerProfile newProfile)
    {
        await DeleteIfReplacedAsync(oldProfile.Endpoint.PasswordSecretRef, newProfile.Endpoint.PasswordSecretRef);
        await DeleteIfReplacedAsync(oldProfile.Endpoint.ClientRandomSecretRef, newProfile.Endpoint.ClientRandomSecretRef);
        await DeleteIfReplacedAsync(oldProfile.Listener.Socks.PasswordSecretRef, newProfile.Listener.Socks.PasswordSecretRef);
    }

    private async Task DeleteIfReplacedAsync(string oldSecretRef, string newSecretRef)
    {
        if (string.IsNullOrWhiteSpace(oldSecretRef) || string.Equals(oldSecretRef, newSecretRef, StringComparison.Ordinal))
        {
            return;
        }

        await _secretStore.DeleteAsync(oldSecretRef);
    }

    private async Task DeleteSecretsAsync(ServerProfile profile)
    {
        await _secretStore.DeleteAsync(profile.Endpoint.PasswordSecretRef);
        await _secretStore.DeleteAsync(profile.Endpoint.ClientRandomSecretRef);
        await _secretStore.DeleteAsync(profile.Listener.Socks.PasswordSecretRef);
    }

    private async Task RunServerTestsAsync(ServerProfile profile)
    {
        var result = await ServerDiagnostics.ConnectionTestAsync(profile, _settings);
        UpdateProfileTestResult(profile, result);
        _log.Info($"Connection test {profile.Endpoint.Hostname}: ping={result.PingMs}ms download={result.DownloadMbps:0.00}Mbps upload={result.UploadMbps:0.00}Mbps");
    }

    private void UpdateProfileTestResult(ServerProfile profile, SpeedTestResult result)
    {
        var index = _profiles.IndexOf(profile);
        if (index < 0)
        {
            index = _profiles.ToList().FindIndex(item => item.Id == profile.Id);
        }

        if (index < 0)
        {
            return;
        }

        var countryCode = string.IsNullOrWhiteSpace(_profiles[index].CountryCode)
            ? result.CountryCode
            : _profiles[index].CountryCode;
        var updated = _profiles[index] with { CountryCode = countryCode, TestResult = result };
        _profiles[index] = updated;
        ProfilesList.SelectedItem = updated;
        RenderSelectedProfile();
        SaveState();
    }

    private async Task ResolveGeoAsync(ServerProfile profile, CancellationToken cancellationToken = default)
    {
        try
        {
            var geo = await _geoLookup.ResolveAsync(profile, cancellationToken);
            if (geo is null)
            {
                return;
            }

            Dispatcher.Invoke(() =>
            {
                var index = _profiles.ToList().FindIndex(item => item.Id == profile.Id);
                if (index < 0)
                {
                    return;
                }

                var current = _profiles[index];
                if (string.Equals(current.CountryCode, geo.CountryCode, StringComparison.OrdinalIgnoreCase))
                {
                    return;
                }

                var updated = current with { CountryCode = geo.CountryCode, CountryName = geo.Country };
                _profiles[index] = updated;
                if (SelectedProfile?.Id == updated.Id)
                {
                    ProfilesList.SelectedItem = updated;
                }

                SaveState();
            });
        }
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or OperationCanceledException or ObjectDisposedException or System.Net.Sockets.SocketException)
        {
            _log.Info($"Geo lookup skipped for {profile.Endpoint.Hostname}: {ex.Message}");
        }
    }

    private void CollapseButton_Click(object sender, RoutedEventArgs e)
    {
        SetWindowMode(_currentMode == WindowMode.Expanded ? WindowMode.Compact : WindowMode.Expanded);
    }

    private void TitleBar_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
    {
        if (e.ClickCount == 2)
        {
            SetWindowMode(_currentMode == WindowMode.Expanded ? WindowMode.Compact : WindowMode.Expanded);
            return;
        }

        try
        {
            DragMove();
        }
        catch (InvalidOperationException)
        {
        }
    }

    private void MinimizeButton_Click(object sender, RoutedEventArgs e)
    {
        WindowState = WindowState.Minimized;
    }

    private void CloseButton_Click(object sender, RoutedEventArgs e)
    {
        HideToTray();
    }

    private void SetWindowMode(WindowMode mode)
    {
        if (_currentMode == mode)
        {
            return;
        }

        ApplyWindowMode(mode, animate: true);
        _settings = _settings with { MainWindowMode = _currentMode.ToString() };
        SaveState();
    }

    private void ApplyWindowMode(WindowMode mode, bool animate)
    {
        _currentMode = mode;
        CloseCorePicker();
        if (!animate)
        {
            BeginAnimation(WidthProperty, null);
            Width = mode == WindowMode.Compact ? CompactWidth : ExpandedWidth;
            Height = WindowHeight;
            MinHeight = WindowHeight;
            MaxHeight = WindowHeight;
            MinWidth = mode == WindowMode.Compact ? CompactWidth : CompactWidth;
            MaxWidth = mode == WindowMode.Compact ? CompactWidth : ExpandedWidth;
            SetLayoutForMode(mode);
            RightPanelContainer.Visibility = mode == WindowMode.Compact ? Visibility.Collapsed : Visibility.Visible;
            RightPanelContainer.Opacity = 1;
            NavButtonsPanel.Visibility = mode == WindowMode.Compact ? Visibility.Collapsed : Visibility.Visible;
            NavButtonsPanel.IsEnabled = mode == WindowMode.Expanded;
            NavButtonsPanel.Opacity = 1;
            SetSidebarConnectVisible(mode == WindowMode.Compact, animate: false);
            SetHeaderStatusDotVisible(mode == WindowMode.Compact, animate: false);
            CollapseArrowRotation.Angle = mode == WindowMode.Compact ? 180 : 0;
            SetExpandCollapseIcon(mode);
            return;
        }

        if (mode == WindowMode.Compact)
        {
            var savedLeft = Left;
            SetNavButtonsVisible(false);
            SetSidebarConnectVisible(true);
            SetHeaderStatusDotVisible(true);
            AnimateCollapseArrow(180);
            SetExpandCollapseIcon(mode);
            SetLayoutForMode(mode);

            FadeOut(RightPanelContainer, 130, () =>
            {
                if (_currentMode != WindowMode.Compact)
                {
                    return;
                }

                RightPanelContainer.Visibility = Visibility.Collapsed;
                AnimateWidth(CompactWidth, () =>
                {
                    Left = savedLeft;
                    MinWidth = CompactWidth;
                    MaxWidth = CompactWidth;
                });
            });
            return;
        }

        MinWidth = CompactWidth;
        MaxWidth = ExpandedWidth;
        SetLayoutForMode(mode);
        SetSidebarConnectVisible(false);
        SetHeaderStatusDotVisible(false);
        AnimateCollapseArrow(0);
        SetExpandCollapseIcon(mode);

        AnimateWidth(ExpandedWidth, () =>
        {
            if (_currentMode != WindowMode.Expanded)
            {
                return;
            }

            RightPanelContainer.Visibility = Visibility.Visible;
            FadeIn(RightPanelContainer, 180);
            SetNavButtonsVisible(true);
        });
    }

    private static WindowMode ParseWindowMode(string value)
    {
        return string.Equals(value, nameof(WindowMode.Compact), StringComparison.OrdinalIgnoreCase)
            ? WindowMode.Compact
            : WindowMode.Expanded;
    }

    private void SetLayoutForMode(WindowMode mode)
    {
        if (mode == WindowMode.Compact)
        {
            SidebarColumn.Width = new GridLength(1, GridUnitType.Star);
            RightColumn.Width = new GridLength(0);
            AppTitleText.Visibility = Visibility.Visible;
            ExpandedCoreTabsPanel.Visibility = Visibility.Collapsed;
            CompactCoreSelectorPanel.Visibility = Visibility.Visible;
            CompactDockPanel.Visibility = Visibility.Visible;
            SidebarToolsPanel.Visibility = Visibility.Visible;
            SidebarImportButton.Margin = new Thickness(0, 0, 0, 6);
            return;
        }

        SidebarColumn.Width = new GridLength(SidebarExpandedWidth);
        RightColumn.Width = new GridLength(1, GridUnitType.Star);
        AppTitleText.Visibility = Visibility.Visible;
        ExpandedCoreTabsPanel.Visibility = Visibility.Visible;
        CompactCoreSelectorPanel.Visibility = Visibility.Collapsed;
        CompactDockPanel.Visibility = Visibility.Collapsed;
        SidebarToolsPanel.Visibility = Visibility.Visible;
        SidebarImportButton.Margin = new Thickness(0, 0, 0, 10);
    }

    private void AnimateWidth(double targetWidth, Action? onCompleted = null)
    {
        var animation = new DoubleAnimation(Width, targetWidth, new Duration(TimeSpan.FromMilliseconds(220)))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseInOut }
        };
        if (onCompleted is not null)
        {
            animation.Completed += (_, _) => onCompleted();
        }

        BeginAnimation(WidthProperty, animation);
    }

    private static void FadeOut(UIElement element, int milliseconds, Action? onCompleted = null)
    {
        var animation = new DoubleAnimation(1, 0, new Duration(TimeSpan.FromMilliseconds(milliseconds)))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn }
        };
        if (onCompleted is not null)
        {
            animation.Completed += (_, _) => onCompleted();
        }

        element.BeginAnimation(UIElement.OpacityProperty, animation);
    }

    private static void FadeIn(UIElement element, int milliseconds, Action? onCompleted = null)
    {
        element.Opacity = 0;
        var animation = new DoubleAnimation(0, 1, new Duration(TimeSpan.FromMilliseconds(milliseconds)))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }
        };
        if (onCompleted is not null)
        {
            animation.Completed += (_, _) => onCompleted();
        }

        element.BeginAnimation(UIElement.OpacityProperty, animation);
    }

    private void AnimateCollapseArrow(double toAngle)
    {
        var animation = new DoubleAnimation(CollapseArrowRotation.Angle, toAngle, new Duration(TimeSpan.FromMilliseconds(200)))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }
        };
        CollapseArrowRotation.BeginAnimation(RotateTransform.AngleProperty, animation);
    }

    private void SetExpandCollapseIcon(WindowMode mode)
    {
        ExpandCollapsePath.Data = Geometry.Parse(mode == WindowMode.Compact
            ? "M5,5 H19 V19 H5 Z"
            : "M5,9 H15 V19 H5 Z M9,5 H19 V15");
    }

    private void SetNavButtonsVisible(bool visible)
    {
        NavButtonsPanel.BeginAnimation(UIElement.OpacityProperty, null);

        if (!visible)
        {
            NavButtonsPanel.IsEnabled = false;
            var fadeOut = new DoubleAnimation(1, 0, new Duration(TimeSpan.FromMilliseconds(120)))
            {
                EasingFunction = new CubicEase { EasingMode = EasingMode.EaseIn }
            };
            fadeOut.Completed += (_, _) =>
            {
                if (_currentMode == WindowMode.Compact)
                {
                    NavButtonsPanel.Visibility = Visibility.Collapsed;
                }
            };
            NavButtonsPanel.BeginAnimation(UIElement.OpacityProperty, fadeOut);
            return;
        }

        NavButtonsPanel.Visibility = Visibility.Visible;
        NavButtonsPanel.IsEnabled = true;
        NavButtonsPanel.Opacity = 0;
        var fadeIn = new DoubleAnimation(0, 1, new Duration(TimeSpan.FromMilliseconds(180)))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }
        };
        NavButtonsPanel.BeginAnimation(UIElement.OpacityProperty, fadeIn);
    }

    private void SetSidebarConnectVisible(bool visible, bool animate = true)
    {
        SidebarConnectButton.BeginAnimation(UIElement.OpacityProperty, null);
        if (SidebarConnectButton.RenderTransform is ScaleTransform scale)
        {
            scale.BeginAnimation(ScaleTransform.ScaleXProperty, null);
            scale.BeginAnimation(ScaleTransform.ScaleYProperty, null);
        }

        if (!visible)
        {
            SidebarConnectButton.Visibility = Visibility.Collapsed;
            SidebarConnectButton.Opacity = 1;
            if (SidebarConnectButton.RenderTransform is ScaleTransform hiddenScale)
            {
                hiddenScale.ScaleX = 1;
                hiddenScale.ScaleY = 1;
            }

            return;
        }

        SidebarConnectButton.Visibility = Visibility.Visible;
        if (!animate)
        {
            SidebarConnectButton.Opacity = 1;
            if (SidebarConnectButton.RenderTransform is ScaleTransform staticScale)
            {
                staticScale.ScaleX = 1;
                staticScale.ScaleY = 1;
            }

            return;
        }

        SidebarConnectButton.Opacity = 0;
        if (SidebarConnectButton.RenderTransform is not ScaleTransform visibleScale)
        {
            visibleScale = new ScaleTransform(0.9, 0.9);
            SidebarConnectButton.RenderTransform = visibleScale;
        }
        else
        {
            visibleScale.ScaleX = 0.9;
            visibleScale.ScaleY = 0.9;
        }

        var fade = new DoubleAnimation(0, 1, new Duration(TimeSpan.FromMilliseconds(200)))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }
        };
        var scaleAnimation = new DoubleAnimation(0.9, 1, new Duration(TimeSpan.FromMilliseconds(200)))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }
        };
        SidebarConnectButton.BeginAnimation(UIElement.OpacityProperty, fade);
        visibleScale.BeginAnimation(ScaleTransform.ScaleXProperty, scaleAnimation);
        visibleScale.BeginAnimation(ScaleTransform.ScaleYProperty, scaleAnimation);
    }

    private void SetHeaderStatusDotVisible(bool visible, bool animate = true)
    {
        HeaderStatusDot.BeginAnimation(UIElement.OpacityProperty, null);
        if (!visible)
        {
            HeaderStatusDot.Visibility = Visibility.Collapsed;
            HeaderStatusDot.Opacity = 1;
            return;
        }

        HeaderStatusDot.Visibility = Visibility.Visible;
        if (!animate)
        {
            HeaderStatusDot.Opacity = 1;
            ApplyHeaderPulse();
            return;
        }

        HeaderStatusDot.Opacity = 0;
        var fade = new DoubleAnimation(0, 1, new Duration(TimeSpan.FromMilliseconds(200)))
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }
        };
        fade.Completed += (_, _) => ApplyHeaderPulse();
        HeaderStatusDot.BeginAnimation(UIElement.OpacityProperty, fade);
    }

    private void UpdateStatusIndicators(bool connected)
    {
        var statusBrush = connected
            ? GetBrush("AccentBrush", Brushes.ForestGreen)
            : GetBrush("MutedTextBrush", Brushes.Gray);
        ServerStatusIndicator.Background = statusBrush;
        HeaderStatusDot.Background = statusBrush;
        CompactStatusDot.Background = statusBrush;

        if (_connected == connected)
        {
            if (HeaderStatusDot.Visibility == Visibility.Visible)
            {
                ApplyHeaderPulse();
            }

            return;
        }

        _connected = connected;
        ApplyScalePulse(ServerStatusIndicator, connected);
        ApplyHeaderPulse();
    }

    private void ApplyHeaderPulse()
    {
        HeaderStatusDot.BeginAnimation(UIElement.OpacityProperty, null);
        if (!_connected)
        {
            HeaderStatusDot.Opacity = 1;
            return;
        }

        if (HeaderStatusDot.Visibility != Visibility.Visible)
        {
            return;
        }

        var pulse = new DoubleAnimation(1, 0.4, new Duration(TimeSpan.FromMilliseconds(1200)))
        {
            AutoReverse = true,
            RepeatBehavior = RepeatBehavior.Forever
        };
        HeaderStatusDot.BeginAnimation(UIElement.OpacityProperty, pulse);
    }

    private static void ApplyScalePulse(Border border, bool enabled)
    {
        if (border.RenderTransform is not ScaleTransform scale)
        {
            scale = new ScaleTransform(1, 1);
            border.RenderTransform = scale;
        }

        scale.BeginAnimation(ScaleTransform.ScaleXProperty, null);
        scale.BeginAnimation(ScaleTransform.ScaleYProperty, null);

        if (!enabled)
        {
            scale.ScaleX = 1;
            scale.ScaleY = 1;
            return;
        }

        var pulse = new DoubleAnimation(1, 1.08, new Duration(TimeSpan.FromMilliseconds(800)))
        {
            AutoReverse = true,
            RepeatBehavior = RepeatBehavior.Forever,
            EasingFunction = new SineEase { EasingMode = EasingMode.EaseInOut }
        };
        scale.BeginAnimation(ScaleTransform.ScaleXProperty, pulse);
        scale.BeginAnimation(ScaleTransform.ScaleYProperty, pulse);
    }

    private void RenderDiagnostics(ServerProfile profile, IReadOnlyList<ServerDiagnosticResult> results)
    {
        _diagnosticsProfileId = profile.Id;
        _diagnostics.Clear();
        foreach (var result in results)
        {
            _diagnostics.Add(new DiagnosticResultView(
                result.Name,
                result.Success ? "OK" : "FAIL",
                result.Message,
                result.Elapsed == TimeSpan.Zero ? "00:00:00" : result.Elapsed.ToString(@"mm\:ss\.fff")));
        }

        var okCount = results.Count(result => result.Success);
        DiagnosticsSummaryText.Text = results.Count == 0
            ? "Диагностика не вернула результатов"
            : $"Последняя проверка: {okCount}/{results.Count} OK";
        DiagnosticsPanel.Visibility = Visibility.Visible;
    }

    private void ClearDiagnosticsIfProfileChanged(ServerProfile profile)
    {
        if (_diagnosticsProfileId.Length == 0 || _diagnosticsProfileId == profile.Id)
        {
            return;
        }

        ClearDiagnostics();
    }

    private void ClearDiagnostics()
    {
        _diagnosticsProfileId = "";
        _diagnostics.Clear();
        DiagnosticsPanel.Visibility = Visibility.Collapsed;
    }

    private void RefreshProfileChrome()
    {
        ProfileCountText.Text = _profiles.Count.ToString();
    }

    private static string ToCountryCodeText(string value)
    {
        var code = value.Trim().ToUpperInvariant();
        return code.Length == 2 && code.All(ch => ch is >= 'A' and <= 'Z')
            ? code
            : "TT";
    }

    private static ServerProfile? GetContextProfile(object sender)
    {
        if (sender is FrameworkElement element && element.DataContext is ServerProfile profile)
        {
            return profile;
        }

        return null;
    }

    private void LoadPersistedState(PersistedDesktopState state)
    {
        _routingProfiles.Clear();
        if (state.RoutingProfiles.Count == 0)
        {
            SeedRoutingProfiles();
        }
        else
        {
            foreach (var routingProfile in state.RoutingProfiles)
            {
                _routingProfiles.Add(routingProfile);
            }
        }

        _profiles.Clear();
        foreach (var profile in state.Profiles)
        {
            _profiles.Add(EnsureRoutingProfile(profile));
        }

        RefreshProfileChrome();

        if (_profiles.Count == 0)
        {
            RenderSelectedProfile();
            return;
        }

        ProfilesList.SelectedIndex = 0;
        RenderSelectedProfile();
        foreach (var profile in _profiles.Where(profile => string.IsNullOrWhiteSpace(profile.CountryCode)).ToArray())
        {
            _ = ResolveGeoAsync(profile, _lifetimeCts.Token);
        }
    }

    private void SaveState()
    {
        try
        {
            _storage.Save(new PersistedDesktopState
            {
                Settings = _settings,
                Profiles = _profiles.ToList(),
                RoutingProfiles = _routingProfiles.ToList(),
                Secrets = new Dictionary<string, string>(_secretStore.Snapshot(), StringComparer.Ordinal)
            });
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            _log.Error($"State save failed: {ex.Message}");
        }
    }

    protected override void OnClosing(CancelEventArgs e)
    {
        _settings = _settings with { MainWindowMode = _currentMode.ToString() };
        SaveState();
        _lifetimeCts.Cancel();
        _stateStore.Changed -= _stateChangedHandler;
        _trafficMetrics.Updated -= HandleTrafficMetricsUpdated;
        _trafficMetrics.Dispose();
        _systemProxy.Dispose();
        if (_trayIcon is not null)
        {
            _trayIcon.Visible = false;
            _trayIcon.Dispose();
        }

        _trayDrawingIcon?.Dispose();
        HeaderStatusDot.BeginAnimation(UIElement.OpacityProperty, null);
        ServerStatusIndicator.BeginAnimation(UIElement.OpacityProperty, null);
        _geoLookup.Dispose();
        _lifetimeCts.Dispose();
        if (_vpnController is IDisposable disposableController)
        {
            disposableController.Dispose();
        }

        base.OnClosing(e);
    }

    private void SeedRoutingProfiles()
    {
        _routingProfiles.Add(new RoutingProfile
        {
            Id = "default",
            Name = "Default",
            Mode = RoutingMode.General,
            KillSwitchEnabled = true,
            Description = LocalizationManager.Instance.Translate("Routing.General")
        });
        _routingProfiles.Add(new RoutingProfile
        {
            Id = "local-bypass",
            Name = "Local bypass",
            Mode = RoutingMode.General,
            KillSwitchEnabled = true,
            Exclusions = new[] { "10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16" },
            Description = LocalizationManager.Instance.Translate("Routing.LocalBypass")
        });
        _routingProfiles.Add(new RoutingProfile
        {
            Id = "selective",
            Name = "Selective",
            Mode = RoutingMode.Selective,
            KillSwitchEnabled = false,
            Description = LocalizationManager.Instance.Translate("Routing.SelectivePreset")
        });
    }

    private static void ShowSelectServerMessage()
    {
        AppDialog.Show(
            Application.Current.MainWindow,
            LocalizationManager.Instance.Translate("Main.SelectServerTitle"),
            LocalizationManager.Instance.Translate("Main.SelectServerMessage"),
            AppDialogTone.Info);
    }

}
