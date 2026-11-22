import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// Copia `text` para a área de transferência e exibe confirmação em `SnackBar`.
Future<void> copyToClipboardWithFeedback(
  BuildContext context,
  String text, {
  String message = 'Copiado para a área de transferência.',
  Duration duration = const Duration(seconds: 2),
}) async {
  await Clipboard.setData(ClipboardData(text: text));
  if (!context.mounted) return;
  ScaffoldMessenger.of(
    context,
  ).showSnackBar(SnackBar(content: Text(message), duration: duration));
}
