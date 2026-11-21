import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:http/http.dart' as http;
import 'package:shared_preferences/shared_preferences.dart';

const String _serverUrlKey = 'netmonitor_server_url';
const String _recentServersKey = 'netmonitor_recent_servers';
const int _maxRecentServers = 8;

/// Resultado de teste de conectividade com uma central NetMonitor.
@immutable
class ServerProbeResult {
  const ServerProbeResult({
    required this.ok,
    required this.normalizedUrl,
    this.statusCode,
    this.needsSetup,
    this.latencyMs,
    this.error,
  });

  final bool ok;
  final String normalizedUrl;
  final int? statusCode;
  final bool? needsSetup;
  final int? latencyMs;
  final String? error;
}

/// Estado da configuração de endereço do servidor NetMonitor.
///
/// - Na Web/PWA: usa a própria origem (`Uri.base.origin`) por padrão.
/// - No Android: exige que o usuário informe/selecione a URL da central,
///   persistida localmente junto do histórico de servidores recentes.
@immutable
class ServerConfigState {
  const ServerConfigState({
    required this.baseUrl,
    required this.recentServers,
    required this.initialized,
  });

  /// URL base sem barra final e sem `/api` (ex.: `https://netmonitor.local`).
  final String baseUrl;
  final List<String> recentServers;
  final bool initialized;

  bool get isConfigured => kIsWeb || baseUrl.isNotEmpty;

  /// Prefixo completo da API REST (ex.: `https://host/api` ou `/api`).
  String get apiBaseUrl {
    if (baseUrl.isEmpty) {
      if (kIsWeb) {
        final origin = _webOrigin();
        return origin.isNotEmpty ? '$origin/api' : '/api';
      }
      return '/api';
    }
    return '$baseUrl/api';
  }

  ServerConfigState copyWith({
    String? baseUrl,
    List<String>? recentServers,
    bool? initialized,
  }) {
    return ServerConfigState(
      baseUrl: baseUrl ?? this.baseUrl,
      recentServers: recentServers ?? this.recentServers,
      initialized: initialized ?? this.initialized,
    );
  }
}

String _webOrigin() {
  if (!kIsWeb) return '';
  final base = Uri.base;
  if (base.scheme == 'http' || base.scheme == 'https') {
    return base.origin;
  }
  return '';
}

/// Normaliza a URL informada pelo operador:
/// - garante esquema (`http://` se omitido);
/// - remove `/api` e barras finais redundantes.
String normalizeServerUrl(String raw) {
  var value = raw.trim();
  if (value.isEmpty) return '';
  if (!value.startsWith('http://') && !value.startsWith('https://')) {
    value = 'http://$value';
  }
  while (value.endsWith('/')) {
    value = value.substring(0, value.length - 1);
  }
  if (value.endsWith('/api')) {
    value = value.substring(0, value.length - 4);
  }
  while (value.endsWith('/')) {
    value = value.substring(0, value.length - 1);
  }
  return value;
}

class ServerConfigNotifier extends Notifier<ServerConfigState> {
  @override
  ServerConfigState build() {
    final initialUrl = kIsWeb ? _webOrigin() : '';
    return ServerConfigState(
      baseUrl: initialUrl,
      recentServers: const <String>[],
      initialized: kIsWeb,
    );
  }

  /// Carrega a URL salva e servidores recentes do armazenamento local.
  Future<void> initialize() async {
    try {
      final prefs = await SharedPreferences.getInstance();
      final saved = prefs.getString(_serverUrlKey) ?? '';
      final recents =
          prefs.getStringList(_recentServersKey) ?? const <String>[];
      final effective = saved.isNotEmpty
          ? normalizeServerUrl(saved)
          : (kIsWeb ? _webOrigin() : '');
      state = ServerConfigState(
        baseUrl: effective,
        recentServers: List.unmodifiable(recents),
        initialized: true,
      );
    } catch (_) {
      state = state.copyWith(initialized: true);
    }
  }

  Future<void> setServerUrl(String rawUrl) async {
    final normalized = normalizeServerUrl(rawUrl);
    if (normalized.isEmpty) return;

    final updatedRecents = <String>[
      normalized,
      ...state.recentServers.where((item) => item != normalized),
    ];
    if (updatedRecents.length > _maxRecentServers) {
      updatedRecents.length = _maxRecentServers;
    }

    state = state.copyWith(
      baseUrl: normalized,
      recentServers: List.unmodifiable(updatedRecents),
      initialized: true,
    );

    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setString(_serverUrlKey, normalized);
      await prefs.setStringList(_recentServersKey, updatedRecents);
    } catch (_) {}
  }

  Future<void> removeRecentServer(String url) async {
    final updated = state.recentServers.where((item) => item != url).toList();
    state = state.copyWith(recentServers: List.unmodifiable(updated));
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setStringList(_recentServersKey, updated);
    } catch (_) {}
  }

  Future<void> clearServerUrl() async {
    final fallback = kIsWeb ? _webOrigin() : '';
    state = state.copyWith(baseUrl: fallback, initialized: true);
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.remove(_serverUrlKey);
    } catch (_) {}
  }

  /// Testa conectividade com `GET <url>/api/auth/setup`.
  Future<ServerProbeResult> probeServer(
    String rawUrl, {
    Duration timeout = const Duration(seconds: 6),
    http.Client? client,
  }) async {
    final normalized = normalizeServerUrl(rawUrl);
    final uri = Uri.tryParse('$normalized/api/auth/setup');
    if (normalized.isEmpty || uri == null || !uri.hasAuthority) {
      return ServerProbeResult(
        ok: false,
        normalizedUrl: normalized,
        error: 'Informe um endereço HTTP ou HTTPS válido.',
      );
    }

    final httpClient = client ?? http.Client();
    final stopwatch = Stopwatch()..start();
    try {
      final response = await httpClient
          .get(
            uri,
            headers: const <String, String>{'Accept': 'application/json'},
          )
          .timeout(timeout);
      stopwatch.stop();

      if (response.statusCode >= 200 && response.statusCode < 300) {
        bool? needsSetup;
        try {
          final decoded = jsonDecode(response.body);
          if (decoded is Map && decoded['needsSetup'] is bool) {
            needsSetup = decoded['needsSetup'] as bool;
          }
        } catch (_) {}
        return ServerProbeResult(
          ok: true,
          normalizedUrl: normalized,
          statusCode: response.statusCode,
          needsSetup: needsSetup,
          latencyMs: stopwatch.elapsedMilliseconds,
        );
      }

      return ServerProbeResult(
        ok: false,
        normalizedUrl: normalized,
        statusCode: response.statusCode,
        latencyMs: stopwatch.elapsedMilliseconds,
        error: 'Servidor respondeu com HTTP ${response.statusCode}.',
      );
    } catch (_) {
      stopwatch.stop();
      return ServerProbeResult(
        ok: false,
        normalizedUrl: normalized,
        latencyMs: stopwatch.elapsedMilliseconds,
        error: 'Não foi possível conectar ao servidor NetMonitor.',
      );
    } finally {
      if (client == null) {
        httpClient.close();
      }
    }
  }
}

final serverConfigProvider =
    NotifierProvider<ServerConfigNotifier, ServerConfigState>(
      ServerConfigNotifier.new,
    );
