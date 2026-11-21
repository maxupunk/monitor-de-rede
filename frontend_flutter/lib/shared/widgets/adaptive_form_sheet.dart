import 'package:flutter/material.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Abre um formulário adaptativo:
/// - Telas compactas (< 600px): `showModalBottomSheet` com scroll e teclado seguro.
/// - Telas médias/amplas (>= 600px): `Dialog` centralizado com largura controlada.
Future<T?> showAdaptiveFormSheet<T>({
  required BuildContext context,
  required String title,
  String? subtitle,
  required WidgetBuilder builder,
  List<Widget>? actions,
  double maxWidth = 560,
  bool isDismissible = true,
}) {
  final isCompact = MediaQuery.sizeOf(context).width < 600;

  if (isCompact) {
    return showModalBottomSheet<T>(
      context: context,
      isScrollControlled: true,
      isDismissible: isDismissible,
      useSafeArea: true,
      builder: (sheetContext) {
        final bottomInset = MediaQuery.viewInsetsOf(sheetContext).bottom;
        return Padding(
          padding: EdgeInsets.only(bottom: bottomInset),
          child: ConstrainedBox(
            constraints: BoxConstraints(
              maxHeight: MediaQuery.sizeOf(sheetContext).height * 0.9,
            ),
            child: _SheetContent(
              title: title,
              subtitle: subtitle,
              actions: actions,
              onClose: isDismissible ? () => Navigator.of(sheetContext).pop() : null,
              child: builder(sheetContext),
            ),
          ),
        );
      },
    );
  }

  return showDialog<T>(
    context: context,
    barrierDismissible: isDismissible,
    builder: (dialogContext) {
      return Dialog(
        insetPadding: const EdgeInsets.all(24),
        shape: RoundedRectangleBorder(borderRadius: AppTheme.borderRadius),
        child: ConstrainedBox(
          constraints: BoxConstraints(
            maxWidth: maxWidth,
            maxHeight: MediaQuery.sizeOf(dialogContext).height * 0.88,
          ),
          child: _SheetContent(
            title: title,
            subtitle: subtitle,
            actions: actions,
            onClose: isDismissible ? () => Navigator.of(dialogContext).pop() : null,
            child: builder(dialogContext),
          ),
        ),
      );
    },
  );
}

class _SheetContent extends StatelessWidget {
  const _SheetContent({
    required this.title,
    this.subtitle,
    required this.child,
    this.actions,
    this.onClose,
  });

  final String title;
  final String? subtitle;
  final Widget child;
  final List<Widget>? actions;
  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Padding(
          padding: const EdgeInsets.fromLTRB(20, 14, 12, 12),
          child: Row(
            children: <Widget>[
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      title,
                      style: context.textTheme.titleMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                        color: scheme.onSurface,
                      ),
                    ),
                    if (subtitle != null && subtitle!.isNotEmpty) ...<Widget>[
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
              if (onClose != null)
                IconButton(
                  tooltip: 'Fechar',
                  onPressed: onClose,
                  icon: const Icon(Icons.close),
                ),
            ],
          ),
        ),
        const Divider(height: 1),
        Flexible(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(20),
            child: child,
          ),
        ),
        if (actions != null && actions!.isNotEmpty) ...<Widget>[
          const Divider(height: 1),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
            child: Wrap(
              alignment: WrapAlignment.end,
              spacing: 8,
              runSpacing: 8,
              children: actions!,
            ),
          ),
        ],
      ],
    );
  }
}
