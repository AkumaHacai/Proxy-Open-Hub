using System.Text.RegularExpressions;

namespace TrustTunnel.Core.Diagnostics;

public sealed record LogEvent(DateTimeOffset Timestamp, string Level, string Message);

public sealed partial class RedactingLog
{
    private readonly List<LogEvent> _events = new();

    public IReadOnlyList<LogEvent> Events => _events;

    public void Info(string message) => Add("info", message);
    public void Error(string message) => Add("error", message);

    public string Export()
    {
        return string.Join(Environment.NewLine, _events.Select(e => $"{e.Timestamp:HH:mm:ss} [{e.Level}] {e.Message}"));
    }

    private void Add(string level, string message)
    {
        _events.Add(new LogEvent(DateTimeOffset.Now, level, Redact(message)));
    }

    public static string Redact(string value)
    {
        var result = TtLinkRegex().Replace(value, "tt://<redacted>");
        result = SecretAssignmentRegex().Replace(result, "$1=<redacted>");
        return result;
    }

    [GeneratedRegex(@"tt://\?\S+")]
    private static partial Regex TtLinkRegex();

    [GeneratedRegex(@"(?i)\b(password|client_random|certificate)\s*=\s*[^;\s]+")]
    private static partial Regex SecretAssignmentRegex();
}
