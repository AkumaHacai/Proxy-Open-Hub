using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Shapes;
using System.Windows.Shell;

namespace TrustTunnel.Desktop;

public static class DialogChrome
{
    private const int DwmWindowCornerPreference = 33;
    private const int DwmRoundCorners = 2;

    public static void Apply(Window window)
    {
        if (window.Content is not UIElement content || content is Border { Tag: "DialogChromeRoot" })
        {
            return;
        }

        window.Content = null;

        var resizeBorder = window.ResizeMode == ResizeMode.NoResize
            ? new Thickness(0)
            : new Thickness(6);
        WindowChrome.SetWindowChrome(window, new WindowChrome
        {
            CaptionHeight = 0,
            ResizeBorderThickness = resizeBorder,
            UseAeroCaptionButtons = false,
            GlassFrameThickness = new Thickness(-1),
            NonClientFrameEdges = NonClientFrameEdges.None,
            CornerRadius = new CornerRadius(22)
        });

        window.WindowStyle = WindowStyle.None;
        window.AllowsTransparency = false;
        window.SetResourceReference(Window.BackgroundProperty, "AppBackgroundBrush");
        window.SourceInitialized += (_, _) => ApplyRoundedCorners(window);

        var root = new Border
        {
            Tag = "DialogChromeRoot",
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(22),
            ClipToBounds = true,
            SnapsToDevicePixels = true
        };
        root.SetResourceReference(Border.BackgroundProperty, "AppBackgroundBrush");
        root.SetResourceReference(Border.BorderBrushProperty, "BorderBrush");

        var grid = new Grid();
        grid.RowDefinitions.Add(new RowDefinition { Height = new GridLength(42) });
        grid.RowDefinitions.Add(new RowDefinition { Height = new GridLength(1, GridUnitType.Star) });
        root.Child = grid;

        var titleBar = CreateTitleBar(window);
        Grid.SetRow(titleBar, 0);
        grid.Children.Add(titleBar);

        Grid.SetRow(content, 1);
        grid.Children.Add(content);

        window.Content = root;
    }

    private static UIElement CreateTitleBar(Window window)
    {
        var titleBar = new Grid();
        titleBar.MouseLeftButtonDown += (_, e) => HandleTitleDrag(window, e);
        titleBar.SetResourceReference(Panel.BackgroundProperty, "SurfaceBrush");
        titleBar.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        titleBar.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        titleBar.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });

        var brand = new Image
        {
            Width = 22,
            Height = 22,
            Margin = new Thickness(12, 0, 8, 0),
            VerticalAlignment = VerticalAlignment.Center,
            Stretch = Stretch.Uniform,
            SnapsToDevicePixels = true,
            Source = new BitmapImage(new Uri($"pack://application:,,,/{AppBrand.IconPngPath}", UriKind.Absolute))
        };
        Grid.SetColumn(brand, 0);
        titleBar.Children.Add(brand);

        var title = new TextBlock
        {
            Text = window.Title,
            FontSize = 12.5,
            FontWeight = FontWeights.SemiBold,
            VerticalAlignment = VerticalAlignment.Center,
            TextTrimming = TextTrimming.CharacterEllipsis
        };
        title.SetResourceReference(TextBlock.ForegroundProperty, "TextBrush");
        Grid.SetColumn(title, 1);
        titleBar.Children.Add(title);

        var buttons = new StackPanel
        {
            Orientation = Orientation.Horizontal,
            Margin = new Thickness(0, 0, 8, 0),
            VerticalAlignment = VerticalAlignment.Center
        };
        Grid.SetColumn(buttons, 2);
        titleBar.Children.Add(buttons);

        var close = new Button
        {
            Style = (Style)Application.Current.Resources["TitleBarCloseButton"],
            Width = 30,
            Height = 30,
            MinWidth = 30,
            MinHeight = 30,
            Padding = new Thickness(0),
            Margin = new Thickness(0),
            ToolTip = LocalizationManager.Instance.Translate("Common.Close"),
            Content = CreateClosePath()
        };
        close.Click += (_, _) => window.Close();
        buttons.Children.Add(close);

        return titleBar;
    }

    private static Path CreateClosePath()
    {
        var path = new Path
        {
            Data = Geometry.Parse("M6,6 L18,18 M18,6 L6,18"),
            StrokeThickness = 1.7,
            StrokeStartLineCap = PenLineCap.Round,
            StrokeEndLineCap = PenLineCap.Round,
            Width = 13,
            Height = 13,
            Stretch = Stretch.Uniform
        };
        path.SetBinding(Shape.StrokeProperty, new Binding(nameof(Control.Foreground))
        {
            RelativeSource = new RelativeSource(RelativeSourceMode.FindAncestor, typeof(Button), 1)
        });
        return path;
    }

    private static void HandleTitleDrag(Window window, MouseButtonEventArgs e)
    {
        if (e.ChangedButton != MouseButton.Left)
        {
            return;
        }

        try
        {
            window.DragMove();
        }
        catch (InvalidOperationException)
        {
        }
    }

    private static void ApplyRoundedCorners(Window window)
    {
        try
        {
            var handle = new WindowInteropHelper(window).Handle;
            if (handle == IntPtr.Zero)
            {
                return;
            }

            var preference = DwmRoundCorners;
            _ = DwmSetWindowAttribute(handle, DwmWindowCornerPreference, ref preference, sizeof(int));
        }
        catch (DllNotFoundException)
        {
        }
        catch (EntryPointNotFoundException)
        {
        }
    }

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr hwnd, int attribute, ref int pvAttribute, int cbAttribute);
}
