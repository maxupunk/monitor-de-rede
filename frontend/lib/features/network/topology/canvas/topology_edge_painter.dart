import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Escala mínima de zoom em que as pílulas de rótulo dos enlaces são exibidas.
const double kTopologyEdgeBadgeMinScale = 0.72;

/// Opacidade aplicada a enlaces fora do caminho destacado ou fora do filtro ativo.
const double kTopologyEdgeDimmedOpacity = 0.22;

/// Sobreposição opcional de resultado de teste de enlace para pintura no canvas.
@immutable
class TopologyLinkTestEdgeOverlay {
  const TopologyLinkTestEdgeOverlay({
    required this.label,
    required this.tone,
    this.active = false,
  });

  final String label;
  final SemanticTone tone;
  final bool active;

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is TopologyLinkTestEdgeOverlay &&
        other.label == label &&
        other.tone == tone &&
        other.active == active;
  }

  @override
  int get hashCode => Object.hash(label, tone, active);
}

/// Painter de alta performance para caixas de agrupamento, enlaces vetoriais,
/// fluxo animado de tráfego ("formigas"), destaque de caminho e pílulas de rótulo.
class TopologyEdgePainter extends CustomPainter {
  TopologyEdgePainter({
    required this.edgePaths,
    this.clusters = const <TopologyClusterBox>[],
    this.showClusterBoxes = true,
    this.highlightedEdgeIds = const <int>{},
    this.dimmedEdgeIds = const <int>{},
    this.selectedEdgeId,
    this.overlayMode = TopologyOverlayMode.status,
    this.showLabels = true,
    this.showTrafficFlow = true,
    this.scale = 1.0,
    this.flowAnimation,
    this.edgeLatencyMs = const <int, double>{},
    this.linkTestOverlay = const <int, TopologyLinkTestEdgeOverlay>{},
    required this.colorScheme,
    required this.semanticColors,
    required this.textTheme,
  }) : super(repaint: flowAnimation);

  final List<TopologyEdgePath> edgePaths;
  final List<TopologyClusterBox> clusters;
  final bool showClusterBoxes;
  final Set<int> highlightedEdgeIds;
  final Set<int> dimmedEdgeIds;
  final int? selectedEdgeId;
  final TopologyOverlayMode overlayMode;
  final bool showLabels;
  final bool showTrafficFlow;
  final double scale;
  final Animation<double>? flowAnimation;
  final Map<int, double> edgeLatencyMs;
  final Map<int, TopologyLinkTestEdgeOverlay> linkTestOverlay;
  final ColorScheme colorScheme;
  final AppSemanticColors semanticColors;
  final TextTheme textTheme;

  bool get _hasPathHighlight => highlightedEdgeIds.isNotEmpty;

  bool _isDimmed(TopologyEdgeModel edge) {
    if (edge.id == selectedEdgeId) return false;
    if (_hasPathHighlight) {
      return !highlightedEdgeIds.contains(edge.id);
    }
    return dimmedEdgeIds.contains(edge.id);
  }

  SemanticTone _resolveTone(TopologyEdgeModel edge) {
    if (overlayMode == TopologyOverlayMode.linkTest) {
      final overlay = linkTestOverlay[edge.id];
      if (overlay != null) return overlay.tone;
    }
    return edgeSemanticTone(
      edge,
      overlay: overlayMode,
      latencyMs: edgeLatencyMs[edge.id],
    );
  }

  @override
  void paint(Canvas canvas, Size size) {
    if (showClusterBoxes && clusters.isNotEmpty) {
      _paintClusterBoxes(canvas);
    }

    if (edgePaths.isEmpty) return;

    final backgroundEdges = <TopologyEdgePath>[];
    final regularEdges = <TopologyEdgePath>[];
    final emphasisEdges = <TopologyEdgePath>[];

    for (final edgePath in edgePaths) {
      final id = edgePath.edge.id;
      final isEmphasis =
          id == selectedEdgeId || highlightedEdgeIds.contains(id);
      if (isEmphasis) {
        emphasisEdges.add(edgePath);
      } else if (_isDimmed(edgePath.edge)) {
        backgroundEdges.add(edgePath);
      } else {
        regularEdges.add(edgePath);
      }
    }

    for (final edgePath in backgroundEdges) {
      _paintEdge(canvas, edgePath, dimmed: true, emphasis: false);
    }
    for (final edgePath in regularEdges) {
      _paintEdge(canvas, edgePath, dimmed: false, emphasis: false);
    }
    for (final edgePath in emphasisEdges) {
      _paintEdge(canvas, edgePath, dimmed: false, emphasis: true);
    }

    // Pílulas de rótulo no ponto médio (somente com zoom >= 0.72 para evitar poluição).
    if (showLabels && scale >= kTopologyEdgeBadgeMinScale) {
      for (final edgePath in backgroundEdges) {
        _paintEdgeBadge(canvas, edgePath, dimmed: true, emphasis: false);
      }
      for (final edgePath in regularEdges) {
        _paintEdgeBadge(canvas, edgePath, dimmed: false, emphasis: false);
      }
      for (final edgePath in emphasisEdges) {
        _paintEdgeBadge(canvas, edgePath, dimmed: false, emphasis: true);
      }
    }
  }

  void _paintClusterBoxes(Canvas canvas) {
    final isDark = colorScheme.brightness == Brightness.dark;
    final fillPaint = Paint()
      ..style = PaintingStyle.fill
      ..color = colorScheme.primaryContainer.withValues(
        alpha: isDark ? 0.14 : 0.22,
      );
    final borderPaint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.4
      ..color = colorScheme.primary.withValues(alpha: isDark ? 0.34 : 0.28);

    for (final cluster in clusters) {
      final rrect = RRect.fromRectAndRadius(
        cluster.bounds,
        const Radius.circular(16),
      );
      canvas.drawRRect(rrect, fillPaint);
      canvas.drawRRect(rrect, borderPaint);

      final labelText = '${cluster.label} · ${cluster.nodeCount}';
      final painter = TextPainter(
        text: TextSpan(
          text: labelText,
          style: (textTheme.labelMedium ?? const TextStyle(fontSize: 12))
              .copyWith(
                fontWeight: FontWeight.w700,
                color: colorScheme.onPrimaryContainer,
              ),
        ),
        maxLines: 1,
        ellipsis: '…',
        textDirection: TextDirection.ltr,
      )..layout(maxWidth: math.max(80.0, cluster.bounds.width - 28));

      final pillRect = Rect.fromLTWH(
        cluster.bounds.left + 12,
        cluster.bounds.top + 10,
        painter.width + 18,
        painter.height + 8,
      );
      final pillRRect = RRect.fromRectAndRadius(
        pillRect,
        const Radius.circular(9),
      );
      canvas.drawRRect(
        pillRRect,
        Paint()
          ..color = colorScheme.primaryContainer.withValues(
            alpha: isDark ? 0.88 : 0.94,
          ),
      );
      canvas.drawRRect(
        pillRRect,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1.0
          ..color = colorScheme.primary.withValues(alpha: 0.45),
      );
      painter.paint(canvas, Offset(pillRect.left + 9, pillRect.top + 4));
    }
  }

  void _paintEdge(
    Canvas canvas,
    TopologyEdgePath edgePath, {
    required bool dimmed,
    required bool emphasis,
  }) {
    final edge = edgePath.edge;
    final isSelected = edge.id == selectedEdgeId;
    final isPathEdge = highlightedEdgeIds.contains(edge.id);
    final edgeStatus = classifyEdgeStatus(edge);
    final linkKind = classifyLinkKind(edge.linkType);
    final tone = _resolveTone(edge);
    final pair = semanticColors.resolve(tone, colorScheme);

    final baseColor = isSelected
        ? colorScheme.primary
        : isPathEdge
        ? (edgeStatus == TopologyEdgeStatus.down
              ? semanticColors.error
              : colorScheme.primary)
        : pair.foreground;

    final alpha = dimmed ? kTopologyEdgeDimmedOpacity : 1.0;
    final effectiveColor = baseColor.withValues(alpha: alpha);
    final baseWidth = edgeBaseStrokeWidth(edge);
    final strokeWidth = isSelected
        ? math.max(4.0, baseWidth + 1.8)
        : isPathEdge
        ? math.max(3.8, baseWidth + 1.5)
        : baseWidth;

    final flutterPath = edgePath.toFlutterPath();

    // Halo suave sob enlaces selecionados ou parte do caminho traçado.
    if (emphasis) {
      final haloColor = isSelected
          ? colorScheme.primary
          : isPathEdge
          ? semanticColors.warning
          : baseColor;
      final outerGlow = Paint()
        ..style = PaintingStyle.stroke
        ..strokeCap = StrokeCap.round
        ..strokeJoin = StrokeJoin.round
        ..strokeWidth = strokeWidth + 7.5
        ..color = haloColor.withValues(alpha: 0.22);
      final innerGlow = Paint()
        ..style = PaintingStyle.stroke
        ..strokeCap = StrokeCap.round
        ..strokeJoin = StrokeJoin.round
        ..strokeWidth = strokeWidth + 3.5
        ..color = haloColor.withValues(alpha: 0.36);
      canvas.drawPath(flutterPath, outerGlow);
      canvas.drawPath(flutterPath, innerGlow);
    }

    final lineStyle = edgeStatus == TopologyEdgeStatus.down
        ? TopologyLineStyle.dashed
        : linkKind.lineStyle;

    final strokePaint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round
      ..strokeWidth = strokeWidth
      ..color = effectiveColor;

    if (lineStyle == TopologyLineStyle.solid) {
      canvas.drawPath(flutterPath, strokePaint);
    } else {
      final dashPath = _buildDashedPath(
        flutterPath,
        dashLength: lineStyle == TopologyLineStyle.dotted ? 3.0 : 8.5,
        gapLength: lineStyle == TopologyLineStyle.dotted ? 5.0 : 6.0,
      );
      canvas.drawPath(dashPath, strokePaint);
    }

    // Conectores discretos nas extremidades quando o zoom está detalhado.
    if (!dimmed && scale >= kTopologyEdgeBadgeMinScale) {
      final dotPaint = Paint()
        ..style = PaintingStyle.fill
        ..color = effectiveColor;
      final dotRadius = emphasis ? 3.8 : 2.8;
      canvas.drawCircle(edgePath.start, dotRadius, dotPaint);
      canvas.drawCircle(edgePath.end, dotRadius, dotPaint);
    }

    // Fluxo animado ("formigas") em enlaces ativos quando habilitado.
    if (showTrafficFlow &&
        flowAnimation != null &&
        !dimmed &&
        edgeStatus != TopologyEdgeStatus.down) {
      _paintTrafficAnts(
        canvas,
        edgePath,
        color: isPathEdge || isSelected ? colorScheme.primary : pair.foreground,
        emphasis: emphasis,
      );
    }
  }

  void _paintTrafficAnts(
    Canvas canvas,
    TopologyEdgePath edgePath, {
    required Color color,
    required bool emphasis,
  }) {
    final phase = flowAnimation!.value;
    final linkOverlay = overlayMode == TopologyOverlayMode.linkTest
        ? linkTestOverlay[edgePath.edge.id]
        : null;
    final speedFactor = linkOverlay != null && linkOverlay.active
        ? 2.2
        : computeTrafficFlowSpeedFactor(edgePath.edge);
    if (speedFactor <= 0.0) return;

    final dots = computeAntDotPositions(
      edgePath,
      phase,
      spacing: emphasis ? 48.0 : 62.0,
      minDots: emphasis ? 2 : 1,
      maxDots: 8,
      speedMultiplier: speedFactor,
    );
    if (dots.isEmpty) return;

    final isDark = colorScheme.brightness == Brightness.dark;
    final coreColor = isDark
        ? Color.lerp(color, Colors.white, 0.55)!
        : Color.lerp(color, Colors.white, 0.28)!;
    final auraPaint = Paint()
      ..style = PaintingStyle.fill
      ..color = color.withValues(alpha: 0.34);
    final corePaint = Paint()
      ..style = PaintingStyle.fill
      ..color = coreColor;

    final halfLen = emphasis ? 5.2 : 4.2;
    final radius = emphasis ? 3.0 : 2.3;

    for (final dot in dots) {
      final angle = math.atan2(dot.tangent.dy, dot.tangent.dx);
      canvas.save();
      canvas.translate(dot.position.dx, dot.position.dy);
      canvas.rotate(angle);
      canvas.drawRRect(
        RRect.fromRectAndRadius(
          Rect.fromCenter(
            center: Offset.zero,
            width: halfLen * 2.5,
            height: radius * 3.1,
          ),
          Radius.circular(radius * 1.5),
        ),
        auraPaint,
      );
      canvas.drawRRect(
        RRect.fromRectAndRadius(
          Rect.fromCenter(
            center: Offset.zero,
            width: halfLen * 2,
            height: radius * 2,
          ),
          Radius.circular(radius),
        ),
        corePaint,
      );
      canvas.restore();
    }
  }

  void _paintEdgeBadge(
    Canvas canvas,
    TopologyEdgePath edgePath, {
    required bool dimmed,
    required bool emphasis,
  }) {
    final edge = edgePath.edge;
    final linkOverlay = overlayMode == TopologyOverlayMode.linkTest
        ? linkTestOverlay[edge.id]
        : null;
    final label =
        linkOverlay?.label ??
        formatEdgeBadgeLabel(
          edge,
          overlay: overlayMode,
          latencyMs: edgeLatencyMs[edge.id],
        );
    if (label.trim().isEmpty) return;

    final tone = _resolveTone(edge);
    final pair = semanticColors.resolve(tone, colorScheme);
    final isSelected = edge.id == selectedEdgeId;
    final alpha = dimmed ? kTopologyEdgeDimmedOpacity : 1.0;

    final textPainter = TextPainter(
      text: TextSpan(
        text: label,
        style: (textTheme.labelSmall ?? const TextStyle(fontSize: 11)).copyWith(
          fontSize: 10.5,
          fontWeight: FontWeight.w700,
          color: (isSelected ? colorScheme.onPrimary : colorScheme.onSurface)
              .withValues(alpha: alpha),
          height: 1.15,
        ),
      ),
      maxLines: 1,
      textDirection: TextDirection.ltr,
    )..layout(maxWidth: 160);

    const hPad = 8.0;
    const vPad = 3.5;
    final pillWidth = textPainter.width + hPad * 2;
    final pillHeight = textPainter.height + vPad * 2;
    final pillRect = Rect.fromCenter(
      center: edgePath.midpoint,
      width: pillWidth,
      height: pillHeight,
    );
    final pillRRect = RRect.fromRectAndRadius(
      pillRect,
      const Radius.circular(8),
    );

    if (!dimmed) {
      canvas.drawRRect(
        pillRRect.shift(const Offset(0, 1.2)),
        Paint()..color = colorScheme.shadow.withValues(alpha: 0.16),
      );
    }

    final fillColor = isSelected
        ? colorScheme.primary
        : colorScheme.surface.withValues(alpha: dimmed ? 0.45 : 0.96);
    final borderColor = isSelected
        ? colorScheme.primary
        : emphasis
        ? semanticColors.warning
        : pair.foreground.withValues(alpha: dimmed ? 0.3 : 0.72);

    canvas.drawRRect(pillRRect, Paint()..color = fillColor);
    canvas.drawRRect(
      pillRRect,
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = emphasis ? 1.6 : 1.1
        ..color = borderColor,
    );

    textPainter.paint(
      canvas,
      Offset(
        pillRect.left + hPad,
        pillRect.top + (pillHeight - textPainter.height) / 2,
      ),
    );
  }

  Path _buildDashedPath(
    Path source, {
    required double dashLength,
    required double gapLength,
  }) {
    final result = Path();
    for (final ui.PathMetric metric in source.computeMetrics()) {
      var distance = 0.0;
      while (distance < metric.length) {
        final next = math.min(distance + dashLength, metric.length);
        result.addPath(metric.extractPath(distance, next), Offset.zero);
        distance = next + gapLength;
      }
    }
    return result;
  }

  @override
  bool shouldRepaint(covariant TopologyEdgePainter oldDelegate) {
    return oldDelegate.edgePaths != edgePaths ||
        oldDelegate.clusters != clusters ||
        oldDelegate.showClusterBoxes != showClusterBoxes ||
        oldDelegate.highlightedEdgeIds != highlightedEdgeIds ||
        oldDelegate.dimmedEdgeIds != dimmedEdgeIds ||
        oldDelegate.selectedEdgeId != selectedEdgeId ||
        oldDelegate.overlayMode != overlayMode ||
        oldDelegate.showLabels != showLabels ||
        oldDelegate.showTrafficFlow != showTrafficFlow ||
        (oldDelegate.scale >= kTopologyEdgeBadgeMinScale) !=
            (scale >= kTopologyEdgeBadgeMinScale) ||
        oldDelegate.edgeLatencyMs != edgeLatencyMs ||
        oldDelegate.linkTestOverlay != linkTestOverlay ||
        oldDelegate.colorScheme != colorScheme ||
        oldDelegate.semanticColors != semanticColors;
  }
}
