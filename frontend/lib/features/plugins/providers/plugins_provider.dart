import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/plugins/data/plugins_repository.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugin_apps_provider.dart';
import 'package:netmonitor_app/shared/device_access/device_access.dart'
    hide CredentialView;

const int _runsLimit = 20;
const int _progressLimit = 120;
const int _progressRunsLimit = 20;

@immutable
class PluginsState {
  const PluginsState({
    this.library = const <PluginSummary>[],
    this.libraryLoading = false,
    this.deviceViews = const <int, DevicePluginsView>{},
    this.deviceLoading = false,
    this.pendingApprovals = const <PluginApprovalRequest>[],
    this.looseRuns = const <int, PluginRunView>{},
    this.runProgress = const <int, List<ProgressSample>>{},
    this.error,
  });

  final List<PluginSummary> library;
  final bool libraryLoading;
  final Map<int, DevicePluginsView> deviceViews;
  final bool deviceLoading;
  final List<PluginApprovalRequest> pendingApprovals;
  final Map<int, PluginRunView> looseRuns;
  final Map<int, List<ProgressSample>> runProgress;
  final String? error;

  PluginApprovalRequest? get nextApproval =>
      pendingApprovals.isNotEmpty ? pendingApprovals.first : null;

  List<PluginRunView> runningRuns(int deviceId) {
    return (deviceViews[deviceId]?.runs ?? const <PluginRunView>[])
        .where((run) => run.status == 'running')
        .toList(growable: false);
  }

  PluginRunView? runById(int deviceId, int? runId) {
    if (runId == null) return null;
    for (final run in deviceViews[deviceId]?.runs ?? const <PluginRunView>[]) {
      if (run.id == runId) return run;
    }
    return looseRuns[runId];
  }

  PluginsState copyWith({
    List<PluginSummary>? library,
    bool? libraryLoading,
    Map<int, DevicePluginsView>? deviceViews,
    bool? deviceLoading,
    List<PluginApprovalRequest>? pendingApprovals,
    Map<int, PluginRunView>? looseRuns,
    Map<int, List<ProgressSample>>? runProgress,
    String? error,
    bool clearError = false,
  }) {
    return PluginsState(
      library: library ?? this.library,
      libraryLoading: libraryLoading ?? this.libraryLoading,
      deviceViews: deviceViews ?? this.deviceViews,
      deviceLoading: deviceLoading ?? this.deviceLoading,
      pendingApprovals: pendingApprovals ?? this.pendingApprovals,
      looseRuns: looseRuns ?? this.looseRuns,
      runProgress: runProgress ?? this.runProgress,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

/// Store global de plugins, execuções e aprovações (SSE-driven, zero polling).
class PluginsNotifier extends Notifier<PluginsState> {
  final Map<int, List<Completer<PluginRunView>>> _finishWaiters =
      <int, List<Completer<PluginRunView>>>{};
  final Map<int, PluginRunView> _recentFinished = <int, PluginRunView>{};

  @override
  PluginsState build() => const PluginsState();

  PluginsRepository get _repo => ref.read(pluginsRepositoryProvider);

  // --- Biblioteca ---

  Future<void> fetchLibrary() async {
    state = state.copyWith(libraryLoading: true, clearError: true);
    try {
      final list = await _repo.fetchLibrary();
      state = state.copyWith(library: list, libraryLoading: false);
    } catch (err) {
      state = state.copyWith(
        libraryLoading: false,
        error: describeError(err, fallback: 'Falha ao carregar os plugins.'),
      );
    }
  }

  Future<PluginDetail> fetchDetail(int id) => _repo.fetchDetail(id);

  Future<DownloadResult> exportPlugin(int id) => _repo.exportPlugin(id);

  Future<PluginPreviewResult> previewPackage(PluginPackage pkg) =>
      _repo.previewPackage(pkg);

  void _upsertSummary(PluginSummary summary) {
    final next = List<PluginSummary>.from(state.library);
    final idx = next.indexWhere((item) => item.id == summary.id);
    if (idx >= 0) {
      next[idx] = summary;
    } else {
      next.add(summary);
    }
    state = state.copyWith(library: List<PluginSummary>.unmodifiable(next));
  }

  Future<PluginDetail> _afterChange(
    PluginDetail detail, {
    int? refreshDeviceId,
  }) async {
    _upsertSummary(detail.summary);
    if (refreshDeviceId != null) {
      await loadDevice(refreshDeviceId);
    }
    return detail;
  }

  Future<PluginDetail> createPlugin(
    PluginPackage pkg, {
    int? deviceId,
    int? refreshDeviceId,
  }) async {
    final detail = await _repo.createPlugin(pkg, deviceId: deviceId);
    return _afterChange(detail, refreshDeviceId: refreshDeviceId);
  }

  Future<PluginDetail> updatePlugin(
    int id,
    PluginPackage pkg, {
    int? refreshDeviceId,
  }) async {
    final detail = await _repo.updatePlugin(id, pkg);
    return _afterChange(detail, refreshDeviceId: refreshDeviceId);
  }

  Future<PluginDetail> importPlugin(
    PluginPackage pkg, {
    int? deviceId,
    int? refreshDeviceId,
  }) async {
    final detail = await _repo.importPlugin(pkg, deviceId: deviceId);
    return _afterChange(detail, refreshDeviceId: refreshDeviceId);
  }

  Future<void> deletePlugin(int id, {int? refreshDeviceId}) async {
    await _repo.deletePlugin(id);
    state = state.copyWith(
      library: state.library.where((p) => p.id != id).toList(growable: false),
    );
    if (refreshDeviceId != null) await loadDevice(refreshDeviceId);
  }

  Future<PluginDetail> lifecycle(
    int id,
    String step, {
    int? refreshDeviceId,
  }) async {
    final detail = await _repo.lifecycle(id, step);
    await ref.read(pluginAppsProvider.notifier).fetchApps();
    return _afterChange(detail, refreshDeviceId: refreshDeviceId);
  }

  Future<PluginDetail> acceptReview(
    int id, {
    required bool acknowledgeRisk,
    int? refreshDeviceId,
  }) async {
    final detail = await _repo.acceptReview(
      id,
      acknowledgeRisk: acknowledgeRisk,
    );
    return _afterChange(detail, refreshDeviceId: refreshDeviceId);
  }

  Future<PluginTestReport> runTests(int id, {int? refreshDeviceId}) async {
    final report = await _repo.runTests(id);
    final detail = await _repo.fetchDetail(id);
    await _afterChange(detail, refreshDeviceId: refreshDeviceId);
    return report;
  }

  /// Ação `reveals: true` — efêmera: NÃO grava em state, histórico ou logs (ADR 012).
  Future<Map<String, dynamic>> revealSecret({
    required int deviceId,
    required int pluginId,
    required String action,
    Map<String, dynamic> params = const <String, dynamic>{},
  }) {
    return _repo.revealSecret(
      deviceId: deviceId,
      pluginId: pluginId,
      action: action,
      params: params,
    );
  }

  // --- Dispositivo ---

  Future<void> loadDevice(int deviceId) async {
    state = state.copyWith(deviceLoading: true, clearError: true);
    try {
      final view = await _repo.fetchDevicePlugins(deviceId);
      state = state.copyWith(
        deviceViews: <int, DevicePluginsView>{
          ...state.deviceViews,
          deviceId: view,
        },
        deviceLoading: false,
      );
    } catch (err) {
      state = state.copyWith(
        deviceLoading: false,
        error: describeError(
          err,
          fallback: 'Falha ao carregar os plugins do dispositivo.',
        ),
      );
    }
  }

  Future<void> installPlugin(int deviceId, int pluginId) async {
    final view = await _repo.installOnDevice(deviceId, pluginId);
    state = state.copyWith(
      deviceViews: <int, DevicePluginsView>{
        ...state.deviceViews,
        deviceId: view,
      },
    );
    await ref.read(pluginAppsProvider.notifier).fetchApps();
  }

  Future<void> uninstallPlugin(int deviceId, int pluginId) async {
    final view = await _repo.uninstallFromDevice(deviceId, pluginId);
    state = state.copyWith(
      deviceViews: <int, DevicePluginsView>{
        ...state.deviceViews,
        deviceId: view,
      },
    );
  }

  Future<Map<String, dynamic>> saveDeviceSettings(
    int deviceId,
    int pluginId,
    Map<String, dynamic> value,
  ) async {
    final saved = await _repo.saveDeviceSettings(deviceId, pluginId, value);
    final view = state.deviceViews[deviceId];
    if (view != null) {
      final updated = view.plugins
          .map(
            (p) =>
                p.plugin.id == pluginId ? p.copyWith(deviceSettings: saved) : p,
          )
          .toList(growable: false);
      state = state.copyWith(
        deviceViews: <int, DevicePluginsView>{
          ...state.deviceViews,
          deviceId: view.copyWith(plugins: updated),
        },
      );
    }
    return saved;
  }

  /// Delega remoção de credencial ao `deviceAccessProvider.notifier` unificado e sincroniza `deviceViews`.
  Future<void> deleteCredential(int deviceId, String kind) async {
    await ref
        .read(deviceAccessProvider.notifier)
        .deleteCredential(deviceId: deviceId, kind: kind);
    final view = state.deviceViews[deviceId];
    if (view == null) return;
    state = state.copyWith(
      deviceViews: <int, DevicePluginsView>{
        ...state.deviceViews,
        deviceId: view.copyWith(
          credentials: view.credentials
              .where((c) => c.kind != kind)
              .toList(growable: false),
        ),
      },
    );
  }

  Future<int> runAction({
    required int deviceId,
    required int pluginId,
    required String action,
    Map<String, dynamic> params = const <String, dynamic>{},
    bool confirmWrite = false,
  }) {
    return _repo.runAction(
      deviceId: deviceId,
      pluginId: pluginId,
      action: action,
      params: params,
      confirmWrite: confirmWrite,
    );
  }

  Future<int> validatePlugin(int deviceId, int pluginId) =>
      _repo.validateOnDevice(deviceId, pluginId);

  Future<void> cancelRun(int runId) => _repo.cancelRun(runId);

  Future<PluginRunView> fetchRun(int runId) async {
    final run = await _repo.fetchRun(runId);
    state = state.copyWith(
      looseRuns: <int, PluginRunView>{...state.looseRuns, run.id: run},
    );
    return run;
  }

  /// Aguarda conclusão de uma execução via SSE (`plugin:run_finished`), sem polling.
  Future<PluginRunView> waitForRun(int runId) async {
    final known = _recentFinished[runId];
    if (known != null) return known;
    final completer = Completer<PluginRunView>();
    _finishWaiters
        .putIfAbsent(runId, () => <Completer<PluginRunView>>[])
        .add(completer);
    final current = await fetchRun(runId);
    if (current.isFinished) _settleRun(current);
    return completer.future;
  }

  void _settleRun(PluginRunView run) {
    final waiters =
        _finishWaiters.remove(run.id) ?? const <Completer<PluginRunView>>[];
    for (final c in waiters) {
      if (!c.isCompleted) c.complete(run);
    }
    _recentFinished[run.id] = run;
    while (_recentFinished.length > _runsLimit) {
      _recentFinished.remove(_recentFinished.keys.first);
    }
  }

  Future<void> respondApproval(String key, {required bool approved}) async {
    try {
      await _repo.respondApproval(key, approved: approved);
    } finally {
      state = state.copyWith(
        pendingApprovals: state.pendingApprovals
            .where((a) => a.key != key)
            .toList(growable: false),
      );
    }
  }

  // --- Handlers SSE (7 eventos de plugins + stream:connected) ---

  void resetApprovals() {
    state = state.copyWith(pendingApprovals: const <PluginApprovalRequest>[]);
  }

  void _upsertRun(PluginRunView run) {
    var nextLoose = state.looseRuns;
    if (nextLoose.containsKey(run.id)) {
      nextLoose = <int, PluginRunView>{...nextLoose, run.id: run};
    }

    var nextViews = state.deviceViews;
    final view = nextViews[run.deviceId];
    if (view != null) {
      final runs = List<PluginRunView>.from(view.runs);
      final idx = runs.indexWhere((r) => r.id == run.id);
      if (idx >= 0) {
        runs[idx] = run;
      } else {
        runs.insert(0, run);
      }
      final capped = runs.length > _runsLimit
          ? runs.sublist(0, _runsLimit)
          : runs;
      nextViews = <int, DevicePluginsView>{
        ...nextViews,
        run.deviceId: view.copyWith(
          runs: List<PluginRunView>.unmodifiable(capped),
        ),
      };
    }

    state = state.copyWith(looseRuns: nextLoose, deviceViews: nextViews);
  }

  void applyRunStarted(Map<String, dynamic> data) {
    final raw = data['run'];
    if (raw is! Map) return;
    _upsertRun(PluginRunView.fromJson(asJsonMap(raw)));
  }

  void applyRunOutput(Map<String, dynamic> data) {
    final runId = parseIntOrNull(data['runId']);
    final deviceId = parseIntOrNull(data['deviceId']);
    final rawEntry = data['entry'];
    if (runId == null || deviceId == null || rawEntry is! Map) return;
    final entry = TranscriptEntry.fromJson(asJsonMap(rawEntry));

    var nextLoose = state.looseRuns;
    final loose = nextLoose[runId];
    if (loose != null && !loose.transcript.any((t) => t.seq == entry.seq)) {
      nextLoose = <int, PluginRunView>{
        ...nextLoose,
        runId: loose.copyWith(
          transcript: <TranscriptEntry>[...loose.transcript, entry],
        ),
      };
    }

    var nextViews = state.deviceViews;
    final view = nextViews[deviceId];
    if (view != null) {
      final updatedRuns = view.runs
          .map((r) {
            if (r.id != runId || r.transcript.any((t) => t.seq == entry.seq)) {
              return r;
            }
            return r.copyWith(
              transcript: <TranscriptEntry>[...r.transcript, entry],
            );
          })
          .toList(growable: false);
      nextViews = <int, DevicePluginsView>{
        ...nextViews,
        deviceId: view.copyWith(runs: updatedRuns),
      };
    }

    state = state.copyWith(looseRuns: nextLoose, deviceViews: nextViews);
  }

  void applyRunFinished(Map<String, dynamic> data) {
    final raw = data['run'];
    if (raw is! Map) return;
    var finished = PluginRunView.fromJson(asJsonMap(raw));
    final prev = state.runById(finished.deviceId, finished.id);
    if (finished.pluginName == null && prev?.pluginName != null) {
      finished = finished.copyWith(pluginName: prev!.pluginName);
    }
    _upsertRun(finished);
    _settleRun(finished);

    // Atualiza firmware do dispositivo quando `detect` devolve `output.firmware`.
    final view = state.deviceViews[finished.deviceId];
    if (view != null &&
        finished.status == 'succeeded' &&
        finished.output is Map) {
      final fw = (finished.output as Map)['firmware'];
      if (fw is String && fw.isNotEmpty) {
        state = state.copyWith(
          deviceViews: <int, DevicePluginsView>{
            ...state.deviceViews,
            finished.deviceId: view.copyWith(firmware: fw),
          },
        );
      }
    }
  }

  void applyRunProgress(Map<String, dynamic> data) {
    final runId = parseIntOrNull(data['runId']);
    final flow = data['flow'] as String?;
    final bps = parseDoubleOrNull(data['bps']);
    final tMs = parseIntOrNull(data['tMs']);
    if (runId == null ||
        (flow != 'tx' && flow != 'rx') ||
        bps == null ||
        tMs == null) {
      return;
    }
    final list = <ProgressSample>[
      ...(state.runProgress[runId] ?? const <ProgressSample>[]),
      ProgressSample(tMs: tMs, flow: flow!, bps: bps),
    ];
    final capped = list.length > _progressLimit
        ? list.sublist(list.length - _progressLimit)
        : list;
    final next = <int, List<ProgressSample>>{
      ...state.runProgress,
      runId: List<ProgressSample>.unmodifiable(capped),
    };
    if (next.length > _progressRunsLimit) {
      final keys = next.keys.toList()..sort();
      for (final old in keys.sublist(0, keys.length - _progressRunsLimit)) {
        next.remove(old);
      }
    }
    state = state.copyWith(runProgress: next);
  }

  void applyApprovalRequired(Map<String, dynamic> data) {
    final req = PluginApprovalRequest.tryFromJson(data);
    if (req == null) return;
    if (state.pendingApprovals.any((a) => a.key == req.key)) return;
    state = state.copyWith(
      pendingApprovals: List<PluginApprovalRequest>.unmodifiable(
        <PluginApprovalRequest>[...state.pendingApprovals, req],
      ),
    );
  }

  void applyApprovalResolved(Map<String, dynamic> data) {
    final key = data['key']?.toString();
    if (key == null || key.isEmpty) return;
    state = state.copyWith(
      pendingApprovals: state.pendingApprovals
          .where((a) => a.key != key)
          .toList(growable: false),
    );
  }
}

final pluginsProvider = NotifierProvider<PluginsNotifier, PluginsState>(
  PluginsNotifier.new,
);
