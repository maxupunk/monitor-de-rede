import 'dart:typed_data';

Future<void> exportFileBytes({
  required Uint8List bytes,
  required String fileName,
  String mimeType = 'application/octet-stream',
}) async {
  // Stub seguro para ambientes sem dart:io nem dart:js_interop.
}
