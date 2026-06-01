using System.Windows;

namespace TrustTunnel.Desktop;

public partial class DiagnosticsWindow : Window
{
    private readonly IReadOnlyList<ServerDiagnosticResult> _results;

    public DiagnosticsWindow(IReadOnlyList<ServerDiagnosticResult> results)
    {
        InitializeComponent();
        _results = results;
        ResultsList.ItemsSource = _results;
    }

    private void CopyButton_Click(object sender, RoutedEventArgs e)
    {
        Clipboard.SetText(string.Join(Environment.NewLine, _results.Select(result => $"{result.Name}: {(result.Success ? "OK" : "FAIL")} {result.Message} ({result.Elapsed})")));
    }
}
