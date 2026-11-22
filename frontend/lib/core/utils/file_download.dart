import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';

import 'file_download_stub.dart'
    if (dart.library.js_interop) 'file_download_web.dart'
    if (dart.library.io) 'file_download_io.dart';

/// Salva um arquivo binário diretamente via Blob `<a download>` na Web ou abre
/// a folha nativa de compartilhamento (`Share.shareXFiles`) em IO/Mobile/Desktop.
Future<bool> saveOrShareBytes({
  required Uint8List bytes,
  required String filename,
  String mimeType = 'application/octet-stream',
  BuildContext? context,
  String? successMessage,
}) async {
  try {
    await platformSaveOrShareBytes(
      bytes: bytes,
      filename: filename,
      mimeType: mimeType,
    );
    if (context != null && context.mounted && successMessage != null) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(successMessage)));
    }
    return true;
  } catch (err) {
    if (context != null && context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            describeError(err, fallback: 'Falha ao exportar arquivo.'),
          ),
        ),
      );
    }
    return false;
  }
}

/// Salva ou compartilha conteúdo textual em UTF-8.
Future<bool> saveOrShareText({
  required String content,
  required String filename,
  String mimeType = 'text/plain; charset=utf-8',
  BuildContext? context,
  String? successMessage,
}) {
  final bytes = Uint8List.fromList(utf8.encode(content));
  return saveOrShareBytes(
    bytes: bytes,
    filename: filename,
    mimeType: mimeType,
    context: context,
    successMessage: successMessage,
  );
}
