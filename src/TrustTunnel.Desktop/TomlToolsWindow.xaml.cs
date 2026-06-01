using System.Windows;
using TrustTunnel.Core.Diagnostics;

namespace TrustTunnel.Desktop;

public partial class TomlToolsWindow : Window
{
    private readonly string _fullToml;

    public TomlToolsWindow(string fullToml)
    {
        InitializeComponent();
        _fullToml = fullToml;
        TomlTextBox.Text = RedactingLog.Redact(fullToml);
    }

    private void CopyRedactedButton_Click(object sender, RoutedEventArgs e)
    {
        Clipboard.SetText(TomlTextBox.Text);
    }

    private void CopyFullButton_Click(object sender, RoutedEventArgs e)
    {
        var confirmed = AppDialog.Confirm(this, "Экспорт секретов", "Полный TOML может содержать пароль, client_random и certificate material. Скопировать?", "Скопировать", "Отмена", AppDialogTone.Warning);
        if (confirmed)
        {
            Clipboard.SetText(_fullToml);
        }
    }
}
