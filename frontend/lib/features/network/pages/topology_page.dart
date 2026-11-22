import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/models/link_test_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/diagnostics_provider.dart';
import 'package:netmonitor_app/features/network/providers/link_tests_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/features/network/providers/topology_provider.dart';
import 'package:netmonitor_app/features/network/providers/wifi_provider.dart';
import 'package:netmonitor_app/features/network/topology/canvas/topology_canvas.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/features/network/topology/widgets/topology_widgets.dart';
import 'package:netmonitor_app/features/network/widgets/device_form_sheet.dart';
import 'package:netmonitor_app/features/network/widgets/diagnostics_dialogs.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';

/// Página orquestradora da Topologia de Rede (Material Design 3, mobile-first + desktop).
/// Toda a atualização ao vivo chega exclusivamente pelo barramento SSE (`topologyProvider`).
class TopologyPage extends ConsumerStatefulWidget {
  const TopologyPage({super.key});

  @override
  ConsumerState<TopologyPage> createState() => _TopologyPageState();
}

@immutable
class _NodeMoveUndoEntry {
  const _NodeMoveUndoEntry({
    required this.nodeId,
    required this.previousPosition,
    required this.wasPreviouslyDragged,
  });

  final int nodeId;
  final Offset previousPosition;
  final bool wasPreviouslyDragged;
}

class _TopologyPageState extends ConsumerState<TopologyPage> {
  final TopologyCanvasController _canvasCtrl = TopologyCanvasController();

  TopologyFilterSpec _filterSpec = const TopologyFilterSpec();
  Map<int, Offset> _customPositions = const <int, Offset>{};
  Map<int, Offset> _lastResolvedPositions = const <int, Offset>{};
  Map<int, Offset>? _preDragSnapshot;
  final Set<int> _draggedNodeIds = <int>{};
  final List<_NodeMoveUndoEntry> _undoStack = <_NodeMoveUndoEntry>[];

  int? _selectedNodeId;
  int? _selectedEdgeId;
  int? _pathPickingSourceId;
  TopologyPathResult? _activePath;

  bool _editLayoutEnabled = false;
  bool _forceEngineLayout = false;
  int _acknowledgedRemoteRevision = 0;

  bool _pingLoading = false;
  QuickPingReport? _pingReport;
  String? _pingError;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_bootstrap);
  }

  Future<void> _bootstrap() async {
    try {
      final saved = await TopologyPrefsStorage.load();
      if (mounted) setState(() => _filterSpec = saved);
    } catch (_) {}
    if (!mounted) return;
    _acknowledgedRemoteRevision = ref
        .read(topologyProvider)
        .remoteLayoutRevision;
    await Future.wait<void>(<Future<void>>[
      ref.read(sitesProvider.notifier).fetchSites(),
      ref.read(topologyProvider.notifier).fetchTopology(),
    ]);
  }

  @override
  void dispose() {
    _canvasCtrl.dispose();
    super.dispose();
  }

  void _updateFilterSpec(TopologyFilterSpec next) {
    setState(() => _filterSpec = next);
    TopologyPrefsStorage.save(next).catchError((_) {});
  }

  void _applySiteFilter(int? siteId) {
    _updateFilterSpec(
      _filterSpec.copyWith(siteId: siteId, clearSiteId: siteId == null),
    );
    setState(() {
      _customPositions = const <int, Offset>{};
      _draggedNodeIds.clear();
      _undoStack.clear();
      _preDragSnapshot = null;
    });
    ref
        .read(topologyProvider.notifier)
        .fetchTopology(siteId: siteId, updateSiteFilter: true);
  }

  void _applyLayoutMode(TopologyLayoutKind kind) {
    final topo = ref.read(topologyProvider);
    final computed = computeTopologyLayoutOutput(
      nodes: topo.nodes,
      edges: topo.edges,
      mode: kind,
      respectSavedPositions: false,
    );
    _updateFilterSpec(_filterSpec.copyWith(layoutMode: kind));
    setState(() {
      _customPositions = computed.positions;
      _forceEngineLayout = false;
      _draggedNodeIds.clear();
      _undoStack.clear();
      _preDragSnapshot = null;
    });
    ref.read(topologyProvider.notifier).clearUnsavedPositions();
    for (final entry in computed.positions.entries) {
      ref
          .read(topologyProvider.notifier)
          .moveNode(
            entry.key,
            TopologyPosition(entry.value.dx, entry.value.dy),
            markUnsaved: false,
          );
    }
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _canvasCtrl.fitToScreen();
    });
  }

  void _handleNodeDragCommitted(int nodeId, Offset finalPos) {
    if (!_editLayoutEnabled) return;
    final topo = ref.read(topologyProvider);
    final base = _customPositions.isNotEmpty
        ? _customPositions
        : (_lastResolvedPositions.isNotEmpty
              ? _lastResolvedPositions
              : offsetsFromTopologyPositions(topo.positions));
    final prevPos = base[nodeId];
    if (prevPos != null && (prevPos - finalPos).distance < 0.5) return;
    _preDragSnapshot ??= Map<int, Offset>.from(base);
    setState(() {
      if (prevPos != null) {
        _undoStack.add(
          _NodeMoveUndoEntry(
            nodeId: nodeId,
            previousPosition: prevPos,
            wasPreviouslyDragged: _draggedNodeIds.contains(nodeId),
          ),
        );
      }
      _draggedNodeIds.add(nodeId);
      _customPositions = <int, Offset>{...base, nodeId: finalPos};
    });
    ref
        .read(topologyProvider.notifier)
        .moveNode(nodeId, TopologyPosition(finalPos.dx, finalPos.dy));
  }

  void _undoLastDrag() {
    if (_undoStack.isEmpty) return;
    final entry = _undoStack.removeLast();
    final base = _customPositions.isNotEmpty
        ? _customPositions
        : _lastResolvedPositions;
    final nextPositions = <int, Offset>{
      ...base,
      entry.nodeId: entry.previousPosition,
    };
    final notifier = ref.read(topologyProvider.notifier);
    setState(() {
      if (!entry.wasPreviouslyDragged) {
        _draggedNodeIds.remove(entry.nodeId);
      }
      if (_undoStack.isEmpty) {
        _draggedNodeIds.clear();
        _preDragSnapshot = null;
      }
      _customPositions = nextPositions;
    });
    final stillUnsaved = _draggedNodeIds.contains(entry.nodeId);
    notifier.moveNode(
      entry.nodeId,
      TopologyPosition(entry.previousPosition.dx, entry.previousPosition.dy),
      markUnsaved: stillUnsaved,
    );
    if (_undoStack.isEmpty) {
      notifier.clearUnsavedPositions();
    }
  }

  Future<void> _saveLayout({List<int>? onlyNodeIds}) async {
    final overridePositions = <int, TopologyPosition>{
      for (final entry in _customPositions.entries)
        entry.key: TopologyPosition(entry.value.dx, entry.value.dy),
    };
    final ok = await ref
        .read(topologyProvider.notifier)
        .saveLayout(
          onlyNodeIds: onlyNodeIds,
          overridePositions: overridePositions.isNotEmpty
              ? overridePositions
              : null,
        );
    if (!mounted) return;
    if (ok) {
      setState(() {
        _draggedNodeIds.clear();
        _undoStack.clear();
        _preDragSnapshot = null;
      });
    }
    final msg = ok
        ? 'Posições da topologia salvas.'
        : (ref.read(topologyProvider).actionError ?? 'Falha ao salvar layout.');
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(msg)));
  }

  void _discardDraggedLayout() {
    final snapshot = _preDragSnapshot;
    if (snapshot == null) return;
    setState(() {
      _customPositions = Map<int, Offset>.from(snapshot);
      _draggedNodeIds.clear();
      _undoStack.clear();
      _preDragSnapshot = null;
    });
    final notifier = ref.read(topologyProvider.notifier);
    notifier.clearUnsavedPositions();
    for (final entry in snapshot.entries) {
      notifier.moveNode(
        entry.key,
        TopologyPosition(entry.value.dx, entry.value.dy),
        markUnsaved: false,
      );
    }
  }

  void _focusAndSelectNode(TopologyNodeModel node, {bool openSheet = true}) {
    final isWide = MediaQuery.sizeOf(context).width >= 900;
    setState(() {
      _selectedNodeId = node.id;
      _selectedEdgeId = null;
      _pingLoading = false;
      _pingReport = null;
      _pingError = null;
    });
    _canvasCtrl.focusNode(
      node.id,
      viewportInset: isWide
          ? const EdgeInsets.only(right: 372, top: 84)
          : const EdgeInsets.only(top: 84, bottom: 180),
    );
    if (!isWide && openSheet) _openMobileNodeInspector(node);
  }

  void _handleNodeTap(TopologyNodeModel node) {
    if (_pathPickingSourceId != null) {
      final startId = _pathPickingSourceId!;
      setState(() => _pathPickingSourceId = null);
      _tracePathBetween(startId, node.id);
      return;
    }
    _focusAndSelectNode(node);
  }

  void _handleEdgeTap(TopologyEdgeModel edge) {
    final isWide = MediaQuery.sizeOf(context).width >= 900;
    setState(() {
      _selectedEdgeId = edge.id;
      _selectedNodeId = null;
    });
    if (!isWide) _openMobileEdgeInspector(edge);
  }

  void _tracePathBetween(int fromId, int toId) {
    final path = findShortestPath(
      ref.read(topologyProvider).edges,
      fromId,
      toId,
    );
    if (path == null) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('Nenhum caminho encontrado entre os equipamentos.'),
        ),
      );
      return;
    }
    setState(() {
      _activePath = path;
      _pathPickingSourceId = null;
    });
    ref.read(topologyProvider.notifier).fetchPath(fromId, toId);
  }

  void _traceUpstream(TopologyNodeModel node) {
    final topo = ref.read(topologyProvider);
    final path = traceUpstreamToRoot(topo.nodes, topo.edges, node.id);
    if (path.nodeIds.length < 2) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('${node.name} já é uma raiz ou está isolado no grafo.'),
        ),
      );
      return;
    }
    setState(() {
      _activePath = path;
      _pathPickingSourceId = null;
    });
  }

  void _clearPath() {
    setState(() {
      _activePath = null;
      _pathPickingSourceId = null;
    });
    ref.read(topologyProvider.notifier).clearHighlightedPath();
  }

  Future<void> _runDesktopQuickPing(TopologyNodeModel node) async {
    setState(() {
      _pingLoading = true;
      _pingError = null;
    });
    try {
      final report = await ref
          .read(diagnosticsServiceProvider)
          .quickPing(node.id);
      if (mounted && _selectedNodeId == node.id) {
        setState(() {
          _pingReport = report;
          _pingLoading = false;
        });
      }
    } catch (err) {
      if (mounted && _selectedNodeId == node.id) {
        setState(() {
          _pingError = err.toString();
          _pingLoading = false;
        });
      }
    }
  }

  Future<void> _editNodeDevice(TopologyNodeModel node) async {
    final existing = ref
        .read(devicesProvider)
        .items
        .where((d) => d.id == node.id)
        .firstOrNull;
    if (existing == null) {
      context.go('/devices/${node.id}');
      return;
    }
    final saved = await showDeviceFormSheet(
      context: context,
      existing: existing,
    );
    if (saved != null && mounted) {
      ref.read(topologyProvider.notifier).fetchTopology(showLoading: false);
    }
  }

  void _startNodeLinkTest(TopologyNodeModel node) {
    ref.read(pendingLinkTestDraftProvider.notifier).state =
        LinkTestRequestDraft(kind: 'server', targetDeviceId: node.id);
    context.go('/link-tests?deviceId=${node.id}');
  }

  void _startEdgeLinkTest(TopologyEdgeModel edge) {
    ref.read(pendingLinkTestDraftProvider.notifier).state =
        LinkTestRequestDraft(kind: 'link', linkId: edge.id);
    context.go('/link-tests');
  }

  void _openMobileNodeInspector(TopologyNodeModel node) {
    final topo = ref.read(topologyProvider);
    showTopologyNodeInspectorSheet(
      context: context,
      node: node,
      allNodes: topo.nodes,
      allEdges: topo.edges,
      pluginActions: topo.pluginActions[node.id] ?? const [],
      canWrite: ref.read(authProvider).canWrite,
      onRunQuickPing: (n) =>
          ref.read(diagnosticsServiceProvider).quickPing(n.id),
      onOpenDetails: () => context.go('/devices/${node.id}'),
      onTracePathFromNode: () => _openPathTargetPicker(node),
      onTraceUpstream: () => _traceUpstream(node),
      onTraceroute: () =>
          showTracerouteDialog(context, initialHost: node.ipAddress),
      onStartLinkTest: () => _startNodeLinkTest(node),
      onNewLinkFromNode: () => _openLinkForm(initialSourceId: node.id),
      onEditDevice: () => _editNodeDevice(node),
      onOpenPluginsTab: () => context.go('/devices/${node.id}?tab=plugins'),
      onFocusNeighbor: _focusAndSelectNode,
      onInspectEdge: _handleEdgeTap,
    );
  }

  void _openMobileEdgeInspector(TopologyEdgeModel edge) {
    showTopologyEdgeInspectorSheet(
      context: context,
      edge: edge,
      allNodes: ref.read(topologyProvider).nodes,
      canWrite: ref.read(authProvider).canWrite,
      onFocusNode: _focusAndSelectNode,
      onStartLinkTest: () => _startEdgeLinkTest(edge),
      onConfirmLink: () => _confirmEdge(edge),
      onEditLink: () => _openLinkForm(existingEdge: edge),
      onDeleteLink: () => _deleteEdge(edge),
    );
  }

  void _openPathTargetPicker(TopologyNodeModel source) {
    showTopologyPathTargetSheet(
      context: context,
      sourceNode: source,
      nodes: ref.read(topologyProvider).nodes,
      onSelectTarget: (target) => _tracePathBetween(source.id, target.id),
      onPickOnCanvas: () => setState(() => _pathPickingSourceId = source.id),
    );
  }

  Future<void> _openLinkForm({
    int? initialSourceId,
    TopologyEdgeModel? existingEdge,
  }) => showTopologyLinkFormSheet(
    context: context,
    nodes: ref.read(topologyProvider).nodes,
    initialSourceId: initialSourceId,
    existingEdge: existingEdge,
    onSubmit: (payload) => existingEdge != null
        ? ref
              .read(topologyProvider.notifier)
              .updateLink(existingEdge.id, payload)
        : ref.read(topologyProvider.notifier).addLink(payload),
  );

  Future<void> _confirmEdge(TopologyEdgeModel edge) =>
      ref.read(topologyProvider.notifier).updateLink(edge.id, <String, dynamic>{
        'sourceInterfaceId': edge.sourceInterfaceId,
        'targetInterfaceId': edge.targetInterfaceId,
        'linkType': edge.linkType,
      });

  Future<void> _deleteEdge(TopologyEdgeModel edge) async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Remover enlace',
      message: 'Remover este enlace da topologia?',
      confirmLabel: 'Remover',
    );
    if (!ok || !mounted) return;
    final deleted = await ref
        .read(topologyProvider.notifier)
        .deleteLink(edge.id);
    if (deleted && mounted && _selectedEdgeId == edge.id) {
      setState(() => _selectedEdgeId = null);
    }
  }

  void _handleHudAction(TopologyHudAction action) {
    final topo = ref.read(topologyProvider);
    switch (action) {
      case TopologyHudAction.saveLayout:
        _saveLayout();
      case TopologyHudAction.resetLayout:
        setState(() {
          _draggedNodeIds.clear();
          _undoStack.clear();
          _preDragSnapshot = null;
          _forceEngineLayout = true;
        });
        _applyLayoutMode(_filterSpec.layoutMode);
      case TopologyHudAction.recalculate:
        ref.read(topologyProvider.notifier).recalculateTopology();
      case TopologyHudAction.addLink:
        _openLinkForm(initialSourceId: _selectedNodeId);
      case TopologyHudAction.unmanagedSwitch:
        showUnmanagedSwitchSheet(
          context: context,
          sites: ref.read(sitesProvider).items,
          initialSiteId: topo.siteId,
          onSubmit: (p) =>
              ref.read(topologyProvider.notifier).createUnmanagedSwitch(p),
        );
      case TopologyHudAction.pathFinder:
        if (_pathPickingSourceId != null) {
          setState(() => _pathPickingSourceId = null);
        } else {
          final start =
              (_selectedNodeId != null
                  ? topo.nodes.where((n) => n.id == _selectedNodeId).firstOrNull
                  : null) ??
              topo.nodes.firstOrNull;
          if (start != null) _openPathTargetPicker(start);
        }
      case TopologyHudAction.legend:
        showTopologyLegendSheet(context);
      case TopologyHudAction.refresh:
        setState(() {
          _customPositions = const <int, Offset>{};
          _draggedNodeIds.clear();
          _undoStack.clear();
          _preDragSnapshot = null;
          _acknowledgedRemoteRevision = topo.remoteLayoutRevision;
        });
        ref.read(topologyProvider.notifier).fetchTopology();
    }
  }

  void _toggleStatusFilter({
    required bool onlyProblems,
    bool offlineOnly = false,
  }) {
    final topo = ref.read(topologyProvider);
    if (offlineOnly) {
      final active = _filterSpec.statuses.contains(TopologyNodeStatus.offline);
      _updateFilterSpec(
        _filterSpec.copyWith(
          statuses: active
              ? const <TopologyNodeStatus>{}
              : const <TopologyNodeStatus>{TopologyNodeStatus.offline},
        ),
      );
      if (!active) {
        final target = topo.nodes
            .where(
              (n) => classifyNodeStatus(n.status) == TopologyNodeStatus.offline,
            )
            .firstOrNull;
        if (target != null) _canvasCtrl.focusNode(target.id);
      }
      return;
    }
    final nextOnly = !_filterSpec.onlyProblems;
    _updateFilterSpec(_filterSpec.copyWith(onlyProblems: nextOnly));
    if (nextOnly) {
      final problem = topo.nodes.where((n) {
        final s = classifyNodeStatus(n.status);
        return s == TopologyNodeStatus.warning ||
            s == TopologyNodeStatus.offline;
      }).firstOrNull;
      if (problem != null) _canvasCtrl.focusNode(problem.id);
    }
  }

  @override
  Widget build(BuildContext context) {
    final topo = ref.watch(topologyProvider);
    final sites = ref.watch(sitesProvider).items;
    final devices = ref.watch(devicesProvider).items;
    final linkTests = ref.watch(linkTestsProvider).allSorted;
    final wifiSnapshots = ref.watch(wifiClientsProvider).snapshots;
    final canWrite = ref.watch(authProvider).canWrite;

    final macIndex = buildTopologyMacIndex(devices);
    final effectiveSpec = _filterSpec.copyWith(
      siteId: topo.siteId,
      clearSiteId: topo.siteId == null,
    );
    final filterResult = applyTopologyFilter(
      nodes: topo.nodes,
      edges: topo.edges,
      spec: effectiveSpec,
      macByNodeId: macIndex,
    );
    final layoutOutput = resolveTopologyLayoutState(
      visibleNodes: filterResult.visibleNodes,
      visibleEdges: filterResult.visibleEdges,
      layoutMode: effectiveSpec.layoutMode,
      providerPositions: topo.positions,
      layoutOverridePositions: _customPositions,
      useEngineLayout: _forceEngineLayout,
    );
    _lastResolvedPositions = layoutOutput.positions;
    final activePath =
        _activePath ??
        (topo.highlightedPath != null
            ? TopologyPathResult.fromModel(topo.highlightedPath!)
            : null);
    final isWide = MediaQuery.sizeOf(context).width >= 900;
    final selectedNode = _selectedNodeId != null
        ? topo.nodes.where((n) => n.id == _selectedNodeId).firstOrNull
        : null;
    final selectedEdge = _selectedEdgeId != null
        ? topo.edges.where((e) => e.id == _selectedEdgeId).firstOrNull
        : null;
    final hasLiveTraffic = filterResult.visibleEdges.any(
      (e) =>
          (e.trafficBps ?? 0) > 0 || (e.inBps ?? 0) > 0 || (e.outBps ?? 0) > 0,
    );
    final rightInset = isWide && (selectedNode != null || selectedEdge != null)
        ? 372.0
        : 0.0;

    return Stack(
      children: <Widget>[
        if (topo.loading && topo.nodes.isEmpty)
          const LoadingStateView(message: 'Carregando mapa de topologia...')
        else if (topo.error != null && topo.nodes.isEmpty)
          ErrorStateView(
            message: topo.error!,
            onRetry: () => ref.read(topologyProvider.notifier).fetchTopology(),
          )
        else if (topo.nodes.isEmpty)
          const EmptyStateView(
            title: 'Nenhum dispositivo na topologia',
            description:
                'Cadastre equipamentos ou execute uma descoberta de rede.',
            icon: Icons.account_tree_outlined,
          )
        else
          Positioned.fill(
            child: TopologyCanvasView(
              controller: _canvasCtrl,
              nodes: filterResult.visibleNodes,
              edges: filterResult.visibleEdges,
              positions: layoutOutput.positions,
              clusters: layoutOutput.clusters,
              edgeStyle: effectiveSpec.edgeStyle,
              overlayMode: effectiveSpec.overlayMode,
              selectedNodeId: _selectedNodeId,
              selectedEdgeId: _selectedEdgeId,
              pathStartNodeId: _pathPickingSourceId,
              highlightedPath: activePath,
              dimmedNodeIds: filterResult.dimmedNodeIds,
              dimmedEdgeIds: filterResult.dimmedEdgeIds,
              showLabels: effectiveSpec.showLabels,
              showIpLabels: effectiveSpec.showIpLabels,
              showHealthBadges: effectiveSpec.showHealthBadges,
              showTrafficFlow:
                  effectiveSpec.showTrafficFlow &&
                  (hasLiveTraffic ||
                      effectiveSpec.overlayMode == TopologyOverlayMode.traffic),
              showClusterBoxes: effectiveSpec.showClusterBoxes,
              showMinimap: effectiveSpec.showMinimap,
              editLayoutEnabled: canWrite && _editLayoutEnabled,
              snapToGrid: effectiveSpec.snapToGrid,
              nodeExtras: buildTopologyNodeExtras(
                nodes: filterResult.visibleNodes,
                pluginActions: topo.pluginActions,
                wifiSnapshots: wifiSnapshots,
              ),
              linkTestOverlay: buildTopologyLinkTestOverlays(
                edges: filterResult.visibleEdges,
                tests: linkTests,
              ),
              onTapNode: _handleNodeTap,
              onDoubleTapNode: (n) => _focusAndSelectNode(n, openSheet: false),
              onLongPressNode: _openPathTargetPicker,
              onTapEdge: _handleEdgeTap,
              onTapEmptySpace: () => setState(() {
                _selectedNodeId = null;
                _selectedEdgeId = null;
              }),
              onNodeDragEnd: _handleNodeDragCommitted,
              onUndo: _undoStack.isNotEmpty ? _undoLastDrag : null,
            ),
          ),
        TopologyHud(
          filterSpec: effectiveSpec,
          healthSummary: computeTopologyHealthSummary(topo.nodes, topo.edges),
          sites: sites,
          canWrite: canWrite,
          editLayoutEnabled: _editLayoutEnabled,
          hasUnsavedLayoutChanges: _draggedNodeIds.isNotEmpty,
          canUndoLayout: _undoStack.isNotEmpty,
          draggedNodeCount: _draggedNodeIds.length,
          savingLayout: topo.savingLayout,
          recalculating: topo.recalculating,
          hasRemoteLayoutUpdate:
              topo.remoteLayoutRevision > _acknowledgedRemoteRevision,
          rightInset: rightInset,
          activePath: activePath,
          pathNodeNames: resolvePathNodeNames(activePath, topo.nodes),
          pathPickingSourceName: _pathPickingSourceId != null
              ? topo.nodes
                    .where((n) => n.id == _pathPickingSourceId)
                    .firstOrNull
                    ?.name
              : null,
          onOpenSearch: () => showTopologySearchSheet(
            context: context,
            nodes: topo.nodes,
            initialQuery: _filterSpec.search,
            macByNodeId: macIndex,
            onSelectNode: _focusAndSelectNode,
            onApplySearchFilter: (q) =>
                _updateFilterSpec(_filterSpec.copyWith(search: q)),
          ),
          onClearSearch: () =>
              _updateFilterSpec(_filterSpec.copyWith(search: '')),
          onOpenFilters: () => showTopologyFiltersSheet(
            context: context,
            initialSpec: effectiveSpec,
            sites: sites,
            onSpecChanged: _updateFilterSpec,
            onSiteChanged: _applySiteFilter,
            onLayoutSelected: _applyLayoutMode,
          ),
          onSiteChanged: _applySiteFilter,
          onLayoutChanged: _applyLayoutMode,
          onOverlayChanged: (m) =>
              _updateFilterSpec(_filterSpec.copyWith(overlayMode: m)),
          onToggleEditLayout: () =>
              setState(() => _editLayoutEnabled = !_editLayoutEnabled),
          onToggleSnapToGrid: () => _updateFilterSpec(
            _filterSpec.copyWith(snapToGrid: !_filterSpec.snapToGrid),
          ),
          onSaveLayout: () =>
              _saveLayout(onlyNodeIds: _draggedNodeIds.toList(growable: false)),
          onUndoLayout: _undoLastDrag,
          onDiscardLayout: _discardDraggedLayout,
          onZoomIn: _canvasCtrl.zoomIn,
          onZoomOut: _canvasCtrl.zoomOut,
          onFitToScreen: _canvasCtrl.fitToScreen,
          onTapProblems: () => _toggleStatusFilter(onlyProblems: true),
          onTapOffline: () =>
              _toggleStatusFilter(onlyProblems: false, offlineOnly: true),
          onClearPath: _clearPath,
          onCancelPathPicking: () =>
              setState(() => _pathPickingSourceId = null),
          onReloadRemoteLayout: () =>
              _handleHudAction(TopologyHudAction.refresh),
          onAction: _handleHudAction,
        ),
        if (isWide && (selectedNode != null || selectedEdge != null))
          Positioned(
            top: 10,
            right: 10,
            bottom: 10,
            width: 360,
            child: TopologyDockedInspector(
              selectedNode: selectedNode,
              selectedEdge: selectedEdge,
              allNodes: topo.nodes,
              allEdges: topo.edges,
              pluginActions: selectedNode != null
                  ? (topo.pluginActions[selectedNode.id] ?? const [])
                  : const [],
              canWrite: canWrite,
              pingLoading: _pingLoading,
              pingReport: _pingReport,
              pingError: _pingError,
              onClose: () => setState(() {
                _selectedNodeId = null;
                _selectedEdgeId = null;
              }),
              onQuickPing: _runDesktopQuickPing,
              onDismissPing: () => setState(() {
                _pingReport = null;
                _pingError = null;
              }),
              onOpenNodeDetails: (n) => context.go('/devices/${n.id}'),
              onTracePathFromNode: _openPathTargetPicker,
              onTraceUpstream: _traceUpstream,
              onTraceroute: (n) =>
                  showTracerouteDialog(context, initialHost: n.ipAddress),
              onStartNodeLinkTest: _startNodeLinkTest,
              onNewLinkFromNode: (n) => _openLinkForm(initialSourceId: n.id),
              onEditDevice: _editNodeDevice,
              onOpenPluginsTab: (n) =>
                  context.go('/devices/${n.id}?tab=plugins'),
              onFocusNode: _focusAndSelectNode,
              onInspectEdge: _handleEdgeTap,
              onStartEdgeLinkTest: _startEdgeLinkTest,
              onConfirmEdge: _confirmEdge,
              onEditEdge: (e) => _openLinkForm(existingEdge: e),
              onDeleteEdge: _deleteEdge,
            ),
          ),
      ],
    );
  }
}
