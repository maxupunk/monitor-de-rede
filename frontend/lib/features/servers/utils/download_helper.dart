import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/file_download.dart';

export 'package:netmonitor_app/core/utils/file_download.dart'
    show saveOrShareBytes, saveOrShareText;

/// Shim de compatibilidade delegando ao utilitário canônico `file_download.dart`
/// (Blob `<a download>` na Web + `Share.shareXFiles` em IO).
Future<void> shareDownloadedBytes(
  BuildContext context, {
  required Uint8List bytes,
  required String filename,
  String mimeType = 'application/octet-stream',
}) async {
  await saveOrShareBytes(
    bytes: bytes,
    filename: filename,
    mimeType: mimeType,
    context: context,
  );
}

Future<void> shareTextFile(
  BuildContext context, {
  required String content,
  required String filename,
}) async {
  await saveOrShareText(content: content, filename: filename, context: context);
}
