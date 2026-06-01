using System.Collections.ObjectModel;
using System.Windows;
using System.Windows.Controls;
using TrustTunnel.Core.Models;
using TrustTunnel.Core.Security;
using TrustTunnel.Core.Validation;

namespace TrustTunnel.Desktop;

public partial class ServerOptionsWindow : Window
{
    private readonly ServerProfile _source;
    private readonly ISecretStore _secretStore;

    public ServerOptionsWindow(ServerProfile profile, ISecretStore secretStore, ObservableCollection<RoutingProfile> routingProfiles)
    {
        InitializeComponent();
        _source = profile;
        _secretStore = secretStore;
        RoutingProfileComboBox.DisplayMemberPath = nameof(RoutingProfile.Name);
        RoutingProfileComboBox.ItemsSource = routingProfiles;
        LoadProfile(profile, routingProfiles);
        _ = LoadSecretsAsync(profile);
    }

    private async Task LoadSecretsAsync(ServerProfile profile)
    {
        PasswordBox.Password = await ReadSecretAsync(profile.Endpoint.PasswordSecretRef);
        ClientRandomBox.Password = await ReadSecretAsync(profile.Endpoint.ClientRandomSecretRef);
        SocksPasswordBox.Password = await ReadSecretAsync(profile.Listener.Socks.PasswordSecretRef);
    }

    private async Task<string> ReadSecretAsync(string secretRef)
    {
        return string.IsNullOrWhiteSpace(secretRef)
            ? ""
            : await _secretStore.ReadAsync(secretRef) ?? "";
    }

    public ServerProfile? ResultProfile { get; private set; }

    private void LoadProfile(ServerProfile profile, ObservableCollection<RoutingProfile> routingProfiles)
    {
        NameTextBox.Text = profile.DisplayName;
        HostnameTextBox.Text = profile.Endpoint.Hostname;
        CustomSniTextBox.Text = profile.Endpoint.CustomSni;
        AddressesTextBox.Text = string.Join(Environment.NewLine, profile.Endpoint.Addresses);
        UsernameTextBox.Text = profile.Endpoint.Username;
        ProtocolComboBox.SelectedIndex = profile.Endpoint.UpstreamProtocol == UpstreamProtocol.Http3 ? 1 : 0;
        ListenerComboBox.SelectedIndex = profile.Listener.Mode == ListenerMode.Socks ? 1 : 0;
        FallbackComboBox.SelectedIndex = profile.Endpoint.FallbackProtocol switch
        {
            UpstreamProtocol.Http2 => 1,
            UpstreamProtocol.Http3 => 2,
            _ => 0
        };

        HasIpv6CheckBox.IsChecked = profile.Endpoint.HasIpv6;
        PostQuantumCheckBox.IsChecked = profile.Endpoint.PostQuantumGroupEnabled;
        AntiDpiCheckBox.IsChecked = profile.Endpoint.AntiDpi;
        SkipVerificationCheckBox.IsChecked = profile.Endpoint.SkipVerification;
        KillSwitchCheckBox.IsChecked = profile.Routing.KillSwitchEnabled;
        ChangeSystemDnsCheckBox.IsChecked = profile.Listener.Tun.ChangeSystemDns;
        UseExistingTunCheckBox.IsChecked = profile.Listener.Tun.UseExisting;
        ServiceModeCheckBox.IsChecked = profile.ServiceModeEnabled;
        AutoConnectCheckBox.IsChecked = profile.AutoConnect;

        RoutingProfileComboBox.SelectedItem = routingProfiles.FirstOrDefault(item => item.Id == profile.RoutingProfileId);
        RoutingModeComboBox.SelectedIndex = profile.Routing.Mode == RoutingMode.Selective ? 1 : 0;
        DnsTextBox.Text = string.Join(Environment.NewLine, profile.Endpoint.DnsUpstreams);
        ExclusionsTextBox.Text = string.Join(Environment.NewLine, profile.Routing.Exclusions);
        IncludedRoutesTextBox.Text = string.Join(Environment.NewLine, profile.Listener.Tun.IncludedRoutes);
        ExcludedRoutesTextBox.Text = string.Join(Environment.NewLine, profile.Listener.Tun.ExcludedRoutes);
        KillSwitchPortsTextBox.Text = string.Join(", ", profile.Routing.KillSwitchAllowPorts);
        MtuTextBox.Text = profile.Listener.Tun.MtuSize.ToString();
        RecvBufferTextBox.Text = profile.Listener.Tun.TcpRecvBufSize.ToString();
        SendBufferTextBox.Text = profile.Listener.Tun.TcpSendBufSize.ToString();

        BoundIfTextBox.Text = profile.Listener.Tun.BoundIf;
        DeviceNameTextBox.Text = profile.Listener.Tun.DeviceName;
        SocksAddressTextBox.Text = profile.Listener.Socks.Address;
        SocksAllowLanCheckBox.IsChecked = profile.Listener.Socks.AllowLanAccess;
        SocksUsernameTextBox.Text = profile.Listener.Socks.Username;
        HttpProxyAddressTextBox.Text = profile.Listener.Socks.HttpProxyAddress;
        HttpProxyAllowLanCheckBox.IsChecked = profile.Listener.Socks.HttpProxyAllowLanAccess;
        CertificatePemTextBox.Text = profile.Endpoint.CertificatePem;
    }

    private async void SaveButton_Click(object sender, RoutedEventArgs e)
    {
        var profile = await BuildProfileAsync();
        if (!Validate(profile, true))
        {
            return;
        }

        ResultProfile = profile;
        DialogResult = true;
    }

    private async Task<ServerProfile> BuildProfileAsync()
    {
        var scope = $"profile/{_source.Id}";
        var passwordRef = _source.Endpoint.PasswordSecretRef;
        var password = SecretValue(PasswordBox, PasswordRevealTextBox);
        if (!string.IsNullOrWhiteSpace(password))
        {
            passwordRef = await _secretStore.SaveAsync(scope, "password", password);
        }

        var clientRandomRef = _source.Endpoint.ClientRandomSecretRef;
        var clientRandom = SecretValue(ClientRandomBox, ClientRandomRevealTextBox);
        if (!string.IsNullOrWhiteSpace(clientRandom))
        {
            clientRandomRef = await _secretStore.SaveAsync(scope, "client_random", clientRandom);
        }

        var socksPasswordRef = _source.Listener.Socks.PasswordSecretRef;
        var socksPassword = SecretValue(SocksPasswordBox, SocksPasswordRevealTextBox);
        if (!string.IsNullOrWhiteSpace(socksPassword))
        {
            socksPasswordRef = await _secretStore.SaveAsync(scope, "socks_password", socksPassword);
        }

        var selectedRouting = RoutingProfileComboBox.SelectedItem as RoutingProfile;
        var routing = (selectedRouting ?? _source.Routing) with
        {
            Mode = RoutingModeComboBox.SelectedIndex == 1 ? RoutingMode.Selective : RoutingMode.General,
            KillSwitchEnabled = KillSwitchCheckBox.IsChecked == true,
            KillSwitchAllowPorts = UiParsing.Ports(KillSwitchPortsTextBox.Text),
            Exclusions = UiParsing.TextList(ExclusionsTextBox.Text)
        };

        return _source with
        {
            DisplayName = UiParsing.EmptyTo(NameTextBox.Text.Trim(), HostnameTextBox.Text.Trim()),
            ServiceModeEnabled = ServiceModeCheckBox.IsChecked == true,
            AutoConnect = AutoConnectCheckBox.IsChecked == true,
            RoutingProfileId = routing.Id,
            Routing = routing,
            Endpoint = _source.Endpoint with
            {
                Hostname = HostnameTextBox.Text.Trim(),
                CustomSni = CustomSniTextBox.Text.Trim(),
                Addresses = UiParsing.TextList(AddressesTextBox.Text),
                Username = UsernameTextBox.Text.Trim(),
                PasswordSecretRef = passwordRef,
                ClientRandomSecretRef = clientRandomRef,
                HasIpv6 = HasIpv6CheckBox.IsChecked == true,
                PostQuantumGroupEnabled = PostQuantumCheckBox.IsChecked == true,
                AntiDpi = AntiDpiCheckBox.IsChecked == true,
                SkipVerification = SkipVerificationCheckBox.IsChecked == true,
                UpstreamProtocol = ProtocolComboBox.SelectedIndex == 1 ? UpstreamProtocol.Http3 : UpstreamProtocol.Http2,
                FallbackProtocol = FallbackComboBox.SelectedIndex switch
                {
                    1 => UpstreamProtocol.Http2,
                    2 => UpstreamProtocol.Http3,
                    _ => null
                },
                DnsUpstreams = UiParsing.TextList(DnsTextBox.Text),
                CertificatePem = CertificatePemTextBox.Text.Trim()
            },
            Listener = _source.Listener with
            {
                Mode = ListenerComboBox.SelectedIndex == 1 ? ListenerMode.Socks : ListenerMode.Tun,
                Tun = _source.Listener.Tun with
                {
                    BoundIf = BoundIfTextBox.Text.Trim(),
                    IncludedRoutes = UiParsing.TextList(IncludedRoutesTextBox.Text),
                    ExcludedRoutes = UiParsing.TextList(ExcludedRoutesTextBox.Text),
                    MtuSize = UiParsing.IntOr(MtuTextBox.Text, 1280),
                    TcpRecvBufSize = UiParsing.IntOr(RecvBufferTextBox.Text, 0),
                    TcpSendBufSize = UiParsing.IntOr(SendBufferTextBox.Text, 0),
                    ChangeSystemDns = ChangeSystemDnsCheckBox.IsChecked == true,
                    DeviceName = DeviceNameTextBox.Text.Trim(),
                    UseExisting = UseExistingTunCheckBox.IsChecked == true
                },
                Socks = _source.Listener.Socks with
                {
                    Address = UiParsing.EmptyTo(SocksAddressTextBox.Text.Trim(), "127.0.0.1:1080"),
                    Username = SocksUsernameTextBox.Text.Trim(),
                    PasswordSecretRef = socksPasswordRef,
                    AllowLanAccess = SocksAllowLanCheckBox.IsChecked == true,
                    HttpProxyAddress = HttpProxyAddressTextBox.Text.Trim(),
                    HttpProxyAllowLanAccess = HttpProxyAllowLanCheckBox.IsChecked == true
                }
            }
        };
    }

    private bool Validate(ServerProfile profile, bool closing)
    {
        if (profile.Endpoint.SkipVerification)
        {
            var confirmed = AppDialog.Confirm(
                this,
                "Опасная настройка",
                "Проверка сертификата отключена. Это снижает безопасность подключения. Сохранить?",
                "Сохранить",
                "Отмена",
                AppDialogTone.Warning);
            if (!confirmed)
            {
                return false;
            }
        }

        var report = TrustTunnelValidators.Validate(profile, skipVerificationConfirmed: true);
        StatusText.Text = report.Issues.Count == 0
            ? "Профиль валиден."
            : string.Join(Environment.NewLine, report.Issues.Select(issue => $"{issue.Code}: {issue.Message}"));

        if (!report.IsValid && closing)
        {
            AppDialog.Show(this, "Профиль не сохранён", StatusText.Text, AppDialogTone.Warning);
        }

        return report.IsValid;
    }

    private void PasswordRevealButton_Click(object sender, RoutedEventArgs e)
    {
        ToggleSecret(PasswordBox, PasswordRevealTextBox);
    }

    private void ClientRandomRevealButton_Click(object sender, RoutedEventArgs e)
    {
        ToggleSecret(ClientRandomBox, ClientRandomRevealTextBox);
    }

    private void SocksPasswordRevealButton_Click(object sender, RoutedEventArgs e)
    {
        ToggleSecret(SocksPasswordBox, SocksPasswordRevealTextBox);
    }

    private static void ToggleSecret(PasswordBox passwordBox, TextBox textBox)
    {
        if (textBox.Visibility == Visibility.Visible)
        {
            passwordBox.Password = textBox.Text;
            textBox.Visibility = Visibility.Collapsed;
            passwordBox.Visibility = Visibility.Visible;
            return;
        }

        textBox.Text = passwordBox.Password;
        passwordBox.Visibility = Visibility.Collapsed;
        textBox.Visibility = Visibility.Visible;
        textBox.Focus();
    }

    private static string SecretValue(PasswordBox passwordBox, TextBox textBox)
    {
        return textBox.Visibility == Visibility.Visible ? textBox.Text : passwordBox.Password;
    }
}
