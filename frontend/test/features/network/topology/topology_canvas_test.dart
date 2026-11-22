import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/canvas/topology_canvas.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

List<TopologyNodeModel> _sampleNodes() => const <TopologyNodeModel>[
  TopologyNodeModel(
    id: 1,
    name: 'Router Borda',
    type: 'router',
    status: 'online',
    siteId: 1,
    siteName: 'Matriz',
    ipAddress: '10.0.0.1',
    cpuPercent: 24,
    memoryPercent: 48,
  ),
  TopologyNodeModel(
    id: 2,
    name: 'Switch Core',
    type: 'switch',
    status: 'online',
    siteId: 1,
    siteName: 'Matriz',
    ipAddress: '10.0.0.2',
    cpuPercent: 42,
    memoryPercent: 61,
  ),
  TopologyNodeModel(
    id: 3,
    name: 'AP Recepção',
    type: 'access_point',
    status: 'warning',
    siteId: 1,
    siteName: 'Matriz',
    ipAddress: '10.0.0.20',
    monitorsDown: 1,
    cpuPercent: 78,
  ),
  TopologyNodeModel(
    id: 4,
    name: 'Switch Sala B',
    type: 'unmanaged_switch',
    status: 'online',
    siteId: 2,
    siteName: 'Filial',
  ),
  TopologyNodeModel(
    id: 5,
    name: 'Servidor ERP',
    type: 'server',
    status: 'offline',
    siteId: 2,
    siteName: 'Filial',
    ipAddress: '10.0.1.10',
    monitorsDown: 2,
    cpuPercent: 94,
    memoryPercent: 91,
  ),
];

List<TopologyEdgeModel> _sampleEdges() => const <TopologyEdgeModel>[
  TopologyEdgeModel(
    id: 101,
    sourceDeviceId: 1,
    targetDeviceId: 2,
    linkType: 'fiber',
    discoveryMethod: 'lldp',
    status: 'up',
    speedBps: 10000000000,
    inBps: 1200000000,
    outBps: 850000000,
    trafficBps: 2050000000,
    utilization: 0.21,
  ),
  TopologyEdgeModel(
    id: 102,
    sourceDeviceId: 2,
    targetDeviceId: 3,
    linkType: 'ethernet',
    discoveryMethod: 'lldp',
    status: 'up',
    speedBps: 1000000000,
    trafficBps: 760000000,
    utilization: 0.76,
  ),
  TopologyEdgeModel(
    id: 103,
    sourceDeviceId: 2,
    targetDeviceId: 4,
    linkType: 'wireless',
    discoveryMethod: 'manual',
    status: 'up',
    speedBps: 100000000,
  ),
  TopologyEdgeModel(
    id: 104,
    sourceDeviceId: 4,
    targetDeviceId: 5,
    linkType: 'ethernet',
    discoveryMethod: 'manual',
    status: 'down',
    speedBps: 1000000000,
  ),
];

Map<int, Offset> _fixedPositions() => const <int, Offset>{
  1: Offset(80, 120),
  2: Offset(440, 120),
  3: Offset(440, 340),
  4: Offset(800, 120),
  5: Offset(800, 340),
};

Widget _buildTestHarness({required ThemeData theme, required Widget child}) {
  return MaterialApp(
    theme: theme,
    home: Scaffold(body: child),
  );
}

void main() {
  group('TopologyCanvasView responsive & theme rendering', () {
    for (final viewport in <(String, Size)>[
      ('mobile 390x844', const Size(390, 844)),
      ('desktop 1280x800', const Size(1280, 800)),
    ]) {
      for (final themeEntry in <(String, ThemeData)>[
        ('dark', AppTheme.dark()),
        ('light', AppTheme.light()),
      ]) {
        testWidgets(
          'renders nodes, edges, clusters, traffic flow & minimap with zero overflow (${viewport.$1}, ${themeEntry.$1})',
          (tester) async {
            tester.view.physicalSize = viewport.$2;
            tester.view.devicePixelRatio = 1.0;
            addTearDown(() {
              tester.view.resetPhysicalSize();
              tester.view.resetDevicePixelRatio();
            });

            final nodes = _sampleNodes();
            final edges = _sampleEdges();
            final layout = computeTopologyLayoutOutput(
              nodes: nodes,
              edges: edges,
              mode: TopologyLayoutKind.grouped,
              respectSavedPositions: false,
            );
            final controller = TopologyCanvasController();
            addTearDown(controller.dispose);

            await tester.pumpWidget(
              _buildTestHarness(
                theme: themeEntry.$2,
                child: TopologyCanvasView(
                  nodes: nodes,
                  edges: edges,
                  positions: layout.positions,
                  clusters: layout.clusters,
                  controller: controller,
                  showMinimap: true,
                  showTrafficFlow: true,
                  showClusterBoxes: true,
                  overlayMode: TopologyOverlayMode.traffic,
                  nodeExtras: const <int, TopologyNodeExtras>{
                    1: TopologyNodeExtras(
                      latencyMs: 3.4,
                      hasPluginActions: true,
                    ),
                    3: TopologyNodeExtras(
                      latencyMs: 42.0,
                      alertCount: 1,
                      wifiClientsCount: 14,
                    ),
                  },
                  edgeLatencyMs: const <int, double>{101: 2.1, 102: 38.5},
                ),
              ),
            );

            await tester.pump();
            await tester.pump(const Duration(milliseconds: 120));

            expect(tester.takeException(), isNull);
            expect(
              find.byKey(const ValueKey<String>('topology-minimap')),
              findsOneWidget,
            );
            expect(
              find.byKey(const ValueKey<String>('topology-edges-canvas')),
              findsOneWidget,
            );
            expect(find.text('Router Borda'), findsOneWidget);
            expect(find.text('Switch Sala B'), findsOneWidget);
          },
        );
      }
    }
  });

  group('Semantic zoom LOD switching', () {
    testWidgets(
      'switches cleanly between macro (0.4x) and detail (1.0x) node LOD',
      (tester) async {
        tester.view.physicalSize = const Size(1280, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(() {
          tester.view.resetPhysicalSize();
          tester.view.resetDevicePixelRatio();
        });

        final controller = TopologyCanvasController();
        addTearDown(controller.dispose);

        await tester.pumpWidget(
          _buildTestHarness(
            theme: AppTheme.dark(),
            child: TopologyCanvasView(
              nodes: _sampleNodes(),
              edges: _sampleEdges(),
              positions: _fixedPositions(),
              controller: controller,
              autoFitOnFirstLayout: false,
              showTrafficFlow: false,
            ),
          ),
        );
        await tester.pumpAndSettle();

        // Escala inicial 1.0x -> cartões detalhados.
        expect(
          find.byKey(const ValueKey<String>('topology-node-detail-1')),
          findsOneWidget,
        );
        expect(
          find.byKey(const ValueKey<String>('topology-node-macro-1')),
          findsNothing,
        );

        // Escala 0.4x -> nós circulares macro (LOD).
        controller.setScale(0.4, animate: false);
        await tester.pump();

        expect(controller.scale, closeTo(0.4, 0.01));
        expect(
          find.byKey(const ValueKey<String>('topology-node-macro-1')),
          findsOneWidget,
        );
        expect(
          find.byKey(const ValueKey<String>('topology-node-detail-1')),
          findsNothing,
        );

        // Volta para 1.0x -> cartões detalhados novamente.
        controller.setScale(1.0, animate: false);
        await tester.pump();

        expect(controller.scale, closeTo(1.0, 0.01));
        expect(
          find.byKey(const ValueKey<String>('topology-node-detail-1')),
          findsOneWidget,
        );
      },
    );
  });

  group('Node & edge interaction, hit-testing, and path dimming', () {
    testWidgets(
      'node tap, double-tap, long-press, edge tap hit-test & empty-space tap',
      (tester) async {
        tester.view.physicalSize = const Size(1280, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(() {
          tester.view.resetPhysicalSize();
          tester.view.resetDevicePixelRatio();
        });

        TopologyNodeModel? tappedNode;
        TopologyNodeModel? doubleTappedNode;
        TopologyNodeModel? longPressedNode;
        TopologyEdgeModel? tappedEdge;
        var emptyTaps = 0;

        final nodes = _sampleNodes().take(2).toList();
        final edges = <TopologyEdgeModel>[_sampleEdges().first];
        const positions = <int, Offset>{
          1: Offset(100, 200),
          2: Offset(520, 200),
        };

        await tester.pumpWidget(
          _buildTestHarness(
            theme: AppTheme.light(),
            child: TopologyCanvasView(
              nodes: nodes,
              edges: edges,
              positions: positions,
              autoFitOnFirstLayout: false,
              showTrafficFlow: false,
              onTapNode: (n) => tappedNode = n,
              onDoubleTapNode: (n) => doubleTappedNode = n,
              onLongPressNode: (n) => longPressedNode = n,
              onTapEdge: (e) => tappedEdge = e,
              onTapEmptySpace: () => emptyTaps += 1,
            ),
          ),
        );
        await tester.pumpAndSettle();

        // 1) Tap em nó.
        await tester.tap(
          find.byKey(const ValueKey<String>('topology-node-detail-1')),
        );
        await tester.pump(kDoubleTapTimeout + const Duration(milliseconds: 50));
        expect(tappedNode?.id, 1);
        expect(emptyTaps, 0);

        // 2) Double-tap em nó.
        final node2Finder = find.byKey(
          const ValueKey<String>('topology-node-detail-2'),
        );
        await tester.tap(node2Finder);
        await tester.pump(const Duration(milliseconds: 80));
        await tester.tap(node2Finder);
        await tester.pumpAndSettle();
        expect(doubleTappedNode?.id, 2);

        // 3) Long-press em nó.
        await tester.longPress(node2Finder);
        await tester.pumpAndSettle();
        expect(longPressedNode?.id, 2);

        // 4) Hit-test em enlace: aresta 101 liga borda direita do nó 1 (x=304, y=248)
        //    até borda esquerda do nó 2 (x=520, y=248). Ponto médio = (412, 248).
        await tester.tapAt(const Offset(412, 248));
        await tester.pumpAndSettle();
        expect(tappedEdge?.id, 101);

        // 5) Toque em espaço vazio limpa seleção.
        await tester.tapAt(const Offset(40, 40));
        await tester.pumpAndSettle();
        expect(emptyTaps, 1);
      },
    );

    testWidgets('dims unrelated nodes when highlightedPath is active', (
      tester,
    ) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(() {
        tester.view.resetPhysicalSize();
        tester.view.resetDevicePixelRatio();
      });

      final nodes = _sampleNodes();
      final edges = _sampleEdges();
      final path = findShortestPath(edges, 1, 3);
      expect(path, isNotNull);

      await tester.pumpWidget(
        _buildTestHarness(
          theme: AppTheme.dark(),
          child: TopologyCanvasView(
            nodes: nodes,
            edges: edges,
            positions: _fixedPositions(),
            highlightedPath: path,
            selectedEdgeId: 101,
            autoFitOnFirstLayout: false,
            showTrafficFlow: false,
            overlayMode: TopologyOverlayMode.linkTest,
            linkTestOverlay: const <int, TopologyLinkTestEdgeOverlay>{
              101: TopologyLinkTestEdgeOverlay(
                label: '940 Mbps',
                tone: SemanticTone.success,
                active: true,
              ),
            },
          ),
        ),
      );
      await tester.pumpAndSettle();

      // Nó 1 está no caminho -> opacidade 1.0.
      final node1Opacity = tester.widget<AnimatedOpacity>(
        find.ancestor(
          of: find.byKey(const ValueKey<String>('topology-node-detail-1')),
          matching: find.byType(AnimatedOpacity),
        ),
      );
      expect(node1Opacity.opacity, 1.0);

      // Nó 5 está fora do caminho -> esmaecido (0.26).
      final node5Opacity = tester.widget<AnimatedOpacity>(
        find.ancestor(
          of: find.byKey(const ValueKey<String>('topology-node-detail-5')),
          matching: find.byType(AnimatedOpacity),
        ),
      );
      expect(node5Opacity.opacity, lessThan(0.4));
    });

    testWidgets(
      'node dragging respects editLayoutEnabled lock and snapToGrid',
      (tester) async {
        tester.view.physicalSize = const Size(1280, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(() {
          tester.view.resetPhysicalSize();
          tester.view.resetDevicePixelRatio();
        });

        Offset? lastMovedPos;
        Offset? finalDraggedPos;
        var editEnabled = false;

        Widget buildCanvas() {
          return _buildTestHarness(
            theme: AppTheme.dark(),
            child: TopologyCanvasView(
              nodes: _sampleNodes().take(2).toList(),
              edges: <TopologyEdgeModel>[_sampleEdges().first],
              positions: const <int, Offset>{
                1: Offset(96, 120),
                2: Offset(480, 120),
              },
              autoFitOnFirstLayout: false,
              showTrafficFlow: false,
              editLayoutEnabled: editEnabled,
              snapToGrid: true,
              onNodePositionChanged: (_, pos) => lastMovedPos = pos,
              onNodeDragEnd: (_, pos) => finalDraggedPos = pos,
            ),
          );
        }

        // Com edição bloqueada, arrastar o nó NÃO move a posição.
        await tester.pumpWidget(buildCanvas());
        await tester.pumpAndSettle();
        await tester.drag(
          find.byKey(const ValueKey<String>('topology-node-detail-1')),
          const Offset(50, 30),
        );
        await tester.pumpAndSettle();
        expect(lastMovedPos, isNull);

        // Habilitando edição, arrastar move alinhado à grade de 24px.
        editEnabled = true;
        await tester.pumpWidget(buildCanvas());
        await tester.pumpAndSettle();
        await tester.drag(
          find.byKey(const ValueKey<String>('topology-node-detail-1')),
          const Offset(50, 30),
        );
        await tester.pumpAndSettle();

        expect(lastMovedPos, isNotNull);
        expect(lastMovedPos!.dx % kTopologyGridSize, 0.0);
        expect(lastMovedPos!.dy % kTopologyGridSize, 0.0);
        expect(finalDraggedPos, isNotNull);
      },
    );

    testWidgets(
      'dragging the same node repeatedly A -> B -> C (negative coords) -> D works across touch & mouse and different nodes',
      (tester) async {
        tester.view.physicalSize = const Size(1280, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(() {
          tester.view.resetPhysicalSize();
          tester.view.resetDevicePixelRatio();
        });

        final controller = TopologyCanvasController();
        addTearDown(controller.dispose);
        var positions = <int, Offset>{
          1: const Offset(240, 220),
          2: const Offset(620, 220),
        };
        // Centraliza a câmera para que coordenadas negativas fiquem visíveis no viewport.
        controller.panToWorld(const Offset(240, 220), animate: false);

        await tester.pumpWidget(
          _buildTestHarness(
            theme: AppTheme.dark(),
            child: StatefulBuilder(
              builder: (context, setLocalState) {
                return TopologyCanvasView(
                  nodes: _sampleNodes().take(2).toList(),
                  edges: <TopologyEdgeModel>[_sampleEdges().first],
                  positions: positions,
                  controller: controller,
                  autoFitOnFirstLayout: false,
                  showTrafficFlow: false,
                  editLayoutEnabled: true,
                  snapToGrid: false,
                  onNodeDragEnd: (nodeId, finalPos) {
                    setLocalState(() {
                      positions = <int, Offset>{...positions, nodeId: finalPos};
                    });
                  },
                );
              },
            ),
          ),
        );
        await tester.pumpAndSettle();
        controller.panToWorld(const Offset(200, 180), animate: false);
        await tester.pumpAndSettle();

        final node1Finder = find.byKey(
          const ValueKey<String>('topology-node-detail-1'),
        );
        final node2Finder = find.byKey(
          const ValueKey<String>('topology-node-detail-2'),
        );

        // Arraste 1 (A -> B): toque simples.
        final posA = positions[1]!;
        await tester.drag(node1Finder, const Offset(110, 70));
        await tester.pumpAndSettle();
        final posB = positions[1]!;
        expect(posB.dx, closeTo(posA.dx + 110, 2.0));
        expect(posB.dy, closeTo(posA.dy + 70, 2.0));

        // Arraste 2 (B -> C): move o MESMO nó para coordenadas NEGATIVAS (x < 0, y < 0)!
        await tester.drag(node1Finder, const Offset(-450, -360));
        await tester.pumpAndSettle();
        final posC = positions[1]!;
        expect(posC.dx, lessThan(0));
        expect(posC.dy, lessThan(0));
        expect(posC.dx, closeTo(posB.dx - 450, 2.0));
        expect(posC.dy, closeTo(posB.dy - 360, 2.0));

        // Arraste 3 (C -> D): arrasta NOVAMENTE com MOUSE a partir das coordenadas negativas!
        final mouseStart = tester.getCenter(node1Finder);
        final mouseGesture = await tester.startGesture(
          mouseStart,
          kind: PointerDeviceKind.mouse,
        );
        await mouseGesture.moveBy(const Offset(220, 180));
        await tester.pump();
        await mouseGesture.up();
        await tester.pumpAndSettle();
        final posD = positions[1]!;
        expect(posD.dx, closeTo(posC.dx + 220, 2.0));
        expect(posD.dy, closeTo(posC.dy + 180, 2.0));

        // Arraste 4: arrasta outro nó (#2) sem afetar o nó #1.
        final node2Initial = positions[2]!;
        await tester.drag(node2Finder, const Offset(85, 65));
        await tester.pumpAndSettle();
        expect(positions[2]!.dx, closeTo(node2Initial.dx + 85, 2.0));
        expect(positions[2]!.dy, closeTo(node2Initial.dy + 65, 2.0));
        expect(positions[1], posD);
      },
    );

    testWidgets(
      'node dragging tracks 1:1 with pointer at zoomed-out (0.5x) and zoomed-in (1.6x) scales',
      (tester) async {
        tester.view.physicalSize = const Size(1280, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(() {
          tester.view.resetPhysicalSize();
          tester.view.resetDevicePixelRatio();
        });

        final controller = TopologyCanvasController();
        addTearDown(controller.dispose);
        var positions = <int, Offset>{
          1: const Offset(200, 200),
          2: const Offset(620, 200),
        };

        await tester.pumpWidget(
          _buildTestHarness(
            theme: AppTheme.dark(),
            child: StatefulBuilder(
              builder: (context, setLocalState) {
                return TopologyCanvasView(
                  nodes: _sampleNodes().take(2).toList(),
                  edges: <TopologyEdgeModel>[_sampleEdges().first],
                  positions: positions,
                  controller: controller,
                  autoFitOnFirstLayout: false,
                  showTrafficFlow: false,
                  editLayoutEnabled: true,
                  snapToGrid: false,
                  onNodeDragEnd: (nodeId, finalPos) {
                    setLocalState(() {
                      positions = <int, Offset>{...positions, nodeId: finalPos};
                    });
                  },
                );
              },
            ),
          ),
        );
        await tester.pumpAndSettle();

        // Zoom out 0.5x (Macro LOD): 50px na tela = 100px no mundo (1:1 sem dupla divisão).
        controller.setScale(0.5, animate: false);
        await tester.pumpAndSettle();
        final beforeZoomOut = positions[1]!;
        await tester.drag(
          find.byKey(const ValueKey<String>('topology-node-macro-1')),
          const Offset(50, 30),
        );
        await tester.pumpAndSettle();
        expect(positions[1]!.dx, closeTo(beforeZoomOut.dx + 100, 3.0));
        expect(positions[1]!.dy, closeTo(beforeZoomOut.dy + 60, 3.0));

        // Zoom in 1.6x (Detail LOD): 80px na tela = 50px no mundo.
        controller.setScale(1.6, animate: false);
        await tester.pumpAndSettle();
        controller.focusNode(1, targetScale: 1.6, animate: false);
        await tester.pumpAndSettle();
        final beforeZoomIn = positions[1]!;
        await tester.drag(
          find.byKey(const ValueKey<String>('topology-node-detail-1')),
          const Offset(80, 48),
        );
        await tester.pumpAndSettle();
        expect(positions[1]!.dx, closeTo(beforeZoomIn.dx + 50, 3.0));
        expect(positions[1]!.dy, closeTo(beforeZoomIn.dy + 30, 3.0));
      },
    );

    testWidgets(
      'empty-space pan still works in edit mode, pointer cancel unlocks drag, and OFF/ON toggle preserves repeatability',
      (tester) async {
        tester.view.physicalSize = const Size(1280, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(() {
          tester.view.resetPhysicalSize();
          tester.view.resetDevicePixelRatio();
        });

        final controller = TopologyCanvasController();
        addTearDown(controller.dispose);
        var editEnabled = true;
        var positions = <int, Offset>{
          1: const Offset(220, 220),
          2: const Offset(620, 220),
        };
        late StateSetter setHostState;

        await tester.pumpWidget(
          _buildTestHarness(
            theme: AppTheme.dark(),
            child: StatefulBuilder(
              builder: (context, setLocalState) {
                setHostState = setLocalState;
                return TopologyCanvasView(
                  nodes: _sampleNodes().take(2).toList(),
                  edges: <TopologyEdgeModel>[_sampleEdges().first],
                  positions: positions,
                  controller: controller,
                  autoFitOnFirstLayout: false,
                  showTrafficFlow: false,
                  editLayoutEnabled: editEnabled,
                  snapToGrid: false,
                  onNodeDragEnd: (nodeId, finalPos) {
                    setLocalState(() {
                      positions = <int, Offset>{...positions, nodeId: finalPos};
                    });
                  },
                );
              },
            ),
          ),
        );
        await tester.pumpAndSettle();

        final node1Finder = find.byKey(
          const ValueKey<String>('topology-node-detail-1'),
        );

        // 1) Arraste em espaço vazio ainda move a câmera mesmo em modo de edição.
        final txBeforePan = controller.transform.getTranslation().x;
        await tester.dragFrom(const Offset(40, 40), const Offset(100, 60));
        await tester.pumpAndSettle();
        expect(
          controller.transform.getTranslation().x,
          isNot(closeTo(txBeforePan, 1.0)),
        );
        expect(positions[1], const Offset(220, 220));

        // 2) Pointer cancel no meio do arraste limpa o estado sem travar o nó.
        final center = tester.getCenter(node1Finder);
        final cancelledGesture = await tester.startGesture(center);
        await cancelledGesture.moveBy(const Offset(60, 40));
        await tester.pump();
        await cancelledGesture.cancel();
        await tester.pumpAndSettle();

        // 3) Arraste imediatamente após cancelamento funciona normalmente.
        final beforeAfterCancel = positions[1]!;
        await tester.drag(node1Finder, const Offset(90, 50));
        await tester.pumpAndSettle();
        expect(positions[1]!.dx, closeTo(beforeAfterCancel.dx + 90, 2.0));

        // 4) Desliga modo edição -> arraste NÃO move o nó.
        setHostState(() => editEnabled = false);
        await tester.pumpAndSettle();
        final lockedPos = positions[1]!;
        await tester.drag(node1Finder, const Offset(80, 40));
        await tester.pumpAndSettle();
        expect(positions[1], lockedPos);

        // 5) Liga modo edição novamente -> arraste volta a mover o nó imediatamente.
        setHostState(() => editEnabled = true);
        await tester.pumpAndSettle();
        await tester.drag(node1Finder, const Offset(75, -45));
        await tester.pumpAndSettle();
        expect(positions[1]!.dx, closeTo(lockedPos.dx + 75, 2.0));
        expect(positions[1]!.dy, closeTo(lockedPos.dy - 45, 2.0));
      },
    );
  });

  group('Minimap & TopologyCanvasController', () {
    testWidgets('minimap renders and tapping pans the camera smoothly', (
      tester,
    ) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(() {
        tester.view.resetPhysicalSize();
        tester.view.resetDevicePixelRatio();
      });

      final controller = TopologyCanvasController();
      addTearDown(controller.dispose);

      await tester.pumpWidget(
        _buildTestHarness(
          theme: AppTheme.dark(),
          child: TopologyCanvasView(
            nodes: _sampleNodes(),
            edges: _sampleEdges(),
            positions: _fixedPositions(),
            controller: controller,
            showMinimap: true,
            showTrafficFlow: false,
            autoFitOnFirstLayout: false,
          ),
        ),
      );
      await tester.pumpAndSettle();

      final minimapFinder = find.byKey(
        const ValueKey<String>('topology-minimap'),
      );
      expect(minimapFinder, findsOneWidget);

      final beforeTx = controller.transform.getTranslation().x;
      final minimapBox = tester.getRect(minimapFinder);
      await tester.tapAt(minimapBox.bottomRight - const Offset(16, 16));
      await tester.pumpAndSettle();

      final afterTx = controller.transform.getTranslation().x;
      expect(afterTx, isNot(equals(beforeTx)));

      // Controller zoomIn / zoomOut / focusNode / fitToScreen animam sem erro.
      controller.zoomIn();
      await tester.pumpAndSettle();
      expect(controller.scale, greaterThan(1.0));

      controller.zoomOut();
      await tester.pumpAndSettle();
      expect(controller.scale, closeTo(1.0, 0.1));

      controller.focusNode(5, targetScale: 1.4);
      await tester.pumpAndSettle();
      expect(controller.scale, closeTo(1.4, 0.05));

      controller.fitToScreen();
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
    });
  });
}
