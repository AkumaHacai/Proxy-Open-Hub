using System.Windows;
using TrustTunnel.Core.Application;
using TrustTunnel.Core.Models;

namespace TrustTunnel.Desktop;

public partial class TomlImportWindow : Window
{
    private readonly TrustTunnelAppService _appService;

    public TomlImportWindow(TrustTunnelAppService appService)
    {
        InitializeComponent();
        _appService = appService;
    }

    public ServerProfile? ResultProfile { get; private set; }

    private async void ImportButton_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            ResultProfile = await _appService.ImportTomlAsync(TomlTextBox.Text);
            DialogResult = true;
        }
        catch (Exception ex)
        {
            ResultText.Text = ex.Message;
        }
    }
}
