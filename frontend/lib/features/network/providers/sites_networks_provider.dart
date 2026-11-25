import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';

// ── Sites ─────────────────────────────────────────────────────────────────────

@immutable
class SitesState {
  const SitesState({
    this.items = const <SiteModel>[],
    this.loading = false,
    this.error,
  });

  final List<SiteModel> items;
  final bool loading;
  final String? error;

  SitesState copyWith({
    List<SiteModel>? items,
    bool? loading,
    String? error,
    bool clearError = false,
  }) {
    return SitesState(
      items: items ?? this.items,
      loading: loading ?? this.loading,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class SitesNotifier extends Notifier<SitesState> {
  @override
  SitesState build() => const SitesState();

  Future<void> fetchSites() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await ref.read(apiClientProvider).get<dynamic>('/sites');
      state = state.copyWith(
        items: asJsonMapList(
          raw,
        ).map(SiteModel.fromJson).toList(growable: false),
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(loading: false, error: describeError(err));
    }
  }

  Future<bool> createSite(Map<String, dynamic> payload) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>('/sites', data: payload);
      final created = SiteModel.fromJson(asJsonMap(raw));
      state = state.copyWith(items: <SiteModel>[...state.items, created]);
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  Future<bool> updateSite(int id, Map<String, dynamic> payload) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .put<dynamic>('/sites/$id', data: payload);
      final updated = SiteModel.fromJson(asJsonMap(raw));
      state = state.copyWith(
        items: state.items
            .map((s) => s.id == id ? updated : s)
            .toList(growable: false),
      );
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  Future<bool> deleteSite(int id) async {
    try {
      await ref.read(apiClientProvider).delete<dynamic>('/sites/$id');
      state = state.copyWith(
        items: state.items.where((s) => s.id != id).toList(growable: false),
      );
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }
}

final sitesProvider = NotifierProvider<SitesNotifier, SitesState>(
  SitesNotifier.new,
);

// ── Networks ──────────────────────────────────────────────────────────────────

@immutable
class NetworksState {
  const NetworksState({
    this.items = const <NetworkModel>[],
    this.loading = false,
    this.scanningId,
    this.error,
  });

  final List<NetworkModel> items;
  final bool loading;
  final int? scanningId;
  final String? error;

  NetworksState copyWith({
    List<NetworkModel>? items,
    bool? loading,
    int? scanningId,
    bool clearScanningId = false,
    String? error,
    bool clearError = false,
  }) {
    return NetworksState(
      items: items ?? this.items,
      loading: loading ?? this.loading,
      scanningId: clearScanningId ? null : (scanningId ?? this.scanningId),
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class NetworksNotifier extends Notifier<NetworksState> {
  @override
  NetworksState build() => const NetworksState();

  Future<void> fetchNetworks() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await ref.read(apiClientProvider).get<dynamic>('/networks');
      state = state.copyWith(
        items: asJsonMapList(
          raw,
        ).map(NetworkModel.fromJson).toList(growable: false),
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(loading: false, error: describeError(err));
    }
  }

  Future<bool> createNetwork(Map<String, dynamic> payload) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>('/networks', data: payload);
      final created = NetworkModel.fromJson(asJsonMap(raw));
      state = state.copyWith(items: <NetworkModel>[...state.items, created]);
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  Future<bool> updateNetwork(int id, Map<String, dynamic> payload) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .put<dynamic>('/networks/$id', data: payload);
      final updated = NetworkModel.fromJson(asJsonMap(raw));
      state = state.copyWith(
        items: state.items
            .map((n) => n.id == id ? updated : n)
            .toList(growable: false),
      );
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  Future<bool> deleteNetwork(int id) async {
    try {
      await ref.read(apiClientProvider).delete<dynamic>('/networks/$id');
      state = state.copyWith(
        items: state.items.where((n) => n.id != id).toList(growable: false),
      );
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  Future<NetworkScanResult?> scanNetwork(int id) async {
    state = state.copyWith(scanningId: id, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>('/networks/$id/scan');
      return NetworkScanResult.fromJson(asJsonMap(raw));
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return null;
    } finally {
      state = state.copyWith(clearScanningId: true);
    }
  }
}

final networksProvider = NotifierProvider<NetworksNotifier, NetworksState>(
  NetworksNotifier.new,
);

// ── Probes ────────────────────────────────────────────────────────────────────

@immutable
class ProbesState {
  const ProbesState({
    this.items = const <ProbeModel>[],
    this.loading = false,
    this.testingId,
    this.error,
  });

  final List<ProbeModel> items;
  final bool loading;
  final int? testingId;
  final String? error;

  int get onlineCount => items.where((p) => p.status == 'online').length;
  int get offlineCount => items.where((p) => p.status == 'offline').length;
  int get revokedCount => items.where((p) => p.status == 'revoked').length;

  ProbesState copyWith({
    List<ProbeModel>? items,
    bool? loading,
    int? testingId,
    bool clearTestingId = false,
    String? error,
    bool clearError = false,
  }) {
    return ProbesState(
      items: items ?? this.items,
      loading: loading ?? this.loading,
      testingId: clearTestingId ? null : (testingId ?? this.testingId),
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class ProbesNotifier extends Notifier<ProbesState> {
  @override
  ProbesState build() => const ProbesState();

  Future<void> fetchProbes() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await ref.read(apiClientProvider).get<dynamic>('/probes');
      state = state.copyWith(
        items: asJsonMapList(
          raw,
        ).map(ProbeModel.fromJson).toList(growable: false),
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(loading: false, error: describeError(err));
    }
  }

  Future<bool> testProbe(int probeId) async {
    state = state.copyWith(testingId: probeId, clearError: true);
    try {
      await ref.read(apiClientProvider).post<dynamic>('/probes/$probeId/test');
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    } finally {
      state = state.copyWith(clearTestingId: true);
    }
  }

  Future<bool> revokeProbe(int probeId) async {
    try {
      await ref
          .read(apiClientProvider)
          .post<dynamic>('/probes/$probeId/revoke');
      state = state.copyWith(
        items: state.items
            .map((p) => p.id == probeId ? p.copyWith(status: 'revoked') : p)
            .toList(growable: false),
      );
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  /// Aplica SSE `probe:status`.
  void applyRealtimeStatus(Map<String, dynamic> data) {
    final id = parseIntOrNull(data['id'] ?? data['probeId']);
    if (id == null) return;
    final index = state.items.indexWhere((p) => p.id == id);
    if (index < 0) {
      fetchProbes();
      return;
    }
    final updated = state.items[index].copyWith(
      status: data['status'] as String?,
      name: data['name'] as String?,
      lastHeartbeatAt:
          (data['lastSeenAt'] ?? data['lastHeartbeatAt']) as String?,
    );
    final next = List<ProbeModel>.from(state.items)..[index] = updated;
    state = state.copyWith(items: List<ProbeModel>.unmodifiable(next));
  }
}

final probesProvider = NotifierProvider<ProbesNotifier, ProbesState>(
  ProbesNotifier.new,
);

// ── Discovery ─────────────────────────────────────────────────────────────────

@immutable
class DiscoveryState {
  const DiscoveryState({
    this.runs = const <DiscoveryRunModel>[],
    this.scan = const ScanSessionState(),
    this.conflicts = const <NetworkConflictModel>[],
    this.environment,
    this.loading = false,
    this.starting = false,
    this.loadingConflicts = false,
    this.error,
  });

  final List<DiscoveryRunModel> runs;
  final ScanSessionState scan;
  final List<NetworkConflictModel> conflicts;
  final DiscoveryEnvironmentModel? environment;
  final bool loading;
  final bool starting;
  final bool loadingConflicts;
  final String? error;

  bool get isScanning => starting || scan.isActive;

  DiscoveryState copyWith({
    List<DiscoveryRunModel>? runs,
    ScanSessionState? scan,
    List<NetworkConflictModel>? conflicts,
    DiscoveryEnvironmentModel? environment,
    bool? loading,
    bool? starting,
    bool? loadingConflicts,
    String? error,
    bool clearError = false,
  }) {
    return DiscoveryState(
      runs: runs ?? this.runs,
      scan: scan ?? this.scan,
      conflicts: conflicts ?? this.conflicts,
      environment: environment ?? this.environment,
      loading: loading ?? this.loading,
      starting: starting ?? this.starting,
      loadingConflicts: loadingConflicts ?? this.loadingConflicts,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class DiscoveryNotifier extends Notifier<DiscoveryState> {
  @override
  DiscoveryState build() => const DiscoveryState();

  Future<void> fetchDiscoveryRuns() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/discovery/runs');
      state = state.copyWith(
        runs: asJsonMapList(
          raw,
        ).map(DiscoveryRunModel.fromJson).toList(growable: false),
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(loading: false, error: describeError(err));
    }
  }

  Future<void> fetchEnvironment() async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/discovery/environment');
      state = state.copyWith(
        environment: DiscoveryEnvironmentModel.fromJson(asJsonMap(raw)),
      );
    } catch (_) {}
  }

  Future<void> fetchConflicts() async {
    state = state.copyWith(loadingConflicts: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/discovery/conflicts');
      state = state.copyWith(
        conflicts: asJsonMapList(
          raw,
        ).map(NetworkConflictModel.fromJson).toList(growable: false),
        loadingConflicts: false,
      );
    } catch (_) {
      state = state.copyWith(loadingConflicts: false);
    }
  }

  Future<int> checkConflicts() async {
    state = state.copyWith(loadingConflicts: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>('/discovery/check-conflicts');
      final map = asJsonMap(raw);
      final parsed = asJsonMapList(
        map['conflicts'],
      ).map(NetworkConflictModel.fromJson).toList(growable: false);
      state = state.copyWith(conflicts: parsed, loadingConflicts: false);
      return parsed.length;
    } catch (err) {
      state = state.copyWith(
        loadingConflicts: false,
        error: describeError(err),
      );
      return 0;
    }
  }

  Future<int?> startScan(int networkId) async {
    state = state.copyWith(starting: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>(
            '/discovery/scan',
            data: <String, dynamic>{'networkId': networkId},
          );
      return parseIntOrNull(asJsonMap(raw)['runId']);
    } catch (err) {
      state = state.copyWith(starting: false, error: describeError(err));
      return null;
    }
  }

  Future<int> adoptHostsBatch({
    required List<StreamedDiscoveryHost> hosts,
    required Set<String> selectedIps,
    int? networkId,
    int? siteId,
  }) async {
    if (selectedIps.isEmpty) return 0;
    var created = 0;
    final devicesNotifier = ref.read(devicesProvider.notifier);
    for (final host in hosts) {
      if (!selectedIps.contains(host.ipAddress)) continue;
      final res = await devicesNotifier.createDevice(<String, dynamic>{
        'name': host.bestName,
        'ipAddress': host.ipAddress,
        if (host.macAddress != null) 'macAddress': host.macAddress,
        if (host.vendor != null) 'vendor': host.vendor,
        'type': host.deviceType ?? 'other',
        'networkId': networkId,
        'siteId': siteId,
        'isMonitored': true,
        // A credencial que respondeu na descoberta já fica no cadastro.
        if (host.snmpDetected) ...<String, dynamic>{
          'snmpEnabled': true,
          'snmpProfileId': ?host.snmpProfileId,
        },
      });
      if (res != null) created++;
    }
    return created;
  }

  Future<bool> cancelScan() async {
    try {
      await ref.read(apiClientProvider).post<dynamic>('/discovery/scan-cancel');
      state = state.copyWith(starting: false);
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  Future<int?> cleanup({int olderThanDays = 7}) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .delete<dynamic>(
            '/discovery/cleanup',
            queryParameters: <String, dynamic>{'olderThanDays': olderThanDays},
          );
      await fetchDiscoveryRuns();
      return parseIntOrNull(asJsonMap(raw)['removedRuns']);
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return null;
    }
  }

  /// Aplica snapshot SSE `discovery:scan`.
  void applyScanSnapshot(Map<String, dynamic> data) {
    final parsed = ScanSessionState.tryFromJson(data);
    if (parsed == null) return;
    state = state.copyWith(
      scan: parsed,
      starting: parsed.status == 'idle' ? state.starting : false,
    );
  }

  /// Aplica SSE `discovery:conflicts_detected`.
  void applyConflictsDetected(Map<String, dynamic> data) {
    if (data['conflicts'] is List) {
      state = state.copyWith(
        conflicts: asJsonMapList(
          data['conflicts'],
        ).map(NetworkConflictModel.fromJson).toList(growable: false),
      );
    } else {
      fetchConflicts();
    }
  }
}

final discoveryProvider = NotifierProvider<DiscoveryNotifier, DiscoveryState>(
  DiscoveryNotifier.new,
);
