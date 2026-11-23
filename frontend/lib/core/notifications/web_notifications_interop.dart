import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

import 'web_notifications_interop_stub.dart'
    if (dart.library.js_interop) 'web_notifications_interop_web.dart'
    as interop;

/// Estado de permissão do navegador para a Notification API.
enum NotificationPermissionState {
  defaultState('default'),
  granted('granted'),
  denied('denied'),
  unsupported('unsupported');

  const NotificationPermissionState(this.wireValue);

  final String wireValue;

  static NotificationPermissionState parse(String? raw) {
    return switch ((raw ?? '').trim().toLowerCase()) {
      'granted' => NotificationPermissionState.granted,
      'denied' => NotificationPermissionState.denied,
      'default' => NotificationPermissionState.defaultState,
      _ => NotificationPermissionState.unsupported,
    };
  }
}

/// Dados de uma assinatura Web Push retornada pelo navegador.
@immutable
class BrowserPushSubscription {
  const BrowserPushSubscription({
    required this.endpoint,
    required this.p256dh,
    required this.auth,
    this.userAgent,
  });

  final String endpoint;
  final String p256dh;
  final String auth;
  final String? userAgent;

  Map<String, dynamic> toApiPayload() => <String, dynamic>{
    'endpoint': endpoint,
    'keys': <String, String>{'p256dh': p256dh, 'auth': auth},
    if (userAgent != null && userAgent!.trim().isNotEmpty)
      'userAgent': userAgent!.trim(),
  };

  static BrowserPushSubscription? tryFromJsonString(String? raw) {
    if (raw == null || raw.trim().isEmpty) return null;
    try {
      final decoded = jsonDecode(raw);
      if (decoded is! Map) return null;
      final map = asJsonMap(decoded);
      final keys = map['keys'] is Map
          ? asJsonMap(map['keys'])
          : const <String, dynamic>{};
      final endpoint = (map['endpoint'] ?? '').toString().trim();
      final p256dh = (keys['p256dh'] ?? '').toString().trim();
      final auth = (keys['auth'] ?? '').toString().trim();
      if (endpoint.isEmpty || p256dh.isEmpty || auth.isEmpty) {
        return null;
      }
      final ua = map['userAgent']?.toString();
      return BrowserPushSubscription(
        endpoint: endpoint,
        p256dh: p256dh,
        auth: auth,
        userAgent: ua,
      );
    } catch (_) {
      return null;
    }
  }
}

/// Contrato abstrato da ponte nativa/JS de notificações (DIP — AGENTS.md §10).
abstract class WebNotificationsBridge {
  bool isSupported();
  bool isWebPushSupported();
  NotificationPermissionState getPermission();
  Future<NotificationPermissionState> requestPermission();
  Future<bool> registerServiceWorker();
  Future<BrowserPushSubscription?> getSubscription();
  Future<BrowserPushSubscription?> subscribe(String vapidPublicKey);
  Future<String?> unsubscribe();
  Future<bool> showLocalNotification({
    required String title,
    required String body,
    String? tag,
    String? url,
  });
}

class PlatformWebNotificationsBridge implements WebNotificationsBridge {
  const PlatformWebNotificationsBridge();

  @override
  bool isSupported() => interop.isNotificationsSupported();

  @override
  bool isWebPushSupported() => interop.isWebPushSupported();

  @override
  NotificationPermissionState getPermission() =>
      NotificationPermissionState.parse(interop.getNotificationPermission());

  @override
  Future<NotificationPermissionState> requestPermission() async {
    final raw = await interop.requestNotificationPermission();
    return NotificationPermissionState.parse(raw);
  }

  @override
  Future<bool> registerServiceWorker() => interop.registerPushServiceWorker();

  @override
  Future<BrowserPushSubscription?> getSubscription() async {
    final raw = await interop.getWebPushSubscriptionJson();
    return BrowserPushSubscription.tryFromJsonString(raw);
  }

  @override
  Future<BrowserPushSubscription?> subscribe(String vapidPublicKey) async {
    final raw = await interop.subscribeWebPushJson(vapidPublicKey);
    return BrowserPushSubscription.tryFromJsonString(raw);
  }

  @override
  Future<String?> unsubscribe() => interop.unsubscribeWebPushEndpoint();

  @override
  Future<bool> showLocalNotification({
    required String title,
    required String body,
    String? tag,
    String? url,
  }) => interop.showLocalBrowserNotification(
    title: title,
    body: body,
    tag: tag,
    url: url,
  );
}

final webNotificationsBridgeProvider = Provider<WebNotificationsBridge>(
  (ref) => const PlatformWebNotificationsBridge(),
);
