using System.Globalization;
using System.Windows;
using System.Windows.Data;
using System.Windows.Media;

namespace TrustTunnel.Desktop;

public sealed class NotNullVisibilityConverter : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        return value is null ? Visibility.Collapsed : Visibility.Visible;
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        throw new NotSupportedException();
    }
}

public sealed class BoolToVisibilityConverter : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        return value is true ? Visibility.Visible : Visibility.Collapsed;
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        throw new NotSupportedException();
    }
}

public sealed class PingToBrushConverter : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        var ping = value is int number ? number : int.MaxValue;
        var color = ping switch
        {
            < 0 => "#B94444",
            < 100 => "#4CAF50",
            <= 200 => "#C99524",
            _ => "#B94444"
        };

        return new SolidColorBrush((Color)ColorConverter.ConvertFromString(color));
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        throw new NotSupportedException();
    }
}

public sealed class CountryFlagConverter : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        var code = value?.ToString()?.Trim().ToUpperInvariant();
        if (string.IsNullOrWhiteSpace(code) || code.Length != 2 || code.Any(ch => ch is < 'A' or > 'Z'))
        {
            return "??";
        }

        var chars = code.Select(ch => char.ConvertFromUtf32(0x1F1E6 + ch - 'A'));
        return string.Concat(chars);
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        throw new NotSupportedException();
    }
}

public sealed class CountryCodeTextConverter : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        var code = value?.ToString()?.Trim().ToUpperInvariant();
        if (string.IsNullOrWhiteSpace(code) || code.Length != 2 || code.Any(ch => ch is < 'A' or > 'Z'))
        {
            return "TT";
        }

        return code;
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        throw new NotSupportedException();
    }
}

public sealed class MbpsTextConverter : IValueConverter
{
    public object Convert(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        return value is double number && number > 0
            ? number.ToString("0.00", CultureInfo.InvariantCulture)
            : "--";
    }

    public object ConvertBack(object? value, Type targetType, object? parameter, CultureInfo culture)
    {
        throw new NotSupportedException();
    }
}
