import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/notifications/web_notifications_interop.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:shared_preferences/shared_preferences.dart';

const String pwaNotificationsEnabledPrefKey = 'pwa_notifications_enabled';

/// Resultado do endpoint `POST /api/push/test`.
@immutable
class TestPushResult {
  const TestPushResult({
    required this.success,
    required this.sent,
    required this.expiredPruned,
    required this.message,
  });

  final bool success;
  final int sent;
  final int expiredPruned;
  final String message;

  factory TestPushResult.fromJson(Map<String, dynamic> json) {
    final success = json['success'] == true;
    return TestPushResult(
      success: success,
      sent: parseIntOrNull(json['sent']) ?? 0,
      expiredPruned: parseIntOrNull(json['expiredPruned']) ?? 0,
      message:
          (json['message'] ??
                  (success
                      ? 'Notificação Web Push enviada com sucesso!'
                      : 'Nenhum dispositivo recebeu a notificação de teste.'))
              .toString(),
    );
  }
}

/// Estado centralizado de notificações PWA (Web Push + avisos locais via SSE).
@immutable
class NotificationsState {
  const NotificationsState({
    this.isSupported = false,
    this.permissionState = NotificationPermissionState.unsupported,
    this.isWebPushSupported = false,
    this.isSubscribed = false,
    this.isSubscribing = false,
    this.localNotificationsEnabled = true,
    this.serverConfigured = false,
    this.subscriptionCount = 0,
    this.vapidPublicKey,
    this.statusLoaded = false,
    this.error,
  });

  final bool isSupported;
  final NotificationPermissionState permissionState;
  final bool isWebPushSupported;
  final bool isSubscribed;
  final bool isSubscribing;
  final bool localNotificationsEnabled;
  final bool serverConfigured;
  final int subscriptionCount;
  final String? vapidPublicKey;
  final bool statusLoaded;
  final String? error;

  NotificationsState copyWith({
    bool? isSupported,
    NotificationPermissionState? permissionState,
    bool? isWebPushSupported,
    bool? isSubscribed,
    bool? isSubscribing,
    bool? localNotificationsEnabled,
    bool? serverConfigured,
    int? subscriptionCount,
    String? vapidPublicKey,
    bool clearVapidPublicKey = false,
    bool? statusLoaded,
    String? error,
    bool clearError = false,
  }) {
    return NotificationsState(
      isSupported: isSupported ?? this.isSupported,
      permissionState: permissionState ?? this.permissionState,
      isWebPushSupported: isWebPushSupported ?? this.isWebPushSupported,
      isSubscribed: isSubscribed ?? this.isSubscribed,
      isSubscribing: isSubscribing ?? this.isSubscribing,
      localNotificationsEnabled:
          localNotificationsEnabled ?? this.localNotificationsEnabled,
      serverConfigured: serverConfigured ?? this.serverConfigured,
      subscriptionCount: subscriptionCount ?? this.subscriptionCount,
      vapidPublicKey: clearVapidPublicKey
          ? null
          : (vapidPublicKey ?? this.vapidPublicKey),
      statusLoaded: statusLoaded ?? this.statusLoaded,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

/// Gerencia permissão do navegador, subscrição Web Push (`/api/push/*`)
/// e disparo de notificações locais reativas via SSE (zero polling).
class NotificationsNotifier extends Notifier<NotificationsState> {
  final List<VoidCallback> _sseUnsubscribers = <VoidCallback>[];
  bool _sseAttached = false;
  bool _disposed = false;

  WebNotificationsBridge get _bridge =>
      ref.read(webNotificationsBridgeProvider);

  @override
  NotificationsState build() {
    _disposed = false;
    final supported = _bridge.isSupported();
    final webPushSupported = _bridge.isWebPushSupported();
    final permission = supported
        ? _bridge.getPermission()
        : NotificationPermissionState.unsupported;

    ref.listen<bool>(authProvider.select((auth) => auth.isAuthenticated), (
      prev,
      isAuthenticated,
    ) {
      if (isAuthenticated) {
        _attachSseListeners();
        Future<void>.microtask(refreshStatus);
      } else {
        _detachSseListeners();
        state = state.copyWith(
          serverConfigured: false,
          subscriptionCount: 0,
          clearVapidPublicKey: true,
          statusLoaded: false,
          clearError: true,
        );
      }
    });

    ref.onDispose(() {
      _disposed = true;
      _detachSseListeners();
    });

    if (ref.read(authProvider).isAuthenticated) {
      _attachSseListeners();
    }

    Future<void>.microtask(_hydrateInitialState);

    return NotificationsState(
      isSupported: supported,
      permissionState: permission,
      isWebPushSupported: webPushSupported,
      localNotificationsEnabled: true,
    );
  }

  Future<void> _hydrateInitialState() async {
    await _loadLocalPreference();
    if (_disposed) return;
    if (ref.read(authProvider).isAuthenticated) {
      _attachSseListeners();
      await refreshStatus();
    } else if (state.isWebPushSupported) {
      await _syncBrowserSubscription();
    }
  }

  /// Garante listeners SSE anexados e status sincronizado ao montar o shell autenticado.
  Future<void> ensureInitialized() async {
    if (_disposed) return;
    if (ref.read(authProvider).isAuthenticated) {
      _attachSseListeners();
    }
    await refreshStatus();
  }

  Future<void> _loadLocalPreference() async {
    try {
      final prefs = await SharedPreferences.getInstance();
      final stored = prefs.getBool(pwaNotificationsEnabledPrefKey);
      if (!_disposed && stored != null) {
        state = state.copyWith(localNotificationsEnabled: stored);
      }
    } catch (_) {}
  }

  Future<void> _persistLocalPreference(bool enabled) async {
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setBool(pwaNotificationsEnabledPrefKey, enabled);
    } catch (_) {}
  }

  void _refreshSupportSnapshot() {
    final supported = _bridge.isSupported();
    final webPushSupported = _bridge.isWebPushSupported();
    final permission = supported
        ? _bridge.getPermission()
        : NotificationPermissionState.unsupported;
    state = state.copyWith(
      isSupported: supported,
      isWebPushSupported: webPushSupported,
      permissionState: permission,
    );
  }

  Future<void> _syncBrowserSubscription() async {
    if (!state.isWebPushSupported) {
      if (!_disposed && state.isSubscribed) {
        state = state.copyWith(isSubscribed: false);
      }
      return;
    }
    try {
      final sub = await _bridge.getSubscription();
      if (!_disposed) {
        state = state.copyWith(isSubscribed: sub != null);
      }
    } catch (_) {
      if (!_disposed) {
        state = state.copyWith(isSubscribed: false);
      }
    }
  }

  /// Sincroniza status do servidor (`GET /push/status`, `GET /push/vapid-public-key`)
  /// e verifica a assinatura ativa no navegador.
  Future<void> refreshStatus() async {
    if (_disposed) return;
    _refreshSupportSnapshot();

    if (ref.read(authProvider).isAuthenticated) {
      final api = ref.read(apiClientProvider);
      try {
        final raw = await api.get<Map<String, dynamic>>('/push/status');
        var vapidKey = (raw['publicKey'] as String?)?.trim();
        if (vapidKey == null || vapidKey.isEmpty) {
          try {
            final keyRaw = await api.get<Map<String, dynamic>>(
              '/push/vapid-public-key',
            );
            vapidKey = (keyRaw['publicKey'] as String?)?.trim();
          } catch (_) {}
        }
        final count =
            parseIntOrNull(
              raw['userSubscriptions'] ??
                  raw['totalSubscriptions'] ??
                  raw['subscriptionCount'],
            ) ??
            0;
        if (!_disposed) {
          state = state.copyWith(
            serverConfigured: raw['configured'] == true,
            subscriptionCount: count,
            vapidPublicKey: (vapidKey != null && vapidKey.isNotEmpty)
                ? vapidKey
                : state.vapidPublicKey,
            statusLoaded: true,
            clearError: true,
          );
        }
      } catch (err) {
        if (!_disposed) {
          state = state.copyWith(statusLoaded: true, error: describeError(err));
        }
      }
    }

    await _syncBrowserSubscription();
  }

  /// Solicita permissão de notificação ao navegador e inscreve Web Push se suportado.
  Future<bool> requestPermission() async {
    _refreshSupportSnapshot();
    if (!state.isSupported) {
      state = state.copyWith(
        permissionState: NotificationPermissionState.unsupported,
      );
      return false;
    }

    try {
      final permission = await _bridge.requestPermission();
      if (_disposed) return false;
      state = state.copyWith(permissionState: permission, clearError: true);

      if (permission == NotificationPermissionState.granted) {
        state = state.copyWith(localNotificationsEnabled: true);
        await _persistLocalPreference(true);
        if (state.isWebPushSupported) {
          await subscribeToWebPush();
        } else {
          await sendLocalNotification(
            'Notificações Ativadas',
            body:
                'Você receberá alertas de rede em tempo real no seu dispositivo.',
            tag: 'netmonitor-notifications-enabled',
            url: '/settings',
          );
        }
        return true;
      }
      return false;
    } catch (err) {
      if (!_disposed) {
        state = state.copyWith(error: describeError(err));
      }
      return false;
    }
  }

  Future<String?> _resolveVapidPublicKey() async {
    final cached = state.vapidPublicKey?.trim();
    if (cached != null && cached.isNotEmpty) {
      return cached;
    }
    final raw = await ref
        .read(apiClientProvider)
        .get<Map<String, dynamic>>('/push/vapid-public-key');
    final key = (raw['publicKey'] as String?)?.trim();
    if (!_disposed && key != null && key.isNotEmpty) {
      state = state.copyWith(vapidPublicKey: key);
    }
    return key;
  }

  /// Inscreve o navegador no Web Push e registra a assinatura no backend.
  Future<bool> subscribeToWebPush() async {
    _refreshSupportSnapshot();
    if (!state.isWebPushSupported) {
      return false;
    }

    state = state.copyWith(isSubscribing: true, clearError: true);
    try {
      if (state.permissionState != NotificationPermissionState.granted) {
        final permission = await _bridge.requestPermission();
        if (_disposed) return false;
        state = state.copyWith(permissionState: permission);
        if (permission != NotificationPermissionState.granted) {
          state = state.copyWith(isSubscribing: false);
          return false;
        }
      }

      final vapidKey = await _resolveVapidPublicKey();
      if (vapidKey == null || vapidKey.isEmpty) {
        throw StateError('Chave pública VAPID não recebida do servidor.');
      }

      await _bridge.registerServiceWorker();
      final subscription = await _bridge.subscribe(vapidKey);
      if (subscription == null) {
        throw StateError(
          'Chaves da subscrição Web Push inválidas ou ausentes.',
        );
      }

      await ref
          .read(apiClientProvider)
          .post<dynamic>(
            '/push/subscriptions',
            data: subscription.toApiPayload(),
          );

      if (_disposed) return false;
      state = state.copyWith(
        isSubscribed: true,
        isSubscribing: false,
        localNotificationsEnabled: true,
        clearError: true,
      );
      await _persistLocalPreference(true);
      await refreshStatus();
      return true;
    } catch (err) {
      if (!_disposed) {
        state = state.copyWith(isSubscribing: false, error: describeError(err));
      }
      return false;
    }
  }

  /// Cancela a assinatura Web Push no navegador e remove o endpoint no backend.
  Future<bool> unsubscribeFromWebPush() async {
    _refreshSupportSnapshot();
    if (!state.isWebPushSupported) {
      return false;
    }

    state = state.copyWith(isSubscribing: true, clearError: true);
    try {
      final endpoint = await _bridge.unsubscribe();
      if (endpoint != null && endpoint.trim().isNotEmpty) {
        await ref
            .read(apiClientProvider)
            .delete<dynamic>(
              '/push/subscriptions',
              data: <String, String>{'endpoint': endpoint.trim()},
            );
      }

      if (_disposed) return false;
      state = state.copyWith(
        isSubscribed: false,
        isSubscribing: false,
        clearError: true,
      );
      await refreshStatus();
      return true;
    } catch (err) {
      if (!_disposed) {
        state = state.copyWith(isSubscribing: false, error: describeError(err));
      }
      return false;
    }
  }

  Future<bool> toggleWebPush(bool enable) {
    return enable ? subscribeToWebPush() : unsubscribeFromWebPush();
  }

  Future<void> setLocalNotificationsEnabled(bool enabled) async {
    if (_disposed) return;
    state = state.copyWith(localNotificationsEnabled: enabled);
    await _persistLocalPreference(enabled);
  }

  /// Dispara notificação Web Push de teste pelo backend (`POST /api/push/test`).
  Future<TestPushResult> sendTestPush() async {
    final res = await ref
        .read(apiClientProvider)
        .post<Map<String, dynamic>>('/push/test');
    final parsed = TestPushResult.fromJson(res);
    if (parsed.expiredPruned > 0) {
      await refreshStatus();
    }
    return parsed;
  }

  /// Exibe notificação local do navegador (quando permissão concedida e ativa).
  Future<bool> sendLocalNotification(
    String title, {
    String? body,
    String? tag,
    String? url,
  }) async {
    if (state.permissionState != NotificationPermissionState.granted ||
        !state.localNotificationsEnabled) {
      return false;
    }
    return _bridge.showLocalNotification(
      title: title,
      body: body ?? '',
      tag: tag,
      url: url ?? '/alerts',
    );
  }

  void _attachSseListeners() {
    if (_sseAttached || _disposed) return;
    _sseAttached = true;

    final sse = ref.read(sseServiceProvider.notifier);

    _sseUnsubscribers.add(
      sse.onEvent(const <String>['alert:triggered'], (data, _) {
        final title =
            (data['title'] ?? data['ruleName'] ?? 'Novo Alerta de Rede')
                .toString();
        final message =
            (data['message'] ?? 'Um novo alerta foi gerado no monitoramento.')
                .toString();
        final tag =
            'alert-${data['id'] ?? DateTime.now().millisecondsSinceEpoch}';
        sendLocalNotification(
          '🚨 $title',
          body: message,
          tag: tag,
          url: '/alerts',
        );
      }),
    );

    _sseUnsubscribers.add(
      sse.onEvent(const <String>['monitor:result'], (data, _) {
        final status = data['status']?.toString();
        if (status == 'down' || status == 'offline') {
          final monitorName =
              (data['monitorName'] ?? data['name'] ?? 'Monitor de Rede')
                  .toString();
          final tag =
              'monitor-down-${data['monitorId'] ?? DateTime.now().millisecondsSinceEpoch}';
          sendLocalNotification(
            '🔴 Monitor Fora do Ar: $monitorName',
            body: 'O monitor $monitorName não respondeu ao teste.',
            tag: tag,
            url: '/monitors',
          );
        }
      }),
    );

    _sseUnsubscribers.add(
      sse.onEvent(const <String>['device:status'], (data, _) {
        if (data['status']?.toString() == 'offline') {
          final deviceName =
              (data['name'] ?? data['deviceName'] ?? 'Dispositivo').toString();
          final tag =
              'device-offline-${data['id'] ?? DateTime.now().millisecondsSinceEpoch}';
          sendLocalNotification(
            '⚠️ Dispositivo Offline: $deviceName',
            body: 'O dispositivo $deviceName perdeu conexão com a rede.',
            tag: tag,
            url: '/devices',
          );
        }
      }),
    );

    _sseUnsubscribers.add(
      sse.onEvent(const <String>['backup:failed'], (data, _) {
        final isRestore = data['kind']?.toString() == 'database_restore';
        final target = (data['name'] ?? 'NetMonitor').toString();
        final message =
            (data['message'] ?? 'Verifique os detalhes na tela de Backup.')
                .toString();
        final tag = 'backup-failed-${data['connectionId'] ?? 'system'}';
        sendLocalNotification(
          isRestore
              ? '🔴 Falha na restauração: $target'
              : '🔴 Falha no backup: $target',
          body: message,
          tag: tag,
          url: '/backup',
        );
      }),
    );
  }

  void _detachSseListeners() {
    for (final unsub in _sseUnsubscribers) {
      unsub();
    }
    _sseUnsubscribers.clear();
    _sseAttached = false;
  }
}

final notificationsProvider =
    NotifierProvider<NotificationsNotifier, NotificationsState>(
      NotificationsNotifier.new,
    );
