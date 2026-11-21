import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:share_plus/share_plus.dart';

/// Salva ou compartilha um arquivo binário/texto baixado no Android e na Web.
Future<void> shareDownloadedBytes(
  BuildContext context, {
  required Uint8List bytes,
  required String filename,
  String mimeType = 'application/octet-stream',
}) async {
  try {
    final file = XFile.fromData(bytes, name: filename, mimeType: mimeType);
    await Share.shareXFiles(<XFile>[file], subject: filename);
  } catch (err) {
    if (!context.mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          'Arquivo $filename (${formatBytes(bytes.length)}) pronto, mas não foi possível compartilhar: $err',
        ),
      ),
    );
  }
}

Future<void> shareTextFile(
  BuildContext context, {
  required String content,
  required String filename,
}) {
  return shareDownloadedBytes(
    context,
    bytes: Uint8List.fromList(utf8.encode(content)),
    filename: filename,
    mimeType: 'text/plain',
  );
}
