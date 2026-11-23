import 'dart:convert';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/network/sse_client.dart';
import 'package:netmonitor_app/core/notifications/notifications_service.dart';
import 'package:netmonitor_app/core/notifications/web_notifications_interop.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/features/core/presentation/settings/widgets/notifications_and_catalogs_cards.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:shared_preferences/shared_preferences.dart';

class _RecordedRequest {
  const _RecordedRequest({
    required this.method,
    required this.path,
    required this.data,
  });

  final String method;
  final String path;
  final Object? data;
}

class _RecordedLocalNotification {
  const _RecordedLocalNotification({
    required this.title,
    required this.body,
    this.tag,
    this.url,
  });

  final String title;
  final String body;
  final String? tag;
  final String? url;
}

class _FakeWebNotificationsBridge implements WebNotificationsBridge {
  _FakeWebNotificationsBridge({
    this.supported = true,
    this.webPushSupported = true,
    this.permission = NotificationPermissionState.defaultState,
    this.requestResult = NotificationPermissionState.granted,
    this.currentSubscription,
  });

  bool supported;
  bool webPushSupported;
  NotificationPermissionState permission;
  NotificationPermissionState requestResult;
  BrowserPushSubscription? currentSubscription;

  int registerSwCalls = 0;
  final List<String> subscribedVapidKeys = <String>[];
  int unsubscribeCalls = 0;
  final List<_RecordedLocalNotification> shownNotifications =
      <_RecordedLocalNotification>[];

  @override
  bool isSupported() => supported;

  @override
  bool isWebPushSupported() => webPushSupported;

  @override
  NotificationPermissionState getPermission() => permission;

  @override
  Future<NotificationPermissionState> requestPermission() async {
    permission = requestResult;
    return permission;
  }

  @override
  Future<bool> registerServiceWorker() async {
    registerSwCalls++;
    return webPushSupported;
  }

  @override
  Future<BrowserPushSubscription?> getSubscription() async =>
      currentSubscription;

  @override
  Future<BrowserPushSubscription?> subscribe(String vapidPublicKey) async {
    subscribedVapidKeys.add(vapidPublicKey);
    const created = BrowserPushSubscription(
      endpoint: 'https://push.example.test/sub/abc123',
      p256dh: 'BOr7fakeP256dhKeyValue',
      auth: 'fakeAuthSecret',
      userAgent: 'NetMonitorTestAgent/1.0',
    );
    currentSubscription = created;
    return created;
  }

  @override
  Future<String?> unsubscribe() async {
    unsubscribeCalls++;
    final endpoint = currentSubscription?.endpoint;
    currentSubscription = null;
    return endpoint;
  }

  @override
  Future<bool> showLocalNotification({
    required String title,
    required String body,
    String? tag,
    String? url,
  }) async {
    shownNotifications.add(
      _RecordedLocalNotification(title: title, body: body, tag: tag, url: url),
    );
    return true;
  }
}

class _MockHttpAdapter implements HttpClientAdapter {
  _MockHttpAdapter(this.handler);

  final ResponseBody Function(RequestOptions options) handler;

  @override
  Future<ResponseBody> fetch(
    RequestOptions options,
    Stream<Uint8List>? requestStream,
    Future<void>? cancelFuture,
  ) async {
    return handler(options);
  }

  @override
  void close({bool force = false}) {}
}

ResponseBody _jsonResponse(int statusCode, Object body) {
  return ResponseBody.fromString(
    jsonEncode(body),
    statusCode,
    headers: <String, List<String>>{
      Headers.contentTypeHeader: <String>[Headers.jsonContentType],
    },
  );
}

class _TestAuthNotifier extends AuthNotifier {
  _TestAuthNotifier(this._initial);

  final AuthState _initial;

  @override
  AuthState build() => _initial;

  void setSignedOut() {
    state = state.copyWith(clearToken: true, clearUser: true);
  }
}

AuthState _authenticatedState() => const AuthState(
  token: 'test-jwt-token',
  user: AuthUser(
    id: 1,
    email: 'admin@netmonitor.local',
    fullName: 'Admin',
    role: UserRole.admin,
  ),
  initialized: true,
);

ApiClient _buildMockApiClient({
  required List<_RecordedRequest> requests,
  required int Function() subscriptionCountProvider,
  void Function(int nextCount)? onSubscriptionCountChanged,
  bool includePublicKeyInStatus = true,
  Map<String, dynamic>? testPushResponse,
}) {
  final dio = Dio(BaseOptions(validateStatus: (_) => true))
    ..httpClientAdapter = _MockHttpAdapter((options) {
      final path = options.uri.path.replaceFirst(RegExp(r'^/api'), '');
      requests.add(
        _RecordedRequest(
          method: options.method.toUpperCase(),
          path: path,
          data: options.data,
        ),
      );

      if (options.method == 'GET' && path == '/push/status') {
        return _jsonResponse(200, <String, dynamic>{
          'configured': true,
          if (includePublicKeyInStatus)
            'publicKey': 'VAPID_PUBLIC_KEY_FROM_STATUS',
          'totalSubscriptions': subscriptionCountProvider(),
          'userSubscriptions': subscriptionCountProvider(),
        });
      }
      if (options.method == 'GET' && path == '/push/vapid-public-key') {
        return _jsonResponse(200, <String, dynamic>{
          'publicKey': 'VAPID_PUBLIC_KEY_FROM_ENDPOINT',
        });
      }
      if (options.method == 'POST' && path == '/push/subscriptions') {
        onSubscriptionCountChanged?.call(subscriptionCountProvider() + 1);
        return _jsonResponse(200, <String, dynamic>{
          'success': true,
          'id': 10,
          'endpoint': 'https://push.example.test/sub/abc123',
        });
      }
      if (options.method == 'DELETE' && path == '/push/subscriptions') {
        final next = subscriptionCountProvider() - 1;
        onSubscriptionCountChanged?.call(next < 0 ? 0 : next);
        return _jsonResponse(200, <String, dynamic>{
          'success': true,
          'rowsAffected': 1,
        });
      }
      if (options.method == 'POST' && path == '/push/test') {
        return _jsonResponse(
          200,
          testPushResponse ??
              <String, dynamic>{
                'success': true,
                'sent': 1,
                'expiredPruned': 0,
                'message':
                    'Notificação de teste enviada com sucesso para 1 dispositivo(s).',
              },
        );
      }

      return _jsonResponse(404, <String, dynamic>{'message': 'Not found'});
    });

  return ApiClient(
    baseUrlProvider: () => 'http://127.0.0.1:5150/api',
    tokenProvider: () => 'test-jwt-token',
    onUnauthorized: () {},
    dio: dio,
  );
}

void main() {
  setUp(() {
    SharedPreferences.setMockInitialValues(<String, Object>{});
  });

  group('BrowserPushSubscription & NotificationPermissionState', () {
    test('parses valid subscription JSON and rejects incomplete payloads', () {
      final valid = BrowserPushSubscription.tryFromJsonString(
        jsonEncode(<String, dynamic>{
          'endpoint': 'https://push.example.test/1',
          'keys': <String, dynamic>{'p256dh': 'k1', 'auth': 'a1'},
          'userAgent': 'Chrome/130',
        }),
      );
      expect(valid, isNotNull);
      expect(valid!.endpoint, 'https://push.example.test/1');
      expect(valid.toApiPayload(), <String, dynamic>{
        'endpoint': 'https://push.example.test/1',
        'keys': <String, String>{'p256dh': 'k1', 'auth': 'a1'},
        'userAgent': 'Chrome/130',
      });

      expect(BrowserPushSubscription.tryFromJsonString('not-json'), isNull);
      expect(
        BrowserPushSubscription.tryFromJsonString(
          jsonEncode(<String, dynamic>{'endpoint': 'https://x', 'keys': {}}),
        ),
        isNull,
      );
      expect(
        NotificationPermissionState.parse('granted'),
        NotificationPermissionState.granted,
      );
      expect(
        NotificationPermissionState.parse('denied'),
        NotificationPermissionState.denied,
      );
      expect(
        NotificationPermissionState.parse('default'),
        NotificationPermissionState.defaultState,
      );
      expect(
        NotificationPermissionState.parse('unknown'),
        NotificationPermissionState.unsupported,
      );
    });
  });

  group('NotificationsNotifier', () {
    test(
      'requestPermission -> granted subscribes Web Push via POST /push/subscriptions and refreshes status',
      () async {
        final requests = <_RecordedRequest>[];
        var subCount = 0;
        final bridge = _FakeWebNotificationsBridge(
          permission: NotificationPermissionState.defaultState,
          requestResult: NotificationPermissionState.granted,
        );
        final api = _buildMockApiClient(
          requests: requests,
          subscriptionCountProvider: () => subCount,
          onSubscriptionCountChanged: (n) => subCount = n,
          includePublicKeyInStatus: false,
        );

        final container = ProviderContainer(
          overrides: <Override>[
            apiClientProvider.overrideWithValue(api),
            webNotificationsBridgeProvider.overrideWithValue(bridge),
            authProvider.overrideWith(
              () => _TestAuthNotifier(_authenticatedState()),
            ),
          ],
        );
        addTearDown(container.dispose);

        final notifier = container.read(notificationsProvider.notifier);
        final granted = await notifier.requestPermission();

        expect(granted, isTrue);
        final state = container.read(notificationsProvider);
        expect(state.permissionState, NotificationPermissionState.granted);
        expect(state.isSubscribed, isTrue);
        expect(state.serverConfigured, isTrue);
        expect(state.subscriptionCount, 1);
        expect(bridge.registerSwCalls, greaterThanOrEqualTo(1));
        expect(bridge.subscribedVapidKeys, <String>[
          'VAPID_PUBLIC_KEY_FROM_ENDPOINT',
        ]);

        final postReq = requests.firstWhere(
          (r) => r.method == 'POST' && r.path == '/push/subscriptions',
        );
        expect(postReq.data, <String, dynamic>{
          'endpoint': 'https://push.example.test/sub/abc123',
          'keys': <String, String>{
            'p256dh': 'BOr7fakeP256dhKeyValue',
            'auth': 'fakeAuthSecret',
          },
          'userAgent': 'NetMonitorTestAgent/1.0',
        });

        final prefs = await SharedPreferences.getInstance();
        expect(prefs.getBool(pwaNotificationsEnabledPrefKey), isTrue);
      },
    );

    test(
      'requestPermission without Web Push sends welcome local notification',
      () async {
        final requests = <_RecordedRequest>[];
        final bridge = _FakeWebNotificationsBridge(
          supported: true,
          webPushSupported: false,
          permission: NotificationPermissionState.defaultState,
          requestResult: NotificationPermissionState.granted,
        );
        final api = _buildMockApiClient(
          requests: requests,
          subscriptionCountProvider: () => 0,
        );

        final container = ProviderContainer(
          overrides: <Override>[
            apiClientProvider.overrideWithValue(api),
            webNotificationsBridgeProvider.overrideWithValue(bridge),
            authProvider.overrideWith(
              () => _TestAuthNotifier(_authenticatedState()),
            ),
          ],
        );
        addTearDown(container.dispose);

        final granted = await container
            .read(notificationsProvider.notifier)
            .requestPermission();
        expect(granted, isTrue);
        expect(bridge.shownNotifications.length, 1);
        expect(bridge.shownNotifications.single.title, 'Notificações Ativadas');
      },
    );

    test(
      'unsubscribeFromWebPush sends DELETE /push/subscriptions with endpoint and updates state',
      () async {
        final requests = <_RecordedRequest>[];
        var subCount = 1;
        final bridge = _FakeWebNotificationsBridge(
          permission: NotificationPermissionState.granted,
          currentSubscription: const BrowserPushSubscription(
            endpoint: 'https://push.example.test/sub/existing',
            p256dh: 'p256',
            auth: 'auth',
          ),
        );
        final api = _buildMockApiClient(
          requests: requests,
          subscriptionCountProvider: () => subCount,
          onSubscriptionCountChanged: (n) => subCount = n,
        );

        final container = ProviderContainer(
          overrides: <Override>[
            apiClientProvider.overrideWithValue(api),
            webNotificationsBridgeProvider.overrideWithValue(bridge),
            authProvider.overrideWith(
              () => _TestAuthNotifier(_authenticatedState()),
            ),
          ],
        );
        addTearDown(container.dispose);

        final notifier = container.read(notificationsProvider.notifier);
        await notifier.refreshStatus();
        expect(container.read(notificationsProvider).isSubscribed, isTrue);

        final ok = await notifier.toggleWebPush(false);
        expect(ok, isTrue);
        expect(bridge.unsubscribeCalls, 1);

        final delReq = requests.firstWhere(
          (r) => r.method == 'DELETE' && r.path == '/push/subscriptions',
        );
        expect(delReq.data, <String, String>{
          'endpoint': 'https://push.example.test/sub/existing',
        });

        final state = container.read(notificationsProvider);
        expect(state.isSubscribed, isFalse);
        expect(state.subscriptionCount, 0);
      },
    );

    test('sendTestPush calls POST /push/test and parses result', () async {
      final requests = <_RecordedRequest>[];
      final bridge = _FakeWebNotificationsBridge(
        permission: NotificationPermissionState.granted,
      );
      final api = _buildMockApiClient(
        requests: requests,
        subscriptionCountProvider: () => 2,
        testPushResponse: <String, dynamic>{
          'success': true,
          'sent': 2,
          'expiredPruned': 1,
          'message':
              'Notificação de teste enviada com sucesso para 2 dispositivo(s).',
        },
      );

      final container = ProviderContainer(
        overrides: <Override>[
          apiClientProvider.overrideWithValue(api),
          webNotificationsBridgeProvider.overrideWithValue(bridge),
          authProvider.overrideWith(
            () => _TestAuthNotifier(_authenticatedState()),
          ),
        ],
      );
      addTearDown(container.dispose);

      final res = await container
          .read(notificationsProvider.notifier)
          .sendTestPush();
      expect(res.success, isTrue);
      expect(res.sent, 2);
      expect(res.expiredPruned, 1);
      expect(res.message, contains('2 dispositivo(s)'));
      expect(
        requests.any((r) => r.method == 'POST' && r.path == '/push/test'),
        isTrue,
      );
    });

    test(
      'SSE events trigger local notifications when authenticated & enabled, and detach on logout',
      () async {
        final requests = <_RecordedRequest>[];
        final bridge = _FakeWebNotificationsBridge(
          permission: NotificationPermissionState.granted,
        );
        final api = _buildMockApiClient(
          requests: requests,
          subscriptionCountProvider: () => 1,
        );
        late _TestAuthNotifier authNotifier;

        final container = ProviderContainer(
          overrides: <Override>[
            apiClientProvider.overrideWithValue(api),
            webNotificationsBridgeProvider.overrideWithValue(bridge),
            authProvider.overrideWith(() {
              authNotifier = _TestAuthNotifier(_authenticatedState());
              return authNotifier;
            }),
          ],
        );
        addTearDown(container.dispose);

        await container
            .read(notificationsProvider.notifier)
            .ensureInitialized();
        final sse = container.read(sseServiceProvider.notifier);

        sse.dispatchForTest(
          const SseEvent(
            type: 'alert:triggered',
            data: <String, dynamic>{
              'id': 11,
              'title': 'Alta Latência',
              'message': 'RTT acima de 200ms',
            },
            timestamp: '2026-11-23T20:00:00Z',
          ),
        );
        sse.dispatchForTest(
          const SseEvent(
            type: 'monitor:result',
            data: <String, dynamic>{
              'monitorId': 5,
              'monitorName': 'Link Fibra',
              'status': 'down',
            },
            timestamp: '2026-11-23T20:00:01Z',
          ),
        );
        sse.dispatchForTest(
          const SseEvent(
            type: 'monitor:result',
            data: <String, dynamic>{
              'monitorId': 6,
              'monitorName': 'Link Backup',
              'status': 'up',
            },
            timestamp: '2026-11-23T20:00:02Z',
          ),
        );
        sse.dispatchForTest(
          const SseEvent(
            type: 'device:status',
            data: <String, dynamic>{
              'id': 9,
              'name': 'Switch Core',
              'status': 'offline',
            },
            timestamp: '2026-11-23T20:00:03Z',
          ),
        );
        sse.dispatchForTest(
          const SseEvent(
            type: 'device:status',
            data: <String, dynamic>{
              'id': 10,
              'name': 'AP Recepção',
              'status': 'online',
            },
            timestamp: '2026-11-23T20:00:04Z',
          ),
        );
        sse.dispatchForTest(
          const SseEvent(
            type: 'backup:failed',
            data: <String, dynamic>{
              'kind': 'database_restore',
              'name': 'Postgres Principal',
              'message': 'Erro de conexão',
              'connectionId': 3,
            },
            timestamp: '2026-11-23T20:00:05Z',
          ),
        );

        await Future<void>.delayed(Duration.zero);

        expect(bridge.shownNotifications.length, 4);
        expect(bridge.shownNotifications[0].title, '🚨 Alta Latência');
        expect(bridge.shownNotifications[0].body, 'RTT acima de 200ms');
        expect(bridge.shownNotifications[0].tag, 'alert-11');
        expect(bridge.shownNotifications[0].url, '/alerts');

        expect(
          bridge.shownNotifications[1].title,
          '🔴 Monitor Fora do Ar: Link Fibra',
        );
        expect(bridge.shownNotifications[1].tag, 'monitor-down-5');
        expect(bridge.shownNotifications[1].url, '/monitors');

        expect(
          bridge.shownNotifications[2].title,
          '⚠️ Dispositivo Offline: Switch Core',
        );
        expect(bridge.shownNotifications[2].tag, 'device-offline-9');
        expect(bridge.shownNotifications[2].url, '/devices');

        expect(
          bridge.shownNotifications[3].title,
          '🔴 Falha na restauração: Postgres Principal',
        );
        expect(bridge.shownNotifications[3].tag, 'backup-failed-3');
        expect(bridge.shownNotifications[3].url, '/backup');

        // Desativar notificações locais impede novos disparos e persiste preferência.
        await container
            .read(notificationsProvider.notifier)
            .setLocalNotificationsEnabled(false);
        final prefs = await SharedPreferences.getInstance();
        expect(prefs.getBool(pwaNotificationsEnabledPrefKey), isFalse);

        sse.dispatchForTest(
          const SseEvent(
            type: 'alert:triggered',
            data: <String, dynamic>{'id': 12, 'title': 'Ignorado'},
            timestamp: '2026-11-23T20:00:06Z',
          ),
        );
        await Future<void>.delayed(Duration.zero);
        expect(bridge.shownNotifications.length, 4);

        // Reativa e efetua logout: listeners SSE devem ser removidos sem vazar.
        await container
            .read(notificationsProvider.notifier)
            .setLocalNotificationsEnabled(true);
        authNotifier.setSignedOut();
        await Future<void>.delayed(Duration.zero);

        sse.dispatchForTest(
          const SseEvent(
            type: 'alert:triggered',
            data: <String, dynamic>{'id': 13, 'title': 'Após logout'},
            timestamp: '2026-11-23T20:00:07Z',
          ),
        );
        await Future<void>.delayed(Duration.zero);
        expect(bridge.shownNotifications.length, 4);
      },
    );
  });

  group('NotificationsCard widget', () {
    testWidgets(
      'renders HTTPS hint and Solicitar permissão when ungranted & Web Push unsupported',
      (tester) async {
        final requests = <_RecordedRequest>[];
        final bridge = _FakeWebNotificationsBridge(
          supported: true,
          webPushSupported: false,
          permission: NotificationPermissionState.defaultState,
        );
        final api = _buildMockApiClient(
          requests: requests,
          subscriptionCountProvider: () => 0,
        );

        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              apiClientProvider.overrideWithValue(api),
              webNotificationsBridgeProvider.overrideWithValue(bridge),
              authProvider.overrideWith(
                () => _TestAuthNotifier(_authenticatedState()),
              ),
            ],
            child: MaterialApp(
              theme: AppTheme.light(),
              home: const Scaffold(
                body: SingleChildScrollView(child: NotificationsCard()),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(find.text('Notificações PWA & Web Push'), findsOneWidget);
        expect(find.text('Permissão do Navegador'), findsOneWidget);
        expect(find.text('PENDENTE'), findsOneWidget);
        expect(find.text('INDISPONÍVEL'), findsOneWidget);
        expect(find.textContaining('HTTPS'), findsOneWidget);
        expect(find.text('Solicitar permissão'), findsOneWidget);
        expect(find.text('Notificações visuais na aba aberta'), findsOneWidget);
      },
    );

    testWidgets(
      'renders active Web Push controls and inline feedback on test push',
      (tester) async {
        final requests = <_RecordedRequest>[];
        final bridge = _FakeWebNotificationsBridge(
          supported: true,
          webPushSupported: true,
          permission: NotificationPermissionState.granted,
          currentSubscription: const BrowserPushSubscription(
            endpoint: 'https://push.example.test/sub/active',
            p256dh: 'p256',
            auth: 'auth',
          ),
        );
        final api = _buildMockApiClient(
          requests: requests,
          subscriptionCountProvider: () => 3,
        );

        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              apiClientProvider.overrideWithValue(api),
              webNotificationsBridgeProvider.overrideWithValue(bridge),
              authProvider.overrideWith(
                () => _TestAuthNotifier(_authenticatedState()),
              ),
            ],
            child: MaterialApp(
              theme: AppTheme.light(),
              home: const Scaffold(
                body: SingleChildScrollView(child: NotificationsCard()),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(find.text('PERMITIDO'), findsOneWidget);
        expect(find.text('ATIVO'), findsOneWidget);
        expect(find.textContaining('3 assinatura(s)'), findsOneWidget);
        expect(
          find.text('Receber alertas em segundo plano (Web Push)'),
          findsOneWidget,
        );
        expect(find.text('Testar Web Push (2º plano)'), findsOneWidget);
        expect(find.text('Teste local (aba aberta)'), findsOneWidget);

        await tester.tap(find.text('Testar Web Push (2º plano)'));
        await tester.pumpAndSettle();

        expect(
          find.textContaining('Notificação de teste enviada com sucesso'),
          findsOneWidget,
        );

        await tester.tap(find.text('Teste local (aba aberta)'));
        await tester.pumpAndSettle();

        expect(bridge.shownNotifications.length, 1);
        expect(
          bridge.shownNotifications.single.title,
          'Notificação de Teste PWA',
        );
      },
    );
  });
}
