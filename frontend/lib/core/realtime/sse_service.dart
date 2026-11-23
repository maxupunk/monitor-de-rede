import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/config/server_config.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/network/sse_client.dart';

const int _feedLimit = 50;
const Duration _refreshDebounce = Duration(milliseconds: 800);
const Duration _interestDebounce = Duration(milliseconds: 300);

/// Barramento auxiliar recebido pelos handlers de domínio SSE.
abstract class SseBus {
  /// Agrupa recargas disparadas em rajada numa chamada só por recurso.
  void scheduleRefresh(String key, VoidCallback action);
}

typedef SseRouteHandler =
    void Function(Map<String, dynamic> data, SseEvent event, SseBus bus);

typedef SseEventListener =
    void Function(Map<String, dynamic> data, SseEvent event);

/// Definição de rota SSE de um domínio (ex.: `alert:triggered` ou `docker:*`).
class SseRoute {
  const SseRoute({
    required this.handle,
    this.ephemeral = false,
    this.isEphemeral,
  });

  final SseRouteHandler handle;
  final bool ephemeral;
  final bool Function(Map<String, dynamic> data)? isEphemeral;

  bool evaluatesAsEphemeral(Map<String, dynamic> data) {
    if (isEphemeral != null) return isEphemeral!(data);
    return ephemeral;
  }
}

class _RegisteredSseRoute {
  const _RegisteredSseRoute({
    required this.pattern,
    required this.isPrefix,
    required this.route,
  });

  final String pattern;
  final bool isPrefix;
  final SseRoute route;

  bool matches(String type) =>
      isPrefix ? type.startsWith(pattern) : type == pattern;
}

@immutable
class RealtimeState {
  const RealtimeState({
    this.connectionState = SseConnectionState.disconnected,
    this.recentEvents = const <SseEvent>[],
    this.lastEventAt,
  });

  final SseConnectionState connectionState;
  final List<SseEvent> recentEvents;
  final String? lastEventAt;

  bool get isConnected => connectionState == SseConnectionState.connected;

  RealtimeState copyWith({
    SseConnectionState? connectionState,
    List<SseEvent>? recentEvents,
    String? lastEventAt,
  }) {
    return RealtimeState(
      connectionState: connectionState ?? this.connectionState,
      recentEvents: recentEvents ?? this.recentEvents,
      lastEventAt: lastEventAt ?? this.lastEventAt,
    );
  }
}

/// Serviço global SSE compartilhado (`GET /api/events/stream`).
///
/// Sem polling (`Timer.periodic` proibido — AGENTS.md §9).
/// Cada área registra suas rotas enquanto visível; telas podem assinar pontualmente
/// com `onEvent` e declarar interesses sob demanda com `acquireInterest`.
class SseService extends Notifier<RealtimeState> implements SseBus {
  late final SseClient _client;
  StreamSubscription<SseEvent>? _eventSub;
  StreamSubscription<SseConnectionState>? _stateSub;

  final Set<_RegisteredSseRoute> _routes = <_RegisteredSseRoute>{};
  final Map<String, Set<SseEventListener>> _listeners =
      <String, Set<SseEventListener>>{};
  final Map<String, Timer> _pendingRefreshes = <String, Timer>{};
  final Map<String, int> _interests = <String, int>{};
  final Set<SseClient> _customClients = <SseClient>{};

  String _connectedInterests = '';
  Timer? _interestTimer;

  @override
  RealtimeState build() {
    _client = SseClient(
      onUnauthorized: () {
        ref.read(sessionTokenProvider.notifier).state = null;
        ref.read(sessionExpiredSignalProvider.notifier).state++;
      },
    );
    _stateSub = _client.connectionStates.listen((connectionState) {
      state = state.copyWith(connectionState: connectionState);
    });
    _eventSub = _client.events.listen(_handleIncomingEvent);

    ref.listen<String?>(sessionTokenProvider, (prev, next) {
      if (next == null || next.isEmpty) {
        disconnect(clearFeed: true);
      } else if (prev != null &&
          prev != next &&
          _client.state != SseConnectionState.disconnected) {
        _client.reconnectNow();
      }
    });
    ref.listen<ServerConfigState>(serverConfigProvider, (prev, next) {
      if (prev != null &&
          prev.apiBaseUrl != next.apiBaseUrl &&
          _client.state != SseConnectionState.disconnected) {
        _client.reconnectNow();
      }
    });

    ref.onDispose(() {
      disconnect(clearFeed: true);
      _eventSub?.cancel();
      _stateSub?.cancel();
      _client.dispose();
    });

    return const RealtimeState();
  }

  /// Registra rotas de um domínio (tipo exato ou prefixo `x:*`).
  VoidCallback registerRoutes(Map<String, SseRoute> table) {
    final added = <_RegisteredSseRoute>[];
    table.forEach((key, route) {
      final isPrefix = key.endsWith('*');
      final entry = _RegisteredSseRoute(
        pattern: isPrefix ? key.substring(0, key.length - 1) : key,
        isPrefix: isPrefix,
        route: route,
      );
      added.add(entry);
      _routes.add(entry);
    });
    return () {
      for (final entry in added) {
        _routes.remove(entry);
      }
    };
  }

  /// Assinatura pontual para widgets/telas com estado local (ex.: gráficos ao vivo).
  VoidCallback onEvent(List<String> types, SseEventListener listener) {
    for (final type in types) {
      _listeners.putIfAbsent(type, () => <SseEventListener>{}).add(listener);
    }
    return () {
      for (final type in types) {
        _listeners[type]?.remove(listener);
      }
    };
  }

  bool hasRoute(String type) => _routes.any((entry) => entry.matches(type));

  @override
  void scheduleRefresh(String key, VoidCallback action) {
    _pendingRefreshes[key]?.cancel();
    _pendingRefreshes[key] = Timer(_refreshDebounce, () {
      _pendingRefreshes.remove(key);
      action();
    });
  }

  String _interestKey() {
    final keys = _interests.keys.toList()..sort();
    return keys.join(',');
  }

  void _scheduleInterestSync() {
    _interestTimer?.cancel();
    _interestTimer = Timer(_interestDebounce, () {
      _interestTimer = null;
      if (_client.state == SseConnectionState.disconnected) return;
      final next = _interestKey();
      if (next == _connectedInterests) return;
      _connectedInterests = next;
      _client.reconnectNow();
    });
  }

  /// Declara interesse sob demanda (ex.: `docker`) com contagem de referências.
  VoidCallback acquireInterest(String name) {
    _interests[name] = (_interests[name] ?? 0) + 1;
    _scheduleInterestSync();
    var released = false;
    return () {
      if (released) return;
      released = true;
      final remaining = (_interests[name] ?? 1) - 1;
      if (remaining > 0) {
        _interests[name] = remaining;
      } else {
        _interests.remove(name);
      }
      _scheduleInterestSync();
    };
  }

  Uri _buildStreamUri() {
    final apiBase = ref.read(serverConfigProvider).apiBaseUrl;
    final token = ref.read(sessionTokenProvider);
    final queryParams = <String, String>{};
    if (token != null && token.isNotEmpty) {
      queryParams['token'] = token;
    }
    if (_connectedInterests.isNotEmpty) {
      queryParams['interest'] = _connectedInterests;
    }
    final rawBase = '$apiBase/events/stream';
    final baseUri = Uri.parse(rawBase);
    return baseUri.replace(
      queryParameters: queryParams.isEmpty ? null : queryParams,
    );
  }

  Map<String, String> _buildStreamHeaders() {
    final token = ref.read(sessionTokenProvider);
    if (token == null || token.isEmpty) return const <String, String>{};
    return <String, String>{'Authorization': 'Bearer $token'};
  }

  void connect() {
    final token = ref.read(sessionTokenProvider);
    if (token == null || token.isEmpty) return;
    _connectedInterests = _interestKey();
    _client.connect(
      urlBuilder: _buildStreamUri,
      headersBuilder: _buildStreamHeaders,
    );
  }

  void disconnect({bool clearFeed = false}) {
    _interestTimer?.cancel();
    _interestTimer = null;
    for (final timer in _pendingRefreshes.values) {
      timer.cancel();
    }
    _pendingRefreshes.clear();
    for (final custom in List<SseClient>.from(_customClients)) {
      custom.dispose();
    }
    _customClients.clear();
    _client.disconnect();
    if (clearFeed && state.recentEvents.isNotEmpty) {
      state = const RealtimeState();
    }
  }

  /// Cria um `SseClient` dedicado para streams secundários autorizados (ex.: `GET /api/logs/stream`).
  SseClient openCustomStream({
    required String path,
    Map<String, String> queryParameters = const <String, String>{},
  }) {
    final normalizedPath = path.startsWith('/') ? path : '/$path';
    final customClient = SseClient(
      onUnauthorized: () {
        ref.read(sessionTokenProvider.notifier).state = null;
        ref.read(sessionExpiredSignalProvider.notifier).state++;
      },
    );
    _customClients.add(customClient);
    customClient.connectionStates.listen(
      (_) {},
      onDone: () => _customClients.remove(customClient),
    );
    customClient.connect(
      urlBuilder: () {
        final apiBase = ref.read(serverConfigProvider).apiBaseUrl;
        final token = ref.read(sessionTokenProvider);
        final params = <String, String>{...queryParameters};
        if (token != null && token.isNotEmpty) {
          params['token'] = token;
        }
        final baseUri = Uri.parse('$apiBase$normalizedPath');
        return baseUri.replace(queryParameters: params.isEmpty ? null : params);
      },
      headersBuilder: _buildStreamHeaders,
    );
    return customClient;
  }

  /// Injeta evento diretamente (útil em testes de contrato SSE).
  @visibleForTesting
  void dispatchForTest(SseEvent event) => _handleIncomingEvent(event);

  void _handleIncomingEvent(SseEvent event) {
    final matched = _routes
        .where((entry) => entry.matches(event.type))
        .toList();
    final isEphemeral = matched.any(
      (entry) => entry.route.evaluatesAsEphemeral(event.data),
    );

    var updatedFeed = state.recentEvents;
    if (event.type != 'stream:connected' && !isEphemeral) {
      updatedFeed = <SseEvent>[event, ...state.recentEvents];
      if (updatedFeed.length > _feedLimit) {
        updatedFeed = updatedFeed.sublist(0, _feedLimit);
      }
      updatedFeed = List<SseEvent>.unmodifiable(updatedFeed);
    }

    state = state.copyWith(
      recentEvents: updatedFeed,
      lastEventAt: event.timestamp,
    );

    for (final entry in matched) {
      try {
        entry.route.handle(event.data, event, this);
      } catch (_) {
        // Isolamento entre domínios.
      }
    }

    _notifyListeners(event);
  }

  void _notifyListeners(SseEvent event) {
    for (final listener in List<SseEventListener>.from(
      _listeners[event.type] ?? const <SseEventListener>[],
    )) {
      try {
        listener(event.data, event);
      } catch (_) {}
    }

    // Alias local (`events.ts`): métricas de `monitor:result` chegam também como `metric:recorded`.
    if (event.type == 'monitor:result' && event.data['metrics'] is List) {
      final metricEvent = SseEvent(
        type: 'metric:recorded',
        data: event.data,
        timestamp: event.timestamp,
      );
      for (final listener in List<SseEventListener>.from(
        _listeners['metric:recorded'] ?? const <SseEventListener>[],
      )) {
        try {
          listener(metricEvent.data, metricEvent);
        } catch (_) {}
      }
    }
  }
}

final sseServiceProvider = NotifierProvider<SseService, RealtimeState>(
  SseService.new,
);
