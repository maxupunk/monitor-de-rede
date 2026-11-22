import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/canvas/topology_canvas_view.dart'
    show extractCanvasScale;
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Minimapa flutuante Material Design 3 que exibe visão geral dos nós,
/// caminho destacado e retângulo translúcido do viewport atual.
/// Permite tocar ou arrastar diretamente para mover a câmera.
class TopologyMinimap extends StatelessWidget {
  const TopologyMinimap({
    super.key,
    required this.nodes,
    required this.edges,
    required this.positions,
    required this.graphBounds,
    required this.viewportSize,
    required this.transformationController,
    required this.onPanToWorld,
    this.nodeSize = kDefaultTopologyNodeSize,
    this.selectedNodeId,
    this.highlightedNodeIds = const <int>{},
    this.highlightedEdgeIds = const <int>{},
    this.size = const Size(168, 112),
  });

  final List<TopologyNodeModel> nodes;
  final List<TopologyEdgeModel> edges;
  final Map<int, Offset> positions;
  final Rect graphBounds;
  final Size viewportSize;
  final TransformationController transformationController;
  final ValueChanged<Offset> onPanToWorld;
  final Size nodeSize;
  final int? selectedNodeId;
  final Set<int> highlightedNodeIds;
  final Set<int> highlightedEdgeIds;
  final Size size;

  static const double _innerPadding = 10.0;

  _MinimapTransform _computeTransform() {
    final worldRect = graphBounds.isEmpty
        ? const Rect.fromLTWH(0, 0, 800, 600)
        : graphBounds.inflate(64.0);
    final drawW = math.max(1.0, size.width - _innerPadding * 2);
    final drawH = math.max(1.0, size.height - _innerPadding * 2);
    final scale = math.min(
      drawW / math.max(1.0, worldRect.width),
      drawH / math.max(1.0, worldRect.height),
    );
    final scaledW = worldRect.width * scale;
    final scaledH = worldRect.height * scale;
    final offsetX = _innerPadding + (drawW - scaledW) / 2;
    final offsetY = _innerPadding + (drawH - scaledH) / 2;
    return _MinimapTransform(
      worldRect: worldRect,
      scale: scale,
      origin: Offset(offsetX, offsetY),
    );
  }

  void _handlePointer(Offset localPosition) {
    final transform = _computeTransform();
    onPanToWorld(transform.toWorld(localPosition));
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final semantics = context.semanticColors;
    final mapTransform = _computeTransform();

    return Material(
      key: const ValueKey<String>('topology-minimap'),
      elevation: 4,
      color: scheme.surface.withValues(alpha: 0.94),
      surfaceTintColor: Colors.transparent,
      shape: RoundedRectangleBorder(
        borderRadius: AppTheme.borderRadius,
        side: BorderSide(color: scheme.outlineVariant),
      ),
      clipBehavior: Clip.antiAlias,
      child: SizedBox(
        width: size.width,
        height: size.height,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTapDown: (details) => _handlePointer(details.localPosition),
          onPanUpdate: (details) => _handlePointer(details.localPosition),
          child: Stack(
            children: <Widget>[
              Positioned.fill(
                child: CustomPaint(
                  painter: _TopologyMinimapPainter(
                    nodes: nodes,
                    edges: edges,
                    positions: positions,
                    nodeSize: nodeSize,
                    mapTransform: mapTransform,
                    viewportSize: viewportSize,
                    transformationController: transformationController,
                    selectedNodeId: selectedNodeId,
                    highlightedNodeIds: highlightedNodeIds,
                    highlightedEdgeIds: highlightedEdgeIds,
                    colorScheme: scheme,
                    semanticColors: semantics,
                  ),
                ),
              ),
              Positioned(
                right: 6,
                top: 5,
                child: ValueListenableBuilder<Matrix4>(
                  valueListenable: transformationController,
                  builder: (context, matrix, _) {
                    final pct = (extractCanvasScale(matrix) * 100)
                        .clamp(10, 400)
                        .round();
                    return IgnorePointer(
                      child: Container(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 5,
                          vertical: 1.5,
                        ),
                        decoration: BoxDecoration(
                          color: scheme.surfaceContainerHigh.withValues(
                            alpha: 0.9,
                          ),
                          borderRadius: BorderRadius.circular(6),
                        ),
                        child: Text(
                          '$pct%',
                          style: context.textTheme.labelSmall?.copyWith(
                            fontSize: 9.5,
                            fontWeight: FontWeight.w700,
                            color: scheme.onSurface,
                          ),
                        ),
                      ),
                    );
                  },
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

@immutable
class _MinimapTransform {
  const _MinimapTransform({
    required this.worldRect,
    required this.scale,
    required this.origin,
  });

  final Rect worldRect;
  final double scale;
  final Offset origin;

  Offset toMinimap(Offset worldPoint) {
    return Offset(
      origin.dx + (worldPoint.dx - worldRect.left) * scale,
      origin.dy + (worldPoint.dy - worldRect.top) * scale,
    );
  }

  Offset toWorld(Offset localPoint) {
    final safeScale = math.max(1e-6, scale);
    return Offset(
      worldRect.left + (localPoint.dx - origin.dx) / safeScale,
      worldRect.top + (localPoint.dy - origin.dy) / safeScale,
    );
  }
}

class _TopologyMinimapPainter extends CustomPainter {
  _TopologyMinimapPainter({
    required this.nodes,
    required this.edges,
    required this.positions,
    required this.nodeSize,
    required this.mapTransform,
    required this.viewportSize,
    required this.transformationController,
    required this.selectedNodeId,
    required this.highlightedNodeIds,
    required this.highlightedEdgeIds,
    required this.colorScheme,
    required this.semanticColors,
  }) : super(repaint: transformationController);

  final List<TopologyNodeModel> nodes;
  final List<TopologyEdgeModel> edges;
  final Map<int, Offset> positions;
  final Size nodeSize;
  final _MinimapTransform mapTransform;
  final Size viewportSize;
  final TransformationController transformationController;
  final int? selectedNodeId;
  final Set<int> highlightedNodeIds;
  final Set<int> highlightedEdgeIds;
  final ColorScheme colorScheme;
  final AppSemanticColors semanticColors;

  @override
  void paint(Canvas canvas, Size size) {
    final edgePaint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 0.9
      ..color = colorScheme.outline.withValues(alpha: 0.45);
    final highlightEdgePaint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.8
      ..color = semanticColors.warning;

    for (final edge in edges) {
      final p1 = positions[edge.sourceDeviceId];
      final p2 = positions[edge.targetDeviceId];
      if (p1 == null || p2 == null) continue;
      final a = mapTransform.toMinimap(
        topologyNodeCenter(p1, nodeSize: nodeSize),
      );
      final b = mapTransform.toMinimap(
        topologyNodeCenter(p2, nodeSize: nodeSize),
      );
      canvas.drawLine(
        a,
        b,
        highlightedEdgeIds.contains(edge.id) ? highlightEdgePaint : edgePaint,
      );
    }

    for (final node in nodes) {
      final pos = positions[node.id];
      if (pos == null) continue;
      final center = mapTransform.toMinimap(
        topologyNodeCenter(pos, nodeSize: nodeSize),
      );
      final status = classifyNodeStatus(node.status);
      final pair = semanticColors.resolve(status.tone, colorScheme);
      final isSelected = node.id == selectedNodeId;
      final isHighlighted = highlightedNodeIds.contains(node.id);
      final radius = isSelected
          ? 4.2
          : isHighlighted
          ? 3.6
          : 2.8;

      if (isSelected || isHighlighted) {
        canvas.drawCircle(
          center,
          radius + 2.0,
          Paint()
            ..color =
                (isSelected ? colorScheme.primary : semanticColors.warning)
                    .withValues(alpha: 0.35),
        );
      }

      canvas.drawCircle(center, radius, Paint()..color = pair.foreground);
    }

    // Retângulo translúcido representando a área visível do viewport atual.
    if (viewportSize.width > 0 && viewportSize.height > 0) {
      final inv = Matrix4.copy(transformationController.value);
      if (inv.invert() != 0.0) {
        final topLeftWorld = MatrixUtils.transformPoint(inv, Offset.zero);
        final bottomRightWorld = MatrixUtils.transformPoint(
          inv,
          Offset(viewportSize.width, viewportSize.height),
        );
        final p1 = mapTransform.toMinimap(topLeftWorld);
        final p2 = mapTransform.toMinimap(bottomRightWorld);
        final rawRect = Rect.fromPoints(p1, p2);
        final clamped = Rect.fromLTRB(
          rawRect.left.clamp(2.0, size.width - 2.0),
          rawRect.top.clamp(2.0, size.height - 2.0),
          rawRect.right.clamp(2.0, size.width - 2.0),
          rawRect.bottom.clamp(2.0, size.height - 2.0),
        );
        if (clamped.width >= 4 && clamped.height >= 4) {
          final rrect = RRect.fromRectAndRadius(
            clamped,
            const Radius.circular(4),
          );
          canvas.drawRRect(
            rrect,
            Paint()..color = colorScheme.primary.withValues(alpha: 0.14),
          );
          canvas.drawRRect(
            rrect,
            Paint()
              ..style = PaintingStyle.stroke
              ..strokeWidth = 1.4
              ..color = colorScheme.primary.withValues(alpha: 0.85),
          );
        }
      }
    }
  }

  @override
  bool shouldRepaint(covariant _TopologyMinimapPainter oldDelegate) {
    return oldDelegate.nodes != nodes ||
        oldDelegate.edges != edges ||
        oldDelegate.positions != positions ||
        oldDelegate.mapTransform.worldRect != mapTransform.worldRect ||
        oldDelegate.viewportSize != viewportSize ||
        oldDelegate.selectedNodeId != selectedNodeId ||
        oldDelegate.highlightedNodeIds != highlightedNodeIds ||
        oldDelegate.highlightedEdgeIds != highlightedEdgeIds ||
        oldDelegate.colorScheme != colorScheme ||
        oldDelegate.semanticColors != semanticColors;
  }
}
