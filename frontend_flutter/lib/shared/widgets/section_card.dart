import 'package:flutter/material.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Card de página padrão do NetMonitor (elevation 2, raio 12, superfície limpa).
class SectionCard extends StatelessWidget {
  const SectionCard({
    super.key,
    required this.child,
    this.title,
    this.subtitle,
    this.icon,
    this.iconTone = SemanticTone.primary,
    this.trailing,
    this.footer,
    this.padding = const EdgeInsets.all(16),
    this.headerPadding = const EdgeInsets.fromLTRB(16, 16, 16, 12),
    this.showHeaderDivider = true,
  });

  final Widget child;
  final String? title;
  final String? subtitle;
  final IconData? icon;
  final SemanticTone iconTone;
  final Widget? trailing;
  final Widget? footer;
  final EdgeInsetsGeometry padding;
  final EdgeInsetsGeometry headerPadding;
  final bool showHeaderDivider;

  bool get _hasHeader =>
      title != null || subtitle != null || icon != null || trailing != null;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final semantics = context.semanticColors;
    final iconColors = semantics.resolve(iconTone, scheme);

    return Card(
      elevation: 2,
      shape: RoundedRectangleBorder(borderRadius: AppTheme.borderRadius),
      clipBehavior: Clip.antiAlias,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (_hasHeader) ...<Widget>[
            Padding(
              padding: headerPadding,
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.center,
                children: <Widget>[
                  if (icon != null) ...<Widget>[
                    Container(
                      width: 36,
                      height: 36,
                      decoration: BoxDecoration(
                        color: iconColors.container,
                        borderRadius: BorderRadius.circular(10),
                      ),
                      child: Icon(icon, size: 20, color: iconColors.onContainer),
                    ),
                    const SizedBox(width: 12),
                  ],
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        if (title != null)
                          Text(
                            title!,
                            style: context.textTheme.titleMedium?.copyWith(
                              fontWeight: FontWeight.w700,
                              color: scheme.onSurface,
                            ),
                          ),
                        if (subtitle != null) ...<Widget>[
                          const SizedBox(height: 2),
                          Text(
                            subtitle!,
                            style: context.textTheme.bodySmall?.copyWith(
                              color: scheme.onSurfaceVariant,
                            ),
                          ),
                        ],
                      ],
                    ),
                  ),
                  if (trailing != null) ...<Widget>[
                    const SizedBox(width: 12),
                    trailing!,
                  ],
                ],
              ),
            ),
            if (showHeaderDivider) const Divider(height: 1),
          ],
          Padding(padding: padding, child: child),
          if (footer != null) ...<Widget>[
            const Divider(height: 1),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
              child: footer!,
            ),
          ],
        ],
      ),
    );
  }
}
