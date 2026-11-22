import 'dart:typed_data';

Future<void> platformSaveOrShareBytes({
  required Uint8List bytes,
  required String filename,
  required String mimeType,
}) {
  throw UnsupportedError(
    'Exportação de arquivo indisponível nesta plataforma.',
  );
}
