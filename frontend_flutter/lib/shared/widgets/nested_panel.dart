import 'package:flutter/material.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Painel interno (border flat, sem elevação, raio 12) para uso dentro de cards ou diálogos.
class NestedPanel extends StatelessWidget {
  const NestedPanel({
    super.key,
    required this.child,
    this.padding = const EdgeInsets.all(14),
    this.tone,
    this.borderColor,
    this.backgroundColor,
  });

  final Widget child;
  final EdgeInsetsGeometry padding;
  final SemanticTone? tone;
  final Color? borderColor;
  final Color? backgroundColor;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final resolvedTone =
        tone != null ? context.semanticColors.resolve(tone!, scheme) : null;

    return Container(
      padding: padding,
      decoration: BoxDecoration(
        color: backgroundColor ??
            resolvedTone?.container.withValues(alpha: 0.45) ??
            scheme.surfaceContainerLow,
        borderRadius: AppTheme.borderRadius,
        border: Border.all(
          color: borderColor ?? resolvedTone?.border ?? scheme.outlineVariant,
        ),
      ),
      child: child,
    );
  }
}
