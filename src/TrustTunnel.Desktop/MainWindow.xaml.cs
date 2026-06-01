using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Net.Http;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
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

namespace TrustTunnel.Desktop;

public partial class MainWindow : Window
{
    private enum WindowMode
    {
        Expanded,
        Compact
    }

    private const double ExpandedWidth = 960;
    private const double CompactWidth = 340;
    private const double WindowHeight = 620;

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
    private AppSettings _settings = new();
    private bool _loadingUi;
    private string _diagnosticsProfileId = "";
    private ServerProfile? _contextProfile;
    private WindowMode _currentMode = WindowMode.Expanded;
    private bool _connected;

    public sealed record DiagnosticResultView(string Name, string Status, string Message, string Elapsed);

    public MainWindow()
    {
        InitializeComponent();
        Width = ExpandedWidth;
        Height = WindowHeight;
        MinHeight = WindowHeight;
        MaxHeight = WindowHeight;
        MinWidth = CompactWidth;
        MaxWidth = ExpandedWidth + 200;
        ResizeMode = ResizeMode.CanResizeWithGrip;

        var persisted = _storage.Load();
        _settings = persisted.Settings;
        _secretStore.Load(persisted.Secrets);
        DesktopTheme.Apply(_settings.Appearance);

        _appService = new TrustTunnelAppService(_secretStore);
        _vpnController = new NativeBridgeVpnController(_stateStore, _log, _secretStore);
        _stateStore.Changed += (_, snapshot) => Dispatcher.Invoke(() => RenderState(snapshot));
        ProfilesList.ItemsSource = _profiles;
        RoutingProfileComboBox.DisplayMemberPath = nameof(RoutingProfile.Name);
        RoutingProfileComboBox.ItemsSource = _routingProfiles;
        DiagnosticsResultsList.ItemsSource = _diagnostics;
        LoadPersistedState(persisted);
        ApplyWindowMode(ParseWindowMode(_settings.MainWindowMode), animate: false);
    }

    private ServerProfile? SelectedProfile => ProfilesList.SelectedItem as ServerProfile;

    private void ImportButton_Click(object sender, RoutedEventArgs e)
    {
        var window = new ImportProfileWindow(_appService, _secretStore) { Owner = this };
        if (window.ShowDialog() == true && window.ResultProfile is { } profile)
        {
            AddProfile(profile);
            _log.Info($"Profile imported for {profile.Endpoint.Hostname}");
            ShowLogHint();
        }
    }

    private void ManualButton_Click(object sender, RoutedEventArgs e)
    {
        var window = new ManualServerWindow(_secretStore) { Owner = this };
        if (window.ShowDialog() == true && window.ResultProfile is { } profile)
        {
            if (!ValidateForSave(profile, out var message))
            {
                AppDialog.Show(this, "Профиль не сохранён", message, AppDialogTone.Warning);
                return;
            }

            AddProfile(profile);
            _log.Info($"Manual profile saved for {profile.Endpoint.Hostname}");
            ShowLogHint();
        }
    }

    private void ServerOptionsButton_Click(object sender, RoutedEventArgs e)
    {
        if (SelectedProfile is not { } profile)
        {
            ShowSelectServerMessage();
            return;
        }

        var window = new ServerOptionsWindow(profile, _secretStore, _routingProfiles) { Owner = this };
        if (window.ShowDialog() == true && window.ResultProfile is { } updated)
        {
            ReplaceSelectedProfile(updated);
            _log.Info($"Profile updated for {updated.Endpoint.Hostname}");
            ShowLogHint();
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

    private void SettingsButton_Click(object sender, RoutedEventArgs e)
    {
        var window = new SettingsWindow(_settings) { Owner = this };
        if (window.ShowDialog() == true)
        {
            _settings = window.ResultSettings;
            DesktopTheme.Apply(_settings.Appearance);
            SaveState();
        }
    }

    private async void TomlToolsButton_Click(object sender, RoutedEventArgs e)
    {
        if (SelectedProfile is not { } profile)
        {
            ShowSelectServerMessage();
            return;
        }

        var toml = await BuildTomlAsync(profile);
        new TomlToolsWindow(toml) { Owner = this }.ShowDialog();
    }

    private void LogsButton_Click(object sender, RoutedEventArgs e)
    {
        new TextViewerWindow("Логи", _log.Export(), true) { Owner = this }.ShowDialog();
    }

    private async void ServerTestsButton_Click(object sender, RoutedEventArgs e)
    {
        if (SelectedProfile is not { } profile)
        {
            ShowSelectServerMessage();
            return;
        }

        await RunServerTestsAsync(profile);
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

        AddProfile(copy);
        _log.Info($"Profile duplicated for {copy.Endpoint.Hostname}");
        ShowLogHint();
    }

    private async Task DeleteProfileAsync(ServerProfile profile)
    {
        var confirmed = AppDialog.Confirm(this, "Удаление сервера", $"Удалить \"{profile.DisplayName}\" и связанные секреты?", "Удалить", "Отмена", AppDialogTone.Danger);
        if (!confirmed)
        {
            return;
        }

        await DeleteSecretsAsync(profile);
        _profiles.Remove(profile);
        _log.Info($"Profile deleted for {profile.Endpoint.Hostname}");
        RenderSelectedProfile();
        SaveState();
    }

    private async void ConnectButton_Click(object sender, RoutedEventArgs e)
    {
        if (SelectedProfile is not { } profile)
        {
            ShowSelectServerMessage();
            return;
        }

        try
        {
            await _vpnController.ConnectAsync(profile);
        }
        catch (VpnException ex)
        {
            _log.Error($"{ex.Code}: {ex.Message}");
        }
    }

    private async void ConnectionActionButton_Click(object sender, RoutedEventArgs e)
    {
        await ToggleConnectionAsync();
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
            await _vpnController.ConnectAsync(profile);
        }
        catch (VpnException ex)
        {
            _log.Error($"{ex.Code}: {ex.Message}");
        }
    }

    private async void DisconnectButton_Click(object sender, RoutedEventArgs e)
    {
        await DisconnectAsync();
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
            return;
        }

        _contextProfile = null;
    }

    private void ProfilesContextMenu_Opened(object sender, RoutedEventArgs e)
    {
        var hasProfile = _contextProfile is not null;
        ContextEditServerMenuItem.Visibility = hasProfile ? Visibility.Visible : Visibility.Collapsed;
        ContextPingServerMenuItem.Visibility = hasProfile ? Visibility.Visible : Visibility.Collapsed;
        ContextDuplicateServerMenuItem.Visibility = hasProfile ? Visibility.Visible : Visibility.Collapsed;
        ContextDeleteServerMenuItem.Visibility = hasProfile ? Visibility.Visible : Visibility.Collapsed;
        ContextAddSeparator.Visibility = Visibility.Visible;
        ContextImportClipboardMenuItem.Visibility = hasProfile ? Visibility.Collapsed : Visibility.Visible;
        ContextImportTomlMenuItem.Visibility = hasProfile ? Visibility.Collapsed : Visibility.Visible;
        ContextImportClipboardMenuItem.IsEnabled = Clipboard.ContainsText();
    }

    private async void ContextImportClipboard_Click(object sender, RoutedEventArgs e)
    {
        if (!Clipboard.ContainsText())
        {
            AppDialog.Show(this, "Импорт из буфера", "В буфере обмена нет текста.", AppDialogTone.Info);
            return;
        }

        try
        {
            var text = Clipboard.GetText();
            var profile = text.TrimStart().StartsWith("tt://", StringComparison.OrdinalIgnoreCase)
                ? await _appService.ImportDeeplinkAsync(text)
                : await _appService.ImportTomlAsync(text);
            AddProfile(profile);
            _log.Info($"Profile imported from clipboard for {profile.Endpoint.Hostname}");
        }
        catch (Exception ex)
        {
            AppDialog.Show(this, "Импорт не выполнен", ex.Message, AppDialogTone.Warning);
        }
    }

    private void ContextImportToml_Click(object sender, RoutedEventArgs e)
    {
        var window = new TomlImportWindow(_appService) { Owner = this };
        if (window.ShowDialog() == true && window.ResultProfile is { } profile)
        {
            AddProfile(profile);
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

    private void AddProfile(ServerProfile profile)
    {
        var withRouting = EnsureRoutingProfile(profile);
        _profiles.Add(withRouting);
        ProfilesList.SelectedItem = withRouting;
        RenderSelectedProfile();
        SaveState();
        _ = ResolveGeoAsync(withRouting);
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

    private void RenderSelectedProfile()
    {
        _loadingUi = true;
        if (SelectedProfile is not { } profile)
        {
            SelectedServerText.Text = "Сервер не выбран";
            StatusText.Text = "Статус: Idle";
            ModeText.Text = "Добавьте или импортируйте профиль.";
            TransportPillText.Text = "-";
            ListenerPillText.Text = "-";
            DnsPillText.Text = "DNS default";
            RoutingProfileComboBox.SelectedIndex = -1;
            ConnectButton.IsEnabled = false;
            ConnectButton.Content = "Подключиться";
            SidebarConnectButton.IsEnabled = false;
            SidebarConnectButton.Content = "Подключиться";
            ClearDiagnostics();
            _loadingUi = false;
            return;
        }

        ClearDiagnosticsIfProfileChanged(profile);
        SelectedServerText.Text = profile.DisplayName;
        ModeText.Text = $"{profile.Endpoint.Hostname} · {profile.Listener.Mode} · {profile.Endpoint.UpstreamProtocol}{(profile.ServiceModeEnabled ? " · Service" : "")}";
        TransportPillText.Text = profile.Endpoint.UpstreamProtocol.ToString();
        ListenerPillText.Text = profile.Listener.Mode.ToString();
        DnsPillText.Text = profile.Endpoint.DnsUpstreams.Count == 0 ? "DNS default" : "DNS custom";
        ConnectButton.IsEnabled = true;
        SidebarConnectButton.IsEnabled = true;
        RoutingProfileComboBox.SelectedItem = _routingProfiles.FirstOrDefault(routing => routing.Id == profile.RoutingProfileId)
            ?? _routingProfiles.FirstOrDefault(routing => routing.Name == profile.Routing.Name);
        RenderState(_stateStore.Current);
        _loadingUi = false;
    }

    private void RenderState(ConnectionSnapshot snapshot)
    {
        StatusText.Text = $"Статус: {snapshot.Phase} · {snapshot.Message}";
        HeaderStatusDot.ToolTip = StatusText.Text;
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
            ConnectionPhase.Connected => "Отключиться",
            ConnectionPhase.Disconnecting => "Отключение...",
            ConnectionPhase.Preparing or ConnectionPhase.Connecting or ConnectionPhase.Authenticating => "Подключение...",
            ConnectionPhase.Reconnecting => "Восстановление...",
            _ => "Подключиться"
        };
        ConnectButton.Content = actionText;
        SidebarConnectButton.Content = actionText;
        UpdateStatusIndicators(snapshot.Phase == ConnectionPhase.Connected);
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

        var updated = _profiles[index] with { TestResult = result };
        _profiles[index] = updated;
        ProfilesList.SelectedItem = updated;
        SaveState();
    }

    private async Task ResolveGeoAsync(ServerProfile profile)
    {
        try
        {
            var geo = await _geoLookup.ResolveAsync(profile);
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
        catch (Exception ex) when (ex is HttpRequestException or TaskCanceledException or System.Net.Sockets.SocketException)
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

    private void MaxRestoreButton_Click(object sender, RoutedEventArgs e)
    {
        SetWindowMode(_currentMode == WindowMode.Expanded ? WindowMode.Compact : WindowMode.Expanded);
    }

    private void CloseButton_Click(object sender, RoutedEventArgs e)
    {
        Close();
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
        if (!animate)
        {
            BeginAnimation(WidthProperty, null);
            Width = mode == WindowMode.Compact ? CompactWidth : ExpandedWidth;
            Height = WindowHeight;
            MinHeight = WindowHeight;
            MaxHeight = WindowHeight;
            MinWidth = mode == WindowMode.Compact ? CompactWidth : CompactWidth;
            MaxWidth = mode == WindowMode.Compact ? CompactWidth : ExpandedWidth + 200;
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
        MaxWidth = ExpandedWidth + 200;
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

        if (_profiles.Count == 0)
        {
            SeedExample();
            return;
        }

        ProfilesList.SelectedIndex = 0;
        RenderSelectedProfile();
        foreach (var profile in _profiles.Where(profile => string.IsNullOrWhiteSpace(profile.CountryCode)).ToArray())
        {
            _ = ResolveGeoAsync(profile);
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
            Description = "Весь трафик через VPN"
        });
        _routingProfiles.Add(new RoutingProfile
        {
            Id = "local-bypass",
            Name = "Local bypass",
            Mode = RoutingMode.General,
            KillSwitchEnabled = true,
            Exclusions = new[] { "10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16" },
            Description = "VPN, кроме локальных сетей"
        });
        _routingProfiles.Add(new RoutingProfile
        {
            Id = "selective",
            Name = "Selective",
            Mode = RoutingMode.Selective,
            KillSwitchEnabled = false,
            Description = "Только выбранные назначения"
        });
    }

    private async void SeedExample()
    {
        var link = "tt://?AAEBARF0dC5oZWwyLm11bXVydS5ydQUGdHR1c2VyBiRZcmdua0o4V2pOV090MXdRVW5jYzllYWt5VU1nb3hjSVpZY0ICFXR0LmhlbDIubXVtdXJ1LnJ1OjQ0Mw";
        try
        {
            AddProfile(await _appService.ImportDeeplinkAsync(link));
            _log.Info("Example profile loaded from bundled test vector.");
        }
        catch (Exception ex)
        {
            _log.Error(ex.Message);
        }
    }

    private static void ShowSelectServerMessage()
    {
        AppDialog.Show(Application.Current.MainWindow, "TrustTunnel", "Сначала выберите сервер.", AppDialogTone.Info);
    }

    private static void ShowLogHint()
    {
        // Logs stay one click away; no toast system in this MVP.
    }
}
