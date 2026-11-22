import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';

@immutable
class TopologyState {
  const TopologyState({
    this.nodes = const <TopologyNodeModel>[],
    this.edges = const <TopologyEdgeModel>[],
    this.positions = const <int, TopologyPosition>{},
    this.pluginActions = const <int, List<TopologyPluginActionModel>>{},
    this.siteId,
    this.layoutMode = TopologyLayoutMode.hierarchical,
    this.viewPrefs = const TopologyViewPrefs(),
    this.highlightedPath,
    this.loading = false,
    this.loaded = false,
    this.recalculating = false,
    this.savingLayout = false,
    this.remoteLayoutRevision = 0,
    this.error,
    this.actionError,
  });

  final List<TopologyNodeModel> nodes;
  final List<TopologyEdgeModel> edges;
  final Map<int, TopologyPosition> positions;
  final Map<int, List<TopologyPluginActionModel>> pluginActions;
  final int? siteId;
  final TopologyLayoutMode layoutMode;
  final TopologyViewPrefs viewPrefs;
  final TopologyPathModel? highlightedPath;
  final bool loading;
  final bool loaded;
  final bool recalculating;
  final bool savingLayout;
  final int remoteLayoutRevision;
  final String? error;
  final String? actionError;

  TopologyState copyWith({
    List<TopologyNodeModel>? nodes,
    List<TopologyEdgeModel>? edges,
    Map<int, TopologyPosition>? positions,
    Map<int, List<TopologyPluginActionModel>>? pluginActions,
    int? siteId,
    bool clearSiteId = false,
    TopologyLayoutMode? layoutMode,
    TopologyViewPrefs? viewPrefs,
    TopologyPathModel? highlightedPath,
    bool clearHighlightedPath = false,
    bool? loading,
    bool? loaded,
    bool? recalculating,
    bool? savingLayout,
    int? remoteLayoutRevision,
    String? error,
    bool clearError = false,
    String? actionError,
    bool clearActionError = false,
  }) {
    return TopologyState(
      nodes: nodes ?? this.nodes,
      edges: edges ?? this.edges,
      positions: positions ?? this.positions,
      pluginActions: pluginActions ?? this.pluginActions,
      siteId: clearSiteId ? null : (siteId ?? this.siteId),
      layoutMode: layoutMode ?? this.layoutMode,
      viewPrefs: viewPrefs ?? this.viewPrefs,
      highlightedPath: clearHighlightedPath
          ? null
          : (highlightedPath ?? this.highlightedPath),
      loading: loading ?? this.loading,
      loaded: loaded ?? this.loaded,
      recalculating: recalculating ?? this.recalculating,
      savingLayout: savingLayout ?? this.savingLayout,
      remoteLayoutRevision: remoteLayoutRevision ?? this.remoteLayoutRevision,
      error: clearError ? null : (error ?? this.error),
      actionError: clearActionError ? null : (actionError ?? this.actionError),
    );
  }
}

class TopologyNotifier extends Notifier<TopologyState> {
  final Map<int, TopologyPosition> _unsavedPositions =
      <int, TopologyPosition>{};

  @override
  TopologyState build() => const TopologyState();

  Future<void> fetchTopology({
    int? siteId,
    bool updateSiteFilter = false,
    bool showLoading = true,
    bool preserveUnsavedPositions = false,
  }) async {
    final targetSite = updateSiteFilter ? siteId : state.siteId;
    final siteChanged = updateSiteFilter && targetSite != state.siteId;
    if (!preserveUnsavedPositions || siteChanged) {
      _unsavedPositions.clear();
    }
    state = state.copyWith(
      siteId: targetSite,
      clearSiteId: updateSiteFilter && siteId == null,
      loading: showLoading ? true : state.loading,
      clearError: true,
    );

    try {
      final api = ref.read(apiClientProvider);
      final query = targetSite != null
          ? <String, dynamic>{'siteId': targetSite}
          : null;
      final results = await Future.wait<Object?>(<Future<Object?>>[
        api.get<dynamic>('/topology', queryParameters: query),
        api
            .get<dynamic>('/topology/layout', queryParameters: query)
            .catchError((_) => null),
      ]);

      final graphMap = asJsonMap(results[0]);
      final parsedNodes = asJsonMapList(
        graphMap['nodes'],
      ).map(TopologyNodeModel.fromJson).toList(growable: false);
      final parsedEdges = asJsonMapList(
        graphMap['edges'],
      ).map(TopologyEdgeModel.fromJson).toList(growable: false);

      final savedPositions = <int, TopologyPosition>{};
      if (results[1] is Map) {
        for (final item in asJsonMapList(asJsonMap(results[1])['nodes'])) {
          final id = parseIntOrNull(item['deviceId'] ?? item['id']);
          final x = parseDoubleOrNull(item['x']);
          final y = parseDoubleOrNull(item['y']);
          if (id != null && x != null && y != null) {
            savedPositions[id] = TopologyPosition(x, y);
          }
        }
      }

      final computed = computeTopologyLayout(
        parsedNodes,
        parsedEdges,
        state.layoutMode,
        existing: savedPositions,
      );

      if (preserveUnsavedPositions && _unsavedPositions.isNotEmpty) {
        final nodeIds = parsedNodes.map((n) => n.id).toSet();
        for (final entry in _unsavedPositions.entries) {
          if (nodeIds.contains(entry.key)) {
            computed[entry.key] = entry.value;
          }
        }
      }

      state = state.copyWith(
        nodes: parsedNodes,
        edges: parsedEdges,
        positions: computed,
        loading: false,
        loaded: true,
      );

      // Carrega ações de plugin para nós visíveis em background.
      if (parsedNodes.isNotEmpty) {
        _fetchPluginActions(
          parsedNodes.map((n) => n.id).toList(growable: false),
        );
      }
    } catch (err) {
      state = state.copyWith(loading: false, error: err.toString());
    }
  }

  Future<void> _fetchPluginActions(List<int> deviceIds) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>(
            '/plugins/topology-actions',
            queryParameters: <String, dynamic>{
              'deviceIds': deviceIds.join(','),
            },
          );
      final map = asJsonMap(raw);
      final out = <int, List<TopologyPluginActionModel>>{};
      map.forEach((key, value) {
        final id = int.tryParse(key);
        if (id != null) {
          out[id] = asJsonMapList(
            value,
          ).map(TopologyPluginActionModel.fromJson).toList(growable: false);
        }
      });
      state = state.copyWith(pluginActions: out);
    } catch (_) {}
  }

  void refreshIfLoaded() {
    if (!state.loaded) return;
    fetchTopology(showLoading: false, preserveUnsavedPositions: true);
  }

  void setViewPrefs(TopologyViewPrefs prefs) {
    state = state.copyWith(viewPrefs: prefs);
  }

  void applyLayoutMode(TopologyLayoutMode mode) {
    final computed = computeTopologyLayout(state.nodes, state.edges, mode);
    _unsavedPositions.clear();
    state = state.copyWith(layoutMode: mode, positions: computed);
  }

  void moveNode(
    int nodeId,
    TopologyPosition nextPosition, {
    bool markUnsaved = true,
  }) {
    if (markUnsaved) {
      _unsavedPositions[nodeId] = nextPosition;
    } else {
      _unsavedPositions.remove(nodeId);
    }
    state = state.copyWith(
      positions: <int, TopologyPosition>{
        ...state.positions,
        nodeId: nextPosition,
      },
    );
  }

  void clearUnsavedPositions({Iterable<int>? nodeIds}) {
    if (nodeIds == null) {
      _unsavedPositions.clear();
      return;
    }
    for (final id in nodeIds) {
      _unsavedPositions.remove(id);
    }
  }

  Future<bool> saveLayout({
    List<int>? onlyNodeIds,
    Map<int, TopologyPosition>? overridePositions,
  }) async {
    final mergedPositions =
        overridePositions != null && overridePositions.isNotEmpty
        ? <int, TopologyPosition>{...state.positions, ...overridePositions}
        : state.positions;
    final targets = onlyNodeIds ?? mergedPositions.keys.toList(growable: false);
    final changes = <Map<String, dynamic>>[];
    for (final id in targets) {
      final pos = mergedPositions[id];
      if (pos != null) {
        changes.add(<String, dynamic>{'deviceId': id, 'x': pos.x, 'y': pos.y});
      }
    }
    if (changes.isEmpty) return true;
    state = state.copyWith(
      positions: mergedPositions,
      savingLayout: true,
      clearActionError: true,
    );
    final api = ref.read(apiClientProvider);
    try {
      await api.patch<dynamic>(
        '/topology/layout',
        data: <String, dynamic>{'siteId': state.siteId, 'nodes': changes},
      );
      clearUnsavedPositions(nodeIds: targets);
      state = state.copyWith(savingLayout: false);
      return true;
    } on ApiException catch (err) {
      if (err.statusCode == 404 || err.statusCode == 405) {
        final allNodes = mergedPositions.entries
            .map(
              (e) => <String, dynamic>{
                'deviceId': e.key,
                'x': e.value.x,
                'y': e.value.y,
              },
            )
            .toList(growable: false);
        await api.put<dynamic>(
          '/topology/layout',
          queryParameters: state.siteId != null
              ? <String, dynamic>{'siteId': state.siteId}
              : null,
          data: <String, dynamic>{'nodes': allNodes},
        );
        clearUnsavedPositions();
        state = state.copyWith(savingLayout: false);
        return true;
      }
      state = state.copyWith(savingLayout: false, actionError: err.message);
      return false;
    } catch (err) {
      state = state.copyWith(savingLayout: false, actionError: err.toString());
      return false;
    }
  }

  Future<bool> addLink(Map<String, dynamic> payload) async {
    state = state.copyWith(clearActionError: true);
    try {
      await ref
          .read(apiClientProvider)
          .post<dynamic>('/topology/links', data: payload);
      await fetchTopology(showLoading: false);
      return true;
    } catch (err) {
      state = state.copyWith(actionError: err.toString());
      return false;
    }
  }

  Future<bool> updateLink(int linkId, Map<String, dynamic> payload) async {
    state = state.copyWith(clearActionError: true);
    try {
      await ref
          .read(apiClientProvider)
          .put<dynamic>('/topology/links/$linkId', data: payload);
      await fetchTopology(showLoading: false);
      return true;
    } catch (err) {
      state = state.copyWith(actionError: err.toString());
      return false;
    }
  }

  Future<bool> deleteLink(int linkId) async {
    state = state.copyWith(clearActionError: true);
    try {
      await ref
          .read(apiClientProvider)
          .delete<dynamic>('/topology/links/$linkId');
      state = state.copyWith(
        edges: state.edges.where((e) => e.id != linkId).toList(growable: false),
      );
      await fetchTopology(showLoading: false);
      return true;
    } catch (err) {
      state = state.copyWith(actionError: err.toString());
      return false;
    }
  }

  Future<bool> createUnmanagedSwitch(Map<String, dynamic> payload) async {
    state = state.copyWith(clearActionError: true);
    try {
      await ref
          .read(apiClientProvider)
          .post<dynamic>('/topology/unmanaged-switch', data: payload);
      await fetchTopology(showLoading: false);
      return true;
    } catch (err) {
      state = state.copyWith(actionError: err.toString());
      return false;
    }
  }

  Future<bool> recalculateTopology() async {
    state = state.copyWith(recalculating: true, clearActionError: true);
    try {
      await ref.read(apiClientProvider).post<dynamic>('/topology/recalculate');
      await fetchTopology(showLoading: false);
      state = state.copyWith(recalculating: false);
      return true;
    } catch (err) {
      state = state.copyWith(recalculating: false, actionError: err.toString());
      return false;
    }
  }

  Future<TopologyPathModel?> fetchPath(int fromId, int toId) async {
    state = state.copyWith(clearActionError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>(
            '/topology/path',
            queryParameters: <String, dynamic>{'from': fromId, 'to': toId},
          );
      final parsed = TopologyPathModel.fromJson(asJsonMap(raw));
      state = state.copyWith(highlightedPath: parsed);
      return parsed;
    } catch (err) {
      state = state.copyWith(actionError: err.toString());
      return null;
    }
  }

  void clearHighlightedPath() {
    state = state.copyWith(clearHighlightedPath: true);
  }

  // ── SSE Realtime Handlers ─────────────────────────────────────────────────

  void applyRealtimeStatus(Map<String, dynamic> data) {
    final id = parseIntOrNull(data['id'] ?? data['deviceId']);
    if (id == null) return;
    final idx = state.nodes.indexWhere((n) => n.id == id);
    if (idx < 0) return;
    final updated = state.nodes[idx].copyWith(
      status: data['status'] as String?,
      name: data['name'] as String?,
      ipAddress: data['ipAddress'] as String?,
    );
    final next = List<TopologyNodeModel>.from(state.nodes)..[idx] = updated;
    state = state.copyWith(nodes: List<TopologyNodeModel>.unmodifiable(next));
  }

  void applyRealtimeHealth(Map<String, dynamic> data) {
    final deviceId = parseIntOrNull(data['deviceId']);
    if (deviceId == null) return;
    final metrics = asJsonMapList(data['metrics']);
    if (metrics.isEmpty) return;

    final nodeIdx = state.nodes.indexWhere((n) => n.id == deviceId);
    final hasConnectedEdge = state.edges.any(
      (e) => e.sourceDeviceId == deviceId || e.targetDeviceId == deviceId,
    );
    if (nodeIdx < 0 && !hasConnectedEdge) return;

    double? cpu;
    double? mem;
    double? deviceInBps;
    double? deviceOutBps;
    final byInterfaceId = <int, ({double? inBps, double? outBps})>{};

    for (final m in metrics) {
      final val = parseDoubleOrNull(m['value']);
      if (val == null || !val.isFinite) continue;
      final name = m['name']?.toString();
      final ifId = parseIntOrNull(m['interfaceId']);
      final isIn = name == 'inBps' || name == 'in_bps';
      final isOut = name == 'outBps' || name == 'out_bps';

      if (ifId == null) {
        if (name == 'cpu_usage') cpu = val.clamp(0.0, 100.0);
        if (name == 'memory_usage') mem = val.clamp(0.0, 100.0);
        if (isIn && val >= 0) deviceInBps = val;
        if (isOut && val >= 0) deviceOutBps = val;
      } else if ((isIn || isOut) && val >= 0) {
        final prev = byInterfaceId[ifId];
        byInterfaceId[ifId] = (
          inBps: isIn ? val : prev?.inBps,
          outBps: isOut ? val : prev?.outBps,
        );
      }
    }

    List<TopologyNodeModel>? nextNodes;
    if (nodeIdx >= 0 && (cpu != null || mem != null)) {
      final updated = List<TopologyNodeModel>.from(state.nodes)
        ..[nodeIdx] = state.nodes[nodeIdx].copyWith(
          cpuPercent: cpu,
          memoryPercent: mem,
        );
      nextNodes = List<TopologyNodeModel>.unmodifiable(updated);
    }

    var edgesChanged = false;
    List<TopologyEdgeModel>? nextEdges;
    final hasDeviceLevelTraffic = deviceInBps != null || deviceOutBps != null;
    if (byInterfaceId.isNotEmpty || hasDeviceLevelTraffic) {
      final isSystemDevice = nodeIdx >= 0 && state.nodes[nodeIdx].isSystem;
      final realLinksForDevice = state.edges
          .where(
            (e) =>
                !e.isVirtualHierarchy &&
                (e.sourceDeviceId == deviceId || e.targetDeviceId == deviceId),
          )
          .toList(growable: false);
      final candidateLinksForDevice = realLinksForDevice.isNotEmpty
          ? realLinksForDevice
          : state.edges
                .where(
                  (e) =>
                      e.sourceDeviceId == deviceId ||
                      e.targetDeviceId == deviceId,
                )
                .toList(growable: false);
      final allowUnboundDeviceTraffic =
          hasDeviceLevelTraffic &&
          (isSystemDevice || candidateLinksForDevice.length == 1);
      final candidateEdgeIds = allowUnboundDeviceTraffic
          ? candidateLinksForDevice.map((e) => e.id).toSet()
          : const <int>{};

      nextEdges = state.edges
          .map((edge) {
            if (edge.sourceDeviceId == deviceId) {
              final ifMatch = edge.sourceInterfaceId != null
                  ? byInterfaceId[edge.sourceInterfaceId!]
                  : null;
              if (ifMatch != null) {
                edgesChanged = true;
                return edge.withTrafficUpdate(
                  inBps: ifMatch.inBps,
                  outBps: ifMatch.outBps,
                  trafficFromSource: true,
                );
              }
              if (candidateEdgeIds.contains(edge.id) &&
                  (edge.sourceInterfaceId == null ||
                      candidateLinksForDevice.length == 1)) {
                edgesChanged = true;
                return edge.withTrafficUpdate(
                  inBps: deviceInBps,
                  outBps: deviceOutBps,
                  trafficFromSource: true,
                );
              }
            }
            if (edge.targetDeviceId == deviceId) {
              final canAcceptTarget =
                  edge.sourceInterfaceId == null || !edge.trafficFromSource;
              final ifMatch = edge.targetInterfaceId != null
                  ? byInterfaceId[edge.targetInterfaceId!]
                  : null;
              if (ifMatch != null && canAcceptTarget) {
                edgesChanged = true;
                return edge.withTrafficUpdate(
                  inBps: ifMatch.inBps,
                  outBps: ifMatch.outBps,
                );
              }
              if (candidateEdgeIds.contains(edge.id) &&
                  canAcceptTarget &&
                  (edge.targetInterfaceId == null ||
                      candidateLinksForDevice.length == 1)) {
                edgesChanged = true;
                return edge.withTrafficUpdate(
                  inBps: deviceInBps,
                  outBps: deviceOutBps,
                );
              }
            }
            return edge;
          })
          .toList(growable: false);
    }

    if (nextNodes != null || edgesChanged) {
      state = state.copyWith(
        nodes: nextNodes,
        edges: edgesChanged ? nextEdges : null,
      );
    }
  }

  void applyRealtimeTraffic(Map<String, dynamic> data) {
    final deviceId = parseIntOrNull(data['deviceId']);
    if (deviceId == null) return;
    final rawInterfaces = asJsonMapList(data['interfaces']);
    if (rawInterfaces.isEmpty) return;

    final deviceEdgeCount = state.edges
        .where(
          (e) => e.sourceDeviceId == deviceId || e.targetDeviceId == deviceId,
        )
        .length;

    Map<String, dynamic>? matchEndpoint(int? ifId, String? ifName) {
      final normName = ifName?.trim().toLowerCase();
      for (final item in rawInterfaces) {
        if (ifId != null && parseIntOrNull(item['id']) == ifId) {
          return item;
        }
        if (normName != null &&
            normName.isNotEmpty &&
            item['name'] != null &&
            item['name'].toString().trim().toLowerCase() == normName) {
          return item;
        }
      }
      if (deviceEdgeCount == 1 &&
          rawInterfaces.length == 1 &&
          ifId == null &&
          (normName == null ||
              normName.isEmpty ||
              rawInterfaces.first['name'] == null)) {
        return rawInterfaces.first;
      }
      return null;
    }

    var changed = false;
    final nextEdges = state.edges
        .map((edge) {
          if (edge.sourceDeviceId != deviceId &&
              edge.targetDeviceId != deviceId) {
            return edge;
          }

          if (edge.sourceDeviceId == deviceId) {
            final m = matchEndpoint(
              edge.sourceInterfaceId,
              edge.sourceInterfaceName,
            );
            if (m != null) {
              final nextIn = parseDoubleOrNull(m['inBps']);
              final nextOut = parseDoubleOrNull(m['outBps']);
              changed = true;
              return edge.withTrafficUpdate(
                inBps: nextIn,
                outBps: nextOut,
                sourceInterfaceStatus: m['operStatus']?.toString(),
                sourceInterfaceSpeed: parseDoubleOrNull(m['speed']),
                trafficFromSource:
                    (nextIn != null || nextOut != null) ||
                    edge.trafficFromSource,
              );
            }
          }
          if (edge.targetDeviceId == deviceId) {
            final m = matchEndpoint(
              edge.targetInterfaceId,
              edge.targetInterfaceName,
            );
            if (m != null) {
              final canUpdateTraffic =
                  edge.sourceInterfaceId == null || !edge.trafficFromSource;
              changed = true;
              return edge.withTrafficUpdate(
                inBps: canUpdateTraffic ? parseDoubleOrNull(m['inBps']) : null,
                outBps: canUpdateTraffic
                    ? parseDoubleOrNull(m['outBps'])
                    : null,
                targetInterfaceStatus: m['operStatus']?.toString(),
                targetInterfaceSpeed: parseDoubleOrNull(m['speed']),
              );
            }
          }
          return edge;
        })
        .toList(growable: false);

    if (changed) {
      state = state.copyWith(edges: nextEdges);
    }
  }

  void applyInterfaceChange(String type, Map<String, dynamic> data) {
    final interfaceId = parseIntOrNull(data['interfaceId']);
    if (interfaceId == null) return;
    final status = type == 'interface:status_change'
        ? data['currentStatus']?.toString()
        : null;
    final speed = parseDoubleOrNull(data['currentSpeed']);
    var changed = false;
    final nextEdges = state.edges
        .map((edge) {
          if (edge.sourceInterfaceId == interfaceId) {
            changed = true;
            return edge.withTrafficUpdate(
              sourceInterfaceStatus: status,
              sourceInterfaceSpeed: speed,
            );
          }
          if (edge.targetInterfaceId == interfaceId) {
            changed = true;
            return edge.withTrafficUpdate(
              targetInterfaceStatus: status,
              targetInterfaceSpeed: speed,
            );
          }
          return edge;
        })
        .toList(growable: false);
    if (changed) state = state.copyWith(edges: nextEdges);
  }

  void applyLayoutUpdated(Map<String, dynamic> data) {
    final eventSite = parseIntOrNull(data['siteId']);
    if (eventSite != state.siteId) return;
    state = state.copyWith(
      remoteLayoutRevision: state.remoteLayoutRevision + 1,
    );
  }
}

final topologyProvider = NotifierProvider<TopologyNotifier, TopologyState>(
  TopologyNotifier.new,
);

/// Calcula coordenadas determinísticas de layout para o grafo de topologia.
Map<int, TopologyPosition> computeTopologyLayout(
  List<TopologyNodeModel> nodes,
  List<TopologyEdgeModel> edges,
  TopologyLayoutMode mode, {
  Map<int, TopologyPosition> existing = const <int, TopologyPosition>{},
}) {
  if (nodes.isEmpty) return const <int, TopologyPosition>{};
  final result = <int, TopologyPosition>{};
  const double canvasOriginX = 120;
  const double canvasOriginY = 100;

  if (existing.isNotEmpty) {
    for (final n in nodes) {
      if (existing.containsKey(n.id)) {
        result[n.id] = existing[n.id]!;
      }
    }
    if (result.length == nodes.length) return result;
  }

  switch (mode) {
    case TopologyLayoutMode.grid:
      final cols = math.max(1, math.sqrt(nodes.length).ceil());
      for (var i = 0; i < nodes.length; i++) {
        final col = i % cols;
        final row = i ~/ cols;
        result.putIfAbsent(
          nodes[i].id,
          () => TopologyPosition(
            canvasOriginX + col * 240.0,
            canvasOriginY + row * 170.0,
          ),
        );
      }
      return result;

    case TopologyLayoutMode.radial:
      const centerX = 700.0;
      const centerY = 500.0;
      final radius = math.max(220.0, nodes.length * 36.0);
      for (var i = 0; i < nodes.length; i++) {
        if (i == 0 && nodes.length > 2) {
          result.putIfAbsent(
            nodes[i].id,
            () => const TopologyPosition(centerX, centerY),
          );
        } else {
          final angle = (2 * math.pi * i) / math.max(1, nodes.length);
          result.putIfAbsent(
            nodes[i].id,
            () => TopologyPosition(
              centerX + radius * math.cos(angle),
              centerY + radius * math.sin(angle),
            ),
          );
        }
      }
      return result;

    case TopologyLayoutMode.force:
      // Inicialização circular + 40 iterações Fruchterman-Reingold determinísticas.
      final pos = <int, List<double>>{};
      final radius = math.max(200.0, nodes.length * 30.0);
      for (var i = 0; i < nodes.length; i++) {
        final angle = (2 * math.pi * i) / math.max(1, nodes.length);
        pos[nodes[i].id] = <double>[
          650 + radius * math.cos(angle),
          450 + radius * math.sin(angle),
        ];
      }
      const double k = 220.0;
      for (var iter = 0; iter < 35; iter++) {
        final disp = <int, List<double>>{
          for (final n in nodes) n.id: <double>[0, 0],
        };
        for (var i = 0; i < nodes.length; i++) {
          for (var j = i + 1; j < nodes.length; j++) {
            final u = nodes[i].id;
            final v = nodes[j].id;
            final dx = pos[u]![0] - pos[v]![0];
            final dy = pos[u]![1] - pos[v]![1];
            final dist = math.max(1.0, math.sqrt(dx * dx + dy * dy));
            final rep = (k * k) / dist;
            final fx = (dx / dist) * rep;
            final fy = (dy / dist) * rep;
            disp[u]![0] += fx;
            disp[u]![1] += fy;
            disp[v]![0] -= fx;
            disp[v]![1] -= fy;
          }
        }
        for (final e in edges) {
          if (!pos.containsKey(e.sourceDeviceId) ||
              !pos.containsKey(e.targetDeviceId)) {
            continue;
          }
          final dx = pos[e.sourceDeviceId]![0] - pos[e.targetDeviceId]![0];
          final dy = pos[e.sourceDeviceId]![1] - pos[e.targetDeviceId]![1];
          final dist = math.max(1.0, math.sqrt(dx * dx + dy * dy));
          final attr = (dist * dist) / k;
          final fx = (dx / dist) * attr * 0.4;
          final fy = (dy / dist) * attr * 0.4;
          disp[e.sourceDeviceId]![0] -= fx;
          disp[e.sourceDeviceId]![1] -= fy;
          disp[e.targetDeviceId]![0] += fx;
          disp[e.targetDeviceId]![1] += fy;
        }
        final temp = 60.0 * (1.0 - iter / 36.0);
        for (final n in nodes) {
          final d = disp[n.id]!;
          final len = math.max(1.0, math.sqrt(d[0] * d[0] + d[1] * d[1]));
          pos[n.id]![0] = (pos[n.id]![0] + (d[0] / len) * math.min(len, temp))
              .clamp(80, 1600);
          pos[n.id]![1] = (pos[n.id]![1] + (d[1] / len) * math.min(len, temp))
              .clamp(80, 1100);
        }
      }
      for (final n in nodes) {
        result.putIfAbsent(
          n.id,
          () => TopologyPosition(pos[n.id]![0], pos[n.id]![1]),
        );
      }
      return result;

    case TopologyLayoutMode.hierarchical:
      int tierOf(TopologyNodeModel n) {
        if (n.isSystem) return 0;
        switch (n.type.toLowerCase()) {
          case 'firewall':
          case 'router':
            return 0;
          case 'switch':
            return 1;
          case 'access_point':
          case 'server':
          case 'nas':
            return 2;
          default:
            return 3;
        }
      }

      final layers = <int, List<TopologyNodeModel>>{};
      for (final n in nodes) {
        layers.putIfAbsent(tierOf(n), () => <TopologyNodeModel>[]).add(n);
      }
      final sortedKeys = layers.keys.toList()..sort();
      for (var rowIdx = 0; rowIdx < sortedKeys.length; rowIdx++) {
        final rowNodes = layers[sortedKeys[rowIdx]]!;
        final totalWidth = (rowNodes.length - 1) * 240.0;
        final startX = math.max(canvasOriginX, 680.0 - totalWidth / 2);
        for (var col = 0; col < rowNodes.length; col++) {
          result.putIfAbsent(
            rowNodes[col].id,
            () => TopologyPosition(
              startX + col * 240.0,
              canvasOriginY + rowIdx * 200.0,
            ),
          );
        }
      }
      return result;
  }
}
