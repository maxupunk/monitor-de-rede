import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';

const Duration _snmpTimeout = Duration(seconds: 120);
const int _metricsLimit = 2000;

@immutable
class DevicesState {
  const DevicesState({
    this.items = const <DeviceModel>[],
    this.loading = false,
    this.error,
  });

  final List<DeviceModel> items;
  final bool loading;
  final String? error;

  int get totalCount => items.length;
  int get onlineCount => items.where((d) => d.status == 'online').length;
  int get offlineCount => items.where((d) => d.status == 'offline').length;
  int get warningCount => items.where((d) => d.status == 'warning').length;
  int get unknownCount => items.where((d) => d.status == 'unknown').length;

  DevicesState copyWith({
    List<DeviceModel>? items,
    bool? loading,
    String? error,
    bool clearError = false,
  }) {
    return DevicesState(
      items: items ?? this.items,
      loading: loading ?? this.loading,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class DevicesNotifier extends Notifier<DevicesState> {
  @override
  DevicesState build() => const DevicesState();

  Future<void> fetchDevices() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await ref.read(apiClientProvider).get<dynamic>('/devices');
      final list = asJsonMapList(
        raw,
      ).map(DeviceModel.fromJson).toList(growable: false);
      state = state.copyWith(items: list, loading: false);
    } catch (err) {
      state = state.copyWith(loading: false, error: err.toString());
    }
  }

  Future<DeviceModel?> createDevice(Map<String, dynamic> payload) async {
    state = state.copyWith(clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>('/devices', data: payload);
      final created = DeviceModel.fromJson(asJsonMap(raw));
      state = state.copyWith(items: <DeviceModel>[...state.items, created]);
      return created;
    } catch (err) {
      state = state.copyWith(error: err.toString());
      return null;
    }
  }

  Future<DeviceModel?> updateDevice(
    int id,
    Map<String, dynamic> payload,
  ) async {
    state = state.copyWith(clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .put<dynamic>('/devices/$id', data: payload);
      final updated = DeviceModel.fromJson(asJsonMap(raw));
      state = state.copyWith(
        items: state.items
            .map((d) => d.id == id ? updated : d)
            .toList(growable: false),
      );
      return updated;
    } catch (err) {
      state = state.copyWith(error: err.toString());
      return null;
    }
  }

  Future<bool> deleteDevice(int id) async {
    state = state.copyWith(clearError: true);
    try {
      await ref.read(apiClientProvider).delete<dynamic>('/devices/$id');
      state = state.copyWith(
        items: state.items.where((d) => d.id != id).toList(growable: false),
      );
      return true;
    } catch (err) {
      state = state.copyWith(error: err.toString());
      return false;
    }
  }

  Future<bool> batchSetParent(List<int> deviceIds, int? parentId) async {
    try {
      await ref
          .read(apiClientProvider)
          .post<dynamic>(
            '/devices/batch-parent',
            data: <String, dynamic>{
              'deviceIds': deviceIds,
              'parentId': parentId,
            },
          );
      await fetchDevices();
      return true;
    } catch (err) {
      state = state.copyWith(error: err.toString());
      return false;
    }
  }

  Future<BandwidthLatencyResponse?> fetchBandwidthLatencySeries({
    Object? deviceId,
    Object? pingTarget,
    String timeframe = '1h',
  }) async {
    try {
      final query = <String, dynamic>{'timeframe': timeframe};
      if (deviceId != null && deviceId != 'all') query['deviceId'] = deviceId;
      if (pingTarget != null && pingTarget != 'all') {
        query['pingTarget'] = pingTarget;
      }
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>(
            '/devices/bandwidth-latency-series',
            queryParameters: query,
          );
      return BandwidthLatencyResponse.fromJson(asJsonMap(raw));
    } catch (_) {
      return null;
    }
  }

  /// Aplica evento SSE `device:status` ou `device:updated` sem recarregar a lista.
  void applyRealtimeStatus(Map<String, dynamic> data) {
    final id = parseIntOrNull(data['id'] ?? data['deviceId']);
    if (id == null) return;
    final index = state.items.indexWhere((d) => d.id == id);
    if (index < 0) return;
    final current = state.items[index];
    final updated = current.copyWith(
      status: data['status'] is String ? data['status'] as String : null,
      name: data['name'] is String ? data['name'] as String : null,
      ipAddress: data['ipAddress'] is String
          ? data['ipAddress'] as String
          : null,
      lastSeenAt: data['lastSeenAt'] is String
          ? data['lastSeenAt'] as String
          : null,
    );
    final next = List<DeviceModel>.from(state.items)..[index] = updated;
    state = state.copyWith(items: List<DeviceModel>.unmodifiable(next));
  }
}

final devicesProvider = NotifierProvider<DevicesNotifier, DevicesState>(
  DevicesNotifier.new,
);

@immutable
class DeviceDetailState {
  const DeviceDetailState({
    this.device,
    this.interfaces = const <DeviceInterfaceModel>[],
    this.monitors = const <DeviceMonitorSummary>[],
    this.metrics = const <DeviceMetricModel>[],
    this.events = const <DeviceEventModel>[],
    this.capabilities,
    this.uptimeByMonitor = const <int, MonitorUptimeModel>{},
    this.scanResult,
    this.loading = false,
    this.pollingSnmp = false,
    this.scanningSnmp = false,
    this.updatingInterfaceId,
    this.error,
  });

  final DeviceModel? device;
  final List<DeviceInterfaceModel> interfaces;
  final List<DeviceMonitorSummary> monitors;
  final List<DeviceMetricModel> metrics;
  final List<DeviceEventModel> events;
  final DeviceCapabilitiesModel? capabilities;
  final Map<int, MonitorUptimeModel> uptimeByMonitor;
  final SnmpScanResultModel? scanResult;
  final bool loading;
  final bool pollingSnmp;
  final bool scanningSnmp;
  final int? updatingInterfaceId;
  final String? error;

  DeviceDetailState copyWith({
    DeviceModel? device,
    List<DeviceInterfaceModel>? interfaces,
    List<DeviceMonitorSummary>? monitors,
    List<DeviceMetricModel>? metrics,
    List<DeviceEventModel>? events,
    DeviceCapabilitiesModel? capabilities,
    Map<int, MonitorUptimeModel>? uptimeByMonitor,
    SnmpScanResultModel? scanResult,
    bool? loading,
    bool? pollingSnmp,
    bool? scanningSnmp,
    int? updatingInterfaceId,
    bool clearUpdatingInterface = false,
    String? error,
    bool clearError = false,
  }) {
    return DeviceDetailState(
      device: device ?? this.device,
      interfaces: interfaces ?? this.interfaces,
      monitors: monitors ?? this.monitors,
      metrics: metrics ?? this.metrics,
      events: events ?? this.events,
      capabilities: capabilities ?? this.capabilities,
      uptimeByMonitor: uptimeByMonitor ?? this.uptimeByMonitor,
      scanResult: scanResult ?? this.scanResult,
      loading: loading ?? this.loading,
      pollingSnmp: pollingSnmp ?? this.pollingSnmp,
      scanningSnmp: scanningSnmp ?? this.scanningSnmp,
      updatingInterfaceId: clearUpdatingInterface
          ? null
          : (updatingInterfaceId ?? this.updatingInterfaceId),
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class DeviceDetailNotifier extends Notifier<DeviceDetailState> {
  @override
  DeviceDetailState build() => const DeviceDetailState();

  Future<void> loadDeviceDetails(int deviceId) async {
    state = state.copyWith(
      loading: true,
      clearError: true,
      metrics: const <DeviceMetricModel>[],
    );
    final api = ref.read(apiClientProvider);

    try {
      final results = await Future.wait<Object?>(<Future<Object?>>[
        api.get<dynamic>('/devices/$deviceId').catchError((_) => null),
        api
            .get<dynamic>('/devices/$deviceId/interfaces')
            .catchError((_) => null),
        api.get<dynamic>('/devices/$deviceId/monitors').catchError((_) => null),
        api.get<dynamic>('/devices/$deviceId/metrics').catchError((_) => null),
        api.get<dynamic>('/devices/$deviceId/events').catchError((_) => null),
        api
            .get<dynamic>('/devices/$deviceId/capabilities')
            .catchError((_) => null),
      ]);

      final devRaw = results[0];
      final device = devRaw is Map
          ? DeviceModel.fromJson(asJsonMap(devRaw))
          : null;
      final interfaces = asJsonMapList(
        results[1],
      ).map(DeviceInterfaceModel.fromJson).toList(growable: false);
      final monitors = asJsonMapList(
        results[2],
      ).map(DeviceMonitorSummary.fromJson).toList(growable: false);
      final rawMetrics = results[3] is Map
          ? (results[3] as Map)['data']
          : results[3];
      final metrics = asJsonMapList(
        rawMetrics,
      ).map(DeviceMetricModel.fromJson).toList(growable: false);
      final rawEvents = results[4] is Map
          ? (results[4] as Map)['data']
          : results[4];
      final events = asJsonMapList(
        rawEvents,
      ).map(DeviceEventModel.fromJson).toList(growable: false);
      final capRaw = results[5];
      final capabilities = capRaw is Map
          ? DeviceCapabilitiesModel.fromJson(asJsonMap(capRaw))
          : null;

      state = state.copyWith(
        device: device,
        interfaces: interfaces,
        monitors: monitors,
        metrics: metrics,
        events: events,
        capabilities: capabilities,
        loading: false,
        error: device == null ? 'Dispositivo não encontrado.' : null,
      );

      // Carrega uptime 24h dos monitores do dispositivo em paralelo.
      if (monitors.isNotEmpty) {
        final uptimes = <int, MonitorUptimeModel>{};
        await Future.wait(
          monitors.take(6).map((m) async {
            try {
              final raw = await api.get<dynamic>(
                '/monitors/${m.id}/uptime',
                queryParameters: const <String, dynamic>{'hours': 24},
              );
              uptimes[m.id] = MonitorUptimeModel.fromJson(asJsonMap(raw));
            } catch (_) {}
          }),
        );
        if (state.device?.id == deviceId) {
          state = state.copyWith(uptimeByMonitor: uptimes);
        }
      }
    } catch (err) {
      state = state.copyWith(loading: false, error: err.toString());
    }
  }

  Future<bool> triggerSnmpPoll(int deviceId) async {
    state = state.copyWith(pollingSnmp: true, clearError: true);
    try {
      await ref
          .read(apiClientProvider)
          .post<dynamic>('/devices/$deviceId/snmp/poll', timeout: _snmpTimeout);
      await loadDeviceDetails(deviceId);
      return true;
    } catch (err) {
      state = state.copyWith(error: err.toString());
      return false;
    } finally {
      state = state.copyWith(pollingSnmp: false);
    }
  }

  Future<SnmpScanResultModel?> scanDeviceSnmp(int deviceId) async {
    state = state.copyWith(scanningSnmp: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>('/devices/$deviceId/snmp/scan', timeout: _snmpTimeout);
      final parsed = SnmpScanResultModel.fromJson(asJsonMap(raw));
      state = state.copyWith(scanResult: parsed);
      return parsed;
    } catch (err) {
      state = state.copyWith(error: err.toString());
      return null;
    } finally {
      state = state.copyWith(scanningSnmp: false);
    }
  }

  Future<bool> applySnmpMonitors(
    int deviceId, {
    bool? enableCpuMonitor,
    bool? enableMemoryMonitor,
    List<int>? monitoredIfIndexes,
    List<String>? monitoredSensors,
    bool? clearRemovedHistory,
  }) async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      await ref
          .read(apiClientProvider)
          .post<dynamic>(
            '/devices/$deviceId/snmp/apply-monitors',
            timeout: _snmpTimeout,
            data: <String, dynamic>{
              'enableCpuMonitor': ?enableCpuMonitor,
              'enableMemoryMonitor': ?enableMemoryMonitor,
              'monitoredIfIndexes': ?monitoredIfIndexes,
              'monitoredSensors': ?monitoredSensors,
              'clearRemovedHistory': ?clearRemovedHistory,
            },
          );
      await loadDeviceDetails(deviceId);
      return true;
    } catch (err) {
      state = state.copyWith(loading: false, error: err.toString());
      return false;
    }
  }

  Future<bool> setInterfaceMonitoring(
    int deviceId,
    int interfaceId,
    bool enabled,
  ) async {
    state = state.copyWith(updatingInterfaceId: interfaceId, clearError: true);
    try {
      await ref
          .read(apiClientProvider)
          .patch<dynamic>(
            '/devices/$deviceId/interfaces/$interfaceId/monitoring',
            timeout: _snmpTimeout,
            data: <String, dynamic>{'enabled': enabled},
          );
      await loadDeviceDetails(deviceId);
      return true;
    } catch (err) {
      state = state.copyWith(error: err.toString());
      return false;
    } finally {
      state = state.copyWith(clearUpdatingInterface: true);
    }
  }

  bool _isCurrentDevice(Map<String, dynamic> data) {
    final current = state.device;
    if (current == null) return false;
    final id = parseIntOrNull(data['deviceId'] ?? data['id']);
    return id != null && id == current.id;
  }

  void applyDeviceStatus(Map<String, dynamic> data) {
    if (!_isCurrentDevice(data) || state.device == null) return;
    final nextStatus = data['status'] as String?;
    if (nextStatus != null) {
      state = state.copyWith(
        device: state.device!.copyWith(status: nextStatus),
      );
    }
  }

  void applyMonitorResult(Map<String, dynamic> data) {
    if (!_isCurrentDevice(data)) return;
    final monitorId = parseIntOrNull(data['monitorId'] ?? data['id']);
    if (monitorId == null) return;
    final updated = state.monitors
        .map((m) {
          if (m.id != monitorId) return m;
          return m.copyWith(
            status: data['status'] as String?,
            latencyMs: parseDoubleOrNull(data['latencyMs']),
            lastCheckedAt:
                (data['finishedAt'] as String?) ??
                DateTime.now().toIso8601String(),
          );
        })
        .toList(growable: false);
    state = state.copyWith(monitors: updated);
  }

  void applyRecordedMetrics(Map<String, dynamic> data) {
    if (!_isCurrentDevice(data)) return;
    final samples = asJsonMapList(data['metrics']);
    if (samples.isEmpty) return;
    final deviceId = parseIntOrNull(data['deviceId']) ?? state.device!.id;
    final next = List<DeviceMetricModel>.from(state.metrics);
    for (final sample in samples) {
      next.add(
        DeviceMetricModel(
          id: DateTime.now().millisecondsSinceEpoch + next.length,
          deviceId: deviceId,
          interfaceId: parseIntOrNull(sample['interfaceId']),
          metricName: (sample['name'] ?? '').toString(),
          metricValue: parseDoubleOrNull(sample['value']) ?? 0,
          unit: sample['unit']?.toString(),
          createdAt: (sample['recordedAt'] ?? DateTime.now().toIso8601String())
              .toString(),
        ),
      );
    }
    final trimmed = next.length > _metricsLimit
        ? next.sublist(next.length - _metricsLimit)
        : next;
    state = state.copyWith(
      metrics: List<DeviceMetricModel>.unmodifiable(trimmed),
    );
  }

  void applyInterfaceTraffic(Map<String, dynamic> data) {
    if (!_isCurrentDevice(data)) return;
    final items = asJsonMapList(data['interfaces']);
    if (items.isEmpty) return;
    final deviceId = parseIntOrNull(data['deviceId']) ?? state.device!.id;
    final now = DateTime.now().toIso8601String();

    final byId = <int, Map<String, dynamic>>{
      for (final item in items)
        if (parseIntOrNull(item['id']) != null)
          parseIntOrNull(item['id'])!: item,
    };

    final nextInterfaces = state.interfaces
        .map((iface) {
          final patch = byId[iface.id];
          if (patch == null) return iface;
          return iface.copyWith(
            operStatus: patch['operStatus']?.toString(),
            speed: parseDoubleOrNull(patch['speed']),
            inBps: parseDoubleOrNull(patch['inBps']),
            outBps: parseDoubleOrNull(patch['outBps']),
          );
        })
        .toList(growable: false);

    final nextMetrics = List<DeviceMetricModel>.from(state.metrics);
    for (final item in items) {
      final ifId = parseIntOrNull(item['id']);
      final inB = parseDoubleOrNull(item['inBps']);
      final outB = parseDoubleOrNull(item['outBps']);
      if (inB != null) {
        nextMetrics.add(
          DeviceMetricModel(
            id: DateTime.now().millisecondsSinceEpoch + nextMetrics.length,
            deviceId: deviceId,
            interfaceId: ifId,
            interfaceName: item['name']?.toString(),
            metricName: 'inBps',
            metricValue: inB,
            unit: 'bps',
            createdAt: now,
          ),
        );
      }
      if (outB != null) {
        nextMetrics.add(
          DeviceMetricModel(
            id: DateTime.now().millisecondsSinceEpoch + nextMetrics.length,
            deviceId: deviceId,
            interfaceId: ifId,
            interfaceName: item['name']?.toString(),
            metricName: 'outBps',
            metricValue: outB,
            unit: 'bps',
            createdAt: now,
          ),
        );
      }
    }
    final trimmed = nextMetrics.length > _metricsLimit
        ? nextMetrics.sublist(nextMetrics.length - _metricsLimit)
        : nextMetrics;
    state = state.copyWith(
      interfaces: nextInterfaces,
      metrics: List<DeviceMetricModel>.unmodifiable(trimmed),
    );
  }

  void applyInterfaceChange(String type, Map<String, dynamic> data) {
    if (!_isCurrentDevice(data)) return;
    final interfaceId = parseIntOrNull(data['interfaceId']);
    final nextInterfaces = state.interfaces
        .map((iface) {
          if (iface.id != interfaceId) return iface;
          return iface.copyWith(
            operStatus: type == 'interface:status_change'
                ? data['currentStatus']?.toString()
                : null,
            speed: type != 'interface:status_change'
                ? parseDoubleOrNull(data['currentSpeed'])
                : null,
          );
        })
        .toList(growable: false);

    final nextEvent = DeviceEventModel(
      id:
          parseIntOrNull(data['alertEventId']) ??
          DateTime.now().millisecondsSinceEpoch,
      deviceId: state.device!.id,
      eventType: type,
      severity: type == 'interface:speed_change' ? 'info' : 'warning',
      message:
          (data['message'] as String?) ?? 'Alteração detectada na interface',
      createdAt: DateTime.now().toIso8601String(),
    );
    final nextEvents = <DeviceEventModel>[nextEvent, ...state.events];
    if (nextEvents.length > 100) nextEvents.length = 100;
    state = state.copyWith(interfaces: nextInterfaces, events: nextEvents);
  }
}

final deviceDetailProvider =
    NotifierProvider<DeviceDetailNotifier, DeviceDetailState>(
      DeviceDetailNotifier.new,
    );

/// Catálogo de sistemas operacionais de equipamento (`GET /api/devices/systems`) + `identify`.
class OperatingSystemsNotifier extends Notifier<List<OperatingSystemOption>> {
  bool _loaded = false;

  @override
  List<OperatingSystemOption> build() => const <OperatingSystemOption>[];

  Future<void> fetchAll({bool force = false}) async {
    if (_loaded && !force) return;
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/devices/systems');
      state = asJsonMapList(
        raw,
      ).map(OperatingSystemOption.fromJson).toList(growable: false);
      _loaded = true;
    } catch (_) {}
  }

  Future<IdentifyResultModel?> identify({
    String? name,
    required String? ipAddress,
    String? snmpVersion,
    String? snmpCommunity,
    String? vendor,
    String? model,
  }) async {
    final raw = await ref
        .read(apiClientProvider)
        .post<dynamic>(
          '/devices/identify',
          data: <String, dynamic>{
            'name': ?name,
            'ipAddress': ipAddress,
            'snmpVersion': snmpVersion,
            'snmpCommunity': snmpCommunity,
            'vendor': vendor,
            'model': model,
          },
        );
    return IdentifyResultModel.fromJson(asJsonMap(raw));
  }
}

final operatingSystemsProvider =
    NotifierProvider<OperatingSystemsNotifier, List<OperatingSystemOption>>(
      OperatingSystemsNotifier.new,
    );

/// Perfis SNMP reutilizáveis (`/api/snmp/profiles`) e testes SNMP pontuais (`/api/snmp/test`).
class SnmpProfilesNotifier extends Notifier<List<SnmpProfileModel>> {
  @override
  List<SnmpProfileModel> build() => const <SnmpProfileModel>[];

  Future<void> fetchProfiles() async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/snmp/profiles');
      state = asJsonMapList(
        raw,
      ).map(SnmpProfileModel.fromJson).toList(growable: false);
    } catch (_) {}
  }

  Future<bool> saveProfile(Map<String, dynamic> payload) async {
    try {
      await ref
          .read(apiClientProvider)
          .post<dynamic>('/snmp/profiles', data: payload);
      await fetchProfiles();
      return true;
    } catch (_) {
      return false;
    }
  }

  Future<bool> deleteProfile(String profileId) async {
    try {
      await ref
          .read(apiClientProvider)
          .delete<dynamic>('/snmp/profiles/${Uri.encodeComponent(profileId)}');
      await fetchProfiles();
      return true;
    } catch (_) {
      return false;
    }
  }

  Future<SnmpTestResult> testSnmp({
    required String host,
    int port = 161,
    String version = 'v2c',
    String community = 'public',
    bool autoDetect = false,
  }) async {
    final raw = await ref
        .read(apiClientProvider)
        .post<dynamic>(
          '/snmp/test',
          timeout: const Duration(seconds: 45),
          data: <String, dynamic>{
            'host': host,
            'port': port,
            'version': version,
            'community': community,
            'autoDetect': autoDetect,
          },
        );
    return SnmpTestResult.fromJson(asJsonMap(raw));
  }

  Future<List<DeviceInterfaceModel>> queryInterfaces({
    required String host,
    int port = 161,
    String version = 'v2c',
    String community = 'public',
  }) async {
    final raw = await ref
        .read(apiClientProvider)
        .post<dynamic>(
          '/snmp/interfaces-query',
          timeout: const Duration(seconds: 60),
          data: <String, dynamic>{
            'host': host,
            'port': port,
            'version': version,
            'community': community,
          },
        );
    return asJsonMapList(
      raw,
    ).map(DeviceInterfaceModel.fromJson).toList(growable: false);
  }
}

final snmpProfilesProvider =
    NotifierProvider<SnmpProfilesNotifier, List<SnmpProfileModel>>(
      SnmpProfilesNotifier.new,
    );

/// Base IEEE MAC OUI (`/api/vendors/*`).
@immutable
class VendorsState {
  const VendorsState({
    this.status,
    this.loading = false,
    this.refreshing = false,
    this.error,
  });

  final VendorRegistryStatus? status;
  final bool loading;
  final bool refreshing;
  final String? error;

  VendorsState copyWith({
    VendorRegistryStatus? status,
    bool? loading,
    bool? refreshing,
    String? error,
    bool clearError = false,
  }) {
    return VendorsState(
      status: status ?? this.status,
      loading: loading ?? this.loading,
      refreshing: refreshing ?? this.refreshing,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class VendorsNotifier extends Notifier<VendorsState> {
  final Map<String, MacVendorLookup> _cache = <String, MacVendorLookup>{};

  @override
  VendorsState build() => const VendorsState();

  Future<void> fetchStatus() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/vendors/status');
      state = state.copyWith(
        status: VendorRegistryStatus.fromJson(asJsonMap(raw)),
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(loading: false, error: err.toString());
    }
  }

  Future<bool> refreshRegistry() async {
    state = state.copyWith(refreshing: true, clearError: true);
    try {
      await ref
          .read(apiClientProvider)
          .post<dynamic>(
            '/vendors/refresh',
            timeout: const Duration(minutes: 5),
          );
      _cache.clear();
      await fetchStatus();
      state = state.copyWith(refreshing: false);
      return true;
    } catch (err) {
      state = state.copyWith(refreshing: false, error: err.toString());
      return false;
    }
  }

  Future<MacVendorLookup?> lookup(String mac) async {
    final digits = mac.replaceAll(RegExp(r'[^0-9a-fA-F]'), '').toLowerCase();
    if (digits.length < 6) return null;
    final key = digits.length > 12 ? digits.substring(0, 12) : digits;
    if (_cache.containsKey(key)) return _cache[key];
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>(
            '/vendors/lookup',
            queryParameters: <String, dynamic>{'mac': key},
          );
      final parsed = MacVendorLookup.fromJson(asJsonMap(raw));
      _cache[key] = parsed;
      return parsed;
    } catch (_) {
      return null;
    }
  }
}

final vendorsProvider = NotifierProvider<VendorsNotifier, VendorsState>(
  VendorsNotifier.new,
);
