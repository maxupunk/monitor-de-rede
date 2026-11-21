import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Bloco de código / comandos / logs com rolagem horizontal e botão de cópia.
class CodeViewer extends StatelessWidget {
  const CodeViewer({
    super.key,
    required this.code,
    this.title,
    this.maxHeight,
    this.copyable = true,
  });

  final String code;
  final String? title;
  final double? maxHeight;
  final bool copyable;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;

    return Container(
      decoration: BoxDecoration(
        color: scheme.surfaceContainerLowest,
        borderRadius: AppTheme.borderRadius,
        border: Border.all(color: scheme.outlineVariant),
      ),
      clipBehavior: Clip.antiAlias,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (title != null || copyable)
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
              decoration: BoxDecoration(
                color: scheme.surfaceContainer,
                border: Border(
                  bottom: BorderSide(color: scheme.outlineVariant),
                ),
              ),
              child: Row(
                children: <Widget>[
                  Expanded(
                    child: Text(
                      title ?? 'Código',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.labelMedium?.copyWith(
                        color: scheme.onSurfaceVariant,
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                  ),
                  if (copyable)
                    TextButton.icon(
                      onPressed: () {
                        Clipboard.setData(ClipboardData(text: code));
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(
                            content: Text('Conteúdo copiado.'),
                            duration: Duration(seconds: 2),
                          ),
                        );
                      },
                      icon: const Icon(Icons.copy, size: 15),
                      label: const Text('Copiar'),
                      style: TextButton.styleFrom(
                        visualDensity: VisualDensity.compact,
                        padding: const EdgeInsets.symmetric(horizontal: 8),
                      ),
                    ),
                ],
              ),
            ),
          ConstrainedBox(
            constraints: BoxConstraints(
              maxHeight: maxHeight ?? double.infinity,
            ),
            child: SingleChildScrollView(
              child: SingleChildScrollView(
                scrollDirection: Axis.horizontal,
                padding: const EdgeInsets.all(14),
                child: SelectableText(
                  code,
                  style: TextStyle(
                    fontFamily: 'monospace',
                    fontSize: 12.5,
                    height: 1.45,
                    color: scheme.onSurface,
                  ),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
