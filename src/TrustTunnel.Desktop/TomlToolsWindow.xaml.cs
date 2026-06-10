using System.Windows;
using System.Text.RegularExpressions;
using TrustTunnel.Core.Diagnostics;

namespace TrustTunnel.Desktop;

public partial class TomlToolsWindow : Window
{
    private const string RedactedMarker = "<redacted>";

    private string _currentFullToml;
    private bool _showingSensitive;

    public TomlToolsWindow(string fullToml)
    {
        InitializeComponent();
        DialogChrome.Apply(this);
        _currentFullToml = fullToml;
        TomlTextBox.Text = RedactingLog.Redact(fullToml);
    }

    public string? ResultToml { get; private set; }

    private void ShowSensitiveCheckBox_Toggled(object sender, RoutedEventArgs e)
    {
        var shouldShowSensitive = ShowSensitiveCheckBox.IsChecked == true;
        if (shouldShowSensitive == _showingSensitive)
        {
            return;
        }

        SyncFullTomlFromEditor();
        _showingSensitive = shouldShowSensitive;
        TomlTextBox.Text = _showingSensitive
            ? _currentFullToml
            : RedactingLog.Redact(_currentFullToml);
    }

    private void CopyTomlButton_Click(object sender, RoutedEventArgs e)
    {
        SyncFullTomlFromEditor();
        Clipboard.SetText(_currentFullToml);
    }

    private void SaveButton_Click(object sender, RoutedEventArgs e)
    {
        SyncFullTomlFromEditor();
        ResultToml = _currentFullToml;
        DialogResult = true;
    }

    private void SyncFullTomlFromEditor()
    {
        _currentFullToml = _showingSensitive
            ? TomlTextBox.Text
            : RestoreRedactedSecrets(TomlTextBox.Text, _currentFullToml);
    }

    private static string RestoreRedactedSecrets(string editedToml, string sourceFullToml)
    {
        var secrets = ExtractSecretValues(sourceFullToml);
        return SecretAssignmentLineRegex().Replace(editedToml, match =>
        {
            var value = match.Groups["value"].Value;
            if (!value.Contains(RedactedMarker, StringComparison.OrdinalIgnoreCase))
            {
                return match.Value;
            }

            var key = match.Groups["key"].Value.ToLowerInvariant();
            if (!secrets.TryGetValue(key, out var values) || values.Count == 0)
            {
                return match.Value;
            }

            var restoredValue = values.Dequeue();
            return $"{match.Groups["indent"].Value}{match.Groups["key"].Value} = {restoredValue}{match.Groups["comment"].Value}";
        });
    }

    private static Dictionary<string, Queue<string>> ExtractSecretValues(string toml)
    {
        var result = new Dictionary<string, Queue<string>>(StringComparer.OrdinalIgnoreCase);
        foreach (Match match in SecretAssignmentLineRegex().Matches(toml))
        {
            var key = match.Groups["key"].Value.ToLowerInvariant();
            if (!result.TryGetValue(key, out var values))
            {
                values = new Queue<string>();
                result[key] = values;
            }

            values.Enqueue(match.Groups["value"].Value.Trim());
        }

        return result;
    }

    [GeneratedRegex(@"(?im)^(?<indent>\s*)(?<key>password|client_random|certificate)\s*=\s*(?<value>[^\r\n#]+?)(?<comment>\s*(?:#.*)?)$")]
    private static partial Regex SecretAssignmentLineRegex();
}
