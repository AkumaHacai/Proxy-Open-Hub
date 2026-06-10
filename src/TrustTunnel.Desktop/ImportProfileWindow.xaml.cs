using System.Windows;
using TrustTunnel.Core.Application;
using TrustTunnel.Core.Models;
using TrustTunnel.Core.Security;

namespace TrustTunnel.Desktop;

public partial class ImportProfileWindow : Window
{
    private readonly TrustTunnelAppService _appService;
    private readonly ISecretStore _secretStore;

    public ImportProfileWindow(TrustTunnelAppService appService, ISecretStore secretStore)
    {
        InitializeComponent();
        DialogChrome.Apply(this);
        _appService = appService;
        _secretStore = secretStore;
        InspectClipboard();
    }

    public ServerProfile? ResultProfile { get; private set; }

    private async void ClipboardImportButton_Click(object sender, RoutedEventArgs e)
    {
        var text = Clipboard.ContainsText() ? Clipboard.GetText() : "";
        await ImportAsync(() => _appService.ImportDeeplinkAsync(text));
    }

    private async void PasteLinkButton_Click(object sender, RoutedEventArgs e)
    {
        if (LinkTextBox.Visibility != Visibility.Visible)
        {
            LinkTextBox.Visibility = Visibility.Visible;
            LinkTextBox.Text = Clipboard.ContainsText() ? Clipboard.GetText() : "";
            LinkTextBox.Focus();
            return;
        }

        await ImportAsync(() => _appService.ImportDeeplinkAsync(LinkTextBox.Text));
    }

    private void TomlButton_Click(object sender, RoutedEventArgs e)
    {
        var window = new TomlImportWindow(_appService) { Owner = this };
        if (window.ShowDialog() == true && window.ResultProfile is { } profile)
        {
            ResultProfile = profile;
            DialogResult = true;
        }
    }

    private void ManualButton_Click(object sender, RoutedEventArgs e)
    {
        var window = new ManualServerWindow(_secretStore) { Owner = this };
        if (window.ShowDialog() == true && window.ResultProfile is { } profile)
        {
            ResultProfile = profile;
            DialogResult = true;
        }
    }

    private async Task ImportAsync(Func<Task<ServerProfile>> import)
    {
        try
        {
            ResultProfile = await import();
            DialogResult = true;
        }
        catch (Exception ex)
        {
            ResultText.Text = ex.Message;
        }
    }

    private void InspectClipboard()
    {
        var clipboardText = Clipboard.ContainsText() ? Clipboard.GetText() : "";
        var hasLink = clipboardText.TrimStart().StartsWith("tt://", StringComparison.OrdinalIgnoreCase);
        ClipboardStatusText.Text = hasLink
            ? LocalizationManager.Instance.Translate("Import.ClipboardFound")
            : LocalizationManager.Instance.Translate("Import.ClipboardMissing");
        ClipboardImportButton.IsEnabled = hasLink;
    }
}
