bool isNotificationsSupported() => false;

bool isWebPushSupported() => false;

String getNotificationPermission() => 'unsupported';

Future<String> requestNotificationPermission() async => 'unsupported';

Future<bool> registerPushServiceWorker() async => false;

Future<String?> getWebPushSubscriptionJson() async => null;

Future<String?> subscribeWebPushJson(String vapidPublicKey) async => null;

Future<String?> unsubscribeWebPushEndpoint() async => null;

Future<bool> showLocalBrowserNotification({
  required String title,
  required String body,
  String? tag,
  String? url,
}) async => false;
