import 'dart:typed_data';

import 'package:netmonitor_app/core/utils/file_download.dart';

/// Exporta ou compartilha bytes delegando ao utilitário canônico `file_download.dart`.
Future<void> exportFileBytes({
  required Uint8List bytes,
  required String fileName,
  String mimeType = 'application/octet-stream',
}) async {
  await saveOrShareBytes(bytes: bytes, filename: fileName, mimeType: mimeType);
}
