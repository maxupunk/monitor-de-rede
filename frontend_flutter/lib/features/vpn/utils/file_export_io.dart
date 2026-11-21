import 'dart:typed_data';

import 'package:share_plus/share_plus.dart';

Future<void> exportFileBytes({
  required Uint8List bytes,
  required String fileName,
  String mimeType = 'application/octet-stream',
}) async {
  final file = XFile.fromData(bytes, name: fileName, mimeType: mimeType);
  await Share.shareXFiles(<XFile>[file], subject: fileName);
}
