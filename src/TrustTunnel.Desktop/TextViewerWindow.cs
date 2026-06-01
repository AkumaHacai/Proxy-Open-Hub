using System.Windows;
using System.Windows.Controls;

namespace TrustTunnel.Desktop;

public sealed class TextViewerWindow : Window
{
    public TextViewerWindow(string title, string text, bool allowCopy)
    {
        Title = title;
        Width = 720;
        Height = 560;
        MinWidth = 520;
        MinHeight = 420;
        WindowStartupLocation = WindowStartupLocation.CenterOwner;

        var root = new Grid { Margin = new Thickness(18) };
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

        var heading = new StackPanel { Margin = new Thickness(0, 0, 0, 14) };
        heading.Children.Add(new TextBlock { Text = title, FontSize = 24, FontWeight = FontWeights.SemiBold });
        heading.Children.Add(new TextBlock { Text = "Содержимое можно скопировать в буфер обмена.", Foreground = (System.Windows.Media.Brush)Application.Current.Resources["MutedTextBrush"] });
        Grid.SetRow(heading, 0);
        root.Children.Add(heading);

        var box = new TextBox
        {
            Text = text,
            IsReadOnly = true,
            FontFamily = new System.Windows.Media.FontFamily("Consolas"),
            AcceptsReturn = true,
            AcceptsTab = true,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto,
            HorizontalScrollBarVisibility = ScrollBarVisibility.Auto
        };
        Grid.SetRow(box, 1);
        root.Children.Add(box);

        var buttons = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Right, Margin = new Thickness(0, 14, 0, 0) };
        if (allowCopy)
        {
            var copy = new Button { Content = "Скопировать" };
            copy.Click += (_, _) => Clipboard.SetText(box.Text);
            buttons.Children.Add(copy);
        }

        var close = new Button { Content = "Закрыть", Style = (Style)Application.Current.Resources["GhostButton"] };
        close.Click += (_, _) => Close();
        buttons.Children.Add(close);
        Grid.SetRow(buttons, 2);
        root.Children.Add(buttons);

        Content = root;
    }
}
