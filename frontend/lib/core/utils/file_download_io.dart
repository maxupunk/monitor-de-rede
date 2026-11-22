import 'dart:typed_data';

import 'package:share_plus/share_plus.dart';

Future<void> platformSaveOrShareBytes({
  required Uint8List bytes,
  required String filename,
  required String mimeType,
}) async {
  final xfile = XFile.fromData(bytes, name: filename, mimeType: mimeType);
  await Share.shareXFiles(<XFile>[xfile], subject: filename);
}
