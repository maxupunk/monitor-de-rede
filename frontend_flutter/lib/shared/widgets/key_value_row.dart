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
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final isNarrow = constraints.maxWidth < 360;
          final labelWidget = Text(
            label,
            style: context.textTheme.bodySmall?.copyWith(
              color: scheme.onSurfaceVariant,
              fontWeight: FontWeight.w600,
            ),
          );
          final valueWidget = Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Flexible(
                child: SelectableText(
                  value,
                  textAlign: isNarrow ? TextAlign.start : TextAlign.end,
                  style: context.textTheme.bodyMedium?.copyWith(
                    color: scheme.onSurface,
                    fontWeight: FontWeight.w600,
                    fontFamily: monospace ? 'monospace' : null,
                  ),
                ),
              ),
              if (copyable && value.isNotEmpty) ...<Widget>[
                const SizedBox(width: 4),
                IconButton(
                  tooltip: 'Copiar',
                  visualDensity: VisualDensity.compact,
                  icon: const Icon(Icons.copy, size: 16),
                  onPressed: () {
                    Clipboard.setData(ClipboardData(text: value));
                    ScaffoldMessenger.of(context).showSnackBar(
                      const SnackBar(
                        content: Text('Copiado para a área de transferência.'),
                        duration: Duration(seconds: 2),
                      ),
                    );
                  },
                ),
              ],
              if (trailing != null) ...<Widget>[
                const SizedBox(width: 6),
                trailing!,
              ],
            ],
          );

          if (isNarrow) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                labelWidget,
                const SizedBox(height: 2),
                valueWidget,
              ],
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
