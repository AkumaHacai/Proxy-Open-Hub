using System.Windows;
using System.Windows.Media;

namespace TrustTunnel.Desktop;

public enum AppThemeMode
{
    Light,
    Dark
}

public enum AccentColor
{
    Forest,
    Ocean,
    Violet,
    Graphite
}

public enum DensityMode
{
    Comfortable,
    Compact
}

public sealed record AppearanceSettings(
    AppThemeMode Theme = AppThemeMode.Light,
    AccentColor Accent = AccentColor.Forest,
    DensityMode Density = DensityMode.Comfortable);

public static class DesktopTheme
{
    public static void Apply(AppearanceSettings settings)
    {
        var resources = Application.Current.Resources;
        var dark = settings.Theme == AppThemeMode.Dark;
        var accent = settings.Accent switch
        {
            AccentColor.Ocean => Color.FromRgb(37, 99, 235),
            AccentColor.Violet => Color.FromRgb(109, 40, 217),
            AccentColor.Graphite => Color.FromRgb(54, 65, 83),
            _ => Color.FromRgb(31, 122, 77)
        };

        resources["AppBackgroundBrush"] = Brush(dark ? "#111412" : "#F6F7F5");
        resources["SurfaceBrush"] = Brush(dark ? "#181B19" : "#FFFFFF");
        resources["SubtleSurfaceBrush"] = Brush(dark ? "#242724" : "#F2F3F0");
        resources["TextBrush"] = Brush(dark ? "#F0F3EF" : "#111513");
        resources["MutedTextBrush"] = Brush(dark ? "#AEB7B0" : "#626B65");
        resources["BorderBrush"] = Brush(dark ? "#353A35" : "#DDE2DA");
        resources["AccentBrush"] = new SolidColorBrush(accent);
        resources["AccentHoverBrush"] = new SolidColorBrush(Darken(accent));
        resources["AccentSoftBrush"] = new SolidColorBrush(Color.FromArgb(dark ? (byte)54 : (byte)32, accent.R, accent.G, accent.B));
        resources["AccentBorderBrush"] = new SolidColorBrush(Color.FromArgb((byte)90, accent.R, accent.G, accent.B));
        resources["AccentTextBrush"] = Brushes.White;
        resources["ControlHoverBrush"] = Brush(dark ? "#2E332F" : "#E7E9E5");
        resources["InputBackgroundBrush"] = Brush(dark ? "#111412" : "#FBFCFA");
        resources["SelectionBrush"] = new SolidColorBrush(Color.FromArgb(dark ? (byte)70 : (byte)45, accent.R, accent.G, accent.B));
        resources["SecondaryButtonBrush"] = Brush(dark ? "#3A403B" : "#59615B");
        resources["DangerBrush"] = Brush("#B94444");
        resources["DangerHoverBrush"] = Brush("#963636");
        resources["DangerSoftBrush"] = Brush(dark ? "#3A2424" : "#F4E2E2");

        resources["RingTrackBrush"] = Brush(dark ? "#3A403B" : "#E3E6E0");
        resources["RingDiscBrush"] = Brush(dark ? "#1E211F" : "#FFFFFF");
        resources["ElevationColor"] = dark ? Color.FromArgb(170, 0, 0, 0) : Color.FromArgb(38, 17, 21, 19);
        resources["AccentGlowColor"] = Color.FromArgb(dark ? (byte)150 : (byte)96, accent.R, accent.G, accent.B);
        resources["AccentBrushColor"] = accent;
        resources["PingGoodBrush"] = Brush(dark ? "#5FC093" : "#2F7D59");
        resources["PingWarnBrush"] = Brush(dark ? "#D9BD6A" : "#9B7E2D");
        resources["PingBadBrush"] = Brush(dark ? "#DD8A72" : "#A8543C");
        resources["ScrollbarTrackBrush"] = Brush(dark ? "#232824" : "#EEF1ED");
        resources["ScrollbarThumbBrush"] = Brush(dark ? "#566059" : "#C6D0C8");
        resources["ScrollbarThumbHoverBrush"] = Brush(dark ? "#748077" : "#9DAAA1");
        resources["RadiusSm"] = new CornerRadius(10);
        resources["RadiusMd"] = new CornerRadius(14);
        resources["RadiusLg"] = new CornerRadius(20);
        resources["RadiusXl"] = new CornerRadius(26);
        resources["RadiusPill"] = new CornerRadius(999);

        resources["ControlPadding"] = settings.Density == DensityMode.Compact ? new Thickness(10, 6, 10, 6) : new Thickness(12, 8, 12, 8);
        resources["FieldMargin"] = settings.Density == DensityMode.Compact ? new Thickness(0, 3, 0, 7) : new Thickness(0, 4, 0, 10);
    }

    private static Color Darken(Color color)
    {
        return Color.FromRgb((byte)(color.R * 0.78), (byte)(color.G * 0.78), (byte)(color.B * 0.78));
    }

    private static SolidColorBrush Brush(string hex) => new((Color)ColorConverter.ConvertFromString(hex));
}
