import 'dart:async';
import 'dart:js_interop';
import 'dart:typed_data';

import 'package:web/web.dart' as web;

const Duration _objectUrlRevokeDelay = Duration(seconds: 1);

Future<void> platformSaveOrShareBytes({
  required Uint8List bytes,
  required String filename,
  required String mimeType,
}) async {
  final parts = <JSAny>[bytes.toJS].toJS;
  final blob = web.Blob(parts, web.BlobPropertyBag(type: mimeType));
  final url = web.URL.createObjectURL(blob);
  final anchor = web.document.createElement('a') as web.HTMLAnchorElement
    ..href = url
    ..download = filename
    ..style.display = 'none';
  web.document.body?.appendChild(anchor);
  anchor.click();
  anchor.remove();
  // Adia a revogação para não abortar downloads em Safari, iOS PWA e Firefox.
  Timer(_objectUrlRevokeDelay, () => web.URL.revokeObjectURL(url));
}
