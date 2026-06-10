using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Effects;

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
        ShowInternal(owner, title, message, tone, confirmText: LocalizationManager.Instance.Translate("Common.Close"), cancelText: null);
    }

    public static bool Confirm(Window? owner, string title, string message, string? confirmText = null, string? cancelText = null, AppDialogTone tone = AppDialogTone.Warning)
    {
        return ShowInternal(
            owner,
            title,
            message,
            tone,
            confirmText ?? LocalizationManager.Instance.Translate("Common.Apply"),
            cancelText ?? LocalizationManager.Instance.Translate("Common.Cancel")) == true;
    }

    private static bool? ShowInternal(Window? owner, string title, string message, AppDialogTone tone, string confirmText, string? cancelText)
    {
        if (tone == AppDialogTone.Danger && cancelText is not null)
        {
            return ShowCompactDanger(owner, title, message, confirmText, cancelText);
        }

        var window = new Window
        {
            Title = title,
            Owner = owner,
            Width = tone == AppDialogTone.Danger ? 392 : 420,
            SizeToContent = SizeToContent.Height,
            WindowStartupLocation = owner is null ? WindowStartupLocation.CenterScreen : WindowStartupLocation.CenterOwner,
            ResizeMode = ResizeMode.NoResize,
            ShowInTaskbar = owner is null,
            UseLayoutRounding = true,
            Content = BuildContent(title, message, tone, confirmText, cancelText)
        };
        window.SetResourceReference(Window.BackgroundProperty, "AppBackgroundBrush");
        DialogChrome.Apply(window);
        return window.ShowDialog();
    }

    private static bool? ShowCompactDanger(Window? owner, string title, string message, string confirmText, string cancelText)
    {
        var window = new Window
        {
            Title = title,
            Owner = owner,
            Width = 356,
            SizeToContent = SizeToContent.Height,
            WindowStartupLocation = owner is null ? WindowStartupLocation.CenterScreen : WindowStartupLocation.CenterOwner,
            ResizeMode = ResizeMode.NoResize,
            ShowInTaskbar = owner is null,
            WindowStyle = WindowStyle.None,
            AllowsTransparency = true,
            Background = Brushes.Transparent,
            UseLayoutRounding = true,
            Content = BuildCompactDangerContent(title, message, confirmText, cancelText)
        };
        window.KeyDown += (_, e) =>
        {
            if (e.Key != Key.Escape)
            {
                return;
            }

            window.DialogResult = false;
        };
        return window.ShowDialog();
    }

    private static UIElement BuildCompactDangerContent(string title, string message, string confirmText, string cancelText)
    {
        var root = new Border
        {
            Margin = new Thickness(18),
            Padding = new Thickness(16),
            CornerRadius = new CornerRadius(18),
            BorderThickness = new Thickness(1),
            SnapsToDevicePixels = true,
            Effect = new DropShadowEffect
            {
                BlurRadius = 26,
                ShadowDepth = 8,
                Direction = 270,
                Opacity = 0.20,
                Color = Colors.Black
            }
        };
        root.SetResourceReference(Border.BackgroundProperty, "SurfaceBrush");
        root.SetResourceReference(Border.BorderBrushProperty, "BorderBrush");
        root.MouseLeftButtonDown += (_, e) =>
        {
            if (e.ChangedButton != MouseButton.Left)
            {
                return;
            }

            try
            {
                Window.GetWindow(root)?.DragMove();
            }
            catch (InvalidOperationException)
            {
            }
        };

        var layout = new Grid();
        layout.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        layout.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.Child = layout;

        var header = new Grid();
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        header.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        Grid.SetRow(header, 0);
        layout.Children.Add(header);

        var mark = new Border
        {
            Width = 34,
            Height = 34,
            CornerRadius = new CornerRadius(17),
            Margin = new Thickness(0, 1, 12, 0),
            VerticalAlignment = VerticalAlignment.Top
        };
        mark.SetResourceReference(Border.BackgroundProperty, "DangerSoftBrush");
        header.Children.Add(mark);

        var markText = new TextBlock
        {
            Text = "!",
            FontSize = 18,
            FontWeight = FontWeights.Bold,
            HorizontalAlignment = HorizontalAlignment.Center,
            VerticalAlignment = VerticalAlignment.Center
        };
        markText.SetResourceReference(TextBlock.ForegroundProperty, "DangerBrush");
        mark.Child = markText;

        var textStack = new StackPanel();
        Grid.SetColumn(textStack, 1);
        header.Children.Add(textStack);

        var titleBlock = new TextBlock
        {
            Text = title,
            FontSize = 17,
            FontWeight = FontWeights.SemiBold,
            TextWrapping = TextWrapping.Wrap
        };
        titleBlock.SetResourceReference(TextBlock.ForegroundProperty, "TextBrush");
        textStack.Children.Add(titleBlock);

        var messageBlock = new TextBlock
        {
            Text = message,
            Margin = new Thickness(0, 5, 0, 0),
            FontSize = 12.5,
            LineHeight = 17,
            TextWrapping = TextWrapping.Wrap
        };
        messageBlock.SetResourceReference(TextBlock.ForegroundProperty, "MutedTextBrush");
        textStack.Children.Add(messageBlock);

        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            HorizontalAlignment = HorizontalAlignment.Right,
            Margin = new Thickness(0, 16, 0, 0)
        };
        Grid.SetRow(buttons, 1);
        layout.Children.Add(buttons);

        var cancel = new Button
        {
            Content = cancelText,
            MinWidth = 92,
            IsCancel = true,
            Style = (Style)Application.Current.Resources["GhostButton"],
            Margin = new Thickness(0, 0, 8, 0)
        };
        cancel.Click += (_, _) => Window.GetWindow(root)!.DialogResult = false;
        buttons.Children.Add(cancel);

        var confirm = new Button
        {
            Content = confirmText,
            MinWidth = 104,
            IsDefault = true,
            Style = (Style)Application.Current.Resources["DangerButton"]
        };
        confirm.Click += (_, _) => Window.GetWindow(root)!.DialogResult = true;
        buttons.Children.Add(confirm);

        return root;
    }

    private static UIElement BuildContent(string title, string message, AppDialogTone tone, string confirmText, string? cancelText)
    {
        var root = new Grid { Margin = new Thickness(16, 14, 16, 16) };
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        root.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });

        var body = new Border { Padding = new Thickness(14) };
        body.SetResourceReference(Border.StyleProperty, "Card");
        Grid.SetRow(body, 0);
        root.Children.Add(body);

        var contentGrid = new Grid();
        contentGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        contentGrid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        body.Child = contentGrid;

        var mark = new Border
        {
            Width = 36,
            Height = 36,
            CornerRadius = new CornerRadius(18),
            Margin = new Thickness(0, 1, 12, 0),
            VerticalAlignment = VerticalAlignment.Top
        };
        mark.SetResourceReference(Border.BackgroundProperty, tone == AppDialogTone.Danger ? "DangerSoftBrush" : "AccentSoftBrush");
        contentGrid.Children.Add(mark);

        var markText = new TextBlock
        {
            Text = tone == AppDialogTone.Info ? "i" : "!",
            FontSize = 18,
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
            FontSize = 17,
            FontWeight = FontWeights.SemiBold,
            TextWrapping = TextWrapping.Wrap
        };
        textStack.Children.Add(titleBlock);

        var messageBlock = new TextBlock
        {
            Text = message,
            Margin = new Thickness(0, 6, 0, 0),
            TextWrapping = TextWrapping.Wrap,
            LineHeight = 18
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
                Style = (Style)Application.Current.Resources["GhostButton"],
                Margin = new Thickness(0, 0, 10, 0)
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
