import 'dart:typed_data';

import 'package:netmonitor_app/features/vpn/utils/file_export_stub.dart'
    if (dart.library.js_interop) 'package:netmonitor_app/features/vpn/utils/file_export_web.dart'
    if (dart.library.io) 'package:netmonitor_app/features/vpn/utils/file_export_io.dart'
    as impl;

/// Exporta ou compartilha bytes (download no Web PWA, share sheet no Android/IO).
Future<void> exportFileBytes({
  required Uint8List bytes,
  required String fileName,
  String mimeType = 'application/octet-stream',
}) {
  return impl.exportFileBytes(
    bytes: bytes,
    fileName: fileName,
    mimeType: mimeType,
  );
}
