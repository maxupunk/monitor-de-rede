import 'dart:math' as math;

import 'package:flutter/material.dart'
    show EdgeInsets, IconData, Icons, Matrix4, Offset, Rect, Size, immutable;
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_presentation.dart';

/// Dimensão padrão de um cartão de nó no canvas (usada em layouts e geometria).
const Size kDefaultTopologyNodeSize = Size(204, 96);

/// Limite de escala abaixo do qual os nós entram em modo macro (LOD circular).
const double kTopologyMacroZoomThreshold = 0.55;

/// Diâmetro e raio do ícone circular exibido no modo macro (`scale < 0.55`).
const double kTopologyMacroNodeDiameter = 54.0;
const double kTopologyMacroNodeRadius = kTopologyMacroNodeDiameter / 2;

/// Passo padrão da grade magnética (`snapToGrid`) ao arrastar nós.
const double kTopologyGridSize = 24.0;

/// Modos de layout automático do motor de topologia.
enum TopologyLayoutKind {
  hierarchical(
    label: 'Hierárquica',
    subtitle: 'Do roteador de borda até a ponta, em camadas sem colisões',
    icon: Icons.account_tree_outlined,
  ),
  radial(
    label: 'Radial',
    subtitle: 'Anéis concêntricos por saltos a partir do núcleo',
    icon: Icons.radar_outlined,
  ),
  force(
    label: 'Força',
    subtitle: 'Organização orgânica determinística por conexões',
    icon: Icons.hub_outlined,
  ),
  grouped(
    label: 'Por Site',
    subtitle: 'Agrupa equipamentos em caixas por site ou categoria',
    icon: Icons.workspaces_outlined,
  );

  const TopologyLayoutKind({
    required this.label,
    required this.subtitle,
    required this.icon,
  });

  final String label;
  final String subtitle;
  final IconData icon;

  /// Converte do enum legado em `topology_models.dart`.
  static TopologyLayoutKind fromLegacy(TopologyLayoutMode mode) {
    return switch (mode) {
      TopologyLayoutMode.hierarchical => TopologyLayoutKind.hierarchical,
      TopologyLayoutMode.radial => TopologyLayoutKind.radial,
      TopologyLayoutMode.force => TopologyLayoutKind.force,
      TopologyLayoutMode.grid => TopologyLayoutKind.grouped,
    };
  }

  /// Converte para o enum legado de `topology_models.dart`.
  TopologyLayoutMode toLegacy() {
    return switch (this) {
      TopologyLayoutKind.hierarchical => TopologyLayoutMode.hierarchical,
      TopologyLayoutKind.radial => TopologyLayoutMode.radial,
      TopologyLayoutKind.force => TopologyLayoutMode.force,
      TopologyLayoutKind.grouped => TopologyLayoutMode.grid,
    };
  }

  static TopologyLayoutKind fromStorageKey(String? raw) {
    for (final kind in values) {
      if (kind.name == raw) return kind;
    }
    return TopologyLayoutKind.hierarchical;
  }
}

/// Caixa delimitadora rotulada de um agrupamento (usada no modo `grouped`).
@immutable
class TopologyClusterBox {
  const TopologyClusterBox({
    required this.id,
    required this.label,
    required this.bounds,
    required this.nodeCount,
    this.siteId,
  });

  final String id;
  final String label;
  final Rect bounds;
  final int nodeCount;
  final int? siteId;

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is TopologyClusterBox &&
        other.id == id &&
        other.label == label &&
        other.bounds == bounds &&
        other.nodeCount == nodeCount &&
        other.siteId == siteId;
  }

  @override
  int get hashCode => Object.hash(id, label, bounds, nodeCount, siteId);
}

/// Resultado completo do cálculo de layout: posições (canto superior esquerdo),
/// caixas de agrupamento (quando aplicável) e limites totais do grafo.
@immutable
class TopologyLayoutOutput {
  const TopologyLayoutOutput({
    required this.positions,
    this.clusters = const <TopologyClusterBox>[],
    required this.bounds,
  });

  final Map<int, Offset> positions;
  final List<TopologyClusterBox> clusters;
  final Rect bounds;

  /// Converte para `Map<int, TopologyPosition>` usado pelo `TopologyNotifier`.
  Map<int, TopologyPosition> toTopologyPositions() =>
      topologyPositionsFromOffsets(positions);
}

/// Converte posições do provider (`TopologyPosition`) para `Offset`.
Map<int, Offset> offsetsFromTopologyPositions(
  Map<int, TopologyPosition> positions,
) {
  return <int, Offset>{
    for (final entry in positions.entries)
      if (entry.value.x.isFinite && entry.value.y.isFinite)
        entry.key: Offset(entry.value.x, entry.value.y),
  };
}

/// Converte `Offset` do motor para `TopologyPosition` do provider.
Map<int, TopologyPosition> topologyPositionsFromOffsets(
  Map<int, Offset> positions,
) {
  return <int, TopologyPosition>{
    for (final entry in positions.entries)
      entry.key: TopologyPosition(entry.value.dx, entry.value.dy),
  };
}

/// Alinha uma posição ao passo de grade mais próximo.
Offset snapOffsetToGrid(Offset point, {double gridSize = kTopologyGridSize}) {
  if (gridSize <= 0 || !gridSize.isFinite) return point;
  return Offset(
    (point.dx / gridSize).round() * gridSize,
    (point.dy / gridSize).round() * gridSize,
  );
}

/// Retângulo ocupado pelo cartão do nó no mundo.
Rect topologyNodeRect(
  Offset topLeft, {
  Size nodeSize = kDefaultTopologyNodeSize,
}) {
  return Rect.fromLTWH(topLeft.dx, topLeft.dy, nodeSize.width, nodeSize.height);
}

/// Centro geométrico do cartão do nó no mundo.
Offset topologyNodeCenter(
  Offset topLeft, {
  Size nodeSize = kDefaultTopologyNodeSize,
}) {
  return Offset(
    topLeft.dx + nodeSize.width / 2,
    topLeft.dy + nodeSize.height / 2,
  );
}

const List<String> _categoryOrder = <String>[
  'gateway',
  'router',
  'firewall',
  'switch',
  'unmanaged_switch',
  'hub',
  'access_point',
  'ap',
  'server',
  'nas',
  'vm',
  'workstation',
  'printer',
  'camera',
  'voip',
  'iot',
  'media',
  'mobile',
  'ups',
  'web_device',
  'other',
  'unknown',
];

int _categoryRank(TopologyNodeModel node) {
  final idx = _categoryOrder.indexOf(node.type.trim().toLowerCase());
  return idx >= 0 ? idx : _categoryOrder.length;
}

/// Comparador estável de nós: sistema primeiro, depois infraestrutura → ponta,
/// depois nome em ordem natural e por fim ID.
int compareTopologyNodes(TopologyNodeModel a, TopologyNodeModel b) {
  if (a.isSystem != b.isSystem) return a.isSystem ? -1 : 1;
  final rankCmp = _categoryRank(a).compareTo(_categoryRank(b));
  if (rankCmp != 0) return rankCmp;
  final nameCmp = _naturalCompare(a.name, b.name);
  if (nameCmp != 0) return nameCmp;
  return a.id.compareTo(b.id);
}

int _naturalCompare(String a, String b) {
  final la = a.toLowerCase();
  final lb = b.toLowerCase();
  final reg = RegExp(r'(\d+|\D+)');
  final ta = reg.allMatches(la).map((m) => m.group(0)!).toList(growable: false);
  final tb = reg.allMatches(lb).map((m) => m.group(0)!).toList(growable: false);
  final len = math.min(ta.length, tb.length);
  for (var i = 0; i < len; i++) {
    final na = int.tryParse(ta[i]);
    final nb = int.tryParse(tb[i]);
    final cmp = (na != null && nb != null)
        ? na.compareTo(nb)
        : ta[i].compareTo(tb[i]);
    if (cmp != 0) return cmp;
  }
  return ta.length.compareTo(tb.length);
}

Map<int, List<int>> _buildAdjacency(
  List<TopologyNodeModel> nodes,
  List<TopologyEdgeModel> edges,
) {
  final byId = <int, TopologyNodeModel>{for (final n in nodes) n.id: n};
  final raw = <int, Set<int>>{for (final n in nodes) n.id: <int>{}};
  for (final e in edges) {
    final a = e.sourceDeviceId;
    final b = e.targetDeviceId;
    if (a == b || !byId.containsKey(a) || !byId.containsKey(b)) continue;
    raw[a]!.add(b);
    raw[b]!.add(a);
  }
  return <int, List<int>>{
    for (final entry in raw.entries)
      entry.key: (entry.value.toList()
        ..sort((x, y) => compareTopologyNodes(byId[x]!, byId[y]!))),
  };
}

Map<int, int> _bfsDistances(int start, Map<int, List<int>> adjacency) {
  final dist = <int, int>{start: 0};
  final queue = <int>[start];
  var head = 0;
  while (head < queue.length) {
    final current = queue[head++];
    final nextDist = dist[current]! + 1;
    for (final other in adjacency[current] ?? const <int>[]) {
      if (dist.containsKey(other)) continue;
      dist[other] = nextDist;
      queue.add(other);
    }
  }
  return dist;
}

bool _isRouterOrGateway(TopologyNodeModel node) {
  final t = node.type.trim().toLowerCase();
  return t == 'router' || t == 'gateway';
}

bool _isFirewall(TopologyNodeModel node) {
  return node.type.trim().toLowerCase() == 'firewall';
}

/// Seleciona a raiz ideal para cada componente conectado do grafo:
/// 1) Gateway/Roteador (ou Firewall), preferindo `parentId == null` / próximo ao servidor NetMonitor;
/// 2) Servidor do sistema (`isSystem`);
/// 3) Nó com `parentId == null` de maior grau;
/// 4) Nó de maior grau no componente.
List<int> chooseTopologyRoots(
  List<TopologyNodeModel> nodes,
  List<TopologyEdgeModel> edges,
) {
  if (nodes.isEmpty) return const <int>[];
  final byId = <int, TopologyNodeModel>{for (final n in nodes) n.id: n};
  final adjacency = _buildAdjacency(nodes, edges);
  int degree(int id) => adjacency[id]?.length ?? 0;

  final sortedNodes = List<TopologyNodeModel>.from(nodes)
    ..sort(compareTopologyNodes);
  final seen = <int>{};
  final roots = <int>[];

  for (final node in sortedNodes) {
    if (seen.contains(node.id) || degree(node.id) == 0) continue;
    final reach = _bfsDistances(node.id, adjacency);
    final members = reach.keys.toList(growable: false);
    seen.addAll(members);

    final systemId = members.where((id) => byId[id]!.isSystem).firstOrNull;
    final fromSystem = systemId != null
        ? _bfsDistances(systemId, adjacency)
        : null;

    int compareCandidates(int a, int b) {
      final na = byId[a]!;
      final nb = byId[b]!;
      final aTop = na.parentId == null || !reach.containsKey(na.parentId);
      final bTop = nb.parentId == null || !reach.containsKey(nb.parentId);
      if (aTop != bTop) return aTop ? -1 : 1;
      if (fromSystem != null) {
        final da = fromSystem[a] ?? 999999;
        final db = fromSystem[b] ?? 999999;
        if (da != db) return da.compareTo(db);
      }
      final degCmp = degree(b).compareTo(degree(a));
      if (degCmp != 0) return degCmp;
      return compareTopologyNodes(na, nb);
    }

    final routers = members
        .where((id) => _isRouterOrGateway(byId[id]!))
        .toList();
    final firewalls = members.where((id) => _isFirewall(byId[id]!)).toList();
    final primary = routers.isNotEmpty ? routers : firewalls;
    if (primary.isNotEmpty) {
      primary.sort(compareCandidates);
      roots.add(primary.first);
    } else if (systemId != null) {
      roots.add(systemId);
    } else {
      final noParent = members
          .where(
            (id) =>
                byId[id]!.parentId == null ||
                !reach.containsKey(byId[id]!.parentId),
          )
          .toList();
      final pool = noParent.isNotEmpty ? noParent : members.toList();
      pool.sort(compareCandidates);
      roots.add(pool.first);
    }
  }
  return List<int>.unmodifiable(roots);
}

/// Calcula o layout completo (`positions`, `clusters`, `bounds`) para o modo dado.
TopologyLayoutOutput computeTopologyLayoutOutput({
  required List<TopologyNodeModel> nodes,
  required List<TopologyEdgeModel> edges,
  required TopologyLayoutKind mode,
  Map<int, Offset>? savedPositions,
  Size nodeSize = kDefaultTopologyNodeSize,
  bool respectSavedPositions = true,
}) {
  if (nodes.isEmpty) {
    return const TopologyLayoutOutput(
      positions: <int, Offset>{},
      clusters: <TopologyClusterBox>[],
      bounds: Rect.zero,
    );
  }

  final validSaved = <int, Offset>{};
  if (respectSavedPositions &&
      savedPositions != null &&
      savedPositions.isNotEmpty) {
    for (final node in nodes) {
      final pos = savedPositions[node.id];
      if (pos != null && pos.dx.isFinite && pos.dy.isFinite) {
        validSaved[node.id] = pos;
      }
    }
  }

  // Se todos os nós já possuem posição salva válida e devemos respeitá-las.
  if (validSaved.length == nodes.length) {
    final clusters = mode == TopologyLayoutKind.grouped
        ? _computeClusterBoxesFromPositions(nodes, validSaved, nodeSize)
        : const <TopologyClusterBox>[];
    final bounds = computeGraphBounds(
      validSaved,
      nodeSize: nodeSize,
      clusters: clusters,
    );
    return TopologyLayoutOutput(
      positions: Map<int, Offset>.unmodifiable(validSaved),
      clusters: clusters,
      bounds: bounds,
    );
  }

  // Se há algumas posições salvas (ex.: chegaram novos dispositivos), mantém os
  // posicionados intactos e encaixa somente os novos sem colisão.
  if (validSaved.isNotEmpty) {
    final merged = _placeUnsavedNodes(
      nodes: nodes,
      edges: edges,
      saved: validSaved,
      nodeSize: nodeSize,
    );
    final clusters = mode == TopologyLayoutKind.grouped
        ? _computeClusterBoxesFromPositions(nodes, merged, nodeSize)
        : const <TopologyClusterBox>[];
    final bounds = computeGraphBounds(
      merged,
      nodeSize: nodeSize,
      clusters: clusters,
    );
    return TopologyLayoutOutput(
      positions: Map<int, Offset>.unmodifiable(merged),
      clusters: clusters,
      bounds: bounds,
    );
  }

  switch (mode) {
    case TopologyLayoutKind.hierarchical:
      final pos = _computeHierarchicalLayout(nodes, edges, nodeSize);
      final bounds = computeGraphBounds(pos, nodeSize: nodeSize);
      return TopologyLayoutOutput(
        positions: Map<int, Offset>.unmodifiable(pos),
        bounds: bounds,
      );
    case TopologyLayoutKind.radial:
      final pos = _computeRadialLayout(nodes, edges, nodeSize);
      final bounds = computeGraphBounds(pos, nodeSize: nodeSize);
      return TopologyLayoutOutput(
        positions: Map<int, Offset>.unmodifiable(pos),
        bounds: bounds,
      );
    case TopologyLayoutKind.force:
      final pos = _computeForceLayout(nodes, edges, nodeSize);
      final bounds = computeGraphBounds(pos, nodeSize: nodeSize);
      return TopologyLayoutOutput(
        positions: Map<int, Offset>.unmodifiable(pos),
        bounds: bounds,
      );
    case TopologyLayoutKind.grouped:
      return _computeGroupedLayout(nodes, edges, nodeSize);
  }
}

/// Atalho que retorna diretamente `Map<int, Offset>` do layout calculado.
Map<int, Offset> computeEngineLayout({
  required List<TopologyNodeModel> nodes,
  required List<TopologyEdgeModel> edges,
  required TopologyLayoutKind mode,
  Map<int, Offset>? savedPositions,
  Size nodeSize = kDefaultTopologyNodeSize,
  bool respectSavedPositions = true,
}) {
  return computeTopologyLayoutOutput(
    nodes: nodes,
    edges: edges,
    mode: mode,
    savedPositions: savedPositions,
    nodeSize: nodeSize,
    respectSavedPositions: respectSavedPositions,
  ).positions;
}

// ── Layout 1: Hierárquico compacto em árvore (sem colisões) ─────────────────

class _LayoutBlock {
  _LayoutBlock({
    required this.width,
    required this.height,
    required this.positions,
    required this.anchorX,
  });

  final double width;
  final double height;
  final Map<int, Offset> positions;
  final double anchorX;
}

Map<int, Offset> _computeHierarchicalLayout(
  List<TopologyNodeModel> nodes,
  List<TopologyEdgeModel> edges,
  Size nodeSize,
) {
  final byId = <int, TopologyNodeModel>{for (final n in nodes) n.id: n};
  final adjacency = _buildAdjacency(nodes, edges);
  final roots = chooseTopologyRoots(nodes, edges);

  final visited = <int>{};
  final children = <int, List<int>>{};
  final componentRoots = <int>[];

  void buildTreeFrom(int rootId) {
    componentRoots.add(rootId);
    visited.add(rootId);
    final queue = <int>[rootId];
    var head = 0;
    while (head < queue.length) {
      final current = queue[head++];
      final kids = <int>[];
      // Filhos explícitos por `parentId` entram primeiro, depois vizinhos por aresta.
      final explicitKids = <int>[
        for (final n in nodes)
          if (n.parentId == current &&
              !visited.contains(n.id) &&
              (adjacency[current]?.contains(n.id) ?? false))
            n.id,
      ]..sort((a, b) => compareTopologyNodes(byId[a]!, byId[b]!));

      for (final kid in explicitKids) {
        if (visited.add(kid)) {
          kids.add(kid);
          queue.add(kid);
        }
      }
      for (final neighbor in adjacency[current] ?? const <int>[]) {
        if (visited.add(neighbor)) {
          kids.add(neighbor);
          queue.add(neighbor);
        }
      }
      children[current] = kids;
    }
  }

  for (final root in roots) {
    if (byId.containsKey(root) && !visited.contains(root)) {
      buildTreeFrom(root);
    }
  }

  final isolated = <int>[];
  final sortedNodes = List<TopologyNodeModel>.from(nodes)
    ..sort(compareTopologyNodes);
  for (final node in sortedNodes) {
    if (visited.contains(node.id)) continue;
    if ((adjacency[node.id]?.isEmpty ?? true)) {
      isolated.add(node.id);
    } else {
      buildTreeFrom(node.id);
    }
  }

  final hGap = math.max(36.0, nodeSize.width * 0.22);
  final levelGap = math.max(72.0, nodeSize.height * 0.78);
  final leafVGap = 32.0;

  _LayoutBlock arrangeLeafGrid(List<int> leaves) {
    final count = leaves.length;
    final cols = math.min(5, math.max(1, math.sqrt(count * 1.5).ceil()));
    final rows = (count / cols).ceil();
    final stepX = nodeSize.width + hGap;
    final stepY = nodeSize.height + leafVGap;
    final positions = <int, Offset>{};
    for (var i = 0; i < count; i++) {
      final col = i % cols;
      final row = i ~/ cols;
      positions[leaves[i]] = Offset(col * stepX, row * stepY);
    }
    final usedCols = math.min(count, cols);
    final width = usedCols * nodeSize.width + (usedCols - 1) * hGap;
    final height = rows * nodeSize.height + (rows - 1) * leafVGap;
    return _LayoutBlock(
      width: width,
      height: height,
      positions: positions,
      anchorX: width / 2,
    );
  }

  _LayoutBlock packSiblingBlocks(List<_LayoutBlock> units) {
    final maxRowWidth = math.max(1600.0, (nodeSize.width + hGap) * 6);
    final totalSingleRow =
        units.fold<double>(0.0, (sum, u) => sum + u.width) +
        hGap * math.max(0, units.length - 1);

    if (units.length <= 1 || totalSingleRow <= maxRowWidth) {
      final positions = <int, Offset>{};
      var x = 0.0;
      var maxH = 0.0;
      for (final unit in units) {
        for (final entry in unit.positions.entries) {
          positions[entry.key] = Offset(entry.value.dx + x, entry.value.dy);
        }
        x += unit.width + hGap;
        if (unit.height > maxH) maxH = unit.height;
      }
      final width = math.max(0.0, x - hGap);
      return _LayoutBlock(
        width: width,
        height: maxH,
        positions: positions,
        anchorX: width / 2,
      );
    }

    // Quebra em fileiras balanceadas quando muitos sub-blocos excedem a largura alvo.
    final rows = <List<_LayoutBlock>>[];
    var currentRow = <_LayoutBlock>[];
    var currentWidth = 0.0;
    for (final unit in units) {
      final nextWidth = currentRow.isEmpty
          ? unit.width
          : currentWidth + hGap + unit.width;
      if (currentRow.isNotEmpty && nextWidth > maxRowWidth) {
        rows.add(currentRow);
        currentRow = <_LayoutBlock>[unit];
        currentWidth = unit.width;
      } else {
        currentRow.add(unit);
        currentWidth = nextWidth;
      }
    }
    if (currentRow.isNotEmpty) rows.add(currentRow);

    final rowWidths = rows
        .map(
          (r) =>
              r.fold<double>(0.0, (s, u) => s + u.width) +
              hGap * math.max(0, r.length - 1),
        )
        .toList(growable: false);
    final totalWidth = rowWidths.reduce(math.max);
    final positions = <int, Offset>{};
    var y = 0.0;
    const rowGap = 56.0;

    for (var rIdx = 0; rIdx < rows.length; rIdx++) {
      final row = rows[rIdx];
      var x = (totalWidth - rowWidths[rIdx]) / 2;
      var rowHeight = 0.0;
      for (final unit in row) {
        for (final entry in unit.positions.entries) {
          positions[entry.key] = Offset(entry.value.dx + x, entry.value.dy + y);
        }
        x += unit.width + hGap;
        if (unit.height > rowHeight) rowHeight = unit.height;
      }
      y += rowHeight + rowGap;
    }

    return _LayoutBlock(
      width: totalWidth,
      height: math.max(0.0, y - rowGap),
      positions: positions,
      anchorX: totalWidth / 2,
    );
  }

  late _LayoutBlock Function(int) arrangeSubtree;
  arrangeSubtree = (int nodeId) {
    final kids = children[nodeId] ?? const <int>[];
    if (kids.isEmpty) {
      return _LayoutBlock(
        width: nodeSize.width,
        height: nodeSize.height,
        positions: <int, Offset>{nodeId: Offset.zero},
        anchorX: nodeSize.width / 2,
      );
    }

    final leafKids = <int>[];
    final branchKids = <int>[];
    for (final kid in kids) {
      if ((children[kid]?.isEmpty ?? true)) {
        leafKids.add(kid);
      } else {
        branchKids.add(kid);
      }
    }

    final childUnits = <_LayoutBlock>[];
    for (final branch in branchKids) {
      childUnits.add(arrangeSubtree(branch));
    }
    if (leafKids.length >= 3) {
      childUnits.add(arrangeLeafGrid(leafKids));
    } else {
      for (final leaf in leafKids) {
        childUnits.add(arrangeSubtree(leaf));
      }
    }

    final pack = packSiblingBlocks(childUnits);
    final width = math.max(nodeSize.width, pack.width);
    final packLeft = (width - pack.width) / 2;
    final anchorX = packLeft + pack.anchorX;
    final rootLeft = (anchorX - nodeSize.width / 2)
        .clamp(0.0, math.max(0.0, width - nodeSize.width))
        .toDouble();
    final positions = <int, Offset>{nodeId: Offset(rootLeft, 0.0)};
    final childTop = nodeSize.height + levelGap;
    for (final entry in pack.positions.entries) {
      positions[entry.key] = Offset(
        entry.value.dx + packLeft,
        entry.value.dy + childTop,
      );
    }
    return _LayoutBlock(
      width: width,
      height: childTop + pack.height,
      positions: positions,
      anchorX: rootLeft + nodeSize.width / 2,
    );
  };

  final blocks = <_LayoutBlock>[
    for (final root in componentRoots) arrangeSubtree(root),
  ];

  const originX = 80.0;
  const originY = 80.0;
  const componentGap = 110.0;

  final result = <int, Offset>{};
  var cursorX = originX;
  var cursorY = originY;
  var rowMaxH = 0.0;
  final wrapWidth = math.max(2200.0, (nodeSize.width + hGap) * 8);

  for (final block in blocks) {
    if (cursorX > originX && cursorX + block.width > originX + wrapWidth) {
      cursorX = originX;
      cursorY += rowMaxH + componentGap;
      rowMaxH = 0.0;
    }
    for (final entry in block.positions.entries) {
      result[entry.key] = Offset(
        entry.value.dx + cursorX,
        entry.value.dy + cursorY,
      );
    }
    cursorX += block.width + componentGap;
    if (block.height > rowMaxH) rowMaxH = block.height;
  }

  if (isolated.isNotEmpty) {
    final isoBlock = arrangeLeafGrid(isolated);
    final isoTop = blocks.isEmpty ? originY : cursorY + rowMaxH + componentGap;
    for (final entry in isoBlock.positions.entries) {
      result[entry.key] = Offset(
        entry.value.dx + originX,
        entry.value.dy + isoTop,
      );
    }
  }

  return _resolveOverlaps(result, nodeSize, padding: 24.0);
}

// ── Layout 2: Radial em anéis concêntricos por saltos ────────────────────────

Map<int, Offset> _computeRadialLayout(
  List<TopologyNodeModel> nodes,
  List<TopologyEdgeModel> edges,
  Size nodeSize,
) {
  final byId = <int, TopologyNodeModel>{for (final n in nodes) n.id: n};
  final adjacency = _buildAdjacency(nodes, edges);
  final roots = chooseTopologyRoots(nodes, edges);

  final visited = <int>{};
  final componentBlocks = <_LayoutBlock>[];
  final cardDiag = math.sqrt(
    nodeSize.width * nodeSize.width + nodeSize.height * nodeSize.height,
  );
  final minChord = cardDiag + 32.0;
  final radialStep = cardDiag + 44.0;

  _LayoutBlock layoutRadialComponent(int rootId) {
    final depth = <int, int>{rootId: 0};
    final parentOf = <int, int?>{rootId: null};
    final queue = <int>[rootId];
    var head = 0;
    visited.add(rootId);
    while (head < queue.length) {
      final cur = queue[head++];
      for (final nb in adjacency[cur] ?? const <int>[]) {
        if (visited.add(nb)) {
          depth[nb] = depth[cur]! + 1;
          parentOf[nb] = cur;
          queue.add(nb);
        }
      }
    }

    final rings = <int, List<int>>{};
    for (final entry in depth.entries) {
      rings.putIfAbsent(entry.value, () => <int>[]).add(entry.key);
    }

    final angleOf = <int, double>{rootId: 0.0};
    final centerCoords = <int, Offset>{rootId: Offset.zero};
    final sortedRings = rings.keys.toList()..sort();
    var prevRadius = 0.0;

    for (final ring in sortedRings) {
      if (ring == 0) continue;
      final ringNodes = rings[ring]!;
      // Ordena pelo ângulo do pai para que os ramos irradiem sem cruzar.
      ringNodes.sort((a, b) {
        final pa = angleOf[parentOf[a]] ?? 0.0;
        final pb = angleOf[parentOf[b]] ?? 0.0;
        final angCmp = pa.compareTo(pb);
        if (angCmp != 0) return angCmp;
        return compareTopologyNodes(byId[a]!, byId[b]!);
      });

      final circumferenceRadius =
          (ringNodes.length * minChord) / (2.0 * math.pi);
      final radius = math.max(prevRadius + radialStep, circumferenceRadius);
      prevRadius = radius;

      final step = (2.0 * math.pi) / ringNodes.length;
      final startAngle = -math.pi / 2 + (ring.isEven ? step / 2 : 0.0);
      for (var i = 0; i < ringNodes.length; i++) {
        final id = ringNodes[i];
        final theta = startAngle + i * step;
        angleOf[id] = theta;
        centerCoords[id] = Offset(
          radius * math.cos(theta),
          radius * math.sin(theta),
        );
      }
    }

    // Normaliza o bloco para origem (0, 0) no canto superior esquerdo.
    var minX = double.infinity;
    var minY = double.infinity;
    var maxX = double.negativeInfinity;
    var maxY = double.negativeInfinity;
    for (final c in centerCoords.values) {
      final left = c.dx - nodeSize.width / 2;
      final top = c.dy - nodeSize.height / 2;
      if (left < minX) minX = left;
      if (top < minY) minY = top;
      if (left + nodeSize.width > maxX) maxX = left + nodeSize.width;
      if (top + nodeSize.height > maxY) maxY = top + nodeSize.height;
    }

    final normalized = <int, Offset>{
      for (final entry in centerCoords.entries)
        entry.key: Offset(
          entry.value.dx - nodeSize.width / 2 - minX,
          entry.value.dy - nodeSize.height / 2 - minY,
        ),
    };
    return _LayoutBlock(
      width: maxX - minX,
      height: maxY - minY,
      positions: normalized,
      anchorX: (maxX - minX) / 2,
    );
  }

  for (final root in roots) {
    if (byId.containsKey(root) && !visited.contains(root)) {
      componentBlocks.add(layoutRadialComponent(root));
    }
  }

  final isolated = <int>[];
  final sortedNodes = List<TopologyNodeModel>.from(nodes)
    ..sort(compareTopologyNodes);
  for (final node in sortedNodes) {
    if (visited.contains(node.id)) continue;
    if ((adjacency[node.id]?.isEmpty ?? true)) {
      isolated.add(node.id);
    } else {
      componentBlocks.add(layoutRadialComponent(node.id));
    }
  }

  final result = <int, Offset>{};
  const originX = 80.0;
  const originY = 80.0;
  const gap = 120.0;
  var cursorX = originX;
  var maxHeight = 0.0;

  for (final block in componentBlocks) {
    for (final entry in block.positions.entries) {
      result[entry.key] = Offset(
        entry.value.dx + cursorX,
        entry.value.dy + originY,
      );
    }
    cursorX += block.width + gap;
    if (block.height > maxHeight) maxHeight = block.height;
  }

  if (isolated.isNotEmpty) {
    final cols = math.max(
      1,
      math.min(6, math.sqrt(isolated.length * 1.6).ceil()),
    );
    final startY = componentBlocks.isEmpty
        ? originY
        : originY + maxHeight + gap;
    for (var i = 0; i < isolated.length; i++) {
      final col = i % cols;
      final row = i ~/ cols;
      result[isolated[i]] = Offset(
        originX + col * (nodeSize.width + 36.0),
        startY + row * (nodeSize.height + 36.0),
      );
    }
  }

  return _resolveOverlaps(result, nodeSize, padding: 24.0);
}

// ── Layout 3: Força determinística (Fruchterman-Reingold + anti-overlap) ────

Map<int, Offset> _computeForceLayout(
  List<TopologyNodeModel> nodes,
  List<TopologyEdgeModel> edges,
  Size nodeSize,
) {
  final sorted = List<TopologyNodeModel>.from(nodes)
    ..sort(compareTopologyNodes);
  final count = sorted.length;
  if (count == 1) {
    return <int, Offset>{sorted.first.id: const Offset(120.0, 120.0)};
  }

  final idIndex = <int, int>{for (var i = 0; i < count; i++) sorted[i].id: i};
  final edgePairs = <(int, int)>[];
  for (final e in edges) {
    final u = idIndex[e.sourceDeviceId];
    final v = idIndex[e.targetDeviceId];
    if (u != null && v != null && u != v) {
      edgePairs.add((u, v));
    }
  }

  final cardDiag = math.sqrt(
    nodeSize.width * nodeSize.width + nodeSize.height * nodeSize.height,
  );
  final idealLength = cardDiag * 1.25;
  final goldenAngle = math.pi * (3.0 - math.sqrt(5.0));

  // Posição inicial determinística em espiral de Vogel (estável para qualquer grafo).
  final xs = List<double>.filled(count, 0.0);
  final ys = List<double>.filled(count, 0.0);
  for (var i = 0; i < count; i++) {
    final r = idealLength * 0.55 * math.sqrt(i + 1.0);
    final theta = i * goldenAngle;
    xs[i] = r * math.cos(theta);
    ys[i] = r * math.sin(theta);
  }

  final iterations = count > 120 ? 55 : 85;
  final initialTemp = idealLength * 0.85;
  final repConst = idealLength * idealLength * 1.35;

  for (var iter = 0; iter < iterations; iter++) {
    final dxAcc = List<double>.filled(count, 0.0);
    final dyAcc = List<double>.filled(count, 0.0);

    // Repulsão entre pares.
    for (var i = 0; i < count; i++) {
      for (var j = i + 1; j < count; j++) {
        var dx = xs[i] - xs[j];
        var dy = ys[i] - ys[j];
        final distSq = dx * dx + dy * dy;
        final dist = distSq < 1.0 ? 1.0 : math.sqrt(distSq);
        if (distSq < 1.0) {
          // Empurrão determinístico para quebrar coincidência exata.
          dx = (i.isEven ? 1.0 : -1.0);
          dy = (j.isEven ? 0.5 : -0.5);
        }
        final force = repConst / dist;
        final fx = (dx / dist) * force;
        final fy = (dy / dist) * force;
        dxAcc[i] += fx;
        dyAcc[i] += fy;
        dxAcc[j] -= fx;
        dyAcc[j] -= fy;
      }
    }

    // Atração de mola nas arestas.
    for (final (u, v) in edgePairs) {
      final dx = xs[u] - xs[v];
      final dy = ys[u] - ys[v];
      final dist = math.max(1.0, math.sqrt(dx * dx + dy * dy));
      final force = (dist * dist) / idealLength * 0.45;
      final fx = (dx / dist) * force;
      final fy = (dy / dist) * force;
      dxAcc[u] -= fx;
      dyAcc[u] -= fy;
      dxAcc[v] += fx;
      dyAcc[v] += fy;
    }

    // Gravidade suave para o centro evita deriva de nós isolados.
    for (var i = 0; i < count; i++) {
      dxAcc[i] -= xs[i] * 0.06;
      dyAcc[i] -= ys[i] * 0.06;
    }

    final temp = initialTemp * (1.0 - iter / (iterations + 1.0));
    for (var i = 0; i < count; i++) {
      final mag = math.sqrt(dxAcc[i] * dxAcc[i] + dyAcc[i] * dyAcc[i]);
      if (mag > 0.0001) {
        final step = math.min(mag, temp);
        xs[i] += (dxAcc[i] / mag) * step;
        ys[i] += (dyAcc[i] / mag) * step;
      }
    }
  }

  // Normaliza para coordenadas positivas confortáveis.
  final minX = xs.reduce(math.min);
  final minY = ys.reduce(math.min);
  const origin = 80.0;
  final raw = <int, Offset>{
    for (var i = 0; i < count; i++)
      sorted[i].id: Offset(xs[i] - minX + origin, ys[i] - minY + origin),
  };

  return _resolveOverlaps(raw, nodeSize, padding: 28.0);
}

// ── Layout 4: Agrupado por Site (ou por Categoria se site único) ─────────────

class _ClusterSpec {
  _ClusterSpec({
    required this.id,
    required this.label,
    required this.nodes,
    this.siteId,
  });

  final String id;
  final String label;
  final List<TopologyNodeModel> nodes;
  final int? siteId;
}

List<_ClusterSpec> _partitionIntoClusters(List<TopologyNodeModel> nodes) {
  final distinctSites = <String>{
    for (final n in nodes)
      if (n.siteName != null && n.siteName!.trim().isNotEmpty)
        n.siteName!.trim()
      else if (n.siteId != null)
        'site:${n.siteId}',
  };

  final useSiteGrouping = distinctSites.length >= 2;
  final map = <String, _ClusterSpec>{};

  for (final node in nodes) {
    final String key;
    final String label;
    final int? siteId;
    if (useSiteGrouping) {
      final cleanSite = node.siteName?.trim();
      if (cleanSite != null && cleanSite.isNotEmpty) {
        key = 'site:${node.siteId ?? cleanSite.toLowerCase()}';
        label = cleanSite;
        siteId = node.siteId;
      } else if (node.siteId != null) {
        key = 'site:${node.siteId}';
        label = 'Site #${node.siteId}';
        siteId = node.siteId;
      } else {
        key = 'site:none';
        label = 'Sem site';
        siteId = null;
      }
    } else {
      final typeKey = node.isSystem ? 'system' : node.type.trim().toLowerCase();
      key = 'type:$typeKey';
      label = topologyNodeTypeLabel(node);
      siteId = node.siteId;
    }

    map
        .putIfAbsent(
          key,
          () => _ClusterSpec(
            id: key,
            label: label,
            nodes: <TopologyNodeModel>[],
            siteId: siteId,
          ),
        )
        .nodes
        .add(node);
  }

  final clusters = map.values.toList();
  for (final c in clusters) {
    c.nodes.sort(compareTopologyNodes);
  }
  clusters.sort((a, b) {
    final countCmp = b.nodes.length.compareTo(a.nodes.length);
    if (countCmp != 0) return countCmp;
    return _naturalCompare(a.label, b.label);
  });
  return clusters;
}

TopologyLayoutOutput _computeGroupedLayout(
  List<TopologyNodeModel> nodes,
  List<TopologyEdgeModel> edges,
  Size nodeSize,
) {
  final specs = _partitionIntoClusters(nodes);
  const padLeft = 28.0;
  const padRight = 28.0;
  const padTop = 52.0; // Espaço para o rótulo do cluster no canto superior.
  const padBottom = 28.0;
  const clusterGap = 68.0;
  const originX = 80.0;
  const originY = 80.0;

  final builtClusters =
      <({_ClusterSpec spec, Map<int, Offset> localPos, double w, double h})>[];

  for (final spec in specs) {
    final memberIds = spec.nodes.map((n) => n.id).toSet();
    final internalEdges = edges
        .where(
          (e) =>
              memberIds.contains(e.sourceDeviceId) &&
              memberIds.contains(e.targetDeviceId),
        )
        .toList(growable: false);

    final Map<int, Offset> rawLocal;
    if (internalEdges.isNotEmpty && spec.nodes.length >= 2) {
      rawLocal = _computeHierarchicalLayout(
        spec.nodes,
        internalEdges,
        nodeSize,
      );
    } else {
      final cols = math.max(
        1,
        math.min(4, math.sqrt(spec.nodes.length * 1.4).ceil()),
      );
      rawLocal = <int, Offset>{};
      for (var i = 0; i < spec.nodes.length; i++) {
        rawLocal[spec.nodes[i].id] = Offset(
          (i % cols) * (nodeSize.width + 32.0),
          (i ~/ cols) * (nodeSize.height + 32.0),
        );
      }
    }

    final innerBounds = computeGraphBounds(rawLocal, nodeSize: nodeSize);
    final shifted = <int, Offset>{
      for (final entry in rawLocal.entries)
        entry.key: Offset(
          entry.value.dx - innerBounds.left + padLeft,
          entry.value.dy - innerBounds.top + padTop,
        ),
    };
    final w = innerBounds.width + padLeft + padRight;
    final h = innerBounds.height + padTop + padBottom;
    builtClusters.add((spec: spec, localPos: shifted, w: w, h: h));
  }

  final cols = math.max(1, math.sqrt(builtClusters.length * 1.35).ceil());
  final positions = <int, Offset>{};
  final clusterBoxes = <TopologyClusterBox>[];

  var rowY = originY;
  for (var i = 0; i < builtClusters.length; i += cols) {
    final slice = builtClusters.sublist(
      i,
      math.min(i + cols, builtClusters.length),
    );
    var cursorX = originX;
    var rowMaxH = 0.0;
    for (final item in slice) {
      for (final entry in item.localPos.entries) {
        positions[entry.key] = Offset(
          entry.value.dx + cursorX,
          entry.value.dy + rowY,
        );
      }
      clusterBoxes.add(
        TopologyClusterBox(
          id: item.spec.id,
          label: item.spec.label,
          bounds: Rect.fromLTWH(cursorX, rowY, item.w, item.h),
          nodeCount: item.spec.nodes.length,
          siteId: item.spec.siteId,
        ),
      );
      cursorX += item.w + clusterGap;
      if (item.h > rowMaxH) rowMaxH = item.h;
    }
    rowY += rowMaxH + clusterGap;
  }

  final immutableClusters = List<TopologyClusterBox>.unmodifiable(clusterBoxes);
  final bounds = computeGraphBounds(
    positions,
    nodeSize: nodeSize,
    clusters: immutableClusters,
  );
  return TopologyLayoutOutput(
    positions: Map<int, Offset>.unmodifiable(positions),
    clusters: immutableClusters,
    bounds: bounds,
  );
}

List<TopologyClusterBox> _computeClusterBoxesFromPositions(
  List<TopologyNodeModel> nodes,
  Map<int, Offset> positions,
  Size nodeSize,
) {
  final specs = _partitionIntoClusters(nodes);
  final out = <TopologyClusterBox>[];
  for (final spec in specs) {
    final subset = <int, Offset>{
      for (final n in spec.nodes)
        if (positions.containsKey(n.id)) n.id: positions[n.id]!,
    };
    if (subset.isEmpty) continue;
    final b = computeGraphBounds(subset, nodeSize: nodeSize);
    out.add(
      TopologyClusterBox(
        id: spec.id,
        label: spec.label,
        bounds: Rect.fromLTRB(
          b.left - 24,
          b.top - 48,
          b.right + 24,
          b.bottom + 24,
        ),
        nodeCount: subset.length,
        siteId: spec.siteId,
      ),
    );
  }
  return List<TopologyClusterBox>.unmodifiable(out);
}

// ── Encaixe de nós novos preservando posições salvas ─────────────────────────

Map<int, Offset> _placeUnsavedNodes({
  required List<TopologyNodeModel> nodes,
  required List<TopologyEdgeModel> edges,
  required Map<int, Offset> saved,
  required Size nodeSize,
}) {
  final positions = Map<int, Offset>.from(saved);
  final pending =
      nodes.where((n) => !positions.containsKey(n.id)).toList(growable: true)
        ..sort(compareTopologyNodes);
  if (pending.isEmpty) return positions;

  final adjacency = _buildAdjacency(nodes, edges);
  final stepX = nodeSize.width + 36.0;
  final stepY = nodeSize.height + 36.0;

  bool isSpotFree(Offset candidate) {
    final candRect = topologyNodeRect(
      candidate,
      nodeSize: nodeSize,
    ).inflate(14);
    for (final existing in positions.values) {
      if (candRect.overlaps(topologyNodeRect(existing, nodeSize: nodeSize))) {
        return false;
      }
    }
    return true;
  }

  Offset findFreeAround(Offset anchor) {
    final ringStep = math.max(stepX, stepY);
    for (var ring = 1; ring <= 10; ring++) {
      final radius = ringStep * ring;
      final slots = 8 * ring;
      for (var i = 0; i < slots; i++) {
        final angle = math.pi / 2 + (2 * math.pi * i) / slots;
        final candidate = Offset(
          (anchor.dx + radius * math.cos(angle)).roundToDouble(),
          (anchor.dy + radius * math.sin(angle)).roundToDouble(),
        );
        if (isSpotFree(candidate)) return candidate;
      }
    }
    return Offset(anchor.dx, anchor.dy + ringStep * 11);
  }

  // Até 4 passes para nós novos encadeados a outros nós novos.
  var remaining = pending;
  for (var pass = 0; pass < 4 && remaining.isNotEmpty; pass++) {
    final next = <TopologyNodeModel>[];
    for (final node in remaining) {
      final neighborOffsets = <Offset>[
        for (final nb in adjacency[node.id] ?? const <int>[])
          if (positions.containsKey(nb)) positions[nb]!,
      ];
      if (neighborOffsets.isEmpty) {
        next.add(node);
        continue;
      }
      final avgX =
          neighborOffsets.fold<double>(0, (s, o) => s + o.dx) /
          neighborOffsets.length;
      final avgY =
          neighborOffsets.fold<double>(0, (s, o) => s + o.dy) /
          neighborOffsets.length;
      positions[node.id] = findFreeAround(Offset(avgX, avgY));
    }
    if (next.length == remaining.length) break;
    remaining = next;
  }

  if (remaining.isNotEmpty) {
    final bounds = computeGraphBounds(positions, nodeSize: nodeSize);
    var cursorX = bounds.isEmpty ? 80.0 : bounds.left;
    final rowY = bounds.isEmpty ? 80.0 : bounds.bottom + 64.0;
    for (final node in remaining) {
      var candidate = Offset(cursorX, rowY);
      while (!isSpotFree(candidate)) {
        cursorX += stepX;
        candidate = Offset(cursorX, rowY);
      }
      positions[node.id] = candidate;
      cursorX += stepX;
    }
  }

  return positions;
}

// ── Passada determinística anti-sobreposição AABB ───────────────────────────

Map<int, Offset> _resolveOverlaps(
  Map<int, Offset> input,
  Size nodeSize, {
  double padding = 24.0,
}) {
  if (input.length <= 1) return input;
  final ids = input.keys.toList()..sort();
  final coords = <int, Offset>{...input};
  final minDx = nodeSize.width + padding;
  final minDy = nodeSize.height + padding;

  for (var iter = 0; iter < 60; iter++) {
    var moved = false;
    for (var i = 0; i < ids.length; i++) {
      final idA = ids[i];
      for (var j = i + 1; j < ids.length; j++) {
        final idB = ids[j];
        final a = coords[idA]!;
        final b = coords[idB]!;
        final dx = b.dx - a.dx;
        final dy = b.dy - a.dy;
        final overlapX = minDx - dx.abs();
        final overlapY = minDy - dy.abs();
        if (overlapX > 0.0 && overlapY > 0.0) {
          moved = true;
          if (overlapX <= overlapY) {
            final dir = dx >= 0 ? 1.0 : -1.0;
            final shift = overlapX / 2.0 + 0.5;
            coords[idA] = Offset(a.dx - dir * shift, a.dy);
            coords[idB] = Offset(b.dx + dir * shift, b.dy);
          } else {
            final dir = dy >= 0 ? 1.0 : -1.0;
            final shift = overlapY / 2.0 + 0.5;
            coords[idA] = Offset(a.dx, a.dy - dir * shift);
            coords[idB] = Offset(b.dx, b.dy + dir * shift);
          }
        }
      }
    }
    if (!moved) break;
  }

  // Garante coordenadas positivas limpas (margem mínima 60px).
  var minX = double.infinity;
  var minY = double.infinity;
  for (final pos in coords.values) {
    if (pos.dx < minX) minX = pos.dx;
    if (pos.dy < minY) minY = pos.dy;
  }
  final shiftX = minX < 60.0 ? 60.0 - minX : 0.0;
  final shiftY = minY < 60.0 ? 60.0 - minY : 0.0;
  if (shiftX == 0.0 && shiftY == 0.0) return coords;
  return <int, Offset>{
    for (final entry in coords.entries)
      entry.key: Offset(entry.value.dx + shiftX, entry.value.dy + shiftY),
  };
}

// ── Limites do grafo e matrizes de câmera (Fit & Focus) ─────────────────────

/// Calcula o retângulo envolvente de todos os nós (e caixas de cluster).
Rect computeGraphBounds(
  Map<int, Offset> positions, {
  Size nodeSize = kDefaultTopologyNodeSize,
  List<TopologyClusterBox> clusters = const <TopologyClusterBox>[],
}) {
  if (positions.isEmpty && clusters.isEmpty) return Rect.zero;
  var minX = double.infinity;
  var minY = double.infinity;
  var maxX = double.negativeInfinity;
  var maxY = double.negativeInfinity;

  for (final pos in positions.values) {
    if (!pos.dx.isFinite || !pos.dy.isFinite) continue;
    if (pos.dx < minX) minX = pos.dx;
    if (pos.dy < minY) minY = pos.dy;
    if (pos.dx + nodeSize.width > maxX) maxX = pos.dx + nodeSize.width;
    if (pos.dy + nodeSize.height > maxY) maxY = pos.dy + nodeSize.height;
  }
  for (final cluster in clusters) {
    if (cluster.bounds.left < minX) minX = cluster.bounds.left;
    if (cluster.bounds.top < minY) minY = cluster.bounds.top;
    if (cluster.bounds.right > maxX) maxX = cluster.bounds.right;
    if (cluster.bounds.bottom > maxY) maxY = cluster.bounds.bottom;
  }
  if (!minX.isFinite || !minY.isFinite || !maxX.isFinite || !maxY.isFinite) {
    return Rect.zero;
  }
  return Rect.fromLTRB(minX, minY, maxX, maxY);
}

/// Calcula a matriz `Matrix4` que enquadra e centraliza `bounds` em `viewport`.
Matrix4 computeFitMatrix(
  Rect bounds,
  Size viewport, {
  EdgeInsets padding = const EdgeInsets.all(48),
  double minScale = 0.2,
  double maxScale = 1.6,
}) {
  if (bounds.isEmpty || viewport.width <= 0 || viewport.height <= 0) {
    return Matrix4.identity();
  }
  final availW = math.max(1.0, viewport.width - padding.horizontal);
  final availH = math.max(1.0, viewport.height - padding.vertical);
  final graphW = math.max(1.0, bounds.width);
  final graphH = math.max(1.0, bounds.height);

  final safeMin = math.min(minScale, maxScale);
  final safeMax = math.max(minScale, maxScale);
  final scale = math
      .min(availW / graphW, availH / graphH)
      .clamp(safeMin, safeMax)
      .toDouble();

  final viewCenterX = padding.left + availW / 2;
  final viewCenterY = padding.top + availH / 2;
  final tx = viewCenterX - bounds.center.dx * scale;
  final ty = viewCenterY - bounds.center.dy * scale;

  return Matrix4.identity()
    ..translateByDouble(tx, ty, 0.0, 1.0)
    ..scaleByDouble(scale, scale, 1.0, 1.0);
}

/// Calcula a matriz `Matrix4` que centraliza suavemente um nó específico no viewport.
Matrix4 computeFocusNodeMatrix(
  Offset nodeTopLeft,
  Size viewport, {
  Size nodeSize = kDefaultTopologyNodeSize,
  double targetScale = 1.15,
  double minScale = 0.2,
  double maxScale = 2.8,
  EdgeInsets viewportInset = EdgeInsets.zero,
}) {
  if (viewport.width <= 0 || viewport.height <= 0) {
    return Matrix4.identity();
  }
  final scale = targetScale
      .clamp(math.min(minScale, maxScale), math.max(minScale, maxScale))
      .toDouble();
  final availW = math.max(1.0, viewport.width - viewportInset.horizontal);
  final availH = math.max(1.0, viewport.height - viewportInset.vertical);
  final center = topologyNodeCenter(nodeTopLeft, nodeSize: nodeSize);
  final tx = viewportInset.left + availW / 2 - center.dx * scale;
  final ty = viewportInset.top + availH / 2 - center.dy * scale;

  return Matrix4.identity()
    ..translateByDouble(tx, ty, 0.0, 1.0)
    ..scaleByDouble(scale, scale, 1.0, 1.0);
}
