import 'dart:async';

import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';

// ── Diagnostics (Playbook, Traceroute, SpeedTest WAN/LAN, QuickPing, PortScan) ──

class DiagnosticsService {
  DiagnosticsService(this._api);

  final ApiClient _api;

  Stream<Map<String, dynamic>> streamPlaybook({
    required String playbookType,
    String? target,
    int? deviceId,
    void Function(VoidCallback cancel)? onCancelAvailable,
  }) {
    return _api.streamNdjson(
      '/diagnostics/playbook/run',
      body: <String, dynamic>{
        'playbookType': playbookType,
        if (target != null && target.trim().isNotEmpty) 'target': target.trim(),
        'deviceId': ?deviceId,
      },
      onCancelAvailable: onCancelAvailable,
    );
  }

  Stream<Map<String, dynamic>> streamTraceroute({
    required String host,
    int maxHops = 30,
    int timeoutMs = 2000,
    int probesPerHop = 3,
    void Function(VoidCallback cancel)? onCancelAvailable,
  }) {
    return _api.streamNdjson(
      '/diagnostics/traceroute',
      body: <String, dynamic>{
        'host': host.trim(),
        'maxHops': maxHops,
        'timeoutMs': timeoutMs,
        'probesPerHop': probesPerHop,
      },
      onCancelAvailable: onCancelAvailable,
    );
  }

  Stream<Map<String, dynamic>> streamPortScan({
    required String host,
    required String protocol,
    required List<int> ports,
    String profile = 'fast',
    int? timeoutMs,
    void Function(VoidCallback cancel)? onCancelAvailable,
  }) {
    return _api.streamNdjson(
      '/port-scan',
      body: <String, dynamic>{
        'host': host.trim(),
        'protocol': protocol,
        'ports': ports,
        'profile': profile,
        'timeoutMs': ?timeoutMs,
      },
      onCancelAvailable: onCancelAvailable,
    );
  }

  Stream<Map<String, dynamic>> streamWanSpeedTest({
    String mode = 'medium',
    void Function(VoidCallback cancel)? onCancelAvailable,
  }) {
    return _api.streamNdjson(
      '/diagnostics/speedtest',
      body: <String, dynamic>{'mode': mode},
      onCancelAvailable: onCancelAvailable,
    );
  }

  Future<SpeedTestResult> runLanSpeedTest({
    required String mode,
    required void Function(SpeedTestProgress progress) onProgress,
    required CancelToken cancelToken,
  }) async {
    var pingCount = 5;
    var downloadBytes = 20000000;
    var uploadBytes = 12000000;
    if (mode == 'simple') {
      pingCount = 3;
      downloadBytes = 8000000;
      uploadBytes = 5000000;
    } else if (mode == 'complete') {
      pingCount = 8;
      downloadBytes = 40000000;
      uploadBytes = 25000000;
    }

    onProgress(
      const SpeedTestProgress(
        phase: 'ping',
        progressPct: 5,
        serverName: 'Servidor Local (LAN)',
        serverLocation: 'Rede Local',
      ),
    );

    final pings = <double>[];
    for (var i = 0; i < pingCount; i++) {
      final sw = Stopwatch()..start();
      await _api.downloadBytes(
        '/diagnostics/speedtest/lan/download',
        queryParameters: const <String, dynamic>{'bytes': 0},
        cancelToken: cancelToken,
      );
      sw.stop();
      pings.add(sw.elapsedMicroseconds / 1000.0);
      final avg = pings.reduce((a, b) => a + b) / pings.length;
      onProgress(
        SpeedTestProgress(
          phase: 'ping',
          progressPct: 5 + ((i + 1) / pingCount) * 15,
          pingMs: avg,
          serverName: 'Servidor Local (LAN)',
          serverLocation: 'Rede Local',
        ),
      );
    }

    final avgPing = pings.reduce((a, b) => a + b) / pings.length;
    var jitter = 0.0;
    if (pings.length > 1) {
      var diffSum = 0.0;
      for (var i = 1; i < pings.length; i++) {
        diffSum += (pings[i] - pings[i - 1]).abs();
      }
      jitter = diffSum / (pings.length - 1);
    }

    // Download LAN
    onProgress(
      SpeedTestProgress(
        phase: 'download',
        progressPct: 22,
        pingMs: avgPing,
        jitterMs: jitter,
        serverName: 'Servidor Local (LAN)',
        serverLocation: 'Rede Local',
      ),
    );
    final dlWatch = Stopwatch()..start();
    final dlResult = await _api.downloadBytes(
      '/diagnostics/speedtest/lan/download',
      queryParameters: <String, dynamic>{'bytes': downloadBytes},
      cancelToken: cancelToken,
      onReceiveProgress: (received, total) {
        final sec = dlWatch.elapsedMicroseconds / 1e6;
        if (sec > 0.05 && received > 0) {
          final mbps = (received * 8) / (sec * 1e6);
          final denom = total > 0 ? total : downloadBytes;
          final pct = (22 + (received / denom) * 45).clamp(22.0, 67.0);
          onProgress(
            SpeedTestProgress(
              phase: 'download',
              progressPct: pct,
              currentMbps: mbps,
              downloadMbps: mbps,
              pingMs: avgPing,
              jitterMs: jitter,
              serverName: 'Servidor Local (LAN)',
              serverLocation: 'Rede Local',
            ),
          );
        }
      },
    );
    dlWatch.stop();
    final dlSec = (dlWatch.elapsedMicroseconds / 1e6).clamp(0.001, 3600.0);
    final finalDownloadMbps = (dlResult.bytes.length * 8) / (dlSec * 1e6);

    // Upload LAN via raw binary POST (`postRawBytes`)
    onProgress(
      SpeedTestProgress(
        phase: 'upload',
        progressPct: 68,
        downloadMbps: finalDownloadMbps,
        pingMs: avgPing,
        jitterMs: jitter,
        serverName: 'Servidor Local (LAN)',
        serverLocation: 'Rede Local',
      ),
    );
    final payload = Uint8List(uploadBytes);
    final ulWatch = Stopwatch()..start();
    await _api.postRawBytes<dynamic>(
      '/diagnostics/speedtest/lan/upload',
      payload,
      cancelToken: cancelToken,
      onSendProgress: (sent, total) {
        final sec = ulWatch.elapsedMicroseconds / 1e6;
        if (sec > 0.05 && sent > 0) {
          final mbps = (sent * 8) / (sec * 1e6);
          final denom = total > 0 ? total : uploadBytes;
          final pct = (68 + (sent / denom) * 28).clamp(68.0, 96.0);
          onProgress(
            SpeedTestProgress(
              phase: 'upload',
              progressPct: pct,
              currentMbps: mbps,
              uploadMbps: mbps,
              downloadMbps: finalDownloadMbps,
              pingMs: avgPing,
              jitterMs: jitter,
              serverName: 'Servidor Local (LAN)',
              serverLocation: 'Rede Local',
            ),
          );
        }
      },
    );
    ulWatch.stop();
    final ulSec = (ulWatch.elapsedMicroseconds / 1e6).clamp(0.001, 3600.0);
    final finalUploadMbps = (uploadBytes * 8) / (ulSec * 1e6);

    final result = SpeedTestResult(
      pingMs: avgPing,
      jitterMs: jitter,
      downloadMbps: finalDownloadMbps,
      uploadMbps: finalUploadMbps,
      serverName: 'Servidor Local (LAN)',
      serverLocation: 'Rede Local (Cliente ↔ Servidor)',
      timestamp: DateTime.now().toIso8601String(),
    );
    onProgress(
      SpeedTestProgress(
        phase: 'complete',
        progressPct: 100,
        pingMs: result.pingMs,
        jitterMs: result.jitterMs,
        downloadMbps: result.downloadMbps,
        uploadMbps: result.uploadMbps,
        serverName: result.serverName,
        serverLocation: result.serverLocation,
      ),
    );
    return result;
  }

  Future<QuickPingReport> quickPing(int deviceId) async {
    final raw = await _api.post<dynamic>(
      '/diagnostics/quick-ping',
      data: <String, dynamic>{'deviceId': deviceId},
      timeout: const Duration(seconds: 25),
    );
    return QuickPingReport.fromJson(asJsonMap(raw));
  }
}

final diagnosticsServiceProvider = Provider<DiagnosticsService>(
  (ref) => DiagnosticsService(ref.read(apiClientProvider)),
);

// ── DNS Servers ───────────────────────────────────────────────────────────────

@immutable
class DnsServersState {
  const DnsServersState({
    this.items = const <DnsServerModel>[],
    this.loading = false,
    this.saving = false,
    this.error,
  });

  final List<DnsServerModel> items;
  final bool loading;
  final bool saving;
  final String? error;

  DnsServersState copyWith({
    List<DnsServerModel>? items,
    bool? loading,
    bool? saving,
    String? error,
    bool clearError = false,
  }) {
    return DnsServersState(
      items: items ?? this.items,
      loading: loading ?? this.loading,
      saving: saving ?? this.saving,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class DnsServersNotifier extends Notifier<DnsServersState> {
  @override
  DnsServersState build() => const DnsServersState();

  Future<void> fetchServers() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/dns/servers');
      state = state.copyWith(
        items: asJsonMapList(
          raw,
        ).map(DnsServerModel.fromJson).toList(growable: false),
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(loading: false, error: err.toString());
    }
  }

  Future<bool> createServer(Map<String, dynamic> payload) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>('/dns/servers', data: payload);
      final created = DnsServerModel.fromJson(asJsonMap(raw));
      state = state.copyWith(
        items: <DnsServerModel>[...state.items, created],
        saving: false,
      );
      return true;
    } catch (err) {
      state = state.copyWith(saving: false, error: err.toString());
      return false;
    }
  }

  Future<bool> updateServer(int id, Map<String, dynamic> payload) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .put<dynamic>('/dns/servers/$id', data: payload);
      final updated = DnsServerModel.fromJson(asJsonMap(raw));
      state = state.copyWith(
        items: state.items
            .map((s) => s.id == id ? updated : s)
            .toList(growable: false),
        saving: false,
      );
      return true;
    } catch (err) {
      state = state.copyWith(saving: false, error: err.toString());
      return false;
    }
  }

  Future<bool> deleteServer(int id) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      await ref.read(apiClientProvider).delete<dynamic>('/dns/servers/$id');
      state = state.copyWith(
        items: state.items.where((s) => s.id != id).toList(growable: false),
        saving: false,
      );
      return true;
    } catch (err) {
      state = state.copyWith(saving: false, error: err.toString());
      return false;
    }
  }
}

final dnsServersProvider =
    NotifierProvider<DnsServersNotifier, DnsServersState>(
      DnsServersNotifier.new,
    );

// ── DNS Performance ───────────────────────────────────────────────────────────

@immutable
class DnsPerformanceState {
  const DnsPerformanceState({
    this.ranking = const <DnsRankingEntry>[],
    this.series = const <DnsSeriesItem>[],
    this.source = 'history',
    this.windowHours = 24,
    this.monitorCount = 0,
    this.measuredAt,
    this.loading = false,
    this.benchmarking = false,
    this.provisioning = false,
    this.error,
  });

  final List<DnsRankingEntry> ranking;
  final List<DnsSeriesItem> series;
  final String source;
  final int windowHours;
  final int monitorCount;
  final String? measuredAt;
  final bool loading;
  final bool benchmarking;
  final bool provisioning;
  final String? error;

  DnsRankingEntry? get fastest {
    for (final item in ranking) {
      if (item.avgLookupTimeMs != null) return item;
    }
    return null;
  }

  double get slowestLatency {
    double maxVal = 0;
    for (final item in ranking) {
      final avg = item.avgLookupTimeMs;
      if (avg != null && avg > maxVal) maxVal = avg;
    }
    return maxVal;
  }

  DnsPerformanceState copyWith({
    List<DnsRankingEntry>? ranking,
    List<DnsSeriesItem>? series,
    String? source,
    int? windowHours,
    int? monitorCount,
    String? measuredAt,
    bool clearMeasuredAt = false,
    bool? loading,
    bool? benchmarking,
    bool? provisioning,
    String? error,
    bool clearError = false,
  }) {
    return DnsPerformanceState(
      ranking: ranking ?? this.ranking,
      series: series ?? this.series,
      source: source ?? this.source,
      windowHours: windowHours ?? this.windowHours,
      monitorCount: monitorCount ?? this.monitorCount,
      measuredAt: clearMeasuredAt ? null : (measuredAt ?? this.measuredAt),
      loading: loading ?? this.loading,
      benchmarking: benchmarking ?? this.benchmarking,
      provisioning: provisioning ?? this.provisioning,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class DnsPerformanceNotifier extends Notifier<DnsPerformanceState> {
  @override
  DnsPerformanceState build() => const DnsPerformanceState();

  Future<void> fetchPerformance({int hours = 24}) async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>(
            '/dns/performance',
            queryParameters: <String, dynamic>{'hours': hours},
          );
      final map = asJsonMap(raw);
      state = state.copyWith(
        ranking: asJsonMapList(
          map['ranking'],
        ).map(DnsRankingEntry.fromJson).toList(growable: false),
        series: asJsonMapList(
          map['series'],
        ).map(DnsSeriesItem.fromJson).toList(growable: false),
        windowHours: parseIntOrNull(map['windowHours']) ?? hours,
        monitorCount: parseIntOrNull(map['monitorCount']) ?? 0,
        source: 'history',
        clearMeasuredAt: true,
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(loading: false, error: err.toString());
    }
  }

  Future<void> runBenchmark({
    Map<String, dynamic> payload = const <String, dynamic>{},
  }) async {
    state = state.copyWith(benchmarking: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>(
            '/dns/benchmark',
            data: payload,
            timeout: const Duration(seconds: 45),
          );
      final map = asJsonMap(raw);
      state = state.copyWith(
        ranking: asJsonMapList(
          map['ranking'],
        ).map(DnsRankingEntry.fromJson).toList(growable: false),
        measuredAt: map['measuredAt'] as String?,
        source: 'benchmark',
        benchmarking: false,
      );
    } catch (err) {
      state = state.copyWith(benchmarking: false, error: err.toString());
    }
  }

  Future<DnsBatchProvisionResponse?> provisionMonitors(
    Map<String, dynamic> payload,
  ) async {
    state = state.copyWith(provisioning: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>('/dns/provision', data: payload);
      state = state.copyWith(provisioning: false);
      return DnsBatchProvisionResponse.fromJson(asJsonMap(raw));
    } catch (err) {
      state = state.copyWith(provisioning: false, error: err.toString());
      return null;
    }
  }

  Future<DnsBatchProvisionResponse?> provisionPingMonitors({
    List<Map<String, dynamic>>? servers,
    int? intervalSeconds,
  }) async {
    state = state.copyWith(provisioning: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>(
            '/dns/provision-ping',
            data: <String, dynamic>{
              'servers': ?servers,
              'intervalSeconds': ?intervalSeconds,
            },
          );
      state = state.copyWith(provisioning: false);
      return DnsBatchProvisionResponse.fromJson(asJsonMap(raw));
    } catch (err) {
      state = state.copyWith(provisioning: false, error: err.toString());
      return null;
    }
  }
}

final dnsPerformanceProvider =
    NotifierProvider<DnsPerformanceNotifier, DnsPerformanceState>(
      DnsPerformanceNotifier.new,
    );
