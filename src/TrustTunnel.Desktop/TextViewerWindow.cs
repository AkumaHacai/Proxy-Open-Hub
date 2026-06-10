using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

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
        UseLayoutRounding = true;
        SetResourceReference(BackgroundProperty, "AppBackgroundBrush");

        var root = new Grid();
        root.SetResourceReference(Panel.BackgroundProperty, "AppBackgroundBrush");
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        root.RowDefinitions.Add(new RowDefinition { Height = new GridLength(64) });

        var body = new Grid { Margin = new Thickness(20, 18, 20, 14) };
        body.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        body.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        Grid.SetRow(body, 0);
        root.Children.Add(body);

        var heading = new StackPanel { Margin = new Thickness(0, 0, 0, 16) };
        heading.Children.Add(new TextBlock
        {
            Text = title,
            FontSize = 24,
            FontWeight = FontWeights.Bold
        });
        var hint = new TextBlock
        {
            Text = allowCopy
                ? LocalizationManager.Instance.Translate("TextViewer.CopyHint")
                : LocalizationManager.Instance.Translate("TextViewer.ViewHint"),
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(0, 4, 0, 0)
        };
        hint.SetResourceReference(TextBlock.ForegroundProperty, "MutedTextBrush");
        heading.Children.Add(hint);
        Grid.SetRow(heading, 0);
        body.Children.Add(heading);

        var editorCard = new Border
        {
            Style = (Style)Application.Current.Resources["DialogFlatCard"],
            Padding = new Thickness(8)
        };
        var box = new TextBox
        {
            Text = text,
            IsReadOnly = true,
            Style = (Style)Application.Current.Resources["DialogEditorTextBox"]
        };
        editorCard.Child = box;
        Grid.SetRow(editorCard, 1);
        body.Children.Add(editorCard);

        var footer = new Border
        {
            Style = (Style)Application.Current.Resources["DialogFooter"]
        };
        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            HorizontalAlignment = HorizontalAlignment.Right,
            VerticalAlignment = VerticalAlignment.Center
        };
        if (allowCopy)
        {
            var copy = new Button { Content = LocalizationManager.Instance.Translate("TextViewer.Copy") };
            copy.Click += (_, _) => Clipboard.SetText(box.Text);
            buttons.Children.Add(copy);
        }

        var close = new Button
        {
            Content = LocalizationManager.Instance.Translate("Common.Close"),
            Style = (Style)Application.Current.Resources["GhostButton"]
        };
        close.Click += (_, _) => Close();
        buttons.Children.Add(close);
        footer.Child = buttons;
        Grid.SetRow(footer, 1);
        root.Children.Add(footer);

        Content = root;
        DialogChrome.Apply(this);
    }
}
