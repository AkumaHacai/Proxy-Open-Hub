using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace TrustTunnel.Desktop;

public enum AppDialogTone
{
    Info,
    Warning,
    Danger
}

public static class AppDialog
{
    public static void Show(Window? owner, string title, string message, AppDialogTone tone = AppDialogTone.Info)
    {
        ShowInternal(owner, title, message, tone, confirmText: "Понятно", cancelText: null);
    }

    public static bool Confirm(Window? owner, string title, string message, string confirmText = "Да", string cancelText = "Нет", AppDialogTone tone = AppDialogTone.Warning)
    {
        return ShowInternal(owner, title, message, tone, confirmText, cancelText) == true;
    }

    private static bool? ShowInternal(Window? owner, string title, string message, AppDialogTone tone, string confirmText, string? cancelText)
    {
        var window = new Window
        {
            Title = title,
            Owner = owner,
            Width = 440,
            SizeToContent = SizeToContent.Height,
            WindowStartupLocation = owner is null ? WindowStartupLocation.CenterScreen : WindowStartupLocation.CenterOwner,
            ResizeMode = ResizeMode.NoResize,
            ShowInTaskbar = owner is null,
            UseLayoutRounding = true,
            Content = BuildContent(title, message, tone, confirmText, cancelText)
        };
        window.SetResourceReference(Window.BackgroundProperty, "AppBackgroundBrush");
        return window.ShowDialog();
    }

    private static UIElement BuildContent(string title, string message, AppDialogTone tone, string confirmText, string? cancelText)
    {
        var root = new Grid { Margin = new Thickness(18) };
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

        var body = new Border { Padding = new Thickness(16) };
        body.SetResourceReference(Border.StyleProperty, "Card");
        Grid.SetRow(body, 0);
        root.Children.Add(body);

        var contentGrid = new Grid();
        contentGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        contentGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        body.Child = contentGrid;

        var mark = new Border
        {
            Width = 42,
            Height = 42,
            CornerRadius = new CornerRadius(21),
            Margin = new Thickness(0, 0, 14, 0),
            VerticalAlignment = VerticalAlignment.Top
        };
        mark.SetResourceReference(Border.BackgroundProperty, tone == AppDialogTone.Danger ? "DangerSoftBrush" : "AccentSoftBrush");
        contentGrid.Children.Add(mark);

        var markText = new TextBlock
        {
            Text = tone == AppDialogTone.Info ? "i" : "!",
            FontSize = 20,
            FontWeight = FontWeights.Bold,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center
        };
        markText.SetResourceReference(TextBlock.ForegroundProperty, tone == AppDialogTone.Danger ? "DangerBrush" : "AccentBrush");
        mark.Child = markText;

        var textStack = new StackPanel();
        Grid.SetColumn(textStack, 1);
        contentGrid.Children.Add(textStack);

        var titleBlock = new TextBlock
        {
            Text = title,
            FontSize = 18,
            FontWeight = FontWeights.SemiBold,
            TextWrapping = TextWrapping.Wrap
        };
        textStack.Children.Add(titleBlock);

        var messageBlock = new TextBlock
        {
            Text = message,
            Margin = new Thickness(0, 8, 0, 0),
            TextWrapping = TextWrapping.Wrap,
            LineHeight = 19
        };
        messageBlock.SetResourceReference(TextBlock.ForegroundProperty, "MutedTextBrush");
        textStack.Children.Add(messageBlock);

        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            HorizontalAlignment = HorizontalAlignment.Right,
            Margin = new Thickness(0, 14, 0, 0)
        };
        Grid.SetRow(buttons, 1);
        root.Children.Add(buttons);

        if (!string.IsNullOrWhiteSpace(cancelText))
        {
            var cancel = new Button
            {
                Content = cancelText,
                MinWidth = 96,
                IsCancel = true,
                Style = (Style)Application.Current.Resources["GhostButton"]
            };
            cancel.Click += (_, _) => Window.GetWindow(root)!.DialogResult = false;
            buttons.Children.Add(cancel);
        }

        var confirm = new Button
        {
            Content = confirmText,
            MinWidth = 108,
            IsDefault = true
        };
        if (tone == AppDialogTone.Danger)
        {
            confirm.Style = (Style)Application.Current.Resources["DangerButton"];
        }

        confirm.Click += (_, _) => Window.GetWindow(root)!.DialogResult = true;
        buttons.Children.Add(confirm);

        return root;
    }
}
