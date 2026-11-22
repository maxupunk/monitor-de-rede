import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Linha chave-valor responsiva com suporte opcional a fonte monoespaçada e cópia.
class KeyValueRow extends StatelessWidget {
  const KeyValueRow({
    super.key,
    required this.label,
    required this.value,
    this.monospace = false,
    this.copyable = false,
    this.trailing,
  });

  final String label;
  final String value;
  final bool monospace;
  final bool copyable;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 7),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isNarrow =
              constraints.maxWidth < (trailing != null ? 400 : 250);
          final labelWidget = Text(
            label,
            style: context.textTheme.bodySmall?.copyWith(
              color: scheme.onSurfaceVariant,
              fontWeight: FontWeight.w600,
            ),
          );
          final textWidget = SelectableText(
            value,
            textAlign: isNarrow ? TextAlign.start : TextAlign.end,
            style: context.textTheme.bodyMedium?.copyWith(
              color: scheme.onSurface,
              fontWeight: FontWeight.w600,
              fontFamily: monospace ? 'monospace' : null,
            ),
          );
          final copyButton = (copyable && value.isNotEmpty)
              ? IconButton(
                  tooltip: 'Copiar',
                  visualDensity: VisualDensity.compact,
                  padding: EdgeInsets.zero,
                  constraints: const BoxConstraints(
                    minWidth: 28,
                    minHeight: 28,
                  ),
                  icon: const Icon(Icons.copy, size: 15),
                  onPressed: () {
                    Clipboard.setData(ClipboardData(text: value));
                    ScaffoldMessenger.of(context).showSnackBar(
                      const SnackBar(
                        content: Text('Copiado para a área de transferência.'),
                        duration: Duration(seconds: 2),
                      ),
                    );
                  },
                )
              : null;

          final Widget valueWidget = (isNarrow && trailing != null)
              ? Wrap(
                  spacing: 6,
                  runSpacing: 4,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: <Widget>[textWidget, ?copyButton, trailing!],
                )
              : Row(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    Flexible(child: textWidget),
                    if (copyButton != null) ...<Widget>[
                      const SizedBox(width: 4),
                      copyButton,
                    ],
                    if (trailing != null) ...<Widget>[
                      const SizedBox(width: 6),
                      Flexible(flex: 0, child: trailing!),
                    ],
                  ],
                );

          if (isNarrow) {
            return SizedBox(
              width: double.infinity,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  labelWidget,
                  const SizedBox(height: 2),
                  valueWidget,
                ],
              ),
            );
          }

          return Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              Expanded(flex: 4, child: labelWidget),
              const SizedBox(width: 12),
              Expanded(
                flex: 6,
                child: Align(
                  alignment: Alignment.centerRight,
                  child: valueWidget,
                ),
              ),
            ],
          );
        },
      ),
    );
  }
}
