import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/network/sse_client.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/core/models/log_models.dart';

const int _liveTailMaxLines = 5000;

@immutable
class LogsState {
  const LogsState({
    this.filters = const LogFilters(),
    this.entries = const <LogEntry>[],
    this.templates = const <String, LogTemplateInfo>{},
    this.sources = const <LogSourceEntry>[],
    this.unknownCount = 0,
    this.nat,
    this.setupGuide,
    this.nextCursor,
    this.hasMore = false,
    this.loading = false,
    this.loadingMore = false,
    this.tailing = false,
    this.error,
  });

  final LogFilters filters;
  final List<LogEntry> entries;
  final Map<String, LogTemplateInfo> templates;
  final List<LogSourceEntry> sources;
  final int unknownCount;
  final LogNatDiagnostics? nat;
  final SetupGuide? setupGuide;
  final String? nextCursor;
  final bool hasMore;
  final bool loading;
  final bool loadingMore;
  final bool tailing;
  final String? error;

  bool get natMasking => (nat?.unresolvedMaskedCount ?? 0) > 0;

  LogTemplateInfo? templateOf(LogEntry entry) =>
      entry.templateHash != null ? templates[entry.templateHash!] : null;

  LogsState copyWith({
    LogFilters? filters,
    List<LogEntry>? entries,
    Map<String, LogTemplateInfo>? templates,
    List<LogSourceEntry>? sources,
    int? unknownCount,
    LogNatDiagnostics? nat,
    SetupGuide? setupGuide,
    String? nextCursor,
    bool clearNextCursor = false,
    bool? hasMore,
    bool? loading,
    bool? loadingMore,
    bool? tailing,
    String? error,
    bool clearError = false,
  }) {
    return LogsState(
      filters: filters ?? this.filters,
      entries: entries ?? this.entries,
      templates: templates ?? this.templates,
      sources: sources ?? this.sources,
      unknownCount: unknownCount ?? this.unknownCount,
      nat: nat ?? this.nat,
      setupGuide: setupGuide ?? this.setupGuide,
      nextCursor: clearNextCursor ? null : (nextCursor ?? this.nextCursor),
      hasMore: hasMore ?? this.hasMore,
      loading: loading ?? this.loading,
      loadingMore: loadingMore ?? this.loadingMore,
      tailing: tailing ?? this.tailing,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class LogsNotifier extends Notifier<LogsState> {
  SseClient? _tailClient;
  StreamSubscription<SseEvent>? _tailSub;
  StreamSubscription<SseConnectionState>? _tailStateSub;

  @override
  LogsState build() {
    ref.onDispose(stopTail);
    return const LogsState();
  }

  Map<String, dynamic> _queryParams({String? before, int limit = 100}) {
    final f = state.filters;
    return <String, dynamic>{
      'limit': limit,
      if (before != null && before.isNotEmpty) 'cursor': before,
      if (f.deviceId != null) 'deviceId': f.deviceId,
      if (f.severity != null) 'severity': f.severity,
      if (f.hours != null)
        'from': DateTime.now()
            .toUtc()
            .subtract(Duration(hours: f.hours!))
            .toIso8601String(),
      if (f.search.trim().isNotEmpty) 'q': f.search.trim(),
      if (f.category != null && f.category!.isNotEmpty) 'category': f.category,
    };
  }

  Future<void> fetchLogs({bool reset = true}) async {
    if (reset) {
      state = state.copyWith(loading: true, clearError: true);
    } else {
      if (!state.hasMore || state.loadingMore || state.nextCursor == null) {
        return;
      }
      state = state.copyWith(loadingMore: true, clearError: true);
    }

    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>(
            '/logs',
            queryParameters: _queryParams(
              before: reset ? null : state.nextCursor,
            ),
          );
      final newEntries = asJsonList<LogEntry>(raw['data'], LogEntry.fromJson);
      final rawTemplates = raw['templates'] is List
          ? raw['templates'] as List
          : const [];
      final updatedTemplates = <String, LogTemplateInfo>{...state.templates};
      for (final t in rawTemplates) {
        final info = LogTemplateInfo.tryFromJson(t);
        if (info != null) updatedTemplates[info.templateHash] = info;
      }

      final meta = raw['meta'] is Map
          ? asJsonMap(raw['meta'])
          : const <String, dynamic>{};
      final nextCursor = (meta['nextCursor'] ?? raw['nextCursor']) as String?;
      final hasMore = meta['hasMore'] == true || raw['hasMore'] == true;

      state = state.copyWith(
        entries: reset
            ? newEntries
            : <LogEntry>[...state.entries, ...newEntries],
        templates: updatedTemplates,
        nextCursor: nextCursor,
        clearNextCursor: nextCursor == null,
        hasMore: hasMore,
        loading: false,
        loadingMore: false,
      );
    } catch (err) {
      state = state.copyWith(
        loading: false,
        loadingMore: false,
        error: describeError(err),
      );
    }
  }

  Future<void> applyFilters(LogFilters next) async {
    final wasTailing = state.tailing;
    if (wasTailing) stopTail();
    state = state.copyWith(filters: next);
    await fetchLogs(reset: true);
    if (wasTailing) startTail();
  }

  Future<void> clearFilters() => applyFilters(const LogFilters());

  void applyTemplateClassified(Map<String, dynamic> data) {
    final info = LogTemplateInfo.tryFromJson(data);
    if (info == null) return;
    state = state.copyWith(
      templates: <String, LogTemplateInfo>{
        ...state.templates,
        info.templateHash: info,
      },
    );
  }

  Future<String?> confirmTemplate(String templateHash, String category) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .put<Map<String, dynamic>>(
            '/logs/templates/$templateHash',
            data: <String, dynamic>{'category': category},
          );
      applyTemplateClassified(raw);
      return null;
    } catch (err) {
      return describeError(err);
    }
  }

  /// Abre stream SSE dedicado `GET /api/logs/stream` (zero polling).
  void startTail() {
    if (_tailClient != null) return;
    final query = <String, String>{
      if (state.filters.deviceId != null)
        'deviceId': '${state.filters.deviceId}',
      if (state.filters.severity != null)
        'severity': '${state.filters.severity}',
    };
    _tailClient = ref
        .read(sseServiceProvider.notifier)
        .openCustomStream(path: '/logs/stream', queryParameters: query);
    _tailSub = _tailClient!.events.listen((event) {
      if (event.type == 'stream:connected') return;
      final entry = LogEntry.fromJson(event.data);
      if (entry.id == 0 && entry.message.isEmpty) return;
      if (state.entries.any((e) => e.id == entry.id && entry.id != 0)) return;
      final next = <LogEntry>[entry, ...state.entries];
      state = state.copyWith(
        entries: next.length > _liveTailMaxLines
            ? next.sublist(0, _liveTailMaxLines)
            : next,
      );
    });
    _tailStateSub = _tailClient!.connectionStates.listen((connState) {
      if (connState == SseConnectionState.disconnected && state.tailing) {
        state = state.copyWith(tailing: false);
      }
    });
    state = state.copyWith(tailing: true);
  }

  void stopTail() {
    _tailSub?.cancel();
    _tailStateSub?.cancel();
    _tailClient?.dispose();
    _tailSub = null;
    _tailStateSub = null;
    _tailClient = null;
    if (state.tailing) {
      state = state.copyWith(tailing: false);
    }
  }

  void toggleTail() {
    if (state.tailing) {
      stopTail();
    } else {
      startTail();
    }
  }

  Future<void> fetchSources() async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>('/logs/sources');
      state = state.copyWith(
        sources: asJsonList<LogSourceEntry>(
          raw['data'],
          LogSourceEntry.fromJson,
        ),
        unknownCount: parseIntOrNull(raw['unknownCount']) ?? 0,
        nat: raw['nat'] is Map
            ? LogNatDiagnostics.fromJson(asJsonMap(raw['nat']))
            : null,
      );
    } catch (_) {}
  }

  Future<void> bindSource(String bindKey, int? deviceId) async {
    await ref
        .read(apiClientProvider)
        .post<dynamic>(
          '/logs/sources/${Uri.encodeComponent(bindKey)}/bind',
          data: <String, dynamic>{'deviceId': deviceId},
        );
    await fetchSources();
  }

  Future<SetupGuide?> fetchSetupGuide({String? address}) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>(
            '/logs/setup-snippet',
            queryParameters: address != null && address.isNotEmpty
                ? <String, dynamic>{'address': address}
                : null,
          );
      final guide = SetupGuide.fromJson(raw);
      state = state.copyWith(setupGuide: guide);
      return guide;
    } catch (_) {
      return null;
    }
  }

  Future<ProvisionHintsResponse?> fetchProvisionHints(int deviceId) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>('/logs/devices/$deviceId/provision-hints');
      return ProvisionHintsResponse.fromJson(raw);
    } catch (_) {
      return null;
    }
  }

  Future<ProvisionLoggingResponse> provisionDevice(
    int deviceId,
    Map<String, dynamic> payload,
  ) async {
    final raw = await ref
        .read(apiClientProvider)
        .post<Map<String, dynamic>>(
          '/logs/devices/$deviceId/provision',
          data: payload,
          timeout: const Duration(seconds: 90),
        );
    final result = ProvisionLoggingResponse.fromJson(raw);
    await fetchSources();
    await fetchLogs(reset: true);
    return result;
  }

  Future<DownloadResult> exportLogs({String format = 'txt'}) {
    final params = _queryParams(limit: 5000)..remove('limit');
    if (format != 'txt') params['format'] = format;
    return ref
        .read(apiClientProvider)
        .downloadBytes('/logs/export', queryParameters: params);
  }
}

final logsProvider = NotifierProvider<LogsNotifier, LogsState>(
  LogsNotifier.new,
);
