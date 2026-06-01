namespace TrustTunnel.Desktop;

public static class UiParsing
{
    public static string[] TextList(string value)
    {
        return value
            .Split(new[] { "\r\n", "\n", ",", ";" }, StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .Where(item => !string.IsNullOrWhiteSpace(item))
            .ToArray();
    }

    public static int[] Ports(string value)
    {
        return TextList(value)
            .Select(item => int.TryParse(item, out var port) ? port : 0)
            .Where(port => port > 0)
            .ToArray();
    }

    public static int IntOr(string value, int fallback)
    {
        return int.TryParse(value, out var parsed) ? parsed : fallback;
    }

    public static string EmptyTo(string value, string fallback)
    {
        return string.IsNullOrWhiteSpace(value) ? fallback : value;
    }
}
