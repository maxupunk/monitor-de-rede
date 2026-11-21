import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';

const int _historyLimit = 60;
const int _operationLinesLimit = 50;
const int _operationsKept = 20;
const int _logLinesKept = 2000;

@immutable
class DockerHostState {
  const DockerHostState({
    this.status,
    this.containers = const <DockerContainerSummary>[],
    this.volumes = const <DockerVolumeSummary>[],
    this.networks = const <DockerNetworkSummary>[],
    this.images = const <DockerImageSummary>[],
    this.metrics,
    this.aggregateHistory = const <DockerAggregateSample>[],
    this.lastUpdatedAt,
  });

  final DockerStatusResponse? status;
  final List<DockerContainerSummary> containers;
  final List<DockerVolumeSummary> volumes;
  final List<DockerNetworkSummary> networks;
  final List<DockerImageSummary> images;
  final DockerMetricsResponse? metrics;
  final List<DockerAggregateSample> aggregateHistory;
  final String? lastUpdatedAt;

  bool get available => status?.available == true;
  int get runningContainers =>
      containers.where((c) => c.state == 'running').length;
  int get stoppedContainers => containers.length - runningContainers;

  DockerContainerMetrics? metricFor(String containerId) {
    final list = metrics?.containers;
    if (list == null) return null;
    for (final m in list) {
      if (m.containerId == containerId) return m;
    }
    return null;
  }

  DockerHostState copyWith({
    DockerStatusResponse? status,
    List<DockerContainerSummary>? containers,
    List<DockerVolumeSummary>? volumes,
    List<DockerNetworkSummary>? networks,
    List<DockerImageSummary>? images,
    DockerMetricsResponse? metrics,
    bool clearMetrics = false,
    List<DockerAggregateSample>? aggregateHistory,
    String? lastUpdatedAt,
  }) {
    return DockerHostState(
      status: status ?? this.status,
      containers: containers ?? this.containers,
      volumes: volumes ?? this.volumes,
      networks: networks ?? this.networks,
      images: images ?? this.images,
      metrics: clearMetrics ? null : (metrics ?? this.metrics),
      aggregateHistory: aggregateHistory ?? this.aggregateHistory,
      lastUpdatedAt: lastUpdatedAt ?? this.lastUpdatedAt,
    );
  }
}

@immutable
class DockerState {
  const DockerState({
    this.hosts = const <DockerHostView>[],
    this.hostsLoaded = false,
    this.hostStates = const <String, DockerHostState>{},
    this.selectedHostKey = '',
    this.operations = const <DockerOperationState>[],
    this.logStreams = const <String, DockerLogStreamState>{},
    this.loading = false,
    this.actionLoading = false,
    this.error,
  });

  final List<DockerHostView> hosts;
  final bool hostsLoaded;
  final Map<String, DockerHostState> hostStates;
  final String selectedHostKey;
  final List<DockerOperationState> operations;
  final Map<String, DockerLogStreamState> logStreams;
  final bool loading;
  final bool actionLoading;
  final String? error;

  DockerHostView? get localHost {
    for (final host in hosts) {
      if (host.kind == 'local') return host;
    }
    return null;
  }

  String get defaultHostKey =>
      localHost?.key ?? (hosts.isNotEmpty ? hosts.first.key : '');

  DockerHostView? get selectedHost {
    for (final host in hosts) {
      if (host.key == selectedHostKey) return host;
    }
    return null;
  }

  DockerHostState get current =>
      hostStates[selectedHostKey] ?? const DockerHostState();

  DockerHostState get localView {
    final local = localHost;
    return local != null
        ? (hostStates[local.key] ?? const DockerHostState())
        : const DockerHostState();
  }

  bool get available => current.available;
  int get runningContainers => current.runningContainers;
  int get stoppedContainers => current.stoppedContainers;

  bool allows(String permission) =>
      selectedHost?.policy.contains(permission) ?? false;

  /// Query `?host=agent-<id>` omitindo o host padrão (o desta central).
  String get hostQuerySuffix {
    if (selectedHostKey.isEmpty || selectedHostKey == defaultHostKey) return '';
    return '?host=${Uri.encodeQueryComponent(selectedHostKey)}';
  }

  List<DockerOperationState> get selectedHostOperations => operations
      .where((op) => op.hostKey == selectedHostKey)
      .toList(growable: false);

  DockerState copyWith({
    List<DockerHostView>? hosts,
    bool? hostsLoaded,
    Map<String, DockerHostState>? hostStates,
    String? selectedHostKey,
    List<DockerOperationState>? operations,
    Map<String, DockerLogStreamState>? logStreams,
    bool? loading,
    bool? actionLoading,
    String? error,
    bool clearError = false,
  }) {
    return DockerState(
      hosts: hosts ?? this.hosts,
      hostsLoaded: hostsLoaded ?? this.hostsLoaded,
      hostStates: hostStates ?? this.hostStates,
      selectedHostKey: selectedHostKey ?? this.selectedHostKey,
      operations: operations ?? this.operations,
      logStreams: logStreams ?? this.logStreams,
      loading: loading ?? this.loading,
      actionLoading: actionLoading ?? this.actionLoading,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

/// Serviço REST dos 33 endpoints Docker por `HostKey = agent-<id>` (ADR 014).
class DockerHostApi {
  const DockerHostApi(this._api, this.hostKey);

  final ApiClient _api;
  final String hostKey;

  String get _base => '/docker/hosts/${Uri.encodeComponent(hostKey)}';

  Future<DockerStatusResponse> status() async {
    final raw = await _api.get<Map<String, dynamic>>('$_base/status');
    return DockerStatusResponse.fromJson(raw);
  }

  Future<DockerMetricsResponse> metrics() async {
    final raw = await _api.get<Map<String, dynamic>>('$_base/metrics');
    return DockerMetricsResponse.fromJson(raw);
  }

  Future<List<DockerContainerSummary>> containers() async {
    final raw = await _api.get<Map<String, dynamic>>('$_base/containers');
    final list = raw['data'];
    if (list is! List) return const <DockerContainerSummary>[];
    return List<DockerContainerSummary>.unmodifiable(
      list.whereType<Map>().map(
        (e) => DockerContainerSummary.fromJson(Map<String, dynamic>.from(e)),
      ),
    );
  }

  Future<DockerContainerDetail> containerDetail(String id) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '$_base/containers/${Uri.encodeComponent(id)}',
    );
    return DockerContainerDetail.fromJson(raw);
  }

  Future<List<DockerLogEntry>> containerLogs(
    String id, {
    String tail = '100',
    bool timestamps = true,
  }) async {
    final raw = await _api.get<List<dynamic>>(
      '$_base/containers/${Uri.encodeComponent(id)}/logs',
      queryParameters: <String, dynamic>{
        'tail': tail,
        'timestamps': timestamps,
      },
    );
    return List<DockerLogEntry>.unmodifiable(
      raw.whereType<Map>().map(
        (e) => DockerLogEntry.fromJson(Map<String, dynamic>.from(e)),
      ),
    );
  }

  Future<String> followLogs(String id, {String tail = '100'}) async {
    final raw = await _api.post<Map<String, dynamic>>(
      '$_base/containers/${Uri.encodeComponent(id)}/logs/follow',
      data: <String, dynamic>{'tail': tail},
    );
    return (raw['streamId'] as String?) ?? '';
  }

  Future<void> clearLogs(String id) async {
    await _api.delete<dynamic>(
      '$_base/containers/${Uri.encodeComponent(id)}/logs',
    );
  }

  Future<void> startContainer(String id) async {
    await _api.post<dynamic>(
      '$_base/containers/${Uri.encodeComponent(id)}/start',
    );
  }

  Future<void> stopContainer(String id) async {
    await _api.post<dynamic>(
      '$_base/containers/${Uri.encodeComponent(id)}/stop',
    );
  }

  Future<void> restartContainer(String id) async {
    await _api.post<dynamic>(
      '$_base/containers/${Uri.encodeComponent(id)}/restart',
    );
  }

  Future<void> updateContainer(String id) async {
    await _api.post<dynamic>(
      '$_base/containers/${Uri.encodeComponent(id)}/update',
    );
  }

  Future<void> removeContainer(String id, {bool force = false}) async {
    await _api.delete<dynamic>(
      '$_base/containers/${Uri.encodeComponent(id)}',
      queryParameters: <String, dynamic>{'force': force},
    );
  }

  Future<List<DockerVolumeSummary>> volumes() async {
    final raw = await _api.get<Map<String, dynamic>>('$_base/volumes');
    final list = raw['data'];
    if (list is! List) return const <DockerVolumeSummary>[];
    return List<DockerVolumeSummary>.unmodifiable(
      list.whereType<Map>().map(
        (e) => DockerVolumeSummary.fromJson(Map<String, dynamic>.from(e)),
      ),
    );
  }

  Future<DockerVolumeDetail> volumeDetail(String name) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '$_base/volumes/${Uri.encodeComponent(name)}',
    );
    return DockerVolumeDetail.fromJson(raw);
  }

  Future<void> removeVolume(String name, {bool force = false}) async {
    await _api.delete<dynamic>(
      '$_base/volumes/${Uri.encodeComponent(name)}',
      queryParameters: <String, dynamic>{'force': force},
    );
  }

  Future<DownloadResult> exportVolume(String name) {
    return _api.downloadBytes(
      '$_base/volumes/${Uri.encodeComponent(name)}/export',
    );
  }

  Future<List<DockerNetworkSummary>> networks() async {
    final raw = await _api.get<Map<String, dynamic>>('$_base/networks');
    final list = raw['data'];
    if (list is! List) return const <DockerNetworkSummary>[];
    return List<DockerNetworkSummary>.unmodifiable(
      list.whereType<Map>().map(
        (e) => DockerNetworkSummary.fromJson(Map<String, dynamic>.from(e)),
      ),
    );
  }

  Future<DockerNetworkDetail> networkDetail(String id) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '$_base/networks/${Uri.encodeComponent(id)}',
    );
    return DockerNetworkDetail.fromJson(raw);
  }

  Future<void> createNetwork(String name, {String driver = 'bridge'}) async {
    await _api.post<dynamic>(
      '$_base/networks',
      data: <String, dynamic>{'name': name, 'driver': driver},
    );
  }

  Future<void> connectNetwork(String networkId, String containerId) async {
    await _api.post<dynamic>(
      '$_base/networks/${Uri.encodeComponent(networkId)}/connect',
      data: <String, dynamic>{'containerId': containerId},
    );
  }

  Future<void> disconnectNetwork(
    String networkId,
    String containerId, {
    bool force = false,
  }) async {
    await _api.post<dynamic>(
      '$_base/networks/${Uri.encodeComponent(networkId)}/disconnect',
      data: <String, dynamic>{'containerId': containerId, 'force': force},
    );
  }

  Future<void> removeNetwork(String id) async {
    await _api.delete<dynamic>('$_base/networks/${Uri.encodeComponent(id)}');
  }

  Future<List<DockerImageSummary>> images() async {
    final raw = await _api.get<Map<String, dynamic>>('$_base/images');
    final list = raw['data'];
    if (list is! List) return const <DockerImageSummary>[];
    return List<DockerImageSummary>.unmodifiable(
      list.whereType<Map>().map(
        (e) => DockerImageSummary.fromJson(Map<String, dynamic>.from(e)),
      ),
    );
  }

  Future<DockerImageDetail> imageDetail(String id) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '$_base/images/${Uri.encodeComponent(id)}',
    );
    return DockerImageDetail.fromJson(raw);
  }

  Future<void> removeImage(String id, {bool force = false}) async {
    await _api.delete<dynamic>(
      '$_base/images/${Uri.encodeComponent(id)}',
      queryParameters: <String, dynamic>{'force': force},
    );
  }

  Future<DockerPruneResponse> pruneImages() async {
    final raw = await _api.post<Map<String, dynamic>>('$_base/images/prune');
    return DockerPruneResponse.fromJson(raw);
  }

  Future<void> pullImage(String image) async {
    await _api.post<dynamic>(
      '$_base/images/pull',
      data: <String, dynamic>{'image': image},
    );
  }

  Future<List<ComposeProject>> composeProjects() async {
    final raw = await _api.get<Map<String, dynamic>>('$_base/compose');
    final list = raw['data'];
    if (list is! List) return const <ComposeProject>[];
    return List<ComposeProject>.unmodifiable(
      list.whereType<Map>().map(
        (e) => ComposeProject.fromJson(Map<String, dynamic>.from(e)),
      ),
    );
  }

  Future<void> composeAction(
    String project,
    String action, {
    String? service,
  }) async {
    await _api.post<dynamic>(
      '$_base/compose/${Uri.encodeComponent(project)}',
      data: <String, dynamic>{'action': action, 'service': service},
    );
  }

  Future<HostHistoryResponse> hostHistory(String range) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '$_base/history',
      queryParameters: <String, dynamic>{'range': range},
    );
    return HostHistoryResponse.fromJson(raw);
  }

  Future<ContainerHistoryResponse> containerHistory(
    String containerName,
    String range,
  ) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '$_base/history/containers/${Uri.encodeComponent(containerName)}',
      queryParameters: <String, dynamic>{'range': range},
    );
    return ContainerHistoryResponse.fromJson(raw);
  }
}

/// Estado global da área Docker — 100% dirigido por SSE (`docker:*` + `probe:status`), ZERO polling!
class DockerNotifier extends Notifier<DockerState> {
  @override
  DockerState build() => const DockerState();

  ApiClient get _apiClient => ref.read(apiClientProvider);

  DockerHostApi apiFor([String? hostKey]) =>
      DockerHostApi(_apiClient, hostKey ?? state.selectedHostKey);

  String _describe(Object err, String fallback) {
    if (err is ApiException && err.message.isNotEmpty) return err.message;
    if (err is NetworkException && err.message.isNotEmpty) return err.message;
    return fallback;
  }

  DockerHostState _appendAggregate(
    DockerHostState hostState,
    DockerMetricsResponse metrics,
  ) {
    if (!metrics.dockerAvailable) return hostState;
    if (hostState.aggregateHistory.isNotEmpty &&
        hostState.aggregateHistory.last.recordedAt == metrics.collectedAt) {
      return hostState.copyWith(metrics: metrics);
    }

    var cpuTotal = 0.0;
    var memTotalUsage = 0;
    var rxTotal = 0;
    var txTotal = 0;
    final cpuByContainer = <String, double>{};
    final memByContainer = <String, int>{};
    final names = <String, String>{};

    for (final c in metrics.containers) {
      cpuTotal += c.cpu.usagePercent;
      memTotalUsage += c.memory.usageBytes;
      rxTotal += c.network.receivedBytes;
      txTotal += c.network.transmittedBytes;
      cpuByContainer[c.containerId] = c.cpu.usagePercent;
      memByContainer[c.containerId] = c.memory.usageBytes;
      names[c.containerId] = c.containerName;
    }

    final engineMem = math.max(hostState.status?.memoryTotalBytes ?? 0, 0);
    final memPct = engineMem > 0
        ? math.min(100.0, (memTotalUsage / engineMem) * 100.0)
        : 0.0;

    final sample = DockerAggregateSample(
      recordedAt: metrics.collectedAt,
      cpuPercent: cpuTotal,
      memoryPercent: memPct,
      memoryUsageBytes: memTotalUsage,
      networkReceivedBytes: rxTotal,
      networkTransmittedBytes: txTotal,
      cpuByContainer: Map<String, double>.unmodifiable(cpuByContainer),
      memoryByContainer: Map<String, int>.unmodifiable(memByContainer),
      containerNames: Map<String, String>.unmodifiable(names),
    );

    final nextHistory = <DockerAggregateSample>[
      ...hostState.aggregateHistory,
      sample,
    ];
    final trimmed = nextHistory.length > _historyLimit
        ? nextHistory.sublist(nextHistory.length - _historyLimit)
        : nextHistory;

    return hostState.copyWith(
      metrics: metrics,
      aggregateHistory: List<DockerAggregateSample>.unmodifiable(trimmed),
      lastUpdatedAt: metrics.collectedAt,
    );
  }

  void _updateHostState(
    String hostKey,
    DockerHostState Function(DockerHostState) updater,
  ) {
    final current = state.hostStates[hostKey] ?? const DockerHostState();
    final updated = updater(current);
    state = state.copyWith(
      hostStates: Map<String, DockerHostState>.unmodifiable(
        <String, DockerHostState>{...state.hostStates, hostKey: updated},
      ),
    );
  }

  Future<void> fetchHosts({String? preferredHostKey}) async {
    try {
      final raw = await _apiClient.get<List<dynamic>>('/docker/hosts');
      final list = raw
          .whereType<Map>()
          .map((e) => DockerHostView.fromJson(Map<String, dynamic>.from(e)))
          .toList(growable: false);

      var nextSelected = preferredHostKey ?? state.selectedHostKey;
      if (nextSelected.isEmpty || !list.any((h) => h.key == nextSelected)) {
        DockerHostView? local;
        for (final h in list) {
          if (h.kind == 'local') {
            local = h;
            break;
          }
        }
        nextSelected = local?.key ?? (list.isNotEmpty ? list.first.key : '');
      }

      state = state.copyWith(
        hosts: List<DockerHostView>.unmodifiable(list),
        hostsLoaded: true,
        selectedHostKey: nextSelected,
        clearError: true,
      );
    } catch (err) {
      state = state.copyWith(
        hostsLoaded: true,
        error: _describe(err, 'Erro ao listar os hosts Docker'),
      );
    }
  }

  void selectHost(String hostKey) {
    final resolved = hostKey.isNotEmpty ? hostKey : state.defaultHostKey;
    if (resolved == state.selectedHostKey) return;
    state = state.copyWith(selectedHostKey: resolved, clearError: true);
  }

  /// Atualização manual explícita (botão "Atualizar" na barra).
  Future<void> refreshAll() async {
    final hostKey = state.selectedHostKey;
    if (hostKey.isEmpty) return;
    state = state.copyWith(loading: true, clearError: true);
    final api = apiFor(hostKey);
    try {
      final status = await api.status();
      if (!status.available) {
        _updateHostState(hostKey, (_) => DockerHostState(status: status));
        state = state.copyWith(loading: false);
        return;
      }
      final results = await Future.wait<dynamic>(<Future<dynamic>>[
        api.containers(),
        api.volumes(),
        api.networks(),
        api.images(),
        api.metrics(),
      ]);
      _updateHostState(hostKey, (prev) {
        final withLists = prev.copyWith(
          status: status,
          containers: results[0] as List<DockerContainerSummary>,
          volumes: results[1] as List<DockerVolumeSummary>,
          networks: results[2] as List<DockerNetworkSummary>,
          images: results[3] as List<DockerImageSummary>,
        );
        return _appendAggregate(withLists, results[4] as DockerMetricsResponse);
      });
      state = state.copyWith(loading: false);
    } catch (err) {
      state = state.copyWith(
        loading: false,
        error: _describe(err, 'Erro ao consultar a Docker Engine'),
      );
    }
  }

  Future<void> refreshContainers() async {
    final hostKey = state.selectedHostKey;
    if (hostKey.isEmpty) return;
    final api = apiFor(hostKey);
    try {
      final containers = await api.containers();
      final status = await api.status();
      final metrics = await api.metrics();
      _updateHostState(hostKey, (prev) {
        final next = prev.copyWith(status: status, containers: containers);
        return _appendAggregate(next, metrics);
      });
    } catch (err) {
      state = state.copyWith(
        error: _describe(err, 'Erro ao atualizar containers'),
      );
    }
  }

  Future<bool> runAction(
    Future<void> Function() action, {
    Future<void> Function()? refresh,
  }) async {
    state = state.copyWith(actionLoading: true, clearError: true);
    try {
      await action();
      if (refresh != null) await refresh();
      state = state.copyWith(actionLoading: false);
      return true;
    } catch (err) {
      state = state.copyWith(
        actionLoading: false,
        error: _describe(err, 'Operação Docker não concluída'),
      );
      return false;
    }
  }

  // ── Handlers SSE (`docker:*` e `probe:status`) ──────────────────────────

  void applyLiveSnapshot(Map<String, dynamic> data) {
    final hostKey = data['hostKey'] as String?;
    if (hostKey == null || hostKey.isEmpty) return;
    final rawStatus = data['status'];
    if (rawStatus is! Map) return;
    final status = DockerStatusResponse.fromJson(
      Map<String, dynamic>.from(rawStatus),
    );

    _updateHostState(hostKey, (prev) {
      if (!status.available) {
        return DockerHostState(status: status);
      }
      final rawContainers = data['containers'];
      final containers = rawContainers is List
          ? List<DockerContainerSummary>.unmodifiable(
              rawContainers.whereType<Map>().map(
                (e) => DockerContainerSummary.fromJson(
                  Map<String, dynamic>.from(e),
                ),
              ),
            )
          : prev.containers;
      final rawMetrics = data['metrics'];
      final nextState = prev.copyWith(status: status, containers: containers);
      if (rawMetrics is Map) {
        return _appendAggregate(
          nextState,
          DockerMetricsResponse.fromJson(Map<String, dynamic>.from(rawMetrics)),
        );
      }
      return nextState;
    });

    if (hostKey == state.selectedHostKey) {
      state = state.copyWith(loading: false, clearError: true);
    }
  }

  void applyInventorySnapshot(Map<String, dynamic> data) {
    final hostKey = data['hostKey'] as String?;
    if (hostKey == null || hostKey.isEmpty) return;

    List<T> parseList<T>(Object? raw, T Function(Map<String, dynamic>) fn) {
      if (raw is! List) return const [];
      return List<T>.unmodifiable(
        raw.whereType<Map>().map((e) => fn(Map<String, dynamic>.from(e))),
      );
    }

    _updateHostState(hostKey, (prev) {
      return prev.copyWith(
        volumes: parseList(data['volumes'], DockerVolumeSummary.fromJson),
        networks: parseList(data['networks'], DockerNetworkSummary.fromJson),
        images: parseList(data['images'], DockerImageSummary.fromJson),
      );
    });
    if (hostKey == state.selectedHostKey) {
      state = state.copyWith(clearError: true);
    }
  }

  void applyOperationEvent(Map<String, dynamic> data) {
    final opId = data['operationId'] as String?;
    final hostKey = data['hostKey'] as String?;
    if (opId == null || hostKey == null) return;

    final now = DateTime.now().toIso8601String();
    final line = data['line'] as String?;
    final msg = data['message'] as String?;
    final nextList = List<DockerOperationState>.from(state.operations);
    final idx = nextList.indexWhere((o) => o.operationId == opId);

    if (idx != -1) {
      final prev = nextList[idx];
      var lines = prev.lines;
      if (line != null && line.isNotEmpty) {
        final appended = <String>[...lines, line];
        lines = appended.length > _operationLinesLimit
            ? appended.sublist(appended.length - _operationLinesLimit)
            : appended;
      }
      nextList[idx] = DockerOperationState(
        operationId: opId,
        hostKey: hostKey,
        kind: prev.kind,
        target: prev.target,
        state: (data['state'] as String?) ?? prev.state,
        lines: List<String>.unmodifiable(lines),
        message: msg ?? prev.message,
        updatedAt: now,
      );
    } else {
      nextList.insert(
        0,
        DockerOperationState(
          operationId: opId,
          hostKey: hostKey,
          kind: (data['kind'] as String?) ?? 'operation',
          target: (data['target'] as String?) ?? '',
          state: (data['state'] as String?) ?? 'running',
          lines: (line != null && line.isNotEmpty)
              ? <String>[line]
              : const <String>[],
          message: msg,
          updatedAt: now,
        ),
      );
      if (nextList.length > _operationsKept) {
        nextList.removeRange(_operationsKept, nextList.length);
      }
    }
    state = state.copyWith(
      operations: List<DockerOperationState>.unmodifiable(nextList),
    );
  }

  void dismissOperation(String operationId) {
    state = state.copyWith(
      operations: List<DockerOperationState>.unmodifiable(
        state.operations.where((o) => o.operationId != operationId),
      ),
    );
  }

  void applyLogStreamEvent(Map<String, dynamic> data) {
    final streamId = data['streamId'] as String?;
    if (streamId == null) return;
    final current = state.logStreams[streamId];
    if (current == null) return;

    final rawEntries = data['entries'];
    var entries = current.entries;
    if (rawEntries is List && rawEntries.isNotEmpty) {
      final parsed = rawEntries.whereType<Map>().map(
        (e) => DockerLogEntry.fromJson(Map<String, dynamic>.from(e)),
      );
      final merged = <DockerLogEntry>[...entries, ...parsed];
      entries = merged.length > _logLinesKept
          ? merged.sublist(merged.length - _logLinesKept)
          : merged;
    }

    final ended = data['ended'] as String? ?? current.ended;
    state = state.copyWith(
      logStreams: Map<String, DockerLogStreamState>.unmodifiable(
        <String, DockerLogStreamState>{
          ...state.logStreams,
          streamId: current.copyWith(
            entries: List<DockerLogEntry>.unmodifiable(entries),
            ended: ended,
          ),
        },
      ),
    );
  }

  Future<String> startLogStream(
    String containerId, {
    String tail = '100',
  }) async {
    final streamId = await apiFor().followLogs(containerId, tail: tail);
    state = state.copyWith(
      logStreams: Map<String, DockerLogStreamState>.unmodifiable(
        <String, DockerLogStreamState>{
          ...state.logStreams,
          streamId: DockerLogStreamState(
            streamId: streamId,
            hostKey: state.selectedHostKey,
            containerId: containerId,
            entries: const <DockerLogEntry>[],
          ),
        },
      ),
    );
    return streamId;
  }

  Future<void> stopLogStream(String streamId) async {
    final next = Map<String, DockerLogStreamState>.from(state.logStreams)
      ..remove(streamId);
    state = state.copyWith(
      logStreams: Map<String, DockerLogStreamState>.unmodifiable(next),
    );
    try {
      await _apiClient.delete<dynamic>(
        '/docker/log-streams/${Uri.encodeComponent(streamId)}',
      );
    } catch (_) {
      // Stream pode já ter encerrado do lado do servidor.
    }
  }

  void applyAgentStatus(Map<String, dynamic> data) {
    final rawId = data['id'] ?? data['probeId'];
    final id = rawId is num
        ? rawId.toInt()
        : int.tryParse(rawId?.toString() ?? '');
    if (id == null) return;
    final idx = state.hosts.indexWhere((h) => h.agentId == id);
    if (idx == -1) return;
    final online = data['status'] == 'online';
    final next = List<DockerHostView>.from(state.hosts);
    next[idx] = next[idx].copyWith(online: online);
    state = state.copyWith(hosts: List<DockerHostView>.unmodifiable(next));
  }
}

final dockerProvider = NotifierProvider<DockerNotifier, DockerState>(
  DockerNotifier.new,
);
