using System.Windows;
using System.Windows.Controls;

namespace TrustTunnel.Desktop;

public partial class SettingsWindow : Window
{
    private int _timeoutSeconds;

    public SettingsWindow(AppSettings settings)
    {
        InitializeComponent();
        DialogChrome.Apply(this);
        LanguageComboBox.ItemsSource = LocalizationManager.Instance.Languages;
        ResultSettings = settings;
        ApplySettingsToControls(settings);
        ShowPage("Appearance");
    }

    public AppSettings ResultSettings { get; private set; }

    private void SettingsNav_Checked(object sender, RoutedEventArgs e)
    {
        if (sender is RadioButton { Tag: string page })
        {
            ShowPage(page);
        }
    }

    private void TimeoutMinusButton_Click(object sender, RoutedEventArgs e)
    {
        _timeoutSeconds = Math.Max(1, _timeoutSeconds - 1);
        UpdateTimeoutText();
    }

    private void TimeoutPlusButton_Click(object sender, RoutedEventArgs e)
    {
        _timeoutSeconds = Math.Min(30, _timeoutSeconds + 1);
        UpdateTimeoutText();
    }

    private void ResetButton_Click(object sender, RoutedEventArgs e)
    {
        ApplySettingsToControls(new AppSettings());
    }

    private void SaveButton_Click(object sender, RoutedEventArgs e)
    {
        ResultSettings = new AppSettings
        {
            Language = SelectedLanguage(),
            Appearance = new AppearanceSettings(
                ThemeDarkRadio.IsChecked == true ? AppThemeMode.Dark : AppThemeMode.Light,
                SelectedAccent(),
                DensityCompactRadio.IsChecked == true ? DensityMode.Compact : DensityMode.Comfortable),
            PingHost = UiParsing.EmptyTo(PingHostTextBox.Text.Trim(), "8.8.8.8"),
            HttpsTestUrl = UiParsing.EmptyTo(HttpsUrlTextBox.Text.Trim(), "https://www.google.com/generate_204"),
            DiagnosticsTimeout = TimeSpan.FromSeconds(Math.Clamp(_timeoutSeconds, 1, 30)),
            DefaultSocksAddress = UiParsing.EmptyTo(SocksAddressTextBox.Text.Trim(), "127.0.0.1:1080"),
            DefaultSocksAllowLan = SocksLanCheckBox.IsChecked == true,
            SystemProxyMode = SystemProxyCheckBox.IsChecked == true ? SystemProxyMode.Socks5 : SystemProxyMode.Off,
            EnableHttpProxyOptions = false,
            DefaultHttpProxyAddress = ResultSettings.DefaultHttpProxyAddress,
            MainWindowMode = ModeCompactRadio.IsChecked == true ? "Compact" : "Expanded"
        };
        DialogResult = true;
    }

    private void ApplySettingsToControls(AppSettings settings)
    {
        LanguageComboBox.SelectedValue = LocalizationManager.Resolve(settings.Language);
        ThemeLightRadio.IsChecked = settings.Appearance.Theme == AppThemeMode.Light;
        ThemeDarkRadio.IsChecked = settings.Appearance.Theme == AppThemeMode.Dark;

        AccentForestRadio.IsChecked = settings.Appearance.Accent == AccentColor.Forest;
        AccentOceanRadio.IsChecked = settings.Appearance.Accent == AccentColor.Ocean;
        AccentVioletRadio.IsChecked = settings.Appearance.Accent == AccentColor.Violet;
        AccentGraphiteRadio.IsChecked = settings.Appearance.Accent == AccentColor.Graphite;

        DensityComfortableRadio.IsChecked = settings.Appearance.Density == DensityMode.Comfortable;
        DensityCompactRadio.IsChecked = settings.Appearance.Density == DensityMode.Compact;

        ModeExpandedRadio.IsChecked = !settings.MainWindowMode.Equals("Compact", StringComparison.OrdinalIgnoreCase);
        ModeCompactRadio.IsChecked = settings.MainWindowMode.Equals("Compact", StringComparison.OrdinalIgnoreCase);

        PingHostTextBox.Text = settings.PingHost;
        HttpsUrlTextBox.Text = settings.HttpsTestUrl;
        _timeoutSeconds = Math.Clamp((int)settings.DiagnosticsTimeout.TotalSeconds, 1, 30);
        UpdateTimeoutText();

        SocksAddressTextBox.Text = settings.DefaultSocksAddress;
        SocksLanCheckBox.IsChecked = settings.DefaultSocksAllowLan;
        SystemProxyCheckBox.IsChecked = settings.SystemProxyMode == SystemProxyMode.Socks5;
    }

    private void ShowPage(string page)
    {
        AppearancePage.Visibility = page == "Appearance" ? Visibility.Visible : Visibility.Collapsed;
        ProxyPage.Visibility = page == "Proxy" ? Visibility.Visible : Visibility.Collapsed;
        DiagnosticsPage.Visibility = page == "Diagnostics" ? Visibility.Visible : Visibility.Collapsed;
        AboutPage.Visibility = page == "About" ? Visibility.Visible : Visibility.Collapsed;
    }

    private AccentColor SelectedAccent() => true switch
    {
        _ when AccentOceanRadio.IsChecked == true => AccentColor.Ocean,
        _ when AccentVioletRadio.IsChecked == true => AccentColor.Violet,
        _ when AccentGraphiteRadio.IsChecked == true => AccentColor.Graphite,
        _ => AccentColor.Forest
    };

    private AppLanguage SelectedLanguage()
    {
        return LanguageComboBox.SelectedValue is AppLanguage language
            ? language
            : LocalizationManager.Resolve(ResultSettings.Language);
    }

    private void UpdateTimeoutText()
    {
        TimeoutValueText.Text = $"{_timeoutSeconds} с";
    }
}
