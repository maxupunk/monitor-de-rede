import 'package:flutter/material.dart';
import 'package:netmonitor_app/shared/widgets/responsive_grid.dart';
import 'package:netmonitor_app/shared/widgets/symmetrical_action_bar.dart';
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
    this.stackTrailingOnCompact,
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

  /// Quando `null` (automático), itens únicos compactos (ex.: `SemanticChip`, `Badge`,
  /// `IconButton`, `Switch`, `TextButton`) permanecem alinhados à direita na mesma linha
  /// do título mesmo em telas mobile, enquanto barras largas/múltiplas (`SegmentedButton`,
  /// `Row`, `Wrap`, `SymmetricalActionBar`) empilham abaixo do título em `< 560px`.
  final bool? stackTrailingOnCompact;

  bool get _hasHeader =>
      title != null || subtitle != null || icon != null || trailing != null;

  static Widget _unwrapSingleChild(Widget widget) {
    var current = widget;
    while (true) {
      if (current is Padding && current.child != null) {
        current = current.child!;
      } else if (current is SizedBox && current.child != null) {
        current = current.child!;
      } else if (current is ConstrainedBox && current.child != null) {
        current = current.child!;
      } else if (current is Align && current.child != null) {
        current = current.child!;
      } else if (current is Tooltip && current.child != null) {
        current = current.child!;
      } else {
        return current;
      }
    }
  }

  static bool _isWideMultiControlTrailing(Widget trailingWidget) {
    final core = _unwrapSingleChild(trailingWidget);
    return core is SegmentedButton ||
        core is Row ||
        core is Wrap ||
        core is Column ||
        core is SymmetricalActionBar ||
        core is ResponsiveFieldRow ||
        core is OverflowBar ||
        core is ToggleButtons ||
        core is SingleChildScrollView;
  }

  Widget _wrapTrailing(Widget trailingWidget, {double? maxWidth}) {
    if (trailingWidget is Wrap) {
      return maxWidth != null
          ? ConstrainedBox(
              constraints: BoxConstraints(maxWidth: maxWidth),
              child: trailingWidget,
            )
          : trailingWidget;
    }
    final scrollable = SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: trailingWidget,
    );
    return maxWidth != null
        ? ConstrainedBox(
            constraints: BoxConstraints(maxWidth: maxWidth),
            child: scrollable,
          )
        : scrollable;
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final semantics = context.semanticColors;
    final iconColors = semantics.resolve(iconTone, scheme);
    final hasIdentity = icon != null || title != null || subtitle != null;

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
              child: LayoutBuilder(
                builder: (context, constraints) {
                  final identityRow = Row(
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
                          child: Icon(
                            icon,
                            size: 20,
                            color: iconColors.onContainer,
                          ),
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
                    ],
                  );

                  if (trailing == null) {
                    return identityRow;
                  }

                  if (!hasIdentity) {
                    return Align(
                      alignment: AlignmentDirectional.centerStart,
                      child: _wrapTrailing(
                        trailing!,
                        maxWidth: constraints.maxWidth,
                      ),
                    );
                  }

                  final isCompactWidth = constraints.maxWidth < 560;
                  final shouldStack =
                      constraints.maxWidth < 240 ||
                      (isCompactWidth &&
                          (stackTrailingOnCompact ??
                              _isWideMultiControlTrailing(trailing!)));

                  if (shouldStack) {
                    return Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        identityRow,
                        const SizedBox(height: 10),
                        Align(
                          alignment: AlignmentDirectional.centerStart,
                          child: _wrapTrailing(
                            trailing!,
                            maxWidth: constraints.maxWidth,
                          ),
                        ),
                      ],
                    );
                  }

                  final trailingMaxRatio = isCompactWidth ? 0.42 : 0.55;
                  return Row(
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: <Widget>[
                      Expanded(child: identityRow),
                      SizedBox(width: isCompactWidth ? 10 : 12),
                      Flexible(
                        flex: 0,
                        child: _wrapTrailing(
                          trailing!,
                          maxWidth: constraints.maxWidth * trailingMaxRatio,
                        ),
                      ),
                    ],
                  );
                },
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
