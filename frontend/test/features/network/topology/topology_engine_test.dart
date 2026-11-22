import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:shared_preferences/shared_preferences.dart';

TopologyNodeModel _node(
  int id, {
  String? name,
  String type = 'switch',
  String status = 'online',
  int? siteId,
  String? siteName,
  String? ipAddress,
  String? vendor,
  String? model,
  int? parentId,
  bool isSystem = false,
  int monitorsDown = 0,
  double? cpuPercent,
  double? memoryPercent,
}) {
  return TopologyNodeModel(
    id: id,
    name: name ?? 'Node $id',
    type: type,
    status: status,
    siteId: siteId,
    siteName: siteName,
    ipAddress: ipAddress,
    vendor: vendor,
    model: model,
    parentId: parentId,
    isSystem: isSystem,
    monitorsDown: monitorsDown,
    cpuPercent: cpuPercent,
    memoryPercent: memoryPercent,
  );
}

TopologyEdgeModel _edge(
  int id,
  int source,
  int target, {
  String linkType = 'ethernet',
  String discoveryMethod = 'lldp',
  String status = 'up',
  double? speedBps = 1000000000,
  double? inBps,
  double? outBps,
  double? trafficBps,
  double? utilization,
  double confidence = 0.9,
  bool confirmed = false,
  String? sourceInterfaceStatus,
  String? targetInterfaceStatus,
  double? sourceInterfaceSpeed,
  double? targetInterfaceSpeed,
}) {
  return TopologyEdgeModel(
    id: id,
    sourceDeviceId: source,
    targetDeviceId: target,
    linkType: linkType,
    discoveryMethod: discoveryMethod,
    status: status,
    speedBps: speedBps,
    inBps: inBps,
    outBps: outBps,
    trafficBps: trafficBps,
    utilization: utilization,
    confidence: confidence,
    confirmed: confirmed,
    sourceInterfaceStatus: sourceInterfaceStatus,
    targetInterfaceStatus: targetInterfaceStatus,
    sourceInterfaceSpeed: sourceInterfaceSpeed,
    targetInterfaceSpeed: targetInterfaceSpeed,
  );
}

void _expectNoOverlaps(
  Map<int, Offset> positions, {
  Size nodeSize = kDefaultTopologyNodeSize,
}) {
  final ids = positions.keys.toList();
  for (var i = 0; i < ids.length; i++) {
    final posA = positions[ids[i]]!;
    expect(posA.dx.isFinite, isTrue, reason: 'Node ${ids[i]} dx not finite');
    expect(posA.dy.isFinite, isTrue, reason: 'Node ${ids[i]} dy not finite');
    final rectA = topologyNodeRect(posA, nodeSize: nodeSize);
    for (var j = i + 1; j < ids.length; j++) {
      final posB = positions[ids[j]]!;
      final rectB = topologyNodeRect(posB, nodeSize: nodeSize);
      expect(
        rectA.overlaps(rectB),
        isFalse,
        reason:
            'Nodes ${ids[i]} ($rectA) and ${ids[j]} ($rectB) must not overlap',
      );
    }
  }
}

void main() {
  group('topology_layouts', () {
    final sampleNodes = <TopologyNodeModel>[
      _node(
        1,
        name: 'Gateway Borda',
        type: 'router',
        siteId: 1,
        siteName: 'Matriz',
      ),
      _node(
        2,
        name: 'Switch Core',
        type: 'switch',
        parentId: 1,
        siteId: 1,
        siteName: 'Matriz',
      ),
      _node(
        3,
        name: 'Switch Acesso 1',
        type: 'switch',
        parentId: 2,
        siteId: 1,
        siteName: 'Matriz',
      ),
      _node(
        4,
        name: 'Switch Acesso 2',
        type: 'switch',
        parentId: 2,
        siteId: 1,
        siteName: 'Matriz',
      ),
      _node(
        5,
        name: 'Servidor App',
        type: 'server',
        parentId: 2,
        siteId: 1,
        siteName: 'Matriz',
        isSystem: true,
      ),
      _node(
        6,
        name: 'AP Recepção',
        type: 'access_point',
        parentId: 3,
        siteId: 1,
        siteName: 'Matriz',
      ),
      _node(
        7,
        name: 'Câmera Portaria',
        type: 'camera',
        parentId: 3,
        siteId: 1,
        siteName: 'Matriz',
      ),
      _node(
        8,
        name: 'Impressora RH',
        type: 'printer',
        parentId: 3,
        siteId: 1,
        siteName: 'Matriz',
      ),
      _node(
        9,
        name: 'PC Financeiro',
        type: 'workstation',
        parentId: 3,
        siteId: 1,
        siteName: 'Matriz',
      ),
      // Segundo componente desconexo em outro site
      _node(
        10,
        name: 'Firewall Filial',
        type: 'firewall',
        siteId: 2,
        siteName: 'Filial Sul',
      ),
      _node(
        11,
        name: 'Switch Filial',
        type: 'switch',
        parentId: 10,
        siteId: 2,
        siteName: 'Filial Sul',
      ),
      _node(
        12,
        name: 'NAS Filial',
        type: 'nas',
        parentId: 11,
        siteId: 2,
        siteName: 'Filial Sul',
      ),
      // Nós isolados
      _node(
        13,
        name: 'UPS Isolado 1',
        type: 'ups',
        siteId: 2,
        siteName: 'Filial Sul',
      ),
      _node(14, name: 'IoT Isolado 2', type: 'iot'),
    ];

    final sampleEdges = <TopologyEdgeModel>[
      _edge(101, 1, 2, linkType: 'fiber', speedBps: 10000000000),
      _edge(102, 2, 3),
      _edge(103, 2, 4),
      _edge(104, 2, 5, linkType: 'fiber', speedBps: 10000000000),
      _edge(105, 3, 6),
      _edge(106, 3, 7),
      _edge(107, 3, 8),
      _edge(108, 3, 9),
      _edge(109, 10, 11, linkType: 'vpn'),
      _edge(110, 11, 12),
    ];

    test(
      'chooseTopologyRoots picks router/firewall per connected component',
      () {
        final roots = chooseTopologyRoots(sampleNodes, sampleEdges);
        expect(roots, containsAll(<int>[1, 10]));
        expect(roots.length, 2);
      },
    );

    for (final mode in TopologyLayoutKind.values) {
      test(
        'layout $mode produces zero overlaps, finite coords and is deterministic',
        () {
          final out1 = computeTopologyLayoutOutput(
            nodes: sampleNodes,
            edges: sampleEdges,
            mode: mode,
          );
          final out2 = computeTopologyLayoutOutput(
            nodes: sampleNodes,
            edges: sampleEdges,
            mode: mode,
          );

          expect(out1.positions.length, sampleNodes.length);
          _expectNoOverlaps(out1.positions);
          expect(out1.bounds.isEmpty, isFalse);
          expect(out1.positions, equals(out2.positions));
        },
      );
    }

    test('hierarchical places root above its children', () {
      final pos = computeEngineLayout(
        nodes: sampleNodes,
        edges: sampleEdges,
        mode: TopologyLayoutKind.hierarchical,
      );
      expect(pos[1]!.dy, lessThan(pos[2]!.dy));
      expect(pos[2]!.dy, lessThan(pos[3]!.dy));
      expect(pos[3]!.dy, lessThan(pos[6]!.dy));
    });

    test('grouped creates non-overlapping labeled cluster boxes by site', () {
      final out = computeTopologyLayoutOutput(
        nodes: sampleNodes,
        edges: sampleEdges,
        mode: TopologyLayoutKind.grouped,
      );
      expect(out.clusters.length, 3); // Matriz, Filial Sul, Sem site
      expect(
        out.clusters.map((c) => c.label),
        containsAll(<String>['Matriz', 'Filial Sul', 'Sem site']),
      );
      for (var i = 0; i < out.clusters.length; i++) {
        for (var j = i + 1; j < out.clusters.length; j++) {
          expect(
            out.clusters[i].bounds.overlaps(out.clusters[j].bounds),
            isFalse,
          );
        }
      }
    });

    test('grouped falls back to device types when graph has a single site', () {
      final singleSiteNodes = <TopologyNodeModel>[
        _node(1, name: 'R1', type: 'router', siteId: 1, siteName: 'Único'),
        _node(2, name: 'S1', type: 'switch', siteId: 1, siteName: 'Único'),
        _node(3, name: 'S2', type: 'switch', siteId: 1, siteName: 'Único'),
        _node(4, name: 'SRV', type: 'server', siteId: 1, siteName: 'Único'),
      ];
      final out = computeTopologyLayoutOutput(
        nodes: singleSiteNodes,
        edges: <TopologyEdgeModel>[_edge(1, 1, 2), _edge(2, 2, 4)],
        mode: TopologyLayoutKind.grouped,
      );
      expect(out.clusters.length, 3); // Roteador, Switch, Servidor
      _expectNoOverlaps(out.positions);
    });

    test(
      'savedPositions are preserved and unsaved nodes are placed collision-free',
      () {
        final saved = <int, Offset>{
          1: const Offset(100, 100),
          2: const Offset(420, 100),
        };
        final subset = sampleNodes.take(5).toList();
        final pos = computeEngineLayout(
          nodes: subset,
          edges: sampleEdges,
          mode: TopologyLayoutKind.hierarchical,
          savedPositions: saved,
        );
        expect(pos[1], const Offset(100, 100));
        expect(pos[2], const Offset(420, 100));
        expect(pos.length, 5);
        _expectNoOverlaps(pos);
      },
    );

    test('empty graph returns empty layout and Rect.zero', () {
      final out = computeTopologyLayoutOutput(
        nodes: const <TopologyNodeModel>[],
        edges: const <TopologyEdgeModel>[],
        mode: TopologyLayoutKind.hierarchical,
      );
      expect(out.positions, isEmpty);
      expect(out.clusters, isEmpty);
      expect(out.bounds, Rect.zero);
    });

    test(
      'computeFitMatrix and computeFocusNodeMatrix center and clamp accurately',
      () {
        final positions = <int, Offset>{
          1: const Offset(100, 100),
          2: const Offset(900, 500),
        };
        final bounds = computeGraphBounds(positions);
        expect(bounds.left, 100);
        expect(bounds.top, 100);
        expect(bounds.right, 900 + kDefaultTopologyNodeSize.width);
        expect(bounds.bottom, 500 + kDefaultTopologyNodeSize.height);

        const viewport = Size(800, 600);
        final fit = computeFitMatrix(
          bounds,
          viewport,
          padding: const EdgeInsets.all(40),
          minScale: 0.2,
          maxScale: 1.5,
        );
        final scale = fit.getMaxScaleOnAxis();
        expect(scale, greaterThanOrEqualTo(0.2));
        expect(scale, lessThanOrEqualTo(1.5));

        final transformedCenter = MatrixUtils.transformPoint(
          fit,
          bounds.center,
        );
        expect(transformedCenter.dx, closeTo(viewport.width / 2, 0.5));
        expect(transformedCenter.dy, closeTo(viewport.height / 2, 0.5));

        final focus = computeFocusNodeMatrix(
          positions[2]!,
          viewport,
          targetScale: 1.25,
          viewportInset: const EdgeInsets.only(right: 200),
        );
        expect(focus.getMaxScaleOnAxis(), closeTo(1.25, 0.001));
        final nodeCenterScreen = MatrixUtils.transformPoint(
          focus,
          topologyNodeCenter(positions[2]!),
        );
        expect(nodeCenterScreen.dx, closeTo(300, 0.5));
        expect(nodeCenterScreen.dy, closeTo(300, 0.5));

        expect(
          computeFitMatrix(Rect.zero, Size.zero),
          equals(Matrix4.identity()),
        );
      },
    );

    test('snapOffsetToGrid snaps to multiples of gridSize', () {
      expect(snapOffsetToGrid(const Offset(35, 61)), const Offset(24, 72));
      expect(
        snapOffsetToGrid(const Offset(37, 61), gridSize: 20),
        const Offset(40, 60),
      );
    });
  });

  group('topology_geometry', () {
    final positions = <int, Offset>{
      1: const Offset(100, 100),
      2: const Offset(560, 100),
      3: const Offset(100, 420),
    };

    test('intersectRectBorder stops cleanly on rectangle border', () {
      final rect = topologyNodeRect(positions[1]!);
      final rightHit = intersectRectBorder(
        rect,
        rect.center + const Offset(500, 0),
      );
      expect(rightHit.dx, closeTo(rect.right, 0.01));
      expect(rightHit.dy, closeTo(rect.center.dy, 0.01));

      final bottomHit = intersectRectBorder(
        rect,
        rect.center + const Offset(0, 500),
      );
      expect(bottomHit.dx, closeTo(rect.center.dx, 0.01));
      expect(bottomHit.dy, closeTo(rect.bottom, 0.01));
    });

    test(
      'parallel multi-edges fan out without overlapping in curved and orthogonal styles',
      () {
        final parallelEdges = <TopologyEdgeModel>[
          _edge(1, 1, 2),
          _edge(2, 1, 2),
          _edge(3, 2, 1), // direção inversa no mesmo par!
        ];

        for (final style in TopologyEdgeStyle.values) {
          final paths = buildTopologyEdgePaths(
            edges: parallelEdges,
            positions: positions,
            style: style,
          );
          expect(paths.length, 3);
          expect(paths.every((p) => p.parallelCount == 3), isTrue);
          expect(paths.map((p) => p.parallelIndex).toSet(), <int>{0, 1, 2});

          // Os pontos médios dos 3 links paralelos precisam estar claramente separados.
          for (var i = 0; i < paths.length; i++) {
            final pathBounds = paths[i].toFlutterPath().getBounds();
            expect(pathBounds.width > 0 || pathBounds.height > 0, isTrue);
            for (var j = i + 1; j < paths.length; j++) {
              final sep = (paths[i].midpoint - paths[j].midpoint).distance;
              expect(
                sep,
                greaterThan(12.0),
                reason:
                    'Parallel edges $i and $j overlap in $style mode (sep=$sep)',
              );
            }
          }
        }
      },
    );

    test(
      'hitTestEdge detects taps along curved and orthogonal edges and selects closest parallel lane',
      () {
        final edges = <TopologyEdgeModel>[
          _edge(10, 1, 2),
          _edge(11, 1, 2),
          _edge(12, 1, 3),
        ];
        final paths = buildTopologyEdgePaths(
          edges: edges,
          positions: positions,
          style: TopologyEdgeStyle.curved,
        );

        final path10 = paths.firstWhere((p) => p.edge.id == 10);
        final path11 = paths.firstWhere((p) => p.edge.id == 11);
        final path12 = paths.firstWhere((p) => p.edge.id == 12);

        expect(hitTestEdge(path10.midpoint, paths)?.id, 10);
        expect(hitTestEdge(path11.midpoint, paths)?.id, 11);
        expect(
          hitTestEdge(
            path12.midpoint + const Offset(10, 0),
            paths,
            tolerance: 22,
          )?.id,
          12,
        );
        expect(hitTestEdge(const Offset(9999, 9999), paths), isNull);
      },
    );

    test(
      'sampleEdgePoint and computeAntDotPositions advance smoothly along path',
      () {
        final paths = buildTopologyEdgePaths(
          edges: <TopologyEdgeModel>[
            _edge(20, 1, 2, inBps: 400000000, speedBps: 1000000000),
            _edge(21, 1, 3, status: 'down'),
          ],
          positions: positions,
        );
        final activePath = paths.firstWhere((p) => p.edge.id == 20);
        final downPath = paths.firstWhere((p) => p.edge.id == 21);

        expect(
          (sampleEdgePoint(activePath, 0.0) - activePath.start).distance,
          lessThan(0.5),
        );
        expect(
          (sampleEdgePoint(activePath, 1.0) - activePath.end).distance,
          lessThan(0.5),
        );
        expect(
          (sampleEdgePoint(activePath, 0.5) - activePath.midpoint).distance,
          lessThan(0.5),
        );

        final speedActive = computeTrafficFlowSpeedFactor(activePath.edge);
        final speedDown = computeTrafficFlowSpeedFactor(downPath.edge);
        expect(speedActive, greaterThan(0.45));
        expect(speedDown, 0.0);

        final dots0 = computeAntDotPositions(
          activePath,
          0.1,
          speedMultiplier: speedActive,
        );
        final dots1 = computeAntDotPositions(
          activePath,
          0.4,
          speedMultiplier: speedActive,
        );
        expect(dots0, isNotEmpty);
        expect(dots0.length, dots1.length);
        expect(dots0.first.position, isNot(equals(dots1.first.position)));

        final dotsDown = computeAntDotPositions(
          downPath,
          0.3,
          speedMultiplier: speedDown,
        );
        expect(dotsDown, isEmpty);
      },
    );
  });

  group('topology_analysis', () {
    final nodes = <TopologyNodeModel>[
      _node(
        1,
        name: 'Gateway Borda',
        type: 'router',
        status: 'online',
        ipAddress: '10.0.0.1',
        vendor: 'MikroTik',
        siteId: 1,
        siteName: 'Matriz',
      ),
      _node(
        2,
        name: 'Switch Core',
        type: 'switch',
        status: 'online',
        ipAddress: '10.0.0.2',
        vendor: 'Ubiquiti',
        model: 'USW-Pro-24',
        siteId: 1,
        siteName: 'Matriz',
      ),
      _node(
        3,
        name: 'Switch Backup',
        type: 'unmanaged_switch',
        status: 'warning',
        ipAddress: '10.0.0.3',
        siteId: 1,
        siteName: 'Matriz',
      ),
      _node(
        4,
        name: 'Câmera Portaria',
        type: 'camera',
        status: 'offline',
        ipAddress: '10.0.1.50',
        vendor: 'Hikvision',
        siteId: 2,
        siteName: 'Portaria',
      ),
      _node(
        5,
        name: 'Servidor Isolado',
        type: 'server',
        status: 'online',
        ipAddress: '10.0.0.99',
        siteId: 1,
        siteName: 'Matriz',
      ),
    ];

    final edges = <TopologyEdgeModel>[
      _edge(1, 1, 2, confidence: 0.95, confirmed: true, speedBps: 10000000000),
      _edge(2, 1, 3, confidence: 0.4, confirmed: false, speedBps: 1000000000),
      _edge(3, 2, 4, confidence: 0.9, confirmed: true, linkType: 'fiber'),
      _edge(4, 3, 4, confidence: 0.3, confirmed: false, status: 'down'),
    ];

    test(
      'findShortestPath prefers fewest hops and breaks ties by confidence',
      () {
        final path = findShortestPath(edges, 1, 4);
        expect(path, isNotNull);
        expect(path!.hopCount, 2);
        // 1 -> 2 -> 4 tem confiança 100+100 vs 1 -> 3 -> 4 (40+30)
        expect(path.nodeIds, <int>[1, 2, 4]);
        expect(path.edgeIds, <int>[1, 3]);

        final selfPath = findShortestPath(edges, 2, 2);
        expect(selfPath!.hopCount, 0);
        expect(selfPath.nodeIds, <int>[2]);

        expect(findShortestPath(edges, 1, 5), isNull);
      },
    );

    test('traceUpstreamToRoot reaches component gateway', () {
      final upstream = traceUpstreamToRoot(nodes, edges, 4);
      expect(upstream.nodeIds.first, 4);
      expect(upstream.nodeIds.last, 1);
      expect(upstream.hopCount, 2);

      final isolated = traceUpstreamToRoot(nodes, edges, 5);
      expect(isolated.nodeIds, <int>[5]);
      expect(isolated.edgeIds, isEmpty);
    });

    test('connectedNeighbors returns sorted neighbors', () {
      final neighbors = connectedNeighbors(edges, 4);
      expect(neighbors.length, 2);
      // Link 3 (up) vem antes do link 4 (down)
      expect(neighbors.first.neighborId, 2);
      expect(neighbors.last.neighborId, 3);
    });

    test(
      'applyTopologyFilter supports statuses, types, onlyProblems, hideIsolated and dim/hide modes',
      () {
        final isolatedHidden = applyTopologyFilter(
          nodes: nodes,
          edges: edges,
          spec: const TopologyFilterSpec(hideIsolated: true),
        );
        expect(
          isolatedHidden.visibleNodes.map((n) => n.id),
          isNot(contains(5)),
        );

        final problemsOnlyDim = applyTopologyFilter(
          nodes: nodes,
          edges: edges,
          spec: const TopologyFilterSpec(
            onlyProblems: true,
            filterDisplayMode: TopologyFilterDisplayMode.dim,
          ),
        );
        expect(problemsOnlyDim.matchedNodeIds, containsAll(<int>[3, 4]));
        expect(problemsOnlyDim.dimmedNodeIds, containsAll(<int>[1, 2, 5]));
        expect(problemsOnlyDim.visibleNodes.length, 5);

        final switchOnlyHide = applyTopologyFilter(
          nodes: nodes,
          edges: edges,
          spec: const TopologyFilterSpec(
            types: <String>{'switch'},
            filterDisplayMode: TopologyFilterDisplayMode.hide,
          ),
        );
        // 'switch' casa com switch gerenciável e unmanaged_switch
        expect(switchOnlyHide.visibleNodes.map((n) => n.id).toSet(), <int>{
          2,
          3,
        });
      },
    );

    test(
      'searchTopologyNodes ranks partial multi-term, accents, IP, MAC and vendor',
      () {
        final swHits = searchTopologyNodes(nodes, 'sw cor');
        expect(swHits, isNotEmpty);
        expect(swHits.first.node.id, 2);
        expect(swHits.first.matchedField, TopologySearchField.name);

        final accentHits = searchTopologyNodes(nodes, 'camera');
        expect(accentHits.first.node.id, 4);

        final ipHits = searchTopologyNodes(nodes, '10.0.1.50');
        expect(ipHits.first.node.id, 4);
        expect(ipHits.first.matchedField, TopologySearchField.ip);

        final macHits = searchTopologyNodes(
          nodes,
          'aa:bb:cc',
          macByNodeId: const <int, String>{1: 'AA:BB:CC:11:22:33'},
        );
        expect(macHits.first.node.id, 1);
        expect(macHits.first.matchedField, TopologySearchField.mac);

        final vendorHits = searchTopologyNodes(nodes, 'mikrotik');
        expect(vendorHits.first.node.id, 1);
        expect(vendorHits.first.matchedField, TopologySearchField.vendor);
      },
    );

    test(
      'TopologyPrefsStorage persists and recovers safely on corrupt storage',
      () async {
        SharedPreferences.setMockInitialValues(<String, Object>{});
        final spec = const TopologyFilterSpec(
          layoutMode: TopologyLayoutKind.radial,
          overlayMode: TopologyOverlayMode.traffic,
          edgeStyle: TopologyEdgeStyle.orthogonal,
          showMinimap: true,
          snapToGrid: true,
          hideIsolated: true,
          onlyProblems: true,
        );
        await TopologyPrefsStorage.save(spec);
        final loaded = await TopologyPrefsStorage.load();
        expect(loaded, equals(spec));

        final prefs = await SharedPreferences.getInstance();
        await prefs.setString(TopologyPrefsStorage.storageKey, '{corrupt json');
        final fallback = await TopologyPrefsStorage.load(prefs: prefs);
        expect(fallback, equals(const TopologyFilterSpec()));

        await TopologyPrefsStorage.clear(prefs: prefs);
      },
    );
  });

  group('topology_presentation', () {
    test('status, utilization, labels and health summary work cleanly', () {
      final nodes = <TopologyNodeModel>[
        _node(1, status: 'online', cpuPercent: 32, memoryPercent: 55),
        _node(2, status: 'warning', cpuPercent: 94, memoryPercent: 60),
        _node(3, status: 'offline'),
        _node(4, status: 'unknown'),
      ];
      final upEdge = _edge(
        1,
        1,
        2,
        speedBps: 1000000000,
        inBps: 250000000,
        outBps: 750000000,
      );
      final downEdge = _edge(
        2,
        2,
        3,
        status: 'up',
        targetInterfaceStatus: 'down',
      );

      expect(classifyEdgeStatus(upEdge), TopologyEdgeStatus.up);
      expect(classifyEdgeStatus(downEdge), TopologyEdgeStatus.down);

      final util = resolveEdgeUtilization(upEdge);
      expect(util, closeTo(0.75, 0.001));
      expect(utilizationTone(util), SemanticTone.warning);
      expect(utilizationTone(0.95), SemanticTone.error);
      expect(utilizationTone(0.20), SemanticTone.success);

      expect(formatEdgeSpeedLabel(upEdge), '1 Gbps');
      expect(formatEdgeTrafficLabel(upEdge), 'RX 250 Mbps · TX 750 Mbps');
      expect(
        formatEdgeBadgeLabel(
          upEdge,
          overlay: TopologyOverlayMode.latency,
          latencyMs: 12.4,
        ),
        '12.4 ms',
      );

      expect(formatNodeHealthSummary(nodes[0]), 'CPU 32% · RAM 55%');
      expect(formatNodeHealthSummary(nodes[1]), '⚠ CPU 94% · RAM 60%');
      expect(formatNodeHealthSummary(nodes[2]), isNull);

      final summary = computeTopologyHealthSummary(nodes, <TopologyEdgeModel>[
        upEdge,
        downEdge,
      ]);
      expect(summary.total, 4);
      expect(summary.online, 1);
      expect(summary.warning, 1);
      expect(summary.offline, 1);
      expect(summary.unknown, 1);
      expect(summary.problemEdges, 2);
      expect(summary.compactLabel, '1 online · 1 alerta · 1 off');

      expect(kTopologyNodeStatusLegend, isNotEmpty);
      expect(kTopologyUtilizationLegend, isNotEmpty);
      expect(kTopologyLinkKindLegend, isNotEmpty);
      expect(kTopologySpeedTierLegend, isNotEmpty);
    });

    test(
      'buildTopologyEdgePaths anchors flush to circular ring in macroLod (scale < 0.55)',
      () {
        const positions = <int, Offset>{
          1: Offset(100, 300),
          2: Offset(380, 140),
          3: Offset(380, 420),
        };
        final edges = <TopologyEdgeModel>[
          _edge(10, 1, 2),
          _edge(11, 2, 3),
          _edge(12, 1, 2),
        ];

        final detailPaths = buildTopologyEdgePaths(
          edges: edges,
          positions: positions,
          macroLod: false,
        );
        final macroPaths = buildTopologyEdgePaths(
          edges: edges,
          positions: positions,
          macroLod: true,
        );

        final c1 = topologyNodeRect(positions[1]!).center;
        final c2 = topologyNodeRect(positions[2]!).center;
        final c3 = topologyNodeRect(positions[3]!).center;

        // Em modo detalhado, a ancoragem fica na borda do cartão completo (distância >> 27px).
        final detailVertical = detailPaths.singleWhere((p) => p.edge.id == 11);
        expect((detailVertical.start - c2).distance, closeTo(48.0, 0.5));

        // Em modo macro (ícone circular), todos os enlaces conectam direto no anel (~26px).
        for (final path in macroPaths) {
          final expectedSrc = path.edge.sourceDeviceId == 1 ? c1 : c2;
          final expectedTgt = path.edge.targetDeviceId == 2 ? c2 : c3;
          expect(
            (path.start - expectedSrc).distance,
            closeTo(kTopologyMacroNodeRadius - 1.0, 0.05),
          );
          expect(
            (path.end - expectedTgt).distance,
            closeTo(kTopologyMacroNodeRadius - 1.0, 0.05),
          );
        }

        // Enlaces paralelos abrem em ângulos distintos sobre o anel circular.
        final pA = macroPaths.singleWhere((p) => p.edge.id == 10);
        final pB = macroPaths.singleWhere((p) => p.edge.id == 12);
        expect((pA.start - pB.start).distance, greaterThan(6.5));

        // Ortogonal em modo macro também ancora no raio circular.
        final orthoMacro = buildTopologyEdgePaths(
          edges: <TopologyEdgeModel>[_edge(20, 1, 2)],
          positions: positions,
          style: TopologyEdgeStyle.orthogonal,
          macroLod: true,
        ).single;
        expect(
          (orthoMacro.start - c1).distance,
          lessThanOrEqualTo(kTopologyMacroNodeRadius + 0.5),
        );
        expect(
          (orthoMacro.end - c2).distance,
          lessThanOrEqualTo(kTopologyMacroNodeRadius + 0.5),
        );
      },
    );
  });
}
