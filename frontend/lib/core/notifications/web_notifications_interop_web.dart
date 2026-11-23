import 'dart:js_interop';

@JS('__netmonitorPush')
external _PushBridge? get _bridge;

extension type _PushBridge(JSObject _) implements JSObject {
  external bool isSupported();
  external bool isWebPushSupported();
  external String getPermission();
  external JSPromise<JSString> requestPermission();
  external JSPromise<JSBoolean> registerServiceWorker();
  external JSPromise<JSString?> getSubscription();
  external JSPromise<JSString?> subscribe(String vapidPublicKey);
  external JSPromise<JSString?> unsubscribe();
  external JSPromise<JSBoolean> showLocalNotification(
    String title,
    String body,
    String? tag,
    String? url,
  );
}

bool isNotificationsSupported() => _bridge?.isSupported() ?? false;

bool isWebPushSupported() => _bridge?.isWebPushSupported() ?? false;

String getNotificationPermission() => _bridge?.getPermission() ?? 'unsupported';

Future<String> requestNotificationPermission() async {
  final bridge = _bridge;
  if (bridge == null) return 'unsupported';
  try {
    final result = await bridge.requestPermission().toDart;
    return result.toDart;
  } catch (_) {
    return getNotificationPermission();
  }
}

Future<bool> registerPushServiceWorker() async {
  final bridge = _bridge;
  if (bridge == null) return false;
  try {
    final result = await bridge.registerServiceWorker().toDart;
    return result.toDart;
  } catch (_) {
    return false;
  }
}

Future<String?> getWebPushSubscriptionJson() async {
  final bridge = _bridge;
  if (bridge == null) return null;
  try {
    final result = await bridge.getSubscription().toDart;
    return result?.toDart;
  } catch (_) {
    return null;
  }
}

Future<String?> subscribeWebPushJson(String vapidPublicKey) async {
  final bridge = _bridge;
  if (bridge == null) return null;
  final result = await bridge.subscribe(vapidPublicKey).toDart;
  return result?.toDart;
}

Future<String?> unsubscribeWebPushEndpoint() async {
  final bridge = _bridge;
  if (bridge == null) return null;
  final result = await bridge.unsubscribe().toDart;
  return result?.toDart;
}

Future<bool> showLocalBrowserNotification({
  required String title,
  required String body,
  String? tag,
  String? url,
}) async {
  final bridge = _bridge;
  if (bridge == null) return false;
  try {
    final result = await bridge
        .showLocalNotification(title, body, tag, url)
        .toDart;
    return result.toDart;
  } catch (_) {
    return false;
  }
}
