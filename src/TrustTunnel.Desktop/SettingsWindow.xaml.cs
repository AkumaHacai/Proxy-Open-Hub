using System.Windows;

namespace TrustTunnel.Desktop;

public partial class SettingsWindow : Window
{
    private readonly AppSettings _sourceSettings;

    public SettingsWindow(AppSettings settings)
    {
        InitializeComponent();
        _sourceSettings = settings;
        ThemeComboBox.SelectedIndex = settings.Appearance.Theme == AppThemeMode.Dark ? 1 : 0;
        AccentComboBox.SelectedIndex = (int)settings.Appearance.Accent;
        DensityComboBox.SelectedIndex = settings.Appearance.Density == DensityMode.Compact ? 1 : 0;
        PingHostTextBox.Text = settings.PingHost;
        HttpsUrlTextBox.Text = settings.HttpsTestUrl;
        TimeoutTextBox.Text = ((int)settings.DiagnosticsTimeout.TotalSeconds).ToString();
        SocksAddressTextBox.Text = settings.DefaultSocksAddress;
        SocksLanCheckBox.IsChecked = settings.DefaultSocksAllowLan;
        HttpProxyOptionsCheckBox.IsChecked = settings.EnableHttpProxyOptions;
        HttpProxyAddressTextBox.Text = settings.DefaultHttpProxyAddress;
        ResultSettings = settings;
    }

    public AppSettings ResultSettings { get; private set; }

    private void SaveButton_Click(object sender, RoutedEventArgs e)
    {
        ResultSettings = new AppSettings
        {
            Appearance = new AppearanceSettings(
                ThemeComboBox.SelectedIndex == 1 ? AppThemeMode.Dark : AppThemeMode.Light,
                (AccentColor)Math.Max(0, AccentComboBox.SelectedIndex),
                DensityComboBox.SelectedIndex == 1 ? DensityMode.Compact : DensityMode.Comfortable),
            PingHost = UiParsing.EmptyTo(PingHostTextBox.Text.Trim(), "8.8.8.8"),
            HttpsTestUrl = UiParsing.EmptyTo(HttpsUrlTextBox.Text.Trim(), "https://www.google.com/generate_204"),
            DiagnosticsTimeout = TimeSpan.FromSeconds(Math.Clamp(UiParsing.IntOr(TimeoutTextBox.Text, 5), 1, 30)),
            DefaultSocksAddress = UiParsing.EmptyTo(SocksAddressTextBox.Text.Trim(), "127.0.0.1:1080"),
            DefaultSocksAllowLan = SocksLanCheckBox.IsChecked == true,
            EnableHttpProxyOptions = HttpProxyOptionsCheckBox.IsChecked == true,
            DefaultHttpProxyAddress = UiParsing.EmptyTo(HttpProxyAddressTextBox.Text.Trim(), "127.0.0.1:8080"),
            MainWindowMode = _sourceSettings.MainWindowMode
        };
        DialogResult = true;
    }
}
