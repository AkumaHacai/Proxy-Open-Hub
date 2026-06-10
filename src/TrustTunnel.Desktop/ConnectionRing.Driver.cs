using System.Windows;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Media.Effects;
using TrustTunnel.Core.State;

namespace TrustTunnel.Desktop;

public partial class MainWindow
{
    private const double RingFullDash = 100.53;
    private static readonly Duration RingFastDuration = new(TimeSpan.FromMilliseconds(180));
    private static readonly Duration RingMediumDuration = new(TimeSpan.FromMilliseconds(260));
    private RingVisualState? _lastRingVisualState;

    private enum RingVisualState
    {
        Idle,
        Connecting,
        Connected,
        Disconnecting
    }

    private void UpdateRing(ConnectionPhase phase)
    {
        var visualState = phase switch
        {
            ConnectionPhase.Connected => RingVisualState.Connected,
            ConnectionPhase.Preparing or
                ConnectionPhase.Connecting or
                ConnectionPhase.Authenticating or
                ConnectionPhase.Reconnecting => RingVisualState.Connecting,
            ConnectionPhase.Disconnecting => RingVisualState.Disconnecting,
            _ => RingVisualState.Idle
        };

        if (_lastRingVisualState == visualState)
        {
            UpdateRingText(visualState);
            return;
        }

        _lastRingVisualState = visualState;

        switch (visualState)
        {
            case RingVisualState.Connected:
                SetRingTopDotVisible(false);
                SetRingProgress(1.0);
                SetRingConnected(true);
                break;

            case RingVisualState.Connecting:
                SetRingTopDotVisible(true);
                SetRingConnected(false);
                SetRingProgress(0.35);
                break;

            case RingVisualState.Disconnecting:
                SetRingTopDotVisible(true);
                SetRingConnected(false);
                SetRingProgress(0);
                break;

            default:
                SetRingTopDotVisible(true);
                SetRingConnected(false);
                SetRingProgress(0);
                break;
        }

        UpdateRingText(visualState);
    }

    private void UpdateRingText(RingVisualState visualState)
    {
        switch (visualState)
        {
            case RingVisualState.Connected:
                RingCaption.Text = LocalizationManager.Instance.Translate("Main.Disconnect").ToUpperInvariant();
                RingHintText.Text = LocalizationManager.Instance.Translate("Main.Connected");
                break;

            case RingVisualState.Connecting:
                RingCaption.Text = LocalizationManager.Instance.Translate("Main.Connecting").ToUpperInvariant();
                RingHintText.Text = LocalizationManager.Instance.Translate("Main.Connecting");
                break;

            case RingVisualState.Disconnecting:
                RingCaption.Text = LocalizationManager.Instance.Translate("Main.Connect").ToUpperInvariant();
                RingHintText.Text = LocalizationManager.Instance.Translate("Main.Disconnecting");
                break;

            default:
                RingCaption.Text = LocalizationManager.Instance.Translate("Main.Connect").ToUpperInvariant();
                RingHintText.Text = SelectedProfile is null
                    ? LocalizationManager.Instance.Translate("Main.SelectServerAndConnect")
                    : LocalizationManager.Instance.Translate("Main.Ready");
                break;
        }
    }

    private void SetRingProgress(double fraction)
    {
        fraction = Math.Clamp(fraction, 0, 1);
        if (fraction >= 0.995)
        {
            RingProgress.ClearValue(System.Windows.Shapes.Shape.StrokeDashArrayProperty);
            return;
        }

        RingProgress.StrokeDashArray = new DoubleCollection { fraction * RingFullDash, RingFullDash };
    }

    private void SetRingConnected(bool connected)
    {
        if (Application.Current.Resources["AccentGlowColor"] is Color glowColor)
        {
            RingGlow.Color = glowColor;
        }

        AnimateOpacity(RingAccentDisc, connected ? 1 : 0, RingMediumDuration);
        AnimateOpacity(RingGlow, connected ? 0.75 : 0, RingMediumDuration);
        SetRingForeground(connected);
    }

    private void SetRingTopDotVisible(bool visible)
    {
        RingTopDot.BeginAnimation(UIElement.OpacityProperty, null);
        if (visible && RingTopDot.Visibility == Visibility.Visible && RingTopDot.Opacity >= 0.99)
        {
            return;
        }

        if (!visible && RingTopDot.Visibility != Visibility.Visible)
        {
            return;
        }

        if (visible)
        {
            RingTopDot.Visibility = Visibility.Visible;
        }

        var animation = new DoubleAnimation(
            visible ? 1 : 0,
            visible ? RingFastDuration : RingMediumDuration)
        {
            EasingFunction = new CubicEase { EasingMode = visible ? EasingMode.EaseOut : EasingMode.EaseIn }
        };
        if (!visible)
        {
            animation.Completed += (_, _) => RingTopDot.Visibility = Visibility.Hidden;
        }

        RingTopDot.BeginAnimation(UIElement.OpacityProperty, animation);
    }

    private void SetRingForeground(bool connected)
    {
        if (connected)
        {
            RingGlyph.Stroke = Brushes.White;
            RingCaption.Foreground = Brushes.White;
            return;
        }

        RingGlyph.SetResourceReference(System.Windows.Shapes.Shape.StrokeProperty, "MutedTextBrush");
        RingCaption.SetResourceReference(System.Windows.Controls.TextBlock.ForegroundProperty, "MutedTextBrush");
    }

    private static void AnimateOpacity(UIElement element, double targetOpacity, Duration duration)
    {
        element.BeginAnimation(UIElement.OpacityProperty, null);
        if (Math.Abs(element.Opacity - targetOpacity) < 0.001)
        {
            element.Opacity = targetOpacity;
            return;
        }

        element.BeginAnimation(UIElement.OpacityProperty, new DoubleAnimation(targetOpacity, duration)
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }
        });
    }

    private static void AnimateOpacity(DropShadowEffect effect, double targetOpacity, Duration duration)
    {
        effect.BeginAnimation(DropShadowEffect.OpacityProperty, null);
        if (Math.Abs(effect.Opacity - targetOpacity) < 0.001)
        {
            effect.Opacity = targetOpacity;
            return;
        }

        effect.BeginAnimation(DropShadowEffect.OpacityProperty, new DoubleAnimation(targetOpacity, duration)
        {
            EasingFunction = new CubicEase { EasingMode = EasingMode.EaseOut }
        });
    }

    private static Brush GetBrush(string key, Brush fallback)
    {
        return Application.Current.Resources[key] as Brush ?? fallback;
    }
}
