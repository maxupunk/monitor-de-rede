import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/core/models/dashboard_models.dart';
import 'package:netmonitor_app/features/core/models/monitor_models.dart';

const int _historyLimit = 100;
const int _gaugeHistoryLimit = 20;

@immutable
class MonitorsState {
  const MonitorsState({
    this.monitors = const <MonitorModel>[],
    this.currentMonitor,
    this.devices = const <DeviceRefOption>[],
    this.probes = const <ProbeRefOption>[],
    this.sites = const <SiteRefOption>[],
    this.loading = false,
    this.runningId,
    this.error,
  });

  final List<MonitorModel> monitors;
  final MonitorModel? currentMonitor;
  final List<DeviceRefOption> devices;
  final List<ProbeRefOption> probes;
  final List<SiteRefOption> sites;
  final bool loading;
  final int? runningId;
  final String? error;

  int get totalCount => monitors.length;
  int get upCount => monitors.where((m) => m.normalizedStatus == 'up').length;
  int get warningCount =>
      monitors.where((m) => m.normalizedStatus == 'warning').length;
  int get downCount =>
      monitors.where((m) => m.normalizedStatus == 'down').length;
  int get disabledCount => monitors.where((m) => !m.isEnabled).length;

  MonitorsState copyWith({
    List<MonitorModel>? monitors,
    MonitorModel? currentMonitor,
    bool clearCurrent = false,
    List<DeviceRefOption>? devices,
    List<ProbeRefOption>? probes,
    List<SiteRefOption>? sites,
    bool? loading,
    int? runningId,
    bool clearRunningId = false,
    String? error,
    bool clearError = false,
  }) {
    return MonitorsState(
      monitors: monitors ?? this.monitors,
      currentMonitor: clearCurrent
          ? null
          : (currentMonitor ?? this.currentMonitor),
      devices: devices ?? this.devices,
      probes: probes ?? this.probes,
      sites: sites ?? this.sites,
      loading: loading ?? this.loading,
      runningId: clearRunningId ? null : (runningId ?? this.runningId),
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class MonitorsNotifier extends Notifier<MonitorsState> {
  @override
  MonitorsState build() => const MonitorsState();

  Future<List<MonitorModel>> fetchMonitors({
    int? deviceId,
    String? status,
    String? type,
    bool? enabled,
  }) async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final params = <String, dynamic>{
        'deviceId': ?deviceId,
        if (status != null && status.isNotEmpty) 'status': status,
        if (type != null && type.isNotEmpty) 'type': type,
        'enabled': ?enabled,
      };
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>(
            '/monitors',
            queryParameters: params.isEmpty ? null : params,
          );
      final list = raw is List
          ? raw
                .whereType<Map>()
                .map((e) => MonitorModel.fromJson(Map<String, dynamic>.from(e)))
                .toList(growable: false)
          : const <MonitorModel>[];
      if (params.isEmpty) {
        state = state.copyWith(monitors: list, loading: false);
      } else {
        state = state.copyWith(loading: false);
      }
      return list;
    } catch (err) {
      state = state.copyWith(loading: false, error: err.toString());
      return const <MonitorModel>[];
    }
  }

  Future<void> fetchReferenceCatalogs() async {
    final api = ref.read(apiClientProvider);
    final results = await Future.wait<dynamic>(<Future<dynamic>>[
      api.get<dynamic>('/devices').catchError((_) => const <dynamic>[]),
      api.get<dynamic>('/probes').catchError((_) => const <dynamic>[]),
      api.get<dynamic>('/sites').catchError((_) => const <dynamic>[]),
    ]);
    List<T> parseList<T>(dynamic raw, T Function(Map<String, dynamic>) mapper) {
      final list = raw is List
          ? raw
          : (raw is Map && raw['data'] is List
                ? raw['data'] as List
                : const []);
      return list
          .whereType<Map>()
          .map((e) => mapper(Map<String, dynamic>.from(e)))
          .toList(growable: false);
    }

    state = state.copyWith(
      devices: parseList(results[0], DeviceRefOption.fromJson),
      probes: parseList(results[1], ProbeRefOption.fromJson),
      sites: parseList(results[2], SiteRefOption.fromJson),
    );
  }

  Future<MonitorModel?> fetchMonitorById(int id) async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>('/monitors/$id');
      final monitor = MonitorModel.fromJson(raw);
      state = state.copyWith(currentMonitor: monitor, loading: false);
      return monitor;
    } catch (err) {
      state = state.copyWith(loading: false, error: err.toString());
      return null;
    }
  }

  Future<bool> createMonitor(Map<String, dynamic> payload) async {
    state = state.copyWith(clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<Map<String, dynamic>>('/monitors', data: payload);
      final created = MonitorModel.fromJson(raw);
      state = state.copyWith(
        monitors: <MonitorModel>[...state.monitors, created],
      );
      return true;
    } catch (err) {
      state = state.copyWith(error: err.toString());
      return false;
    }
  }

  Future<bool> updateMonitor(int id, Map<String, dynamic> payload) async {
    state = state.copyWith(clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .put<Map<String, dynamic>>('/monitors/$id', data: payload);
      final updated = MonitorModel.fromJson(raw);
      state = state.copyWith(
        monitors: state.monitors
            .map((m) => m.id == id ? updated : m)
            .toList(growable: false),
        currentMonitor: state.currentMonitor?.id == id
            ? updated
            : state.currentMonitor,
      );
      return true;
    } catch (err) {
      state = state.copyWith(error: err.toString());
      return false;
    }
  }

  Future<bool> deleteMonitor(int id) async {
    state = state.copyWith(clearError: true);
    try {
      await ref.read(apiClientProvider).delete<dynamic>('/monitors/$id');
      state = state.copyWith(
        monitors: state.monitors
            .where((m) => m.id != id)
            .toList(growable: false),
        clearCurrent: state.currentMonitor?.id == id,
      );
      return true;
    } catch (err) {
      state = state.copyWith(error: err.toString());
      return false;
    }
  }

  Future<bool> runMonitor(int id) async {
    state = state.copyWith(runningId: id, clearError: true);
    try {
      await ref.read(apiClientProvider).post<dynamic>('/monitors/$id/run');
      if (state.currentMonitor?.id == id) {
        await fetchMonitorById(id);
      } else {
        await fetchMonitors();
      }
      return true;
    } catch (err) {
      state = state.copyWith(error: err.toString());
      return false;
    } finally {
      state = state.copyWith(clearRunningId: true);
    }
  }

  Future<bool> toggleEnabled(int id, bool enable) async {
    state = state.copyWith(clearError: true);
    try {
      final path = enable ? '/monitors/$id/enable' : '/monitors/$id/disable';
      final raw = await ref
          .read(apiClientProvider)
          .post<Map<String, dynamic>>(path);
      final updated = MonitorModel.fromJson(raw);
      state = state.copyWith(
        monitors: state.monitors
            .map((m) => m.id == id ? updated : m)
            .toList(growable: false),
        currentMonitor: state.currentMonitor?.id == id
            ? updated
            : state.currentMonitor,
      );
      return true;
    } catch (err) {
      state = state.copyWith(error: err.toString());
      return false;
    }
  }

  Future<MonitorUptimeResponse?> fetchUptime(int id, {int hours = 24}) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>(
            '/monitors/$id/uptime',
            queryParameters: <String, dynamic>{'hours': hours},
          );
      return MonitorUptimeResponse.fromJson(raw);
    } catch (_) {
      return null;
    }
  }

  Future<Map<String, dynamic>?> fetchBaseline(int id) async {
    try {
      return await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>('/monitors/$id/baseline');
    } catch (_) {
      return null;
    }
  }

  Future<List<DeviceInterfaceRef>> fetchDeviceInterfaces(int deviceId) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/devices/$deviceId/interfaces');
      final list = raw is List
          ? raw
          : (raw is Map && raw['data'] is List
                ? raw['data'] as List
                : const []);
      return list
          .whereType<Map>()
          .map((e) => DeviceInterfaceRef.fromJson(Map<String, dynamic>.from(e)))
          .toList(growable: false);
    } catch (_) {
      return const <DeviceInterfaceRef>[];
    }
  }

  Future<List<DeviceInterfaceRef>> scanDeviceInterfaces(int deviceId) async {
    try {
      await ref
          .read(apiClientProvider)
          .post<dynamic>('/devices/$deviceId/snmp/scan');
      return await fetchDeviceInterfaces(deviceId);
    } catch (_) {
      return const <DeviceInterfaceRef>[];
    }
  }

  Future<List<SaasPresetItem>> fetchSaasPresets() async {
    final raw = await ref
        .read(apiClientProvider)
        .get<Map<String, dynamic>>('/monitors/saas/presets');
    final list = raw['presets'] is List
        ? raw['presets'] as List
        : (raw['data'] is List ? raw['data'] as List : const []);
    return list
        .whereType<Map>()
        .map((e) => SaasPresetItem.fromJson(Map<String, dynamic>.from(e)))
        .toList(growable: false);
  }

  Future<Map<String, dynamic>> provisionSaasPresets(
    List<String> presetIds, {
    int? intervalSeconds,
    int? timeoutSeconds,
  }) async {
    final res = await ref
        .read(apiClientProvider)
        .post<Map<String, dynamic>>(
          '/monitors/saas/provision',
          data: <String, dynamic>{
            'presetIds': presetIds,
            'intervalSeconds': ?intervalSeconds,
            'timeoutSeconds': ?timeoutSeconds,
          },
        );
    await fetchMonitors();
    return res;
  }

  Future<MonitorTimeSeriesResponse?> fetchTimeSeries({
    int? monitorId,
    String timeframe = '1h',
  }) async {
    try {
      final res = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>(
            '/monitors/timeseries',
            queryParameters: <String, dynamic>{
              'monitorId': ?monitorId,
              'timeframe': timeframe,
            },
          );
      return MonitorTimeSeriesResponse.fromJson(res);
    } catch (_) {
      return null;
    }
  }

  /// Aplica `monitor:result` vindo do SSE em memória (zero polling).
  void applyRealtimeResult(Map<String, dynamic> data) {
    final id = ((data['monitorId'] ?? data['id']) as num?)?.toInt();
    if (id == null) return;
    final status = data['status']?.toString();
    final latency = (data['latencyMs'] as num?)?.toDouble();
    final finishedAt =
        data['finishedAt']?.toString() ??
        DateTime.now().toUtc().toIso8601String();
    final result = MonitorResult(
      id: DateTime.now().millisecondsSinceEpoch,
      monitorId: id,
      status: status ?? 'unknown',
      startedAt: data['startedAt']?.toString() ?? finishedAt,
      finishedAt: finishedAt,
      durationMs: (data['durationMs'] as num?)?.toInt() ?? 0,
      latencyMs: latency,
      message: data['message'] as String?,
    );

    MonitorModel patch(MonitorModel target, {required bool withStats}) {
      final combined = <MonitorResult>[...target.recentResults, result];
      final trimmed = combined.length > _historyLimit
          ? combined.sublist(combined.length - _historyLimit)
          : combined;
      return target.copyWith(
        status: status ?? target.status,
        lastLatencyMs: latency ?? target.lastLatencyMs,
        lastCheckedAt: finishedAt,
        recentResults: trimmed,
        stats: withStats ? MonitorStats.compute(trimmed) : target.stats,
      );
    }

    final nextList = state.monitors
        .map((m) => m.id == id ? patch(m, withStats: false) : m)
        .toList(growable: false);
    final nextCurrent = state.currentMonitor?.id == id
        ? patch(state.currentMonitor!, withStats: true)
        : state.currentMonitor;

    state = state.copyWith(monitors: nextList, currentMonitor: nextCurrent);
  }

  /// Aplica `metric:recorded` nos monitores SNMP gauge/tráfego.
  void applyRealtimeMetrics(Map<String, dynamic> data) {
    final deviceId = (data['deviceId'] as num?)?.toInt();
    final rawMetrics = data['metrics'];
    if (deviceId == null || rawMetrics is! List || rawMetrics.isEmpty) return;
    final metrics = rawMetrics
        .whereType<Map>()
        .map((e) => Map<String, dynamic>.from(e))
        .toList(growable: false);

    MonitorModel patch(MonitorModel target) {
      if (target.deviceId != deviceId || !target.isSnmpGauge) return target;
      if (target.isTrafficMonitor) {
        final ifName = target.configuration['ifName']?.toString().toLowerCase();
        Map<String, dynamic>? findSample(String metricName) {
          for (final m in metrics.reversed) {
            if (m['name'] != metricName) continue;
            if (ifName != null &&
                m['interfaceName'] != null &&
                m['interfaceName'].toString().toLowerCase() != ifName) {
              continue;
            }
            return m;
          }
          return null;
        }

        final inSample = findSample('inBps');
        final outSample = findSample('outBps');
        if (inSample == null && outSample == null) return target;
        final recordedAt =
            (inSample?['recordedAt'] ??
                    outSample?['recordedAt'] ??
                    DateTime.now().toUtc().toIso8601String())
                .toString();
        final inVal = (inSample?['value'] as num?)?.toDouble();
        final outVal = (outSample?['value'] as num?)?.toDouble();
        List<MonitorGaugeSample> append(
          List<MonitorGaugeSample> list,
          double? val,
        ) {
          if (val == null) return list;
          final next = <MonitorGaugeSample>[
            ...list,
            MonitorGaugeSample(value: val, recordedAt: recordedAt),
          ];
          return next.length > _gaugeHistoryLimit
              ? next.sublist(next.length - _gaugeHistoryLimit)
              : next;
        }

        final nextIn = append(target.inHistory, inVal);
        final nextOut = append(target.outHistory, outVal);
        return target.copyWith(
          inBps: inVal ?? target.inBps,
          outBps: outVal ?? target.outBps,
          inHistory: nextIn,
          outHistory: nextOut,
          gaugeHistory: nextIn.isNotEmpty ? nextIn : nextOut,
        );
      }

      final mode = target.snmpMode;
      final queryName = mode == 'memory' || mode == 'memory_usage'
          ? 'memory_usage'
          : (mode == 'cpu' ? 'cpu_usage' : mode);
      final match = metrics.reversed.cast<Map<String, dynamic>?>().firstWhere(
        (m) => m?['name'] == queryName || m?['name'] == mode,
        orElse: () => null,
      );
      if (match == null) return target;
      final val = (match['value'] as num?)?.toDouble();
      if (val == null) return target;
      final recordedAt =
          (match['recordedAt'] ?? DateTime.now().toUtc().toIso8601String())
              .toString();
      final nextHistory = <MonitorGaugeSample>[
        ...target.gaugeHistory,
        MonitorGaugeSample(value: val, recordedAt: recordedAt),
      ];
      return target.copyWith(
        gaugeMetric: MonitorGaugeMetric(
          name: queryName,
          value: val,
          unit: (match['unit'] ?? '%').toString(),
          recordedAt: recordedAt,
        ),
        gaugeHistory: nextHistory.length > _gaugeHistoryLimit
            ? nextHistory.sublist(nextHistory.length - _gaugeHistoryLimit)
            : nextHistory,
      );
    }

    state = state.copyWith(
      monitors: state.monitors.map(patch).toList(growable: false),
      currentMonitor: state.currentMonitor != null
          ? patch(state.currentMonitor!)
          : null,
    );
  }

  /// Aplica `interface:traffic` efêmero em memória.
  void applyInterfaceTraffic(Map<String, dynamic> data) {
    final deviceId = (data['deviceId'] as num?)?.toInt();
    final rawIfs = data['interfaces'];
    if (deviceId == null || rawIfs is! List || rawIfs.isEmpty) return;
    final interfaces = rawIfs
        .whereType<Map>()
        .map((e) => Map<String, dynamic>.from(e))
        .toList(growable: false);

    MonitorModel patch(MonitorModel target) {
      if (target.deviceId != deviceId || !target.isTrafficMonitor) {
        return target;
      }
      final ifIndex = (target.configuration['ifIndex'] as num?)?.toInt();
      final ifName = target.configuration['ifName']?.toString().toLowerCase();
      final match = interfaces.cast<Map<String, dynamic>?>().firstWhere((intf) {
        if (intf == null) return false;
        if (ifIndex != null && intf['ifIndex'] is num) {
          return (intf['ifIndex'] as num).toInt() == ifIndex;
        }
        if (ifName != null && intf['name'] != null) {
          return intf['name'].toString().toLowerCase() == ifName;
        }
        return false;
      }, orElse: () => null);
      if (match == null) return target;
      final nowIso = DateTime.now().toUtc().toIso8601String();
      final inVal = (match['inBps'] as num?)?.toDouble();
      final outVal = (match['outBps'] as num?)?.toDouble();
      List<MonitorGaugeSample> append(
        List<MonitorGaugeSample> list,
        double? val,
      ) {
        if (val == null) return list;
        final next = <MonitorGaugeSample>[
          ...list,
          MonitorGaugeSample(value: val, recordedAt: nowIso),
        ];
        return next.length > _gaugeHistoryLimit
            ? next.sublist(next.length - _gaugeHistoryLimit)
            : next;
      }

      return target.copyWith(
        inBps: inVal ?? target.inBps,
        outBps: outVal ?? target.outBps,
        inHistory: append(target.inHistory, inVal),
        outHistory: append(target.outHistory, outVal),
      );
    }

    state = state.copyWith(
      monitors: state.monitors.map(patch).toList(growable: false),
      currentMonitor: state.currentMonitor != null
          ? patch(state.currentMonitor!)
          : null,
    );
  }
}

final monitorsProvider = NotifierProvider<MonitorsNotifier, MonitorsState>(
  MonitorsNotifier.new,
);
