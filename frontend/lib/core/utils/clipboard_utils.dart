import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'clipboard_fallback_stub.dart'
    if (dart.library.js_interop) 'clipboard_fallback_web.dart';

/// Copia `text` para a área de transferência, com fallback na Web para origens
/// HTTP não seguras (LAN/VPN) quando `navigator.clipboard` não está disponível.
///
/// Retorna `true` se a cópia teve sucesso ou `false` caso ambos os caminhos
/// falhem, sem propagar exceções não tratadas.
Future<bool> copyToClipboard(
  String text, {
  bool Function(String)? fallback,
}) async {
  try {
    await Clipboard.setData(ClipboardData(text: text));
    return true;
  } catch (_) {
    try {
      final copyFallback = fallback ?? platformCopyToClipboardFallback;
      return copyFallback(text);
    } catch (_) {
      return false;
    }
  }
}

/// Copia `text` para a área de transferência e exibe confirmação (ou aviso de
/// falha) em `SnackBar`.
Future<bool> copyToClipboardWithFeedback(
  BuildContext context,
  String text, {
  String message = 'Copiado para a área de transferência.',
  String failureMessage =
      'Não foi possível copiar para a área de transferência.',
  Duration duration = const Duration(seconds: 2),
  bool Function(String)? fallback,
}) async {
  final copied = await copyToClipboard(text, fallback: fallback);
  if (!context.mounted) return copied;
  ScaffoldMessenger.of(context)
    ..hideCurrentSnackBar()
    ..showSnackBar(
      SnackBar(
        content: Text(copied ? message : failureMessage),
        duration: duration,
      ),
    );
  return copied;
}
