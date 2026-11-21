import 'package:flutter/material.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Diálogo padronizado de confirmação para ações críticas ou destrutivas.
Future<bool> showConfirmActionDialog({
  required BuildContext context,
  required String title,
  required String message,
  String confirmLabel = 'Confirmar',
  String cancelLabel = 'Cancelar',
  SemanticTone tone = SemanticTone.error,
  IconData icon = Icons.warning_amber_rounded,
  Widget? details,
}) async {
  final confirmed = await showDialog<bool>(
    context: context,
    builder: (dialogContext) {
      final scheme = dialogContext.colorScheme;
      final colors = dialogContext.semanticColors.resolve(tone, scheme);

      return AlertDialog(
        shape: RoundedRectangleBorder(borderRadius: AppTheme.borderRadius),
        icon: Container(
          width: 48,
          height: 48,
          decoration: BoxDecoration(
            color: colors.container,
            borderRadius: BorderRadius.circular(12),
          ),
          child: Icon(icon, color: colors.onContainer, size: 26),
        ),
        title: Text(
          title,
          textAlign: TextAlign.center,
          style: dialogContext.textTheme.titleMedium?.copyWith(
            fontWeight: FontWeight.w700,
          ),
        ),
        content: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 420),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text(
                message,
                style: dialogContext.textTheme.bodyMedium?.copyWith(
                  color: scheme.onSurface,
                ),
              ),
              if (details != null) ...<Widget>[
                const SizedBox(height: 12),
                details,
              ],
            ],
          ),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: Text(cancelLabel),
          ),
          FilledButton(
            style: FilledButton.styleFrom(
              backgroundColor: colors.foreground,
              foregroundColor: colors.onForeground,
            ),
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: Text(confirmLabel),
          ),
        ],
      );
    },
  );
  return confirmed == true;
}
