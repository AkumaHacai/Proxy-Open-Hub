using System.Windows;

namespace TrustTunnel.Desktop;

public partial class App : Application
{
    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        DesktopTheme.Apply(new AppearanceSettings());
    }
}
