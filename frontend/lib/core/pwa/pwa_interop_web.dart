import 'dart:js_interop';
import 'dart:ui';

@JS('__netmonitorPwa')
external _PwaBridge? get _bridge;

extension type _PwaBridge(JSObject _) implements JSObject {
  external bool canInstall();
  external bool isInstalled();
  external bool isIosSafari();
  external JSPromise<JSBoolean> promptInstall();
  external void subscribe(JSFunction callback);
}

bool canInstallPwa() => _bridge?.canInstall() ?? false;

bool isPwaInstalled() => _bridge?.isInstalled() ?? false;

bool isIosSafari() => _bridge?.isIosSafari() ?? false;

Future<bool> promptPwaInstall() async {
  final bridge = _bridge;
  if (bridge == null) return false;
  try {
    final accepted = await bridge.promptInstall().toDart;
    return accepted.toDart;
  } catch (_) {
    return false;
  }
}

void subscribePwaChanges(VoidCallback onChanged) {
  _bridge?.subscribe(onChanged.toJS);
}
