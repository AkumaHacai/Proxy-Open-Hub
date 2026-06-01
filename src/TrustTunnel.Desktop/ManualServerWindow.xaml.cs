using System.Windows;
using System.Windows.Controls;
using TrustTunnel.Core.Models;
using TrustTunnel.Core.Security;

namespace TrustTunnel.Desktop;

public partial class ManualServerWindow : Window
{
    private readonly ISecretStore _secretStore;

    public ManualServerWindow(ISecretStore secretStore)
    {
        InitializeComponent();
        _secretStore = secretStore;
    }

    public ServerProfile? ResultProfile { get; private set; }

    private async void SaveButton_Click(object sender, RoutedEventArgs e)
    {
        var id = Guid.NewGuid().ToString("n");
        var hostname = HostnameTextBox.Text.Trim();
        var passwordRef = await _secretStore.SaveAsync($"profile/{id}", "password", SecretValue(PasswordBox, PasswordRevealTextBox));
        ResultProfile = new ServerProfile
        {
            Id = id,
            DisplayName = string.IsNullOrWhiteSpace(NameTextBox.Text) ? hostname : NameTextBox.Text.Trim(),
            Endpoint = new EndpointConfig
            {
                Hostname = hostname,
                Addresses = new[] { AddressTextBox.Text.Trim() },
                Username = UsernameTextBox.Text.Trim(),
                PasswordSecretRef = passwordRef,
                UpstreamProtocol = Http3CheckBox.IsChecked == true ? UpstreamProtocol.Http3 : UpstreamProtocol.Http2,
                DnsUpstreams = CustomDnsCheckBox.IsChecked == true ? new[] { "tls://1.1.1.1" } : Array.Empty<string>()
            },
            Routing = new RoutingProfile { KillSwitchEnabled = KillSwitchCheckBox.IsChecked == true },
            Listener = new ListenerConfig { Mode = ListenerMode.Tun }
        };
        DialogResult = true;
    }

    private void PasswordRevealButton_Click(object sender, RoutedEventArgs e)
    {
        ToggleSecret(PasswordBox, PasswordRevealTextBox);
    }

    private static void ToggleSecret(PasswordBox passwordBox, TextBox textBox)
    {
        if (passwordBox.Visibility == Visibility.Visible)
        {
            textBox.Text = passwordBox.Password;
            passwordBox.Visibility = Visibility.Collapsed;
            textBox.Visibility = Visibility.Visible;
            textBox.Focus();
            textBox.CaretIndex = textBox.Text.Length;
            return;
        }

        passwordBox.Password = textBox.Text;
        textBox.Visibility = Visibility.Collapsed;
        passwordBox.Visibility = Visibility.Visible;
        passwordBox.Focus();
    }

    private static string SecretValue(PasswordBox passwordBox, TextBox textBox)
    {
        return textBox.Visibility == Visibility.Visible ? textBox.Text : passwordBox.Password;
    }
}
