import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/core/utils/mdi_icons.dart';
import 'package:netmonitor_app/features/plugins/data/plugins_repository.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';

const int _batchesLimit = 15;

class _LaunchedBatch {
  _LaunchedBatch({
    required this.pluginId,
    required this.action,
    required this.params,
    this.deviceParams,
    required this.devices,
  });

  final int pluginId;
  final String action;
  final Map<String, dynamic> params;
  final Map<int, Map<String, dynamic>>? deviceParams;
  final Set<int> devices;
}

@immutable
class PluginAppsState {
  const PluginAppsState({
    this.apps = const <PluginApp>[],
    this.fleets = const <int, FleetView>{},
    this.lastSteps = const <int, TranscriptEntry>{},
    this.loading = false,
    this.identifying = false,
    this.error,
  });

  final List<PluginApp> apps;
  final Map<int, FleetView> fleets;
  final Map<int, TranscriptEntry> lastSteps;
  final bool loading;
  final bool identifying;
  final String? error;

  PluginBatchView? latestBatch(int pluginId, String action) {
    final fleet = fleets[pluginId];
    if (fleet == null) return null;
    for (final batch in fleet.batches) {
      if (batch.action == action && batch.finishedAt != null) return batch;
    }
    return null;
  }

  PluginBatchView? batchById(int pluginId, int? batchId) {
    if (batchId == null) return null;
    final fleet = fleets[pluginId];
    if (fleet == null) return null;
    for (final batch in fleet.batches) {
      if (batch.id == batchId) return batch;
    }
    return null;
  }

  PluginAppsState copyWith({
    List<PluginApp>? apps,
    Map<int, FleetView>? fleets,
    Map<int, TranscriptEntry>? lastSteps,
    bool? loading,
    bool? identifying,
    String? error,
    bool clearError = false,
  }) {
    return PluginAppsState(
      apps: apps ?? this.apps,
      fleets: fleets ?? this.fleets,
      lastSteps: lastSteps ?? this.lastSteps,
      loading: loading ?? this.loading,
      identifying: identifying ?? this.identifying,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

/// Store de Aplicativos de Frota (`stores/pluginApps.ts`) — sincroniza automaticamente
/// `pluginAppsNavProvider` para que o grupo "Aplicativos" apareça no menu lateral.
class PluginAppsNotifier extends Notifier<PluginAppsState> {
  /// Cache estritamente efêmero em memória dos parâmetros lançados nesta aba.
  final Map<int, _LaunchedBatch> _launched = <int, _LaunchedBatch>{};

  @override
  PluginAppsState build() => const PluginAppsState();

  PluginsRepository get _repo => ref.read(pluginsRepositoryProvider);

  Future<void> fetchApps() async {
    try {
      final list = await _repo.fetchApps();
      state = state.copyWith(apps: list);
      ref.read(pluginAppsNavProvider.notifier).state = list
          .map(
            (app) => NavApp(
              id: app.id,
              title: app.title,
              icon: iconFromMdiName(app.icon, fallback: Icons.apps),
            ),
          )
          .toList(growable: false);
    } catch (_) {
      state = state.copyWith(apps: const <PluginApp>[]);
      ref.read(pluginAppsNavProvider.notifier).state = const <NavApp>[];
    }
  }

  Future<void> loadFleet(int pluginId) async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final fleet = await _repo.fetchFleet(pluginId);
      state = state.copyWith(
        fleets: <int, FleetView>{...state.fleets, pluginId: fleet},
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(
        loading: false,
        error: describeError(err, fallback: 'Falha ao carregar o aplicativo.'),
      );
    }
  }

  Future<Map<String, dynamic>> saveFleetSettings(
    int pluginId,
    Map<String, dynamic> value,
  ) async {
    final saved = await _repo.saveFleetSettings(pluginId, value);
    final fleet = state.fleets[pluginId];
    if (fleet != null) {
      state = state.copyWith(
        fleets: <int, FleetView>{
          ...state.fleets,
          pluginId: fleet.copyWith(settings: saved),
        },
      );
    }
    return saved;
  }

  Future<Map<String, dynamic>> saveMemberSettings(
    int pluginId,
    int deviceId,
    Map<String, dynamic> value,
  ) async {
    final saved = await _repo.saveDeviceSettings(deviceId, pluginId, value);
    final fleet = state.fleets[pluginId];
    if (fleet != null) {
      final updated = fleet.members
          .map((m) => m.deviceId == deviceId ? m.copyWith(settings: saved) : m)
          .toList(growable: false);
      state = state.copyWith(
        fleets: <int, FleetView>{
          ...state.fleets,
          pluginId: fleet.copyWith(members: updated),
        },
      );
    }
    return saved;
  }

  Future<void> addMember(int pluginId, int deviceId) async {
    await ref.read(pluginsProvider.notifier).installPlugin(deviceId, pluginId);
    await loadFleet(pluginId);
  }

  Future<void> removeMember(int pluginId, int deviceId) async {
    await _repo.uninstallFromDevice(deviceId, pluginId);
    await loadFleet(pluginId);
    await fetchApps();
  }

  Future<int> runAction({
    required int pluginId,
    required String action,
    required List<int> deviceIds,
    required bool confirmWrite,
    Map<String, dynamic> params = const <String, dynamic>{},
    Map<int, Map<String, dynamic>>? deviceParams,
  }) async {
    final batchId = await _repo.runFleetAction(
      pluginId: pluginId,
      action: action,
      deviceIds: deviceIds,
      confirmWrite: confirmWrite,
      params: params,
      deviceParams: deviceParams,
    );
    _launched[batchId] = _LaunchedBatch(
      pluginId: pluginId,
      action: action,
      params: params,
      deviceParams: deviceParams,
      devices: deviceIds.toSet(),
    );
    return batchId;
  }

  bool canRerun(int batchId, int deviceId) =>
      _launched[batchId]?.devices.contains(deviceId) ?? false;

  Future<int?> rerunDevice(
    int batchId,
    int deviceId, {
    required bool confirmWrite,
  }) async {
    final kept = _launched[batchId];
    if (kept == null || !kept.devices.contains(deviceId)) return null;
    final own = kept.deviceParams?[deviceId];
    final nextId = await runAction(
      pluginId: kept.pluginId,
      action: kept.action,
      deviceIds: <int>[deviceId],
      confirmWrite: confirmWrite,
      params: kept.params,
      deviceParams: own != null
          ? <int, Map<String, dynamic>>{deviceId: own}
          : null,
    );
    kept.devices.remove(deviceId);
    if (kept.devices.isEmpty) _launched.remove(batchId);
    return nextId;
  }

  void forgetLaunched(int pluginId) {
    _launched.removeWhere((_, value) => value.pluginId == pluginId);
  }

  Future<FleetView> identify(
    int pluginId, {
    List<int> deviceIds = const <int>[],
  }) async {
    state = state.copyWith(identifying: true, clearError: true);
    try {
      final view = await _repo.identifyFleet(pluginId, deviceIds);
      state = state.copyWith(
        fleets: <int, FleetView>{...state.fleets, pluginId: view},
        identifying: false,
      );
      return view;
    } catch (err) {
      state = state.copyWith(
        identifying: false,
        error: describeError(
          err,
          fallback: 'Falha ao identificar equipamentos.',
        ),
      );
      rethrow;
    }
  }

  Future<void> cancelBatch(int batchId) => _repo.cancelBatch(batchId);

  Future<void> applyPatch(PluginBatchView batch) async {
    final updated = await _repo.applyBatchPatch(batch.id);
    _upsertBatch(updated);
    await loadFleet(batch.pluginId);
  }

  // --- Handlers SSE (`plugin:batch_updated`, `plugin:run_output`, `plugin:run_step`, `plugin:apps_changed`) ---

  void _upsertBatch(PluginBatchView batch) {
    final fleet = state.fleets[batch.pluginId];
    if (fleet == null) return;
    final batches = List<PluginBatchView>.from(fleet.batches);
    final idx = batches.indexWhere((b) => b.id == batch.id);
    if (idx >= 0) {
      batches[idx] = batch;
    } else {
      batches.insert(0, batch);
    }
    final capped = batches.length > _batchesLimit
        ? batches.sublist(0, _batchesLimit)
        : batches;
    if (batches.length > _batchesLimit) {
      for (final dropped in batches.sublist(_batchesLimit)) {
        _launched.remove(dropped.id);
      }
    }
    state = state.copyWith(
      fleets: <int, FleetView>{
        ...state.fleets,
        batch.pluginId: fleet.copyWith(
          batches: List<PluginBatchView>.unmodifiable(capped),
        ),
      },
    );
  }

  void applyBatchUpdated(Map<String, dynamic> data) {
    final raw = data['batch'];
    if (raw is! Map) return;
    _upsertBatch(PluginBatchView.fromJson(asJsonMap(raw)));
  }

  void applyRunStep(Map<String, dynamic> data) {
    final runId = parseIntOrNull(data['runId']);
    final rawEntry = data['entry'];
    if (runId == null || rawEntry is! Map) return;
    final entry = TranscriptEntry.fromJson(asJsonMap(rawEntry));
    state = state.copyWith(
      lastSteps: <int, TranscriptEntry>{...state.lastSteps, runId: entry},
    );
  }
}

final pluginAppsProvider =
    NotifierProvider<PluginAppsNotifier, PluginAppsState>(
      PluginAppsNotifier.new,
    );
