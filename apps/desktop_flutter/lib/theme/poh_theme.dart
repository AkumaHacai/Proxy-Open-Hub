import 'package:flutter/material.dart';

/// Shared motion tokens, mirrored from the reference design
/// (`width .34s cubic-bezier(.4,0,.2,1)`, theme `.3s ease`). Use these
/// everywhere so the window resize and the in-window layout share one
/// timing/curve and never drift apart (the "jumping" layout).
class PohMotion {
  const PohMotion._();

  /// compact <-> expanded morph (window + layout).
  static const Duration morph = Duration(milliseconds: 300);

  /// theme / colour cross-fades.
  static const Duration theme = Duration(milliseconds: 320);

  /// hover / small affordances.
  static const Duration quick = Duration(milliseconds: 150);

  /// cubic-bezier(0.4, 0, 0.2, 1) == Flutter's fastOutSlowIn.
  static const Curve standard = Curves.fastOutSlowIn;

  /// entrance / fade-in.
  static const Curve decel = Curves.easeOutCubic;
}

enum PohThemeMode { light, dark }

extension PohThemeModeX on PohThemeMode {
  ThemeMode get materialMode {
    return switch (this) {
      PohThemeMode.light => ThemeMode.light,
      PohThemeMode.dark => ThemeMode.dark,
    };
  }
}

enum PohAccent {
  forest(Color(0xFF1F7A4D)),
  ocean(Color(0xFF2563EB)),
  violet(Color(0xFF7C3AED)),
  graphite(Color(0xFF364153)),
  amber(Color(0xFFC97428));

  const PohAccent(this.color);

  final Color color;
}

@immutable
class PohPalette extends ThemeExtension<PohPalette> {
  const PohPalette({
    required this.background,
    required this.surface,
    required this.subtle,
    required this.text,
    required this.muted,
    required this.border,
    required this.hover,
    required this.input,
    required this.accent,
    required this.accentSoft,
    required this.glow,
    required this.deskStart,
    required this.deskEnd,
  });

  final Color background;
  final Color surface;
  final Color subtle;
  final Color text;
  final Color muted;
  final Color border;
  final Color hover;
  final Color input;
  final Color accent;
  final Color accentSoft;
  final Color glow;
  final Color deskStart;
  final Color deskEnd;

  static PohPalette of(BuildContext context) {
    return Theme.of(context).extension<PohPalette>()!;
  }

  static PohPalette light(Color accent) {
    final color = accent;
    return PohPalette(
      background: const Color(0xFFF6F7F5),
      surface: Colors.white,
      subtle: const Color(0xFFF2F3F0),
      text: const Color(0xFF111513),
      muted: const Color(0xFF626B65),
      border: const Color(0xFFDDE2DA),
      hover: const Color(0xFFE7E9E5),
      input: const Color(0xFFFBFCFA),
      accent: color,
      accentSoft: color.withValues(alpha: 0.12),
      glow: color.withValues(alpha: 0.30),
      deskStart: const Color(0xFFEEF0EC),
      deskEnd: const Color(0xFFD8DCD4),
    );
  }

  static PohPalette dark(Color accent) {
    final color = accent;
    return PohPalette(
      background: const Color(0xFF111412),
      surface: const Color(0xFF181B19),
      subtle: const Color(0xFF242724),
      text: const Color(0xFFF0F3EF),
      muted: const Color(0xFFAEB7B0),
      border: const Color(0xFF353A35),
      hover: const Color(0xFF2E332F),
      input: const Color(0xFF111412),
      accent: color,
      accentSoft: color.withValues(alpha: 0.22),
      glow: color.withValues(alpha: 0.45),
      deskStart: const Color(0xFF1B1F1C),
      deskEnd: const Color(0xFF0C0E0C),
    );
  }

  @override
  PohPalette copyWith({
    Color? background,
    Color? surface,
    Color? subtle,
    Color? text,
    Color? muted,
    Color? border,
    Color? hover,
    Color? input,
    Color? accent,
    Color? accentSoft,
    Color? glow,
    Color? deskStart,
    Color? deskEnd,
  }) {
    return PohPalette(
      background: background ?? this.background,
      surface: surface ?? this.surface,
      subtle: subtle ?? this.subtle,
      text: text ?? this.text,
      muted: muted ?? this.muted,
      border: border ?? this.border,
      hover: hover ?? this.hover,
      input: input ?? this.input,
      accent: accent ?? this.accent,
      accentSoft: accentSoft ?? this.accentSoft,
      glow: glow ?? this.glow,
      deskStart: deskStart ?? this.deskStart,
      deskEnd: deskEnd ?? this.deskEnd,
    );
  }

  @override
  PohPalette lerp(ThemeExtension<PohPalette>? other, double t) {
    if (other is! PohPalette) {
      return this;
    }

    return PohPalette(
      background: Color.lerp(background, other.background, t)!,
      surface: Color.lerp(surface, other.surface, t)!,
      subtle: Color.lerp(subtle, other.subtle, t)!,
      text: Color.lerp(text, other.text, t)!,
      muted: Color.lerp(muted, other.muted, t)!,
      border: Color.lerp(border, other.border, t)!,
      hover: Color.lerp(hover, other.hover, t)!,
      input: Color.lerp(input, other.input, t)!,
      accent: Color.lerp(accent, other.accent, t)!,
      accentSoft: Color.lerp(accentSoft, other.accentSoft, t)!,
      glow: Color.lerp(glow, other.glow, t)!,
      deskStart: Color.lerp(deskStart, other.deskStart, t)!,
      deskEnd: Color.lerp(deskEnd, other.deskEnd, t)!,
    );
  }
}

ThemeData buildPohTheme({
  required PohThemeMode mode,
  PohAccent? accent,
  Color? accentColor,
}) {
  final seed = accentColor ?? accent?.color ?? PohAccent.forest.color;
  final palette = mode == PohThemeMode.dark
      ? PohPalette.dark(seed)
      : PohPalette.light(seed);

  final base = ThemeData(
    brightness: mode == PohThemeMode.dark ? Brightness.dark : Brightness.light,
    useMaterial3: true,
    fontFamily: 'Inter',
  );

  return base.copyWith(
    scaffoldBackgroundColor: palette.background,
    textTheme: base.textTheme.apply(
      bodyColor: palette.text,
      displayColor: palette.text,
    ),
    colorScheme: base.colorScheme.copyWith(
      primary: palette.accent,
      secondary: palette.accent,
      surface: palette.surface,
      onSurface: palette.text,
    ),
    filledButtonTheme: FilledButtonThemeData(
      style: ButtonStyle(
        backgroundColor: WidgetStateProperty.resolveWith((states) {
          if (states.contains(WidgetState.disabled)) {
            return palette.border;
          }
          return palette.accent;
        }),
        foregroundColor: WidgetStateProperty.all(Colors.white),
        overlayColor: WidgetStateProperty.all(
          Colors.white.withValues(alpha: 0.10),
        ),
        textStyle: WidgetStateProperty.all(
          const TextStyle(fontSize: 13, fontWeight: FontWeight.w900),
        ),
        padding: WidgetStateProperty.all(
          const EdgeInsets.symmetric(horizontal: 24, vertical: 14),
        ),
        shape: WidgetStateProperty.all(
          RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        ),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: ButtonStyle(
        foregroundColor: WidgetStateProperty.all(palette.text),
        side: WidgetStateProperty.all(BorderSide(color: palette.border)),
        overlayColor: WidgetStateProperty.all(palette.hover),
        textStyle: WidgetStateProperty.all(
          const TextStyle(fontSize: 13, fontWeight: FontWeight.w800),
        ),
        padding: WidgetStateProperty.all(
          const EdgeInsets.symmetric(horizontal: 22, vertical: 13),
        ),
        shape: WidgetStateProperty.all(
          RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        ),
      ),
    ),
    textButtonTheme: TextButtonThemeData(
      style: ButtonStyle(
        foregroundColor: WidgetStateProperty.all(palette.text),
        overlayColor: WidgetStateProperty.all(palette.hover),
        textStyle: WidgetStateProperty.all(
          const TextStyle(fontSize: 13, fontWeight: FontWeight.w800),
        ),
        shape: WidgetStateProperty.all(
          RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        ),
      ),
    ),
    iconButtonTheme: IconButtonThemeData(
      style: ButtonStyle(
        foregroundColor: WidgetStateProperty.all(palette.muted),
        overlayColor: WidgetStateProperty.all(palette.hover),
      ),
    ),
    inputDecorationTheme: InputDecorationTheme(
      filled: true,
      fillColor: palette.input,
      labelStyle: TextStyle(color: palette.muted),
      hintStyle: TextStyle(color: palette.muted),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(10),
        borderSide: BorderSide(color: palette.border),
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(10),
        borderSide: BorderSide(color: palette.accent),
      ),
    ),
    tooltipTheme: TooltipThemeData(
      decoration: BoxDecoration(
        color: palette.surface,
        borderRadius: BorderRadius.circular(8),
        border: Border.all(color: palette.border),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withValues(alpha: 0.18),
            blurRadius: 16,
            offset: const Offset(0, 8),
          ),
        ],
      ),
      textStyle: TextStyle(
        color: palette.text,
        fontSize: 12,
        fontWeight: FontWeight.w700,
      ),
    ),
    scrollbarTheme: ScrollbarThemeData(
      thumbColor: WidgetStateProperty.resolveWith((states) {
        final alpha = states.contains(WidgetState.hovered) ? 0.56 : 0.34;
        return palette.muted.withValues(alpha: alpha);
      }),
      trackColor: WidgetStateProperty.all(Colors.transparent),
      trackBorderColor: WidgetStateProperty.all(Colors.transparent),
      thickness: WidgetStateProperty.resolveWith((states) {
        return states.contains(WidgetState.hovered) ? 8 : 6;
      }),
      radius: const Radius.circular(999),
      crossAxisMargin: 3,
      mainAxisMargin: 8,
    ),
    extensions: [palette],
  );
}
