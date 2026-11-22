import 'dart:convert';
import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_geometry.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_layouts.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_presentation.dart';
import 'package:shared_preferences/shared_preferences.dart';

// ── Caminhos e Vizinhança (paridade com `path.ts` e backend `path.rs`) ───────

/// Resultado de busca de caminho entre dois nós na topologia.
@immutable
class TopologyPathResult {
  const TopologyPathResult({required this.nodeIds, required this.edgeIds});

  factory TopologyPathResult.fromModel(TopologyPathModel model) {
    return TopologyPathResult(
      nodeIds: List<int>.unmodifiable(model.deviceIds),
      edgeIds: List<int>.unmodifiable(model.edgeIds),
    );
  }

  final List<int> nodeIds;
  final List<int> edgeIds;

  /// Número de saltos (arestas) do caminho.
  int get hopCount => edgeIds.length;
  bool get isEmpty => nodeIds.isEmpty;
  bool get isSingleNode => nodeIds.length == 1 && edgeIds.isEmpty;

  Set<int> get nodeIdSet => nodeIds.toSet();
  Set<int> get edgeIdSet => edgeIds.toSet();

  TopologyPathModel toModel() =>
      TopologyPathModel(deviceIds: nodeIds, edgeIds: edgeIds);

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is TopologyPathResult &&
        listEquals(other.nodeIds, nodeIds) &&
        listEquals(other.edgeIds, edgeIds);
  }

  @override
  int get hashCode =>
      Object.hash(Object.hashAll(nodeIds), Object.hashAll(edgeIds));
}

/// Vizinho conectado a um nó com o respectivo enlace.
@immutable
class TopologyNeighborLink {
  const TopologyNeighborLink({
    required this.neighborId,
    required this.edge,
    required this.isOutgoing,
  });

  final int neighborId;
  final TopologyEdgeModel edge;
  final bool isOutgoing;
}

class _PathStep {
  const _PathStep({
    required this.neighbor,
    required this.confidence,
    required this.edgeId,
  });

  final int neighbor;
  final int confidence;
  final int edgeId;
}

int _pathConfidence(TopologyEdgeModel edge) {
  if (edge.confirmed) return 100;
  final pct = edgeConfidencePercent(edge.confidence) ?? 0;
  return pct.clamp(0, 100);
}

int _comparePathCost((int, int) a, (int, int) b) {
  final hopsCmp = a.$1.compareTo(b.$1);
  if (hopsCmp != 0) return hopsCmp;
  return a.$2.compareTo(b.$2);
}

class _MinPathQueue {
  final List<({(int, int) cost, int device})> _items =
      <({(int, int) cost, int device})>[];

  bool get isNotEmpty => _items.isNotEmpty;

  bool _less(int i, int j) {
    final x = _items[i];
    final y = _items[j];
    final c = _comparePathCost(x.cost, y.cost);
    if (c != 0) return c < 0;
    return x.device < y.device;
  }

  void _swap(int i, int j) {
    final tmp = _items[i];
    _items[i] = _items[j];
    _items[j] = tmp;
  }

  void push((int, int) cost, int device) {
    _items.add((cost: cost, device: device));
    var i = _items.length - 1;
    while (i > 0) {
      final parent = (i - 1) >> 1;
      if (!_less(i, parent)) break;
      _swap(i, parent);
      i = parent;
    }
  }

  ({(int, int) cost, int device}) pop() {
    final top = _items.first;
    final last = _items.removeLast();
    if (_items.isNotEmpty) {
      _items[0] = last;
      var i = 0;
      while (true) {
        final left = 2 * i + 1;
        final right = left + 1;
        var smallest = i;
        if (left < _items.length && _less(left, smallest)) smallest = left;
        if (right < _items.length && _less(right, smallest)) smallest = right;
        if (smallest == i) break;
        _swap(i, smallest);
        i = smallest;
      }
    }
    return top;
  }
}

/// Menor caminho entre `fromNodeId` e `toNodeId` no grafo não-direcionado.
///
/// Usa a mesma régua determinística do backend (`services/topology/path.rs`):
/// menos saltos primeiro; em empate, maior confiança somada (confirmado = 100);
/// no empate restante, menor ID. Retorna `null` se não houver caminho.
TopologyPathResult? findShortestPath(
  List<TopologyEdgeModel> edges,
  int fromNodeId,
  int toNodeId,
) {
  if (fromNodeId == toNodeId) {
    return TopologyPathResult(
      nodeIds: List<int>.unmodifiable(<int>[fromNodeId]),
      edgeIds: const <int>[],
    );
  }

  final graph = <int, List<_PathStep>>{};
  for (final edge in edges) {
    final a = edge.sourceDeviceId;
    final b = edge.targetDeviceId;
    if (a == b) continue;
    final conf = _pathConfidence(edge);
    graph
        .putIfAbsent(a, () => <_PathStep>[])
        .add(_PathStep(neighbor: b, confidence: conf, edgeId: edge.id));
    graph
        .putIfAbsent(b, () => <_PathStep>[])
        .add(_PathStep(neighbor: a, confidence: conf, edgeId: edge.id));
  }
  for (final list in graph.values) {
    list.sort((x, y) {
      final nbCmp = x.neighbor.compareTo(y.neighbor);
      if (nbCmp != 0) return nbCmp;
      final confCmp = y.confidence.compareTo(x.confidence);
      if (confCmp != 0) return confCmp;
      return x.edgeId.compareTo(y.edgeId);
    });
  }

  final best = <int, (int, int)>{fromNodeId: (0, 0)};
  final prev = <int, ({int device, int edgeId})>{};
  final queue = _MinPathQueue()..push((0, 0), fromNodeId);

  while (queue.isNotEmpty) {
    final entry = queue.pop();
    final known = best[entry.device];
    if (known != null && _comparePathCost(known, entry.cost) < 0) continue;
    if (entry.device == toNodeId) break;

    for (final step in graph[entry.device] ?? const <_PathStep>[]) {
      final nextCost = (
        entry.cost.$1 + 1,
        entry.cost.$2 + (100 - step.confidence),
      );
      final seen = best[step.neighbor];
      if (seen == null || _comparePathCost(nextCost, seen) < 0) {
        best[step.neighbor] = nextCost;
        prev[step.neighbor] = (device: entry.device, edgeId: step.edgeId);
        queue.push(nextCost, step.neighbor);
      }
    }
  }

  if (!prev.containsKey(toNodeId)) return null;

  final nodeIds = <int>[toNodeId];
  final edgeIds = <int>[];
  var cursor = toNodeId;
  while (cursor != fromNodeId) {
    final step = prev[cursor];
    if (step == null) return null;
    edgeIds.add(step.edgeId);
    nodeIds.add(step.device);
    cursor = step.device;
  }

  return TopologyPathResult(
    nodeIds: List<int>.unmodifiable(nodeIds.reversed),
    edgeIds: List<int>.unmodifiable(edgeIds.reversed),
  );
}

/// Traça o caminho upstream de `nodeId` até a raiz/gateway do seu componente.
TopologyPathResult traceUpstreamToRoot(
  List<TopologyNodeModel> nodes,
  List<TopologyEdgeModel> edges,
  int nodeId,
) {
  final roots = chooseTopologyRoots(nodes, edges);
  TopologyPathResult? best;
  for (final rootId in roots) {
    final candidate = findShortestPath(edges, nodeId, rootId);
    if (candidate == null) continue;
    if (best == null || candidate.hopCount < best.hopCount) {
      best = candidate;
    }
  }
  if (best != null) return best;

  // Fallback pela cadeia explícita de `parentId` caso não existam arestas até a raiz.
  final byId = <int, TopologyNodeModel>{for (final n in nodes) n.id: n};
  if (byId.containsKey(nodeId)) {
    final chain = <int>[nodeId];
    final visited = <int>{nodeId};
    var cur = byId[nodeId]!;
    while (cur.parentId != null &&
        byId.containsKey(cur.parentId) &&
        visited.add(cur.parentId!)) {
      chain.add(cur.parentId!);
      cur = byId[cur.parentId]!;
    }
    if (chain.length > 1) {
      return TopologyPathResult(
        nodeIds: List<int>.unmodifiable(chain),
        edgeIds: const <int>[],
      );
    }
  }

  return TopologyPathResult(
    nodeIds: List<int>.unmodifiable(<int>[nodeId]),
    edgeIds: const <int>[],
  );
}

/// Lista todos os vizinhos conectados diretamente a `nodeId`, ordenados por
/// relevância (enlaces ativos primeiro, maior velocidade/tráfego e depois ID).
List<TopologyNeighborLink> connectedNeighbors(
  List<TopologyEdgeModel> edges,
  int nodeId,
) {
  final out = <TopologyNeighborLink>[];
  for (final edge in edges) {
    if (edge.sourceDeviceId == nodeId && edge.targetDeviceId != nodeId) {
      out.add(
        TopologyNeighborLink(
          neighborId: edge.targetDeviceId,
          edge: edge,
          isOutgoing: true,
        ),
      );
    } else if (edge.targetDeviceId == nodeId && edge.sourceDeviceId != nodeId) {
      out.add(
        TopologyNeighborLink(
          neighborId: edge.sourceDeviceId,
          edge: edge,
          isOutgoing: false,
        ),
      );
    }
  }

  out.sort((a, b) {
    final sa = classifyEdgeStatus(a.edge) == TopologyEdgeStatus.down ? 1 : 0;
    final sb = classifyEdgeStatus(b.edge) == TopologyEdgeStatus.down ? 1 : 0;
    if (sa != sb) return sa.compareTo(sb);
    final speedA = resolveEdgeSpeedBps(a.edge) ?? 0.0;
    final speedB = resolveEdgeSpeedBps(b.edge) ?? 0.0;
    final speedCmp = speedB.compareTo(speedA);
    if (speedCmp != 0) return speedCmp;
    final idCmp = a.neighborId.compareTo(b.neighborId);
    if (idCmp != 0) return idCmp;
    return a.edge.id.compareTo(b.edge.id);
  });
  return List<TopologyNeighborLink>.unmodifiable(out);
}

// ── Filtros & Preferências Visuais ──────────────────────────────────────────

/// Como elementos que não correspondem ao filtro aparecem no canvas.
enum TopologyFilterDisplayMode {
  dim(label: 'Esmaecer'),
  hide(label: 'Ocultar');

  const TopologyFilterDisplayMode({required this.label});

  final String label;

  static TopologyFilterDisplayMode fromStorageKey(String? raw) {
    return raw == 'hide'
        ? TopologyFilterDisplayMode.hide
        : TopologyFilterDisplayMode.dim;
  }
}

/// Especificação imutável de filtros de dados e preferências visuais do mapa.
@immutable
class TopologyFilterSpec {
  const TopologyFilterSpec({
    this.search = '',
    this.siteId,
    this.statuses = const <TopologyNodeStatus>{},
    this.types = const <String>{},
    this.linkKinds = const <TopologyLinkKind>{},
    this.onlyProblems = false,
    this.hideIsolated = false,
    this.filterDisplayMode = TopologyFilterDisplayMode.dim,
    this.overlayMode = TopologyOverlayMode.status,
    this.layoutMode = TopologyLayoutKind.hierarchical,
    this.edgeStyle = TopologyEdgeStyle.curved,
    this.showLabels = true,
    this.showIpLabels = true,
    this.showHealthBadges = true,
    this.showTrafficFlow = true,
    this.showMinimap = false,
    this.showClusterBoxes = true,
    this.snapToGrid = false,
  });

  final String search;
  final int? siteId;
  final Set<TopologyNodeStatus> statuses;
  final Set<String> types;
  final Set<TopologyLinkKind> linkKinds;
  final bool onlyProblems;
  final bool hideIsolated;
  final TopologyFilterDisplayMode filterDisplayMode;
  final TopologyOverlayMode overlayMode;
  final TopologyLayoutKind layoutMode;
  final TopologyEdgeStyle edgeStyle;
  final bool showLabels;
  final bool showIpLabels;
  final bool showHealthBadges;
  final bool showTrafficFlow;
  final bool showMinimap;
  final bool showClusterBoxes;
  final bool snapToGrid;

  /// Contagem de filtros de dados ativos (para badge no botão de filtros).
  int get activeFilterCount {
    var count = 0;
    if (search.trim().isNotEmpty) count += 1;
    if (siteId != null) count += 1;
    count += statuses.length;
    count += types.length;
    count += linkKinds.length;
    if (onlyProblems) count += 1;
    if (hideIsolated) count += 1;
    return count;
  }

  bool get hasActiveFilters => activeFilterCount > 0;

  TopologyFilterSpec copyWith({
    String? search,
    int? siteId,
    bool clearSiteId = false,
    Set<TopologyNodeStatus>? statuses,
    Set<String>? types,
    Set<TopologyLinkKind>? linkKinds,
    bool? onlyProblems,
    bool? hideIsolated,
    TopologyFilterDisplayMode? filterDisplayMode,
    TopologyOverlayMode? overlayMode,
    TopologyLayoutKind? layoutMode,
    TopologyEdgeStyle? edgeStyle,
    bool? showLabels,
    bool? showIpLabels,
    bool? showHealthBadges,
    bool? showTrafficFlow,
    bool? showMinimap,
    bool? showClusterBoxes,
    bool? snapToGrid,
  }) {
    return TopologyFilterSpec(
      search: search ?? this.search,
      siteId: clearSiteId ? null : (siteId ?? this.siteId),
      statuses: statuses != null
          ? Set<TopologyNodeStatus>.unmodifiable(statuses)
          : this.statuses,
      types: types != null ? Set<String>.unmodifiable(types) : this.types,
      linkKinds: linkKinds != null
          ? Set<TopologyLinkKind>.unmodifiable(linkKinds)
          : this.linkKinds,
      onlyProblems: onlyProblems ?? this.onlyProblems,
      hideIsolated: hideIsolated ?? this.hideIsolated,
      filterDisplayMode: filterDisplayMode ?? this.filterDisplayMode,
      overlayMode: overlayMode ?? this.overlayMode,
      layoutMode: layoutMode ?? this.layoutMode,
      edgeStyle: edgeStyle ?? this.edgeStyle,
      showLabels: showLabels ?? this.showLabels,
      showIpLabels: showIpLabels ?? this.showIpLabels,
      showHealthBadges: showHealthBadges ?? this.showHealthBadges,
      showTrafficFlow: showTrafficFlow ?? this.showTrafficFlow,
      showMinimap: showMinimap ?? this.showMinimap,
      showClusterBoxes: showClusterBoxes ?? this.showClusterBoxes,
      snapToGrid: snapToGrid ?? this.snapToGrid,
    );
  }

  /// Limpa apenas os filtros de dados preservando preferências visuais e layout.
  TopologyFilterSpec clearDataFilters() {
    return copyWith(
      search: '',
      clearSiteId: true,
      statuses: const <TopologyNodeStatus>{},
      types: const <String>{},
      linkKinds: const <TopologyLinkKind>{},
      onlyProblems: false,
      hideIsolated: false,
    );
  }

  Map<String, dynamic> toJson() {
    return <String, dynamic>{
      'siteId': siteId,
      'statuses': statuses.map((s) => s.name).toList(growable: false),
      'types': types.toList(growable: false),
      'linkKinds': linkKinds.map((k) => k.name).toList(growable: false),
      'onlyProblems': onlyProblems,
      'hideIsolated': hideIsolated,
      'filterDisplayMode': filterDisplayMode.name,
      'overlayMode': overlayMode.name,
      'layoutMode': layoutMode.name,
      'edgeStyle': edgeStyle.name,
      'showLabels': showLabels,
      'showIpLabels': showIpLabels,
      'showHealthBadges': showHealthBadges,
      'showTrafficFlow': showTrafficFlow,
      'showMinimap': showMinimap,
      'showClusterBoxes': showClusterBoxes,
      'snapToGrid': snapToGrid,
    };
  }

  factory TopologyFilterSpec.fromJson(Map<String, dynamic> json) {
    final rawStatuses = json['statuses'];
    final parsedStatuses = <TopologyNodeStatus>{};
    if (rawStatuses is List) {
      for (final item in rawStatuses) {
        final s = TopologyNodeStatus.tryFromStorageKey(item?.toString());
        if (s != null) parsedStatuses.add(s);
      }
    }

    final rawTypes = json['types'];
    final parsedTypes = <String>{};
    if (rawTypes is List) {
      for (final item in rawTypes) {
        final t = item?.toString().trim();
        if (t != null && t.isNotEmpty) parsedTypes.add(t);
      }
    }

    final rawKinds = json['linkKinds'];
    final parsedKinds = <TopologyLinkKind>{};
    if (rawKinds is List) {
      for (final item in rawKinds) {
        final k = TopologyLinkKind.tryFromStorageKey(item?.toString());
        if (k != null) parsedKinds.add(k);
      }
    }

    final rawSite = json['siteId'];
    final siteId = rawSite is int
        ? rawSite
        : (rawSite is num ? rawSite.toInt() : null);

    return TopologyFilterSpec(
      siteId: siteId,
      statuses: Set<TopologyNodeStatus>.unmodifiable(parsedStatuses),
      types: Set<String>.unmodifiable(parsedTypes),
      linkKinds: Set<TopologyLinkKind>.unmodifiable(parsedKinds),
      onlyProblems: json['onlyProblems'] == true,
      hideIsolated: json['hideIsolated'] == true,
      filterDisplayMode: TopologyFilterDisplayMode.fromStorageKey(
        json['filterDisplayMode'] as String?,
      ),
      overlayMode: TopologyOverlayMode.fromStorageKey(
        json['overlayMode'] as String?,
      ),
      layoutMode: TopologyLayoutKind.fromStorageKey(
        json['layoutMode'] as String?,
      ),
      edgeStyle: TopologyEdgeStyle.fromStorageKey(json['edgeStyle'] as String?),
      showLabels: json['showLabels'] != false,
      showIpLabels: json['showIpLabels'] != false,
      showHealthBadges: json['showHealthBadges'] != false,
      showTrafficFlow: json['showTrafficFlow'] != false,
      showMinimap: json['showMinimap'] == true,
      showClusterBoxes: json['showClusterBoxes'] != false,
      snapToGrid: json['snapToGrid'] == true,
    );
  }

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is TopologyFilterSpec &&
        other.search == search &&
        other.siteId == siteId &&
        setEquals(other.statuses, statuses) &&
        setEquals(other.types, types) &&
        setEquals(other.linkKinds, linkKinds) &&
        other.onlyProblems == onlyProblems &&
        other.hideIsolated == hideIsolated &&
        other.filterDisplayMode == filterDisplayMode &&
        other.overlayMode == overlayMode &&
        other.layoutMode == layoutMode &&
        other.edgeStyle == edgeStyle &&
        other.showLabels == showLabels &&
        other.showIpLabels == showIpLabels &&
        other.showHealthBadges == showHealthBadges &&
        other.showTrafficFlow == showTrafficFlow &&
        other.showMinimap == showMinimap &&
        other.showClusterBoxes == showClusterBoxes &&
        other.snapToGrid == snapToGrid;
  }

  @override
  int get hashCode => Object.hash(
    search,
    siteId,
    Object.hashAllUnordered(statuses),
    Object.hashAllUnordered(types),
    Object.hashAllUnordered(linkKinds),
    onlyProblems,
    hideIsolated,
    filterDisplayMode,
    overlayMode,
    layoutMode,
    edgeStyle,
    showLabels,
    showIpLabels,
    showHealthBadges,
    showTrafficFlow,
    showMinimap,
    showClusterBoxes,
    snapToGrid,
  );
}

/// Resultado da aplicação de filtros sobre nós e enlaces.
@immutable
class TopologyFilterResult {
  const TopologyFilterResult({
    required this.visibleNodes,
    required this.visibleEdges,
    required this.matchedNodeIds,
    required this.matchedEdgeIds,
    required this.dimmedNodeIds,
    required this.dimmedEdgeIds,
    required this.isFiltered,
  });

  final List<TopologyNodeModel> visibleNodes;
  final List<TopologyEdgeModel> visibleEdges;
  final Set<int> matchedNodeIds;
  final Set<int> matchedEdgeIds;
  final Set<int> dimmedNodeIds;
  final Set<int> dimmedEdgeIds;
  final bool isFiltered;
}

bool _isProblemEdge(TopologyEdgeModel edge) {
  if (classifyEdgeStatus(edge) == TopologyEdgeStatus.down) return true;
  final level = classifyUtilization(resolveEdgeUtilization(edge));
  return level == TopologyUtilizationLevel.high ||
      level == TopologyUtilizationLevel.critical;
}

bool _isProblemNode(TopologyNodeModel node) {
  final status = classifyNodeStatus(node.status);
  if (status == TopologyNodeStatus.offline ||
      status == TopologyNodeStatus.warning) {
    return true;
  }
  return node.monitorsDown > 0;
}

/// Aplica `TopologyFilterSpec` sobre nós e enlaces sem mutar as listas de entrada.
TopologyFilterResult applyTopologyFilter({
  required List<TopologyNodeModel> nodes,
  required List<TopologyEdgeModel> edges,
  required TopologyFilterSpec spec,
  Map<int, String>? macByNodeId,
}) {
  final connectedIds = <int>{
    for (final e in edges) ...<int>[e.sourceDeviceId, e.targetDeviceId],
  };

  // Site e "ocultar isolados" sempre recortam o universo base visível.
  final baseNodes = nodes
      .where((n) {
        if (spec.siteId != null && n.siteId != spec.siteId) return false;
        if (spec.hideIsolated && !connectedIds.contains(n.id)) return false;
        return true;
      })
      .toList(growable: false);

  final baseNodeIds = baseNodes.map((n) => n.id).toSet();
  final baseEdges = edges
      .where(
        (e) =>
            baseNodeIds.contains(e.sourceDeviceId) &&
            baseNodeIds.contains(e.targetDeviceId),
      )
      .toList(growable: false);

  final hasMatchFilters =
      spec.search.trim().isNotEmpty ||
      spec.statuses.isNotEmpty ||
      spec.types.isNotEmpty ||
      spec.linkKinds.isNotEmpty ||
      spec.onlyProblems;

  if (!hasMatchFilters) {
    return TopologyFilterResult(
      visibleNodes: List<TopologyNodeModel>.unmodifiable(baseNodes),
      visibleEdges: List<TopologyEdgeModel>.unmodifiable(baseEdges),
      matchedNodeIds: Set<int>.unmodifiable(baseNodeIds),
      matchedEdgeIds: Set<int>.unmodifiable(baseEdges.map((e) => e.id)),
      dimmedNodeIds: const <int>{},
      dimmedEdgeIds: const <int>{},
      isFiltered: spec.hasActiveFilters,
    );
  }

  final searchHits = spec.search.trim().isNotEmpty
      ? searchTopologyNodes(
          baseNodes,
          spec.search,
          limit: baseNodes.length,
          macByNodeId: macByNodeId,
        ).map((h) => h.node.id).toSet()
      : null;

  final problemEdges = <int>{};
  final nodesWithProblemEdge = <int>{};
  if (spec.onlyProblems) {
    for (final edge in baseEdges) {
      if (!_isProblemEdge(edge)) continue;
      problemEdges.add(edge.id);
      nodesWithProblemEdge.add(edge.sourceDeviceId);
      nodesWithProblemEdge.add(edge.targetDeviceId);
    }
  }

  final normalizedTypes = spec.types.map((t) => t.trim().toLowerCase()).toSet();

  final matchedNodeIds = <int>{};
  for (final node in baseNodes) {
    if (searchHits != null && !searchHits.contains(node.id)) continue;
    if (spec.statuses.isNotEmpty &&
        !spec.statuses.contains(classifyNodeStatus(node.status))) {
      continue;
    }
    if (normalizedTypes.isNotEmpty) {
      final rawType = node.type.trim().toLowerCase();
      final isSwitchMatch =
          normalizedTypes.contains('switch') && isUnmanagedSwitchNode(node);
      if (!normalizedTypes.contains(rawType) && !isSwitchMatch) continue;
    }
    if (spec.onlyProblems &&
        !_isProblemNode(node) &&
        !nodesWithProblemEdge.contains(node.id)) {
      continue;
    }
    matchedNodeIds.add(node.id);
  }

  final matchedEdgeIds = <int>{};
  for (final edge in baseEdges) {
    if (spec.linkKinds.isNotEmpty &&
        !spec.linkKinds.contains(classifyLinkKind(edge.linkType))) {
      continue;
    }
    final endpointsMatch =
        matchedNodeIds.contains(edge.sourceDeviceId) &&
        matchedNodeIds.contains(edge.targetDeviceId);
    if (spec.onlyProblems
        ? (problemEdges.contains(edge.id) || endpointsMatch)
        : endpointsMatch) {
      matchedEdgeIds.add(edge.id);
    }
  }

  if (spec.filterDisplayMode == TopologyFilterDisplayMode.hide) {
    final visibleNodes = baseNodes
        .where((n) => matchedNodeIds.contains(n.id))
        .toList(growable: false);
    final visibleEdges = baseEdges
        .where((e) => matchedEdgeIds.contains(e.id))
        .toList(growable: false);
    return TopologyFilterResult(
      visibleNodes: List<TopologyNodeModel>.unmodifiable(visibleNodes),
      visibleEdges: List<TopologyEdgeModel>.unmodifiable(visibleEdges),
      matchedNodeIds: Set<int>.unmodifiable(matchedNodeIds),
      matchedEdgeIds: Set<int>.unmodifiable(matchedEdgeIds),
      dimmedNodeIds: const <int>{},
      dimmedEdgeIds: const <int>{},
      isFiltered: true,
    );
  }

  final dimmedNodeIds = <int>{
    for (final n in baseNodes)
      if (!matchedNodeIds.contains(n.id)) n.id,
  };
  final dimmedEdgeIds = <int>{
    for (final e in baseEdges)
      if (!matchedEdgeIds.contains(e.id)) e.id,
  };

  return TopologyFilterResult(
    visibleNodes: List<TopologyNodeModel>.unmodifiable(baseNodes),
    visibleEdges: List<TopologyEdgeModel>.unmodifiable(baseEdges),
    matchedNodeIds: Set<int>.unmodifiable(matchedNodeIds),
    matchedEdgeIds: Set<int>.unmodifiable(matchedEdgeIds),
    dimmedNodeIds: Set<int>.unmodifiable(dimmedNodeIds),
    dimmedEdgeIds: Set<int>.unmodifiable(dimmedEdgeIds),
    isFiltered: true,
  );
}

// ── Busca ranqueada com tolerância a acentos e termos parciais ──────────────

/// Campo que mais pontuou na busca de um nó.
enum TopologySearchField {
  name(label: 'Nome', weight: 1.0),
  ip(label: 'IP', weight: 0.95),
  mac(label: 'MAC', weight: 0.90),
  vendor(label: 'Fabricante', weight: 0.60),
  model(label: 'Modelo', weight: 0.60),
  type(label: 'Tipo', weight: 0.50),
  site(label: 'Site', weight: 0.40);

  const TopologySearchField({required this.label, required this.weight});

  final String label;
  final double weight;
}

/// Resultado ranqueado da busca de nós.
@immutable
class TopologySearchHit {
  const TopologySearchHit({
    required this.node,
    required this.score,
    required this.matchedField,
  });

  final TopologyNodeModel node;
  final double score;
  final TopologySearchField matchedField;
}

const Map<String, String> _diacriticMap = <String, String>{
  'á': 'a',
  'à': 'a',
  'â': 'a',
  'ã': 'a',
  'ä': 'a',
  'é': 'e',
  'è': 'e',
  'ê': 'e',
  'ë': 'e',
  'í': 'i',
  'ì': 'i',
  'î': 'i',
  'ï': 'i',
  'ó': 'o',
  'ò': 'o',
  'ô': 'o',
  'õ': 'o',
  'ö': 'o',
  'ú': 'u',
  'ù': 'u',
  'û': 'u',
  'ü': 'u',
  'ç': 'c',
  'ñ': 'n',
};

String _normalizeSearchText(String text) {
  final lower = text.toLowerCase().trim();
  if (lower.isEmpty) return '';
  final buf = StringBuffer();
  for (var i = 0; i < lower.length; i++) {
    final ch = lower[i];
    buf.write(_diacriticMap[ch] ?? ch);
  }
  return buf.toString();
}

int _subsequenceScore(String query, String text) {
  var qi = 0;
  var gaps = 0;
  var last = -1;
  for (var ti = 0; ti < text.length && qi < query.length; ti++) {
    if (text.codeUnitAt(ti) == query.codeUnitAt(qi)) {
      if (last >= 0) gaps += ti - last - 1;
      last = ti;
      qi += 1;
    }
  }
  if (qi < query.length || gaps > query.length) return 0;
  return math.max(1, 20 - gaps);
}

int _termScore(String term, String field) {
  if (term.isEmpty || field.isEmpty) return 0;
  if (field == term) return 100;
  if (field.startsWith(term)) return 80;
  final words = field.split(RegExp(r'[\s\-_./:]+'));
  if (words.any((w) => w.startsWith(term))) return 65;
  if (field.contains(term)) return 50;
  return _subsequenceScore(term, field);
}

/// Busca ranqueada por nome, IP, MAC (opcional), fabricante, modelo, tipo e site.
///
/// Suporta múltiplos termos parciais (ex.: `"sw cor"` encontra `"Switch Core"`)
/// e ignora acentos em português.
List<TopologySearchHit> searchTopologyNodes(
  List<TopologyNodeModel> nodes,
  String query, {
  int limit = 15,
  Map<int, String>? macByNodeId,
}) {
  final normalized = _normalizeSearchText(query);
  if (normalized.isEmpty) return const <TopologySearchHit>[];
  final terms = normalized
      .split(RegExp(r'\s+'))
      .where((t) => t.isNotEmpty)
      .toList(growable: false);
  if (terms.isEmpty) return const <TopologySearchHit>[];

  final hits = <TopologySearchHit>[];
  for (final node in nodes) {
    final fields = <(TopologySearchField, String)>[
      (TopologySearchField.name, _normalizeSearchText(node.name)),
      (TopologySearchField.ip, _normalizeSearchText(node.ipAddress ?? '')),
      if (macByNodeId != null && macByNodeId[node.id] != null)
        (TopologySearchField.mac, _normalizeSearchText(macByNodeId[node.id]!)),
      (TopologySearchField.vendor, _normalizeSearchText(node.vendor ?? '')),
      (TopologySearchField.model, _normalizeSearchText(node.model ?? '')),
      (
        TopologySearchField.type,
        _normalizeSearchText('${topologyNodeTypeLabel(node)} ${node.type}'),
      ),
      (TopologySearchField.site, _normalizeSearchText(node.siteName ?? '')),
    ];

    var totalScore = 0.0;
    TopologySearchField? bestField;
    var bestFieldScore = -1.0;
    var allMatched = true;

    for (final term in terms) {
      var termBest = 0.0;
      for (final (field, value) in fields) {
        final raw = _termScore(term, value);
        // IP/MAC exigem trecho contínuo para evitar coincidências numéricas aleatórias.
        final isIdentifier =
            field == TopologySearchField.ip || field == TopologySearchField.mac;
        final adjusted = (isIdentifier && raw < 50 ? 0 : raw) * field.weight;
        if (adjusted > termBest) termBest = adjusted;
        if (adjusted > bestFieldScore) {
          bestFieldScore = adjusted;
          bestField = field;
        }
      }
      if (termBest <= 0) {
        allMatched = false;
        break;
      }
      totalScore += termBest;
    }

    if (!allMatched || bestField == null) continue;
    hits.add(
      TopologySearchHit(
        node: node,
        score: totalScore / terms.length,
        matchedField: bestField,
      ),
    );
  }

  hits.sort((a, b) {
    final cmp = b.score.compareTo(a.score);
    if (cmp != 0) return cmp;
    return compareTopologyNodes(a.node, b.node);
  });

  if (limit > 0 && hits.length > limit) {
    return List<TopologySearchHit>.unmodifiable(hits.sublist(0, limit));
  }
  return List<TopologySearchHit>.unmodifiable(hits);
}

// ── Persistência local de preferências visuais (`SharedPreferences`) ────────

/// Persistência resiliente das preferências visuais e filtros da topologia.
class TopologyPrefsStorage {
  const TopologyPrefsStorage._();

  static const String storageKey = 'netmonitor.topology.prefs.v3';

  /// Carrega as preferências salvas; devolve o padrão seguro em caso de erro.
  static Future<TopologyFilterSpec> load({SharedPreferences? prefs}) async {
    try {
      final store = prefs ?? await SharedPreferences.getInstance();
      final raw = store.getString(storageKey);
      if (raw == null || raw.trim().isEmpty) {
        return const TopologyFilterSpec();
      }
      final decoded = jsonDecode(raw);
      if (decoded is! Map) return const TopologyFilterSpec();
      return TopologyFilterSpec.fromJson(Map<String, dynamic>.from(decoded));
    } catch (_) {
      return const TopologyFilterSpec();
    }
  }

  /// Salva as preferências no armazenamento local sem lançar exceções de I/O.
  static Future<void> save(
    TopologyFilterSpec spec, {
    SharedPreferences? prefs,
  }) async {
    try {
      final store = prefs ?? await SharedPreferences.getInstance();
      await store.setString(storageKey, jsonEncode(spec.toJson()));
    } catch (_) {
      // Armazenamento indisponível (ex.: aba privada): segue em memória.
    }
  }

  /// Remove as preferências persistidas.
  static Future<void> clear({SharedPreferences? prefs}) async {
    try {
      final store = prefs ?? await SharedPreferences.getInstance();
      await store.remove(storageKey);
    } catch (_) {}
  }
}
