import 'dart:async';
import 'dart:convert';
import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/network/sse_transport.dart';

/// Máximo de espera no backoff exponencial (30s — igual a `events.ts`).
const int maxSseReconnectDelayMs = 30000;

enum SseConnectionState {
  disconnected,
  connecting,
  connected,
  reconnecting,
}

/// Evento SSE tipado entregue pelo barramento (`GET /api/events/stream`).
@immutable
class SseEvent {
  const SseEvent({
    required this.type,
    required this.data,
    required this.timestamp,
  });

  final String type;
  final Map<String, dynamic> data;
  final String timestamp;

  static SseEvent? tryParseJson(String rawData, {DateTime? now}) {
    if (rawData.trim().isEmpty) return null;
    try {
      final decoded = jsonDecode(rawData);
      if (decoded is! Map) return null;
      final map = Map<String, dynamic>.from(decoded);
      final type = (map['type'] ?? map['event'] ?? 'unknown').toString();
      final rawPayload = map['data'];
      final data = rawPayload is Map
          ? Map<String, dynamic>.from(rawPayload)
          : <String, dynamic>{};
      final timestamp =
          (map['timestamp'] ?? (now ?? DateTime.now().toUtc()).toIso8601String()).toString();
      return SseEvent(type: type, data: data, timestamp: timestamp);
    } catch (_) {
      return null;
    }
  }
}

/// Parser incremental de fluxo `text/event-stream` (RFC / WHATWG SSE).
/// Suporta chunks parciais, múltiplas linhas `data:`, comentários `:`, `\r\n` e `\n`.
class SseLineParser {
  String _remainder = '';
  final List<String> _dataLines = <String>[];

  List<String> feed(String chunk) {
    final messages = <String>[];
    _remainder += chunk;
    final lines = _remainder.split('\n');
    _remainder = lines.isNotEmpty ? lines.removeLast() : '';

    for (final rawLine in lines) {
      final line = rawLine.endsWith('\r')
          ? rawLine.substring(0, rawLine.length - 1)
          : rawLine;
      _processLine(line, messages);
    }
    return messages;
  }

  List<String> flush() {
    final messages = <String>[];
    if (_remainder.isNotEmpty) {
      final line = _remainder.endsWith('\r')
          ? _remainder.substring(0, _remainder.length - 1)
          : _remainder;
      _remainder = '';
      _processLine(line, messages);
    }
    if (_dataLines.isNotEmpty) {
      messages.add(_dataLines.join('\n'));
      _dataLines.clear();
    }
    return messages;
  }

  void _processLine(String line, List<String> messages) {
    if (line.isEmpty) {
      if (_dataLines.isNotEmpty) {
        messages.add(_dataLines.join('\n'));
        _dataLines.clear();
      }
      return;
    }
    if (line.startsWith(':')) {
      // Comentário / keep-alive SSE.
      return;
    }
    if (line.startsWith('data:')) {
      var value = line.substring(5);
      if (value.startsWith(' ')) value = value.substring(1);
      _dataLines.add(value);
    }
  }
}

/// Calcula o atraso de reconexão exponencial: 1s, 2s, 4s, 8s, 16s, máx 30s.
Duration computeSseBackoffDelay(int attempt) {
  final clampedAttempt = attempt.clamp(0, 10);
  final ms = math.min(1000 * math.pow(2, clampedAttempt).toInt(), maxSseReconnectDelayMs);
  return Duration(milliseconds: ms);
}

/// Cliente SSE multiplataforma com reconexão por backoff exponencial (ZERO polling).
class SseClient {
  SseClient({
    SseTransport Function()? transportFactory,
    Duration Function(int attempt)? backoffCalculator,
  })  : _transportFactory = transportFactory ?? createPlatformSseTransport,
        _backoffCalculator = backoffCalculator ?? computeSseBackoffDelay;

  final SseTransport Function() _transportFactory;
  final Duration Function(int attempt) _backoffCalculator;

  final StreamController<SseEvent> _eventController =
      StreamController<SseEvent>.broadcast();
  final StreamController<SseConnectionState> _stateController =
      StreamController<SseConnectionState>.broadcast();

  SseTransport? _transport;
  Timer? _reconnectTimer;
  Uri Function()? _urlBuilder;
  Map<String, String> Function()? _headersBuilder;

  int _reconnectAttempts = 0;
  bool _manualDisconnect = true;
  SseConnectionState _state = SseConnectionState.disconnected;

  Stream<SseEvent> get events => _eventController.stream;
  Stream<SseConnectionState> get connectionStates => _stateController.stream;
  SseConnectionState get state => _state;
  bool get isConnected => _state == SseConnectionState.connected;
  int get reconnectAttempts => _reconnectAttempts;

  void _setState(SseConnectionState next) {
    if (_state == next) return;
    _state = next;
    if (!_stateController.isClosed) {
      _stateController.add(next);
    }
  }

  /// Abre (ou mantém aberta) a conexão SSE para a URL informada.
  void connect({
    required Uri Function() urlBuilder,
    Map<String, String> Function()? headersBuilder,
  }) {
    _urlBuilder = urlBuilder;
    _headersBuilder = headersBuilder;
    _manualDisconnect = false;
    _reconnectTimer?.cancel();
    _reconnectTimer = null;
    if (_transport != null) return;
    _openTransport();
  }

  /// Reabre o stream imediatamente (ex.: mudança no conjunto de `interest`).
  void reconnectNow() {
    if (_manualDisconnect || _urlBuilder == null) return;
    _reconnectTimer?.cancel();
    _reconnectTimer = null;
    _closeTransport();
    _openTransport();
  }

  void _openTransport() {
    if (_manualDisconnect || _urlBuilder == null) return;
    _setState(
      _reconnectAttempts == 0
          ? SseConnectionState.connecting
          : SseConnectionState.reconnecting,
    );

    final transport = _transportFactory();
    _transport = transport;
    transport.connect(
      url: _urlBuilder!(),
      headers: _headersBuilder?.call() ?? const <String, String>{},
      onOpen: () {
        if (_transport != transport) return;
        _reconnectAttempts = 0;
        _setState(SseConnectionState.connected);
      },
      onMessage: (rawData) {
        if (_transport != transport) return;
        final event = SseEvent.tryParseJson(rawData);
        if (event != null && !_eventController.isClosed) {
          _eventController.add(event);
        }
      },
      onError: (_) {
        if (_transport != transport) return;
        _closeTransport();
        if (!_manualDisconnect) {
          _scheduleReconnect();
        }
      },
    );
  }

  void _scheduleReconnect() {
    if (_reconnectTimer != null || _manualDisconnect) return;
    _setState(SseConnectionState.reconnecting);
    final delay = _backoffCalculator(_reconnectAttempts);
    _reconnectAttempts += 1;
    // Timer one-shot de reconexão após queda (AGENTS.md §9 — nunca Timer.periodic).
    _reconnectTimer = Timer(delay, () {
      _reconnectTimer = null;
      _openTransport();
    });
  }

  void _closeTransport() {
    _transport?.close();
    _transport = null;
  }

  void disconnect() {
    _manualDisconnect = true;
    _reconnectTimer?.cancel();
    _reconnectTimer = null;
    _closeTransport();
    _reconnectAttempts = 0;
    _setState(SseConnectionState.disconnected);
  }

  void dispose() {
    disconnect();
    _eventController.close();
    _stateController.close();
  }
}
