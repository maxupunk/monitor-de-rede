import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/canvas/topology_edge_painter.dart';
import 'package:netmonitor_app/features/network/topology/canvas/topology_minimap.dart';
import 'package:netmonitor_app/features/network/topology/canvas/topology_node_card.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Escala mínima de zoom do canvas interativo.
const double kTopologyMinScale = 0.2;

/// Escala máxima de zoom do canvas interativo.
const double kTopologyMaxScale = 2.8;

/// Extrai a escala 2D (plano XY) de uma matriz `Matrix4`.
///
/// Diferente de `Matrix4.getMaxScaleOnAxis()`, preserva valores `< 1.0` mesmo
/// quando a matriz aplica escala apenas em X e Y (mantendo Z em `1.0`).
double extractCanvasScale(Matrix4 matrix) {
  final s = matrix.storage;
  final scaleX = math.sqrt(s[0] * s[0] + s[1] * s[1]);
  final scaleY = math.sqrt(s[4] * s[4] + s[5] * s[5]);
  final avg = (scaleX + scaleY) * 0.5;
  return avg.isFinite && avg > 0 ? avg : 1.0;
}

/// Controlador imperativo de câmera para `TopologyCanvasView`.
/// Todas as transições de câmera usam animações suaves `Matrix4Tween`.
class TopologyCanvasController extends ChangeNotifier {
  _TopologyCanvasViewState? _state;

  bool get isAttached => _state != null;

  double get scale => _state?._currentScale ?? 1.0;

  Matrix4 get transform =>
      _state?._transformController.value ?? Matrix4.identity();

  void _attach(_TopologyCanvasViewState state) {
    _state = state;
  }

  void _detach(_TopologyCanvasViewState state) {
    if (_state == state) {
      _state = null;
    }
  }

  void _notifyStateChanged() {
    notifyListeners();
  }

  /// Enquadra todos os nós visíveis (e caixas de agrupamento) no viewport.
  void fitToScreen({EdgeInsets? padding, bool animate = true}) {
    _state?._fitToScreen(padding: padding, animate: animate);
  }

  /// Centraliza suavemente a câmera sobre o nó informado.
  void focusNode(
    int nodeId, {
    double targetScale = 1.15,
    EdgeInsets viewportInset = EdgeInsets.zero,
    bool animate = true,
  }) {
    _state?._focusNode(
      nodeId,
      targetScale: targetScale,
      viewportInset: viewportInset,
      animate: animate,
    );
  }

  /// Aproxima o zoom mantendo o centro do viewport estável.
  void zoomIn({double factor = 1.28, bool animate = true}) {
    _state?._zoomBy(factor, animate: animate);
  }

  /// Afasta o zoom mantendo o centro do viewport estável.
  void zoomOut({double factor = 1.28, bool animate = true}) {
    _state?._zoomBy(1.0 / factor, animate: animate);
  }

  /// Define uma escala específica ao redor de um ponto focal do viewport.
  void setScale(
    double targetScale, {
    Offset? focalViewportPoint,
    bool animate = true,
  }) {
    _state?._setScale(
      targetScale,
      focalViewportPoint: focalViewportPoint,
      animate: animate,
    );
  }

  /// Move o centro da câmera para uma coordenada do mundo.
  void panToWorld(Offset worldPoint, {bool animate = false}) {
    _state?._panToWorld(worldPoint, animate: animate);
  }
}

/// Canvas interativo de topologia com zoom semântico (LOD), pintura vetorial
/// de enlaces/fluxo animado, grade de pontos infinita, minimapa e atalhos.
class TopologyCanvasView extends StatefulWidget {
  const TopologyCanvasView({
    super.key,
    required this.nodes,
    required this.edges,
    required this.positions,
    this.clusters = const <TopologyClusterBox>[],
    this.controller,
    this.nodeSize = kDefaultTopologyNodeSize,
    this.edgeStyle = TopologyEdgeStyle.curved,
    this.overlayMode = TopologyOverlayMode.status,
    this.selectedNodeId,
    this.selectedEdgeId,
    this.pathStartNodeId,
    this.highlightedPath,
    this.dimmedNodeIds = const <int>{},
    this.dimmedEdgeIds = const <int>{},
    this.showLabels = true,
    this.showIpLabels = true,
    this.showHealthBadges = true,
    this.showTrafficFlow = true,
    this.showClusterBoxes = true,
    this.showMinimap = false,
    this.editLayoutEnabled = false,
    this.snapToGrid = false,
    this.autoFitOnFirstLayout = true,
    this.fitPadding = const EdgeInsets.fromLTRB(48, 88, 48, 72),
    this.minimapMargin = const EdgeInsets.only(left: 12, bottom: 16),
    this.nodeExtras = const <int, TopologyNodeExtras>{},
    this.edgeLatencyMs = const <int, double>{},
    this.linkTestOverlay = const <int, TopologyLinkTestEdgeOverlay>{},
    this.onTapNode,
    this.onDoubleTapNode,
    this.onLongPressNode,
    this.onTapEdge,
    this.onTapEmptySpace,
    this.onNodePositionChanged,
    this.onNodeDragEnd,
    this.onUndo,
    this.onDeleteSelected,
    this.onScaleChanged,
  });

  final List<TopologyNodeModel> nodes;
  final List<TopologyEdgeModel> edges;
  final Map<int, Offset> positions;
  final List<TopologyClusterBox> clusters;
  final TopologyCanvasController? controller;
  final Size nodeSize;
  final TopologyEdgeStyle edgeStyle;
  final TopologyOverlayMode overlayMode;
  final int? selectedNodeId;
  final int? selectedEdgeId;
  final int? pathStartNodeId;
  final TopologyPathResult? highlightedPath;
  final Set<int> dimmedNodeIds;
  final Set<int> dimmedEdgeIds;
  final bool showLabels;
  final bool showIpLabels;
  final bool showHealthBadges;
  final bool showTrafficFlow;
  final bool showClusterBoxes;
  final bool showMinimap;
  final bool editLayoutEnabled;
  final bool snapToGrid;
  final bool autoFitOnFirstLayout;
  final EdgeInsets fitPadding;
  final EdgeInsets minimapMargin;
  final Map<int, TopologyNodeExtras> nodeExtras;
  final Map<int, double> edgeLatencyMs;
  final Map<int, TopologyLinkTestEdgeOverlay> linkTestOverlay;
  final ValueChanged<TopologyNodeModel>? onTapNode;
  final ValueChanged<TopologyNodeModel>? onDoubleTapNode;
  final ValueChanged<TopologyNodeModel>? onLongPressNode;
  final ValueChanged<TopologyEdgeModel>? onTapEdge;
  final VoidCallback? onTapEmptySpace;
  final void Function(int nodeId, Offset nextPosition)? onNodePositionChanged;
  final void Function(int nodeId, Offset finalPosition)? onNodeDragEnd;
  final VoidCallback? onUndo;
  final VoidCallback? onDeleteSelected;
  final ValueChanged<double>? onScaleChanged;

  @override
  State<TopologyCanvasView> createState() => _TopologyCanvasViewState();
}

class _TopologyCanvasViewState extends State<TopologyCanvasView>
    with TickerProviderStateMixin {
  final TransformationController _transformController =
      TransformationController();
  final FocusNode _canvasFocusNode = FocusNode(debugLabel: 'topology-canvas');
  final GlobalKey _canvasAreaKey = GlobalKey(
    debugLabel: 'topology-canvas-area',
  );

  late final AnimationController _cameraAnimController;
  late final AnimationController _flowAnimController;
  Animation<Matrix4>? _cameraAnimation;

  Size _viewportSize = Size.zero;
  double _currentScale = 1.0;
  bool _hasAutoFitted = false;

  // Estado transitório de arraste 1:1 ancorado no espaço do mundo (60fps local).
  int? _draggingNodeId;
  Offset? _dragStartNodePos;
  Offset? _dragStartWorldPointer;
  Offset? _dragPreviewPos;
  int _activePointerCount = 0;

  // Cache de geometria recalculada somente quando entradas geométricas mudam.
  List<TopologyEdgePath> _cachedEdgePaths = const <TopologyEdgePath>[];
  Rect _cachedGraphBounds = Rect.zero;

  Map<int, Offset> get _effectivePositions {
    final dragId = _draggingNodeId;
    final preview = _dragPreviewPos;
    if (dragId == null || preview == null) return widget.positions;
    return <int, Offset>{...widget.positions, dragId: preview};
  }

  @override
  void initState() {
    super.initState();
    _cameraAnimController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 280),
    )..addListener(_onCameraTick);

    _flowAnimController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 2200),
    );

    _transformController.addListener(_onTransformChanged);
    widget.controller?._attach(this);

    _rebuildGeometry();
    _syncFlowAnimation();
  }

  @override
  void didUpdateWidget(covariant TopologyCanvasView oldWidget) {
    super.didUpdateWidget(oldWidget);

    if (oldWidget.controller != widget.controller) {
      oldWidget.controller?._detach(this);
      widget.controller?._attach(this);
    }

    if ((!widget.editLayoutEnabled && _draggingNodeId != null) ||
        (_draggingNodeId != null &&
            !widget.positions.containsKey(_draggingNodeId))) {
      _draggingNodeId = null;
      _dragStartNodePos = null;
      _dragStartWorldPointer = null;
      _dragPreviewPos = null;
    }

    final geometryChanged =
        oldWidget.edges != widget.edges ||
        oldWidget.positions != widget.positions ||
        oldWidget.clusters != widget.clusters ||
        oldWidget.showClusterBoxes != widget.showClusterBoxes ||
        oldWidget.nodeSize != widget.nodeSize ||
        oldWidget.edgeStyle != widget.edgeStyle ||
        oldWidget.editLayoutEnabled != widget.editLayoutEnabled;

    if (geometryChanged) {
      _rebuildGeometry();
    }

    if (oldWidget.showTrafficFlow != widget.showTrafficFlow ||
        oldWidget.edges != widget.edges) {
      _syncFlowAnimation();
    }

    if (!_hasAutoFitted &&
        widget.autoFitOnFirstLayout &&
        widget.positions.isNotEmpty &&
        _viewportSize != Size.zero) {
      _scheduleInitialFit();
    }
  }

  @override
  void dispose() {
    widget.controller?._detach(this);
    _transformController.removeListener(_onTransformChanged);
    _transformController.dispose();
    _cameraAnimController.removeListener(_onCameraTick);
    _cameraAnimController.dispose();
    _flowAnimController.dispose();
    _canvasFocusNode.dispose();
    super.dispose();
  }

  bool get _isMacroLod => _currentScale < kTopologyMacroZoomThreshold;

  void _rebuildGeometry() {
    final positions = _effectivePositions;
    _cachedEdgePaths = buildTopologyEdgePaths(
      edges: widget.edges,
      positions: positions,
      nodeSize: widget.nodeSize,
      style: widget.edgeStyle,
      macroLod: _isMacroLod,
    );
    _cachedGraphBounds = computeGraphBounds(
      positions,
      nodeSize: widget.nodeSize,
      clusters: widget.showClusterBoxes
          ? widget.clusters
          : const <TopologyClusterBox>[],
    );
  }

  void _syncFlowAnimation() {
    final shouldAnimate =
        widget.showTrafficFlow &&
        widget.edges.any(
          (e) => classifyEdgeStatus(e) != TopologyEdgeStatus.down,
        );
    if (shouldAnimate) {
      if (!_flowAnimController.isAnimating) {
        _flowAnimController.repeat();
      }
    } else if (_flowAnimController.isAnimating) {
      _flowAnimController.stop();
    }
  }

  void _scheduleInitialFit() {
    _hasAutoFitted = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || widget.positions.isEmpty || _viewportSize == Size.zero) {
        return;
      }
      _fitToScreen(animate: false);
    });
  }

  void _onCameraTick() {
    final anim = _cameraAnimation;
    if (anim != null) {
      _transformController.value = anim.value;
    }
  }

  void _onTransformChanged() {
    final nextScale = extractCanvasScale(_transformController.value);
    if (!nextScale.isFinite || nextScale <= 0) return;

    final prevMacro = _currentScale < kTopologyMacroZoomThreshold;
    final nextMacro = nextScale < kTopologyMacroZoomThreshold;
    final prevBadge = _currentScale >= kTopologyEdgeBadgeMinScale;
    final nextBadge = nextScale >= kTopologyEdgeBadgeMinScale;
    final prevSecondaryBadge =
        _currentScale >= kTopologyEdgeBadgeSecondaryMinScale;
    final nextSecondaryBadge = nextScale >= kTopologyEdgeBadgeSecondaryMinScale;
    final meaningfulDelta = (nextScale - _currentScale).abs() >= 0.015;

    if (prevMacro != nextMacro ||
        prevBadge != nextBadge ||
        prevSecondaryBadge != nextSecondaryBadge ||
        meaningfulDelta) {
      setState(() {
        _currentScale = nextScale;
        if (prevMacro != nextMacro) {
          _rebuildGeometry();
        }
      });
      widget.onScaleChanged?.call(nextScale);
      widget.controller?._notifyStateChanged();
    }
  }

  void _animateToMatrix(Matrix4 target, {required bool animate}) {
    _cameraAnimController.stop();
    if (!animate) {
      _cameraAnimation = null;
      _transformController.value = target;
      return;
    }
    _cameraAnimation =
        Matrix4Tween(
          begin: Matrix4.copy(_transformController.value),
          end: target,
        ).animate(
          CurvedAnimation(
            parent: _cameraAnimController,
            curve: Curves.easeOutCubic,
          ),
        );
    _cameraAnimController.forward(from: 0.0);
  }

  void _fitToScreen({EdgeInsets? padding, bool animate = true}) {
    if (_viewportSize == Size.zero || _cachedGraphBounds.isEmpty) return;
    final matrix = computeFitMatrix(
      _cachedGraphBounds,
      _viewportSize,
      padding: padding ?? widget.fitPadding,
      minScale: kTopologyMinScale,
      maxScale: 1.25,
    );
    _animateToMatrix(matrix, animate: animate);
  }

  void _focusNode(
    int nodeId, {
    double targetScale = 1.15,
    EdgeInsets viewportInset = EdgeInsets.zero,
    bool animate = true,
  }) {
    final pos = widget.positions[nodeId];
    if (pos == null || _viewportSize == Size.zero) return;
    final matrix = computeFocusNodeMatrix(
      pos,
      _viewportSize,
      nodeSize: widget.nodeSize,
      targetScale: targetScale,
      minScale: kTopologyMinScale,
      maxScale: kTopologyMaxScale,
      viewportInset: viewportInset,
    );
    _animateToMatrix(matrix, animate: animate);
  }

  void _zoomBy(double factor, {bool animate = true}) {
    _setScale(_currentScale * factor, animate: animate);
  }

  void _setScale(
    double targetScale, {
    Offset? focalViewportPoint,
    bool animate = true,
  }) {
    if (_viewportSize == Size.zero) return;
    final clamped = targetScale
        .clamp(kTopologyMinScale, kTopologyMaxScale)
        .toDouble();
    final current = _transformController.value;
    final currentScale = extractCanvasScale(current);
    if (currentScale <= 0 || !currentScale.isFinite) return;

    final focal =
        focalViewportPoint ??
        Offset(_viewportSize.width / 2, _viewportSize.height / 2);
    final inv = Matrix4.copy(current);
    if (inv.invert() == 0.0) return;
    final worldFocal = MatrixUtils.transformPoint(inv, focal);

    final tx = focal.dx - worldFocal.dx * clamped;
    final ty = focal.dy - worldFocal.dy * clamped;
    final next = Matrix4.identity()
      ..translateByDouble(tx, ty, 0.0, 1.0)
      ..scaleByDouble(clamped, clamped, clamped, 1.0);
    _animateToMatrix(next, animate: animate);
  }

  void _panToWorld(Offset worldPoint, {bool animate = false}) {
    if (_viewportSize == Size.zero) return;
    final scale = extractCanvasScale(
      _transformController.value,
    ).clamp(kTopologyMinScale, kTopologyMaxScale).toDouble();
    final tx = _viewportSize.width / 2 - worldPoint.dx * scale;
    final ty = _viewportSize.height / 2 - worldPoint.dy * scale;
    final next = Matrix4.identity()
      ..translateByDouble(tx, ty, 0.0, 1.0)
      ..scaleByDouble(scale, scale, scale, 1.0);
    _animateToMatrix(next, animate: animate);
  }

  void _handleCanvasTapUp(TapUpDetails details) {
    if (!_canvasFocusNode.hasFocus) {
      _canvasFocusNode.requestFocus();
    }
    final inv = Matrix4.copy(_transformController.value);
    if (inv.invert() == 0.0) {
      widget.onTapEmptySpace?.call();
      return;
    }
    final worldPoint = MatrixUtils.transformPoint(inv, details.localPosition);
    final worldTolerance = (22.0 / math.max(0.2, _currentScale)).clamp(
      16.0,
      48.0,
    );
    final hitEdge = hitTestEdge(
      worldPoint,
      _cachedEdgePaths,
      tolerance: worldTolerance,
    );
    if (hitEdge != null) {
      _canvasFocusNode.requestFocus();
      widget.onTapEdge?.call(hitEdge);
    } else {
      widget.onTapEmptySpace?.call();
    }
  }

  Offset? _globalToWorld(Offset globalPosition) {
    final box = _canvasAreaKey.currentContext?.findRenderObject() as RenderBox?;
    if (box == null || !box.hasSize) return null;
    final viewportPoint = box.globalToLocal(globalPosition);
    final inv = Matrix4.copy(_transformController.value);
    if (inv.invert() == 0.0) return null;
    return MatrixUtils.transformPoint(inv, viewportPoint);
  }

  void _handleNodeDragStart(int nodeId, Offset globalPosition) {
    if (!widget.editLayoutEnabled || _activePointerCount > 1) return;
    final startPos = _effectivePositions[nodeId];
    final worldPtr = _globalToWorld(globalPosition);
    if (startPos == null || worldPtr == null) return;
    _cameraAnimController.stop();
    HapticFeedback.selectionClick();
    setState(() {
      _draggingNodeId = nodeId;
      _dragStartNodePos = startPos;
      _dragStartWorldPointer = worldPtr;
      _dragPreviewPos = startPos;
    });
  }

  void _handleNodeDragUpdate(int nodeId, Offset globalPosition) {
    if (!widget.editLayoutEnabled || _activePointerCount > 1) {
      if (_draggingNodeId == nodeId) {
        _handleNodeDragCancel(nodeId);
      }
      return;
    }
    final worldPtr = _globalToWorld(globalPosition);
    if (worldPtr == null) return;

    if (_draggingNodeId != nodeId ||
        _dragStartNodePos == null ||
        _dragStartWorldPointer == null) {
      final fallbackPos = widget.positions[nodeId];
      if (fallbackPos == null) return;
      _draggingNodeId = nodeId;
      _dragStartNodePos = fallbackPos;
      _dragStartWorldPointer = worldPtr;
    }

    final rawNext = _dragStartNodePos! + (worldPtr - _dragStartWorldPointer!);
    final effective = widget.snapToGrid ? snapOffsetToGrid(rawNext) : rawNext;
    if (_dragPreviewPos == effective) return;
    setState(() {
      _dragPreviewPos = effective;
      _rebuildGeometry();
    });
    widget.onNodePositionChanged?.call(nodeId, effective);
  }

  void _handleNodeDragEnd(int nodeId) {
    if (_draggingNodeId != nodeId) return;
    final finalPos =
        _dragPreviewPos ?? _dragStartNodePos ?? widget.positions[nodeId];
    setState(() {
      _draggingNodeId = null;
      _dragStartNodePos = null;
      _dragStartWorldPointer = null;
      _dragPreviewPos = null;
      _rebuildGeometry();
    });
    if (finalPos != null) {
      widget.onNodeDragEnd?.call(nodeId, finalPos);
    }
  }

  void _handleNodeDragCancel(int nodeId) {
    if (_draggingNodeId != nodeId) return;
    setState(() {
      _draggingNodeId = null;
      _dragStartNodePos = null;
      _dragStartWorldPointer = null;
      _dragPreviewPos = null;
      _rebuildGeometry();
    });
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final semantics = context.semanticColors;
    final textTheme = context.textTheme;

    final highlightedNodeIds =
        widget.highlightedPath?.nodeIdSet ?? const <int>{};
    final highlightedEdgeIds =
        widget.highlightedPath?.edgeIdSet ?? const <int>{};
    final hasPathHighlight = highlightedNodeIds.isNotEmpty;
    final effectivePositions = _effectivePositions;
    final orderedNodes = _draggingNodeId == null
        ? widget.nodes
        : <TopologyNodeModel>[
            for (final n in widget.nodes)
              if (n.id != _draggingNodeId) n,
            for (final n in widget.nodes)
              if (n.id == _draggingNodeId) n,
          ];

    final worldWidth = math.max(
      2200.0,
      _cachedGraphBounds.isEmpty ? 2200.0 : _cachedGraphBounds.right + 640.0,
    );
    final worldHeight = math.max(
      1400.0,
      _cachedGraphBounds.isEmpty ? 1400.0 : _cachedGraphBounds.bottom + 640.0,
    );
    final boundaryMargin = EdgeInsets.fromLTRB(
      math.max(
        2400.0,
        _cachedGraphBounds.isEmpty ? 2400.0 : -_cachedGraphBounds.left + 1400.0,
      ),
      math.max(
        2400.0,
        _cachedGraphBounds.isEmpty ? 2400.0 : -_cachedGraphBounds.top + 1400.0,
      ),
      2400.0,
      2400.0,
    );

    return CallbackShortcuts(
      bindings: <ShortcutActivator, VoidCallback>{
        const SingleActivator(LogicalKeyboardKey.equal): () => _zoomBy(1.28),
        const SingleActivator(LogicalKeyboardKey.add): () => _zoomBy(1.28),
        const SingleActivator(LogicalKeyboardKey.numpadAdd): () =>
            _zoomBy(1.28),
        const SingleActivator(LogicalKeyboardKey.minus): () =>
            _zoomBy(1.0 / 1.28),
        const SingleActivator(LogicalKeyboardKey.numpadSubtract): () =>
            _zoomBy(1.0 / 1.28),
        const SingleActivator(LogicalKeyboardKey.digit0): () => _fitToScreen(),
        const SingleActivator(LogicalKeyboardKey.numpad0): () => _fitToScreen(),
        const SingleActivator(LogicalKeyboardKey.keyZ, control: true): () =>
            widget.onUndo?.call(),
        const SingleActivator(LogicalKeyboardKey.keyZ, meta: true): () =>
            widget.onUndo?.call(),
        const SingleActivator(LogicalKeyboardKey.delete): () {
          final focused = FocusManager.instance.primaryFocus?.context?.widget;
          if (focused is EditableText) return;
          widget.onDeleteSelected?.call();
        },
        const SingleActivator(LogicalKeyboardKey.escape): () =>
            widget.onTapEmptySpace?.call(),
      },
      child: Focus(
        focusNode: _canvasFocusNode,
        child: LayoutBuilder(
          builder: (context, constraints) {
            final nextSize = Size(constraints.maxWidth, constraints.maxHeight);
            if (nextSize != _viewportSize &&
                nextSize.width.isFinite &&
                nextSize.height.isFinite &&
                nextSize.width > 0 &&
                nextSize.height > 0) {
              _viewportSize = nextSize;
              if (!_hasAutoFitted &&
                  widget.autoFitOnFirstLayout &&
                  widget.positions.isNotEmpty) {
                _scheduleInitialFit();
              }
            }

            final minimapSize = nextSize.width >= 720
                ? const Size(196, 126)
                : const Size(156, 104);

            return Stack(
              children: <Widget>[
                // Grade de pontos infinita adaptada ao tema claro e escuro.
                Positioned.fill(
                  child: ColoredBox(
                    color: scheme.surfaceContainerLowest,
                    child: CustomPaint(
                      painter: _TopologyDotGridPainter(
                        transformationController: _transformController,
                        dotColor: scheme.outlineVariant.withValues(
                          alpha: scheme.brightness == Brightness.dark
                              ? 0.55
                              : 0.72,
                        ),
                      ),
                    ),
                  ),
                ),

                // Área interativa com pinch-zoom, pan e detecção de toque em enlaces.
                Positioned.fill(
                  child: Listener(
                    behavior: HitTestBehavior.translucent,
                    onPointerDown: (_) {
                      _activePointerCount += 1;
                      if (_activePointerCount > 1 && _draggingNodeId != null) {
                        _handleNodeDragCancel(_draggingNodeId!);
                      }
                    },
                    onPointerUp: (_) {
                      _activePointerCount = math.max(
                        0,
                        _activePointerCount - 1,
                      );
                    },
                    onPointerCancel: (_) {
                      _activePointerCount = math.max(
                        0,
                        _activePointerCount - 1,
                      );
                      if (_activePointerCount == 0 && _draggingNodeId != null) {
                        _handleNodeDragCancel(_draggingNodeId!);
                      }
                    },
                    child: GestureDetector(
                      key: _canvasAreaKey,
                      behavior: HitTestBehavior.translucent,
                      onTapUp: _handleCanvasTapUp,
                      child: InteractiveViewer(
                        key: const ValueKey<String>(
                          'topology-interactive-viewer',
                        ),
                        transformationController: _transformController,
                        constrained: false,
                        panEnabled: _draggingNodeId == null,
                        scaleEnabled: true,
                        boundaryMargin: boundaryMargin,
                        minScale: kTopologyMinScale,
                        maxScale: kTopologyMaxScale,
                        onInteractionStart: (_) => _cameraAnimController.stop(),
                        child: _UnboundedWorldStack(
                          children: <Widget>[
                            // Camada vetorial de clusters, enlaces, fluxo e badges.
                            SizedBox(
                              width: worldWidth,
                              height: worldHeight,
                              child: IgnorePointer(
                                child: RepaintBoundary(
                                  child: CustomPaint(
                                    key: const ValueKey<String>(
                                      'topology-edges-canvas',
                                    ),
                                    painter: TopologyEdgePainter(
                                      edgePaths: _cachedEdgePaths,
                                      clusters: widget.clusters,
                                      showClusterBoxes: widget.showClusterBoxes,
                                      highlightedEdgeIds: highlightedEdgeIds,
                                      dimmedEdgeIds: widget.dimmedEdgeIds,
                                      selectedEdgeId: widget.selectedEdgeId,
                                      overlayMode: widget.overlayMode,
                                      showLabels: widget.showLabels,
                                      showTrafficFlow: widget.showTrafficFlow,
                                      scale: _currentScale,
                                      flowAnimation: widget.showTrafficFlow
                                          ? _flowAnimController
                                          : null,
                                      edgeLatencyMs: widget.edgeLatencyMs,
                                      linkTestOverlay: widget.linkTestOverlay,
                                      colorScheme: scheme,
                                      semanticColors: semantics,
                                      textTheme: textTheme,
                                    ),
                                  ),
                                ),
                              ),
                            ),

                            // Camada de cartões de nós Material 3 com LOD.
                            for (final node in orderedNodes)
                              if (effectivePositions[node.id] != null)
                                Positioned(
                                  key: ValueKey<String>(
                                    'topology-node-pos-${node.id}',
                                  ),
                                  left: effectivePositions[node.id]!.dx,
                                  top: effectivePositions[node.id]!.dy,
                                  child: TopologyNodeCard(
                                    node: node,
                                    size: widget.nodeSize,
                                    scale: _currentScale,
                                    selected: widget.selectedNodeId == node.id,
                                    highlighted: highlightedNodeIds.contains(
                                      node.id,
                                    ),
                                    isPathStart:
                                        widget.pathStartNodeId == node.id,
                                    dimmed:
                                        _draggingNodeId != node.id &&
                                        widget.selectedNodeId != node.id &&
                                        (hasPathHighlight
                                            ? !highlightedNodeIds.contains(
                                                node.id,
                                              )
                                            : widget.dimmedNodeIds.contains(
                                                node.id,
                                              )),
                                    showIpLabel: widget.showIpLabels,
                                    showHealthBadges: widget.showHealthBadges,
                                    editLayoutEnabled: widget.editLayoutEnabled,
                                    isDragging: _draggingNodeId == node.id,
                                    overlayMode: widget.overlayMode,
                                    extras: widget.nodeExtras[node.id],
                                    onTap: () => widget.onTapNode?.call(node),
                                    onDoubleTap: () {
                                      if (widget.onDoubleTapNode != null) {
                                        widget.onDoubleTapNode!(node);
                                      } else {
                                        _focusNode(node.id);
                                      }
                                    },
                                    onLongPress: widget.onLongPressNode != null
                                        ? () => widget.onLongPressNode!(node)
                                        : null,
                                    onDragStart: widget.editLayoutEnabled
                                        ? (pos) =>
                                              _handleNodeDragStart(node.id, pos)
                                        : null,
                                    onDragUpdate: widget.editLayoutEnabled
                                        ? (pos) => _handleNodeDragUpdate(
                                            node.id,
                                            pos,
                                          )
                                        : null,
                                    onDragEnd: widget.editLayoutEnabled
                                        ? () => _handleNodeDragEnd(node.id)
                                        : null,
                                    onDragCancel: widget.editLayoutEnabled
                                        ? () => _handleNodeDragCancel(node.id)
                                        : null,
                                  ),
                                ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),

                // Minimapa flutuante (canto inferior esquerdo, alternável).
                if (widget.showMinimap && widget.nodes.isNotEmpty)
                  Positioned(
                    left: widget.minimapMargin.left,
                    bottom: widget.minimapMargin.bottom,
                    child: TopologyMinimap(
                      nodes: widget.nodes,
                      edges: widget.edges,
                      positions: effectivePositions,
                      graphBounds: _cachedGraphBounds,
                      viewportSize: nextSize,
                      transformationController: _transformController,
                      onPanToWorld: (worldPt) => _panToWorld(worldPt),
                      nodeSize: widget.nodeSize,
                      selectedNodeId: widget.selectedNodeId,
                      highlightedNodeIds: highlightedNodeIds,
                      highlightedEdgeIds: highlightedEdgeIds,
                      size: minimapSize,
                    ),
                  ),
              ],
            );
          },
        ),
      ),
    );
  }
}

/// Stack do mundo da topologia com hit-testing irrestrito às bordas `(0..w, 0..h)`.
///
/// Sem este override, `RenderBox.hitTest` descarta qualquer toque/arraste em
/// nós movidos para coordenadas negativas (`x < 0` ou `y < 0`) mesmo quando
/// visíveis com `Clip.none`, travando o nó após o primeiro arraste.
class _UnboundedWorldStack extends Stack {
  const _UnboundedWorldStack({super.children}) : super(clipBehavior: Clip.none);

  @override
  RenderStack createRenderObject(BuildContext context) {
    return _RenderUnboundedWorldStack(
      alignment: alignment,
      textDirection: textDirection ?? Directionality.maybeOf(context),
      fit: fit,
      clipBehavior: clipBehavior,
    );
  }
}

class _RenderUnboundedWorldStack extends RenderStack {
  _RenderUnboundedWorldStack({
    super.alignment,
    super.textDirection,
    super.fit,
    super.clipBehavior,
  });

  @override
  bool hitTest(BoxHitTestResult result, {required Offset position}) {
    if (hitTestChildren(result, position: position)) {
      result.add(BoxHitTestEntry(this, position));
      return true;
    }
    if (size.contains(position) && hitTestSelf(position)) {
      result.add(BoxHitTestEntry(this, position));
      return true;
    }
    return false;
  }
}

class _TopologyDotGridPainter extends CustomPainter {
  _TopologyDotGridPainter({
    required this.transformationController,
    required this.dotColor,
  }) : super(repaint: transformationController);

  final TransformationController transformationController;
  final Color dotColor;

  @override
  void paint(Canvas canvas, Size size) {
    final matrix = transformationController.value;
    final scale = extractCanvasScale(matrix).clamp(0.15, 4.0);
    final translation = matrix.getTranslation();

    // Adapta o espaçamento da grade conforme o zoom para evitar ruído visual.
    var worldStep = kTopologyGridSize;
    while (worldStep * scale < 18.0) {
      worldStep *= 2.0;
    }
    final screenStep = worldStep * scale;
    final offsetX = translation.x % screenStep;
    final offsetY = translation.y % screenStep;

    final startX = offsetX < 0 ? offsetX + screenStep : offsetX;
    final startY = offsetY < 0 ? offsetY + screenStep : offsetY;
    final dotRadius = scale >= 0.8 ? 1.25 : 1.0;

    final paint = Paint()
      ..color = dotColor
      ..style = PaintingStyle.fill;

    for (var x = startX; x < size.width; x += screenStep) {
      for (var y = startY; y < size.height; y += screenStep) {
        canvas.drawCircle(Offset(x, y), dotRadius, paint);
      }
    }
  }

  @override
  bool shouldRepaint(covariant _TopologyDotGridPainter oldDelegate) {
    return oldDelegate.dotColor != dotColor ||
        oldDelegate.transformationController != transformationController;
  }
}
