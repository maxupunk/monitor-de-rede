import 'package:flutter/material.dart';

/// Tokens semânticos obrigatórios do NetMonitor (AGENTS.md §12).
/// Sem cinza desbotado para estado ou ação; `plugin` é token fixo de produto.
enum SemanticTone {
  primary,
  secondary,
  success,
  warning,
  info,
  error,
  caution,
  plugin,
  ai,
}

@immutable
class SemanticColorPair {
  const SemanticColorPair({
    required this.foreground,
    required this.onForeground,
    required this.container,
    required this.onContainer,
    required this.border,
  });

  final Color foreground;
  final Color onForeground;
  final Color container;
  final Color onContainer;
  final Color border;
}

@immutable
class AppSemanticColors extends ThemeExtension<AppSemanticColors> {
  const AppSemanticColors({
    required this.success,
    required this.onSuccess,
    required this.successContainer,
    required this.onSuccessContainer,
    required this.warning,
    required this.onWarning,
    required this.warningContainer,
    required this.onWarningContainer,
    required this.info,
    required this.onInfo,
    required this.infoContainer,
    required this.onInfoContainer,
    required this.error,
    required this.onError,
    required this.errorContainer,
    required this.onErrorContainer,
    required this.caution,
    required this.onCaution,
    required this.cautionContainer,
    required this.onCautionContainer,
    required this.plugin,
    required this.onPlugin,
    required this.pluginContainer,
    required this.onPluginContainer,
    required this.ai,
    required this.onAi,
    required this.aiContainer,
    required this.onAiContainer,
  });

  final Color success;
  final Color onSuccess;
  final Color successContainer;
  final Color onSuccessContainer;

  final Color warning;
  final Color onWarning;
  final Color warningContainer;
  final Color onWarningContainer;

  final Color info;
  final Color onInfo;
  final Color infoContainer;
  final Color onInfoContainer;

  final Color error;
  final Color onError;
  final Color errorContainer;
  final Color onErrorContainer;

  final Color caution;
  final Color onCaution;
  final Color cautionContainer;
  final Color onCautionContainer;

  /// Cor exclusiva de tudo que vem de plugin (AGENTS.md §12).
  final Color plugin;
  final Color onPlugin;
  final Color pluginContainer;
  final Color onPluginContainer;

  /// Cor de destaque exclusiva para recursos de IA (`AiAskButton`, `AiAlertSummary`).
  final Color ai;
  final Color onAi;
  final Color aiContainer;
  final Color onAiContainer;

  static const AppSemanticColors dark = AppSemanticColors(
    success: Color(0xFF4CAF50),
    onSuccess: Color(0xFF042107),
    successContainer: Color(0xFF143518),
    onSuccessContainer: Color(0xFFA5F2A9),
    warning: Color(0xFFFFC107),
    onWarning: Color(0xFF261A00),
    warningContainer: Color(0xFF3E2E00),
    onWarningContainer: Color(0xFFFFE082),
    info: Color(0xFF2196F3),
    onInfo: Color(0xFF001E36),
    infoContainer: Color(0xFF0D2D4A),
    onInfoContainer: Color(0xFF90CAF9),
    error: Color(0xFFFF5252),
    onError: Color(0xFF2E0002),
    errorContainer: Color(0xFF4A1215),
    onErrorContainer: Color(0xFFFFB4AB),
    caution: Color(0xFFFF9100),
    onCaution: Color(0xFF2A1500),
    cautionContainer: Color(0xFF452400),
    onCautionContainer: Color(0xFFFFCC80),
    plugin: Color(0xFF1DE9B6),
    onPlugin: Color(0xFF00291F),
    pluginContainer: Color(0xFF083A2E),
    onPluginContainer: Color(0xFFA7FFEB),
    ai: Color(0xFFCE93D8),
    onAi: Color(0xFF31003F),
    aiContainer: Color(0xFF3D164A),
    onAiContainer: Color(0xFFF3D5FA),
  );

  static const AppSemanticColors light = AppSemanticColors(
    success: Color(0xFF2E7D32),
    onSuccess: Color(0xFFFFFFFF),
    successContainer: Color(0xFFDCEFD9),
    onSuccessContainer: Color(0xFF133B16),
    warning: Color(0xFFB26A00),
    onWarning: Color(0xFFFFFFFF),
    warningContainer: Color(0xFFFFEBC2),
    onWarningContainer: Color(0xFF4A2B00),
    info: Color(0xFF0277BD),
    onInfo: Color(0xFFFFFFFF),
    infoContainer: Color(0xFFD6EEFC),
    onInfoContainer: Color(0xFF003252),
    error: Color(0xFFC62828),
    onError: Color(0xFFFFFFFF),
    errorContainer: Color(0xFFFCDADA),
    onErrorContainer: Color(0xFF5A0B0B),
    caution: Color(0xFFE65100),
    onCaution: Color(0xFFFFFFFF),
    cautionContainer: Color(0xFFFFE0CC),
    onCautionContainer: Color(0xFF521B00),
    plugin: Color(0xFF00796B),
    onPlugin: Color(0xFFFFFFFF),
    pluginContainer: Color(0xFFD4F2EE),
    onPluginContainer: Color(0xFF00332C),
    ai: Color(0xFF7B1FA2),
    onAi: Color(0xFFFFFFFF),
    aiContainer: Color(0xFFF3E0FA),
    onAiContainer: Color(0xFF38004D),
  );

  SemanticColorPair resolve(SemanticTone tone, ColorScheme scheme) {
    switch (tone) {
      case SemanticTone.primary:
        return SemanticColorPair(
          foreground: scheme.primary,
          onForeground: scheme.onPrimary,
          container: scheme.primaryContainer,
          onContainer: scheme.onPrimaryContainer,
          border: scheme.primary.withValues(alpha: 0.45),
        );
      case SemanticTone.secondary:
        return SemanticColorPair(
          foreground: scheme.secondary,
          onForeground: scheme.onSecondary,
          container: scheme.secondaryContainer,
          onContainer: scheme.onSecondaryContainer,
          border: scheme.secondary.withValues(alpha: 0.45),
        );
      case SemanticTone.success:
        return SemanticColorPair(
          foreground: success,
          onForeground: onSuccess,
          container: successContainer,
          onContainer: onSuccessContainer,
          border: success.withValues(alpha: 0.45),
        );
      case SemanticTone.warning:
        return SemanticColorPair(
          foreground: warning,
          onForeground: onWarning,
          container: warningContainer,
          onContainer: onWarningContainer,
          border: warning.withValues(alpha: 0.45),
        );
      case SemanticTone.info:
        return SemanticColorPair(
          foreground: info,
          onForeground: onInfo,
          container: infoContainer,
          onContainer: onInfoContainer,
          border: info.withValues(alpha: 0.45),
        );
      case SemanticTone.error:
        return SemanticColorPair(
          foreground: error,
          onForeground: onError,
          container: errorContainer,
          onContainer: onErrorContainer,
          border: error.withValues(alpha: 0.45),
        );
      case SemanticTone.caution:
        return SemanticColorPair(
          foreground: caution,
          onForeground: onCaution,
          container: cautionContainer,
          onContainer: onCautionContainer,
          border: caution.withValues(alpha: 0.45),
        );
      case SemanticTone.plugin:
        return SemanticColorPair(
          foreground: plugin,
          onForeground: onPlugin,
          container: pluginContainer,
          onContainer: onPluginContainer,
          border: plugin.withValues(alpha: 0.45),
        );
      case SemanticTone.ai:
        return SemanticColorPair(
          foreground: ai,
          onForeground: onAi,
          container: aiContainer,
          onContainer: onAiContainer,
          border: ai.withValues(alpha: 0.45),
        );
    }
  }

  @override
  AppSemanticColors copyWith({
    Color? success,
    Color? onSuccess,
    Color? successContainer,
    Color? onSuccessContainer,
    Color? warning,
    Color? onWarning,
    Color? warningContainer,
    Color? onWarningContainer,
    Color? info,
    Color? onInfo,
    Color? infoContainer,
    Color? onInfoContainer,
    Color? error,
    Color? onError,
    Color? errorContainer,
    Color? onErrorContainer,
    Color? caution,
    Color? onCaution,
    Color? cautionContainer,
    Color? onCautionContainer,
    Color? plugin,
    Color? onPlugin,
    Color? pluginContainer,
    Color? onPluginContainer,
    Color? ai,
    Color? onAi,
    Color? aiContainer,
    Color? onAiContainer,
  }) {
    return AppSemanticColors(
      success: success ?? this.success,
      onSuccess: onSuccess ?? this.onSuccess,
      successContainer: successContainer ?? this.successContainer,
      onSuccessContainer: onSuccessContainer ?? this.onSuccessContainer,
      warning: warning ?? this.warning,
      onWarning: onWarning ?? this.onWarning,
      warningContainer: warningContainer ?? this.warningContainer,
      onWarningContainer: onWarningContainer ?? this.onWarningContainer,
      info: info ?? this.info,
      onInfo: onInfo ?? this.onInfo,
      infoContainer: infoContainer ?? this.infoContainer,
      onInfoContainer: onInfoContainer ?? this.onInfoContainer,
      error: error ?? this.error,
      onError: onError ?? this.onError,
      errorContainer: errorContainer ?? this.errorContainer,
      onErrorContainer: onErrorContainer ?? this.onErrorContainer,
      caution: caution ?? this.caution,
      onCaution: onCaution ?? this.onCaution,
      cautionContainer: cautionContainer ?? this.cautionContainer,
      onCautionContainer: onCautionContainer ?? this.onCautionContainer,
      plugin: plugin ?? this.plugin,
      onPlugin: onPlugin ?? this.onPlugin,
      pluginContainer: pluginContainer ?? this.pluginContainer,
      onPluginContainer: onPluginContainer ?? this.onPluginContainer,
      ai: ai ?? this.ai,
      onAi: onAi ?? this.onAi,
      aiContainer: aiContainer ?? this.aiContainer,
      onAiContainer: onAiContainer ?? this.onAiContainer,
    );
  }

  @override
  AppSemanticColors lerp(ThemeExtension<AppSemanticColors>? other, double t) {
    if (other is! AppSemanticColors) return this;
    return AppSemanticColors(
      success: Color.lerp(success, other.success, t)!,
      onSuccess: Color.lerp(onSuccess, other.onSuccess, t)!,
      successContainer: Color.lerp(
        successContainer,
        other.successContainer,
        t,
      )!,
      onSuccessContainer: Color.lerp(
        onSuccessContainer,
        other.onSuccessContainer,
        t,
      )!,
      warning: Color.lerp(warning, other.warning, t)!,
      onWarning: Color.lerp(onWarning, other.onWarning, t)!,
      warningContainer: Color.lerp(
        warningContainer,
        other.warningContainer,
        t,
      )!,
      onWarningContainer: Color.lerp(
        onWarningContainer,
        other.onWarningContainer,
        t,
      )!,
      info: Color.lerp(info, other.info, t)!,
      onInfo: Color.lerp(onInfo, other.onInfo, t)!,
      infoContainer: Color.lerp(infoContainer, other.infoContainer, t)!,
      onInfoContainer: Color.lerp(onInfoContainer, other.onInfoContainer, t)!,
      error: Color.lerp(error, other.error, t)!,
      onError: Color.lerp(onError, other.onError, t)!,
      errorContainer: Color.lerp(errorContainer, other.errorContainer, t)!,
      onErrorContainer: Color.lerp(
        onErrorContainer,
        other.onErrorContainer,
        t,
      )!,
      caution: Color.lerp(caution, other.caution, t)!,
      onCaution: Color.lerp(onCaution, other.onCaution, t)!,
      cautionContainer: Color.lerp(
        cautionContainer,
        other.cautionContainer,
        t,
      )!,
      onCautionContainer: Color.lerp(
        onCautionContainer,
        other.onCautionContainer,
        t,
      )!,
      plugin: Color.lerp(plugin, other.plugin, t)!,
      onPlugin: Color.lerp(onPlugin, other.onPlugin, t)!,
      pluginContainer: Color.lerp(pluginContainer, other.pluginContainer, t)!,
      onPluginContainer: Color.lerp(
        onPluginContainer,
        other.onPluginContainer,
        t,
      )!,
      ai: Color.lerp(ai, other.ai, t)!,
      onAi: Color.lerp(onAi, other.onAi, t)!,
      aiContainer: Color.lerp(aiContainer, other.aiContainer, t)!,
      onAiContainer: Color.lerp(onAiContainer, other.onAiContainer, t)!,
    );
  }
}

extension AppThemeContextExtension on BuildContext {
  AppSemanticColors get semanticColors =>
      Theme.of(this).extension<AppSemanticColors>() ?? AppSemanticColors.dark;

  ColorScheme get colorScheme => Theme.of(this).colorScheme;
  TextTheme get textTheme => Theme.of(this).textTheme;
}

/// Tema unificado Material Design 3 com alto contraste (AGENTS.md §12).
class AppTheme {
  AppTheme._();

  static const double cardRadius = 12.0;
  static final BorderRadius borderRadius = BorderRadius.circular(cardRadius);

  static ThemeData dark() {
    const scheme = ColorScheme(
      brightness: Brightness.dark,
      primary: Color(0xFF42A5F5),
      onPrimary: Color(0xFF001C38),
      primaryContainer: Color(0xFF0D3B66),
      onPrimaryContainer: Color(0xFFD1E4FF),
      secondary: Color(0xFF00ACC1),
      onSecondary: Color(0xFF002024),
      secondaryContainer: Color(0xFF004F58),
      onSecondaryContainer: Color(0xFFB2EBF2),
      tertiary: Color(0xFF82B1FF),
      onTertiary: Color(0xFF002657),
      tertiaryContainer: Color(0xFF143D73),
      onTertiaryContainer: Color(0xFFD6E3FF),
      error: Color(0xFFFF5252),
      onError: Color(0xFF2E0002),
      errorContainer: Color(0xFF4A1215),
      onErrorContainer: Color(0xFFFFB4AB),
      surface: Color(0xFF1A1D21),
      onSurface: Color(0xFFF1F5F9),
      surfaceContainerLowest: Color(0xFF111317),
      surfaceContainerLow: Color(0xFF16191D),
      surfaceContainer: Color(0xFF1E2228),
      surfaceContainerHigh: Color(0xFF252A32),
      surfaceContainerHighest: Color(0xFF2E343E),
      onSurfaceVariant: Color(0xFFCBD5E1),
      outline: Color(0xFF5A6577),
      outlineVariant: Color(0xFF333B47),
      shadow: Color(0xFF000000),
      scrim: Color(0xFF000000),
      inverseSurface: Color(0xFFF1F5F9),
      onInverseSurface: Color(0xFF121519),
      inversePrimary: Color(0xFF1976D2),
    );

    return _buildTheme(
      scheme: scheme,
      scaffoldBackground: const Color(0xFF121417),
      semantics: AppSemanticColors.dark,
    );
  }

  static ThemeData light() {
    const scheme = ColorScheme(
      brightness: Brightness.light,
      primary: Color(0xFF1565C0),
      onPrimary: Color(0xFFFFFFFF),
      primaryContainer: Color(0xFFD7E8FF),
      onPrimaryContainer: Color(0xFF002B5C),
      secondary: Color(0xFF00838F),
      onSecondary: Color(0xFFFFFFFF),
      secondaryContainer: Color(0xFFCFF3F7),
      onSecondaryContainer: Color(0xFF00363C),
      tertiary: Color(0xFF1976D2),
      onTertiary: Color(0xFFFFFFFF),
      tertiaryContainer: Color(0xFFDBE9FF),
      onTertiaryContainer: Color(0xFF002A54),
      error: Color(0xFFC62828),
      onError: Color(0xFFFFFFFF),
      errorContainer: Color(0xFFFCDADA),
      onErrorContainer: Color(0xFF5A0B0B),
      surface: Color(0xFFFFFFFF),
      onSurface: Color(0xFF0F172A),
      surfaceContainerLowest: Color(0xFFFFFFFF),
      surfaceContainerLow: Color(0xFFF8FAFC),
      surfaceContainer: Color(0xFFF1F5F9),
      surfaceContainerHigh: Color(0xFFE7EDF4),
      surfaceContainerHighest: Color(0xFFDCE4EE),
      // 75%+ de contraste sobre branco — nunca cinza desbotado (AGENTS.md §12).
      onSurfaceVariant: Color(0xFF334155),
      outline: Color(0xFF64748B),
      outlineVariant: Color(0xFFCBD5E1),
      shadow: Color(0xFF0F172A),
      scrim: Color(0xFF000000),
      inverseSurface: Color(0xFF1E293B),
      onInverseSurface: Color(0xFFF8FAFC),
      inversePrimary: Color(0xFF90CAF9),
    );

    return _buildTheme(
      scheme: scheme,
      scaffoldBackground: const Color(0xFFF4F6F9),
      semantics: AppSemanticColors.light,
    );
  }

  static ThemeData _buildTheme({
    required ColorScheme scheme,
    required Color scaffoldBackground,
    required AppSemanticColors semantics,
  }) {
    final base = ThemeData(
      useMaterial3: true,
      colorScheme: scheme,
      scaffoldBackgroundColor: scaffoldBackground,
      visualDensity: VisualDensity.standard,
      extensions: <ThemeExtension<dynamic>>[semantics],
    );

    return base.copyWith(
      appBarTheme: AppBarTheme(
        backgroundColor: scheme.surface,
        foregroundColor: scheme.onSurface,
        elevation: 0,
        scrolledUnderElevation: 2,
        surfaceTintColor: Colors.transparent,
        shape: Border(bottom: BorderSide(color: scheme.outlineVariant)),
        titleTextStyle: base.textTheme.titleMedium?.copyWith(
          color: scheme.onSurface,
          fontWeight: FontWeight.w700,
        ),
      ),
      cardTheme: CardThemeData(
        color: scheme.surface,
        elevation: 2,
        shadowColor: scheme.shadow.withValues(alpha: 0.16),
        surfaceTintColor: Colors.transparent,
        margin: EdgeInsets.zero,
        shape: RoundedRectangleBorder(borderRadius: borderRadius),
      ),
      dialogTheme: DialogThemeData(
        backgroundColor: scheme.surface,
        surfaceTintColor: Colors.transparent,
        shape: RoundedRectangleBorder(borderRadius: borderRadius),
      ),
      bottomSheetTheme: BottomSheetThemeData(
        backgroundColor: scheme.surface,
        surfaceTintColor: Colors.transparent,
        showDragHandle: true,
        shape: const RoundedRectangleBorder(
          borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
        ),
      ),
      navigationBarTheme: NavigationBarThemeData(
        backgroundColor: scheme.surface,
        surfaceTintColor: Colors.transparent,
        indicatorColor: scheme.primaryContainer,
        labelTextStyle: WidgetStateProperty.resolveWith((states) {
          final selected = states.contains(WidgetState.selected);
          return TextStyle(
            fontSize: 12,
            fontWeight: selected ? FontWeight.w700 : FontWeight.w500,
            color: selected ? scheme.primary : scheme.onSurfaceVariant,
          );
        }),
        iconTheme: WidgetStateProperty.resolveWith((states) {
          final selected = states.contains(WidgetState.selected);
          return IconThemeData(
            color: selected ? scheme.primary : scheme.onSurfaceVariant,
          );
        }),
      ),
      navigationRailTheme: NavigationRailThemeData(
        backgroundColor: scheme.surface,
        indicatorColor: scheme.primaryContainer,
        selectedIconTheme: IconThemeData(color: scheme.primary),
        unselectedIconTheme: IconThemeData(color: scheme.onSurfaceVariant),
        selectedLabelTextStyle: TextStyle(
          fontSize: 12,
          fontWeight: FontWeight.w700,
          color: scheme.primary,
        ),
        unselectedLabelTextStyle: TextStyle(
          fontSize: 12,
          fontWeight: FontWeight.w500,
          color: scheme.onSurfaceVariant,
        ),
      ),
      drawerTheme: DrawerThemeData(
        backgroundColor: scheme.surface,
        surfaceTintColor: Colors.transparent,
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: scheme.surfaceContainerLow,
        contentPadding: const EdgeInsets.symmetric(
          horizontal: 14,
          vertical: 12,
        ),
        border: OutlineInputBorder(
          borderRadius: borderRadius,
          borderSide: BorderSide(color: scheme.outlineVariant),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: borderRadius,
          borderSide: BorderSide(color: scheme.outlineVariant),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: borderRadius,
          borderSide: BorderSide(color: scheme.primary, width: 2),
        ),
        errorBorder: OutlineInputBorder(
          borderRadius: borderRadius,
          borderSide: BorderSide(color: scheme.error),
        ),
        focusedErrorBorder: OutlineInputBorder(
          borderRadius: borderRadius,
          borderSide: BorderSide(color: scheme.error, width: 2),
        ),
        labelStyle: TextStyle(color: scheme.onSurfaceVariant),
        hintStyle: TextStyle(
          color: scheme.onSurfaceVariant.withValues(alpha: 0.8),
        ),
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          minimumSize: const Size(0, 44),
          padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 10),
          shape: RoundedRectangleBorder(borderRadius: borderRadius),
          textStyle: const TextStyle(fontWeight: FontWeight.w600),
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          minimumSize: const Size(0, 44),
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
          shape: RoundedRectangleBorder(borderRadius: borderRadius),
          side: BorderSide(color: scheme.primary),
          textStyle: const TextStyle(fontWeight: FontWeight.w600),
        ),
      ),
      textButtonTheme: TextButtonThemeData(
        style: TextButton.styleFrom(
          minimumSize: const Size(0, 40),
          shape: RoundedRectangleBorder(borderRadius: borderRadius),
          textStyle: const TextStyle(fontWeight: FontWeight.w600),
        ),
      ),
      chipTheme: base.chipTheme.copyWith(
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10)),
        side: BorderSide(color: scheme.outlineVariant),
        labelStyle: TextStyle(
          color: scheme.onSurface,
          fontWeight: FontWeight.w600,
          fontSize: 12,
        ),
      ),
      dividerTheme: DividerThemeData(
        color: scheme.outlineVariant,
        thickness: 1,
        space: 1,
      ),
      snackBarTheme: SnackBarThemeData(
        behavior: SnackBarBehavior.floating,
        shape: RoundedRectangleBorder(borderRadius: borderRadius),
      ),
    );
  }
}
