import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Escala mínima de zoom em que as pílulas de rótulo dos enlaces são exibidas.
const double kTopologyEdgeBadgeMinScale = 0.55;

/// Escala mínima de zoom em que a 2ª linha (`capacidade · utilização`) aparece na pílula.
const double kTopologyEdgeBadgeSecondaryMinScale = 0.68;

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

    // Pílulas NOC de rótulo no ponto médio (zoom >= 0.55), com afastamento anti-colisão.
    if (showLabels && scale >= kTopologyEdgeBadgeMinScale) {
      final layouts = <_PreparedEdgeBadge>[
        for (final ep in backgroundEdges)
          ?_prepareEdgeBadge(ep, dimmed: true, emphasis: false),
        for (final ep in regularEdges)
          ?_prepareEdgeBadge(ep, dimmed: false, emphasis: false),
        for (final ep in emphasisEdges)
          ?_prepareEdgeBadge(ep, dimmed: false, emphasis: true),
      ];
      _resolveBadgeOverlaps(layouts);
      for (final badge in layouts) {
        _paintPreparedEdgeBadge(canvas, badge);
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

  _PreparedEdgeBadge? _prepareEdgeBadge(
    TopologyEdgePath edgePath, {
    required bool dimmed,
    required bool emphasis,
  }) {
    final edge = edgePath.edge;
    final linkOverlay = overlayMode == TopologyOverlayMode.linkTest
        ? linkTestOverlay[edge.id]
        : null;
    final spec = resolveEdgeBadgeSpec(
      edge,
      overlay: overlayMode,
      latencyMs: edgeLatencyMs[edge.id],
      linkTestLabel: linkOverlay?.label,
      linkTestTone: linkOverlay?.tone,
    );
    if (spec.primaryLabel.trim().isEmpty && !spec.hasDirectionalTraffic) {
      return null;
    }

    final isSelected = edge.id == selectedEdgeId;
    final alpha = dimmed ? kTopologyEdgeDimmedOpacity : 1.0;
    final pair = semanticColors.resolve(spec.tone, colorScheme);
    final baseStyle = textTheme.labelSmall ?? const TextStyle(fontSize: 11);

    final primaryTextColor = isSelected
        ? colorScheme.onPrimaryContainer
        : spec.isDown
        ? pair.onContainer
        : colorScheme.onSurface;
    final secondaryTextColor = isSelected
        ? colorScheme.onPrimaryContainer.withValues(alpha: 0.85 * alpha)
        : colorScheme.onSurfaceVariant.withValues(alpha: alpha);

    TextPainter? primaryPainter;
    TextPainter? rxPainter;
    TextPainter? sepPainter;
    TextPainter? txPainter;

    const arrowBox = 8.5;
    const arrowGap = 2.5;
    const sepGap = 4.5;

    double line1Width;
    double line1Height;

    if (spec.hasDirectionalTraffic) {
      final dirStyle = baseStyle.copyWith(
        fontSize: 10.0,
        fontWeight: FontWeight.w700,
        color: primaryTextColor.withValues(alpha: alpha),
        height: 1.15,
      );
      rxPainter = TextPainter(
        text: TextSpan(text: spec.rxLabel, style: dirStyle),
        maxLines: 1,
        textDirection: TextDirection.ltr,
      )..layout(maxWidth: 84);
      sepPainter = TextPainter(
        text: TextSpan(
          text: '·',
          style: dirStyle.copyWith(color: secondaryTextColor),
        ),
        maxLines: 1,
        textDirection: TextDirection.ltr,
      )..layout();
      txPainter = TextPainter(
        text: TextSpan(text: spec.txLabel, style: dirStyle),
        maxLines: 1,
        textDirection: TextDirection.ltr,
      )..layout(maxWidth: 84);

      line1Width =
          arrowBox +
          arrowGap +
          rxPainter.width +
          sepGap +
          sepPainter.width +
          sepGap +
          arrowBox +
          arrowGap +
          txPainter.width;
      line1Height = math.max(
        arrowBox,
        math.max(rxPainter.height, txPainter.height),
      );
    } else {
      primaryPainter = TextPainter(
        text: TextSpan(
          text: spec.primaryLabel,
          style: baseStyle.copyWith(
            fontSize: 10.2,
            fontWeight: FontWeight.w700,
            color: primaryTextColor.withValues(alpha: alpha),
            height: 1.15,
          ),
        ),
        maxLines: 1,
        ellipsis: '…',
        textDirection: TextDirection.ltr,
      )..layout(maxWidth: 164);
      line1Width = primaryPainter.width + (spec.isDown ? 10.0 : 0.0);
      line1Height = primaryPainter.height;
    }

    TextPainter? secondaryPainter;
    final showSecondary =
        spec.secondaryLabel != null &&
        spec.secondaryLabel!.trim().isNotEmpty &&
        scale >= kTopologyEdgeBadgeSecondaryMinScale;
    if (showSecondary) {
      secondaryPainter = TextPainter(
        text: TextSpan(
          text: spec.secondaryLabel,
          style: baseStyle.copyWith(
            fontSize: 9.2,
            fontWeight: FontWeight.w600,
            color: secondaryTextColor,
            height: 1.1,
          ),
        ),
        maxLines: 1,
        ellipsis: '…',
        textDirection: TextDirection.ltr,
      )..layout(maxWidth: 164);
    }

    final hasUtilBar =
        spec.utilization != null && spec.utilization!.isFinite && !spec.isDown;
    const hPad = 7.5;
    const topPad = 4.0;
    final bottomPad = hasUtilBar ? 7.0 : 4.0;
    const lineGap = 1.8;

    final contentWidth = secondaryPainter != null
        ? math.max(line1Width, secondaryPainter.width)
        : line1Width;
    final contentHeight =
        line1Height +
        (secondaryPainter != null ? lineGap + secondaryPainter.height : 0.0);

    return _PreparedEdgeBadge(
      edge: edge,
      spec: spec,
      pair: pair,
      naturalCenter: edgePath.midpoint,
      center: edgePath.midpoint,
      size: Size(contentWidth + hPad * 2, contentHeight + topPad + bottomPad),
      dimmed: dimmed,
      emphasis: emphasis,
      isSelected: isSelected,
      hasUtilBar: hasUtilBar,
      line1Width: line1Width,
      line1Height: line1Height,
      hPad: hPad,
      topPad: topPad,
      lineGap: lineGap,
      primaryPainter: primaryPainter,
      rxPainter: rxPainter,
      sepPainter: sepPainter,
      txPainter: txPainter,
      secondaryPainter: secondaryPainter,
    );
  }

  void _resolveBadgeOverlaps(List<_PreparedEdgeBadge> badges) {
    if (badges.length < 2) return;
    const minGap = 5.0;
    const maxShiftX = 34.0;
    const maxShiftY = 18.0;

    for (var pass = 0; pass < 2; pass++) {
      for (var i = 0; i < badges.length; i++) {
        for (var j = i + 1; j < badges.length; j++) {
          final a = badges[i];
          final b = badges[j];
          if (a.dimmed != b.dimmed) continue;
          final rectA = a.rect.inflate(minGap / 2);
          final rectB = b.rect.inflate(minGap / 2);
          if (!rectA.overlaps(rectB)) continue;

          final overlapX =
              math.min(rectA.right, rectB.right) -
              math.max(rectA.left, rectB.left);
          final overlapY =
              math.min(rectA.bottom, rectB.bottom) -
              math.max(rectA.top, rectB.top);
          if (overlapX <= 0 || overlapY <= 0) continue;

          final dx = b.center.dx - a.center.dx;
          if (dx.abs() >= 6.0) {
            final dir = dx >= 0 ? 1.0 : -1.0;
            final step = overlapX * 0.52;
            a.shift(dx: -dir * step, dy: 0, maxDx: maxShiftX, maxDy: maxShiftY);
            b.shift(dx: dir * step, dy: 0, maxDx: maxShiftX, maxDy: maxShiftY);
          } else {
            final dy = b.center.dy - a.center.dy;
            final dir = dy >= 0 ? 1.0 : -1.0;
            final step = overlapY * 0.52;
            a.shift(dx: 0, dy: -dir * step, maxDx: maxShiftX, maxDy: maxShiftY);
            b.shift(dx: 0, dy: dir * step, maxDx: maxShiftX, maxDy: maxShiftY);
          }
        }
      }
    }
  }

  void _paintPreparedEdgeBadge(Canvas canvas, _PreparedEdgeBadge badge) {
    final pillRect = badge.rect;
    final pillRRect = RRect.fromRectAndRadius(
      pillRect,
      const Radius.circular(8.5),
    );
    final alpha = badge.dimmed ? kTopologyEdgeDimmedOpacity : 1.0;

    if (!badge.dimmed) {
      canvas.drawRRect(
        pillRRect.shift(const Offset(0, 1.4)),
        Paint()..color = colorScheme.shadow.withValues(alpha: 0.18),
      );
    }

    final fillColor = badge.isSelected
        ? colorScheme.primaryContainer.withValues(
            alpha: badge.dimmed ? 0.5 : 0.98,
          )
        : badge.spec.isDown
        ? badge.pair.container.withValues(alpha: badge.dimmed ? 0.45 : 0.96)
        : colorScheme.surface.withValues(alpha: badge.dimmed ? 0.48 : 0.97);

    final borderColor = badge.isSelected
        ? colorScheme.primary
        : badge.spec.isDown
        ? semanticColors.error.withValues(alpha: badge.dimmed ? 0.36 : 0.88)
        : badge.emphasis
        ? semanticColors.warning
        : badge.pair.foreground.withValues(alpha: badge.dimmed ? 0.32 : 0.76);

    canvas.drawRRect(pillRRect, Paint()..color = fillColor);
    canvas.drawRRect(
      pillRRect,
      Paint()
        ..style = PaintingStyle.stroke
        ..strokeWidth = badge.isSelected || badge.emphasis ? 1.7 : 1.15
        ..color = borderColor,
    );

    final line1Left = pillRect.left + (pillRect.width - badge.line1Width) / 2.0;
    final line1Top = pillRect.top + badge.topPad;

    if (badge.spec.hasDirectionalTraffic) {
      const arrowBox = 8.5;
      const arrowGap = 2.5;
      const sepGap = 4.5;

      final rxArrowColor =
          (badge.isSelected ? semanticColors.success : semanticColors.success)
              .withValues(alpha: alpha);
      final txArrowColor = colorScheme.primary.withValues(alpha: alpha);

      var cursorX = line1Left;
      final centerY = line1Top + badge.line1Height / 2.0;

      _paintMiniArrow(
        canvas,
        center: Offset(cursorX + arrowBox / 2.0, centerY),
        downward: true,
        color: rxArrowColor,
        size: arrowBox,
      );
      cursorX += arrowBox + arrowGap;

      final rxP = badge.rxPainter!;
      rxP.paint(
        canvas,
        Offset(cursorX, line1Top + (badge.line1Height - rxP.height) / 2.0),
      );
      cursorX += rxP.width + sepGap;

      final sepP = badge.sepPainter!;
      sepP.paint(
        canvas,
        Offset(cursorX, line1Top + (badge.line1Height - sepP.height) / 2.0),
      );
      cursorX += sepP.width + sepGap;

      _paintMiniArrow(
        canvas,
        center: Offset(cursorX + arrowBox / 2.0, centerY),
        downward: false,
        color: txArrowColor,
        size: arrowBox,
      );
      cursorX += arrowBox + arrowGap;

      final txP = badge.txPainter!;
      txP.paint(
        canvas,
        Offset(cursorX, line1Top + (badge.line1Height - txP.height) / 2.0),
      );
    } else if (badge.primaryPainter != null) {
      final p = badge.primaryPainter!;
      var textX = line1Left;
      if (badge.spec.isDown) {
        final dotCenter = Offset(
          line1Left + 3.2,
          line1Top + badge.line1Height / 2.0,
        );
        canvas.drawCircle(
          dotCenter,
          3.0,
          Paint()..color = semanticColors.error.withValues(alpha: alpha),
        );
        textX += 10.0;
      }
      p.paint(
        canvas,
        Offset(textX, line1Top + (badge.line1Height - p.height) / 2.0),
      );
    }

    if (badge.secondaryPainter != null) {
      final secP = badge.secondaryPainter!;
      final secLeft = pillRect.left + (pillRect.width - secP.width) / 2.0;
      final secTop = line1Top + badge.line1Height + badge.lineGap;
      secP.paint(canvas, Offset(secLeft, secTop));
    }

    if (badge.hasUtilBar) {
      final util = badge.spec.utilization!.clamp(0.0, 1.0);
      final utilColor = semanticColors
          .resolve(utilizationTone(util), colorScheme)
          .foreground
          .withValues(alpha: alpha);
      final trackLeft = pillRect.left + 6.5;
      final trackWidth = math.max(8.0, pillRect.width - 13.0);
      final trackRect = Rect.fromLTWH(
        trackLeft,
        pillRect.bottom - 4.8,
        trackWidth,
        2.5,
      );
      canvas.drawRRect(
        RRect.fromRectAndRadius(trackRect, const Radius.circular(2)),
        Paint()
          ..color = colorScheme.surfaceContainerHighest.withValues(
            alpha: alpha * 0.9,
          ),
      );
      if (util > 0.0) {
        final fillW = math.min(trackWidth, math.max(3.5, trackWidth * util));
        final fillRect = Rect.fromLTWH(
          trackLeft,
          trackRect.top,
          fillW,
          trackRect.height,
        );
        canvas.drawRRect(
          RRect.fromRectAndRadius(fillRect, const Radius.circular(2)),
          Paint()..color = utilColor,
        );
      }
    }
  }

  /// Desenha mini-seta vetorial nítida (Download `downward: true` ou Upload `false`)
  /// sem depender de glifos de fonte Unicode (`↓`/`↑`).
  void _paintMiniArrow(
    Canvas canvas, {
    required Offset center,
    required bool downward,
    required Color color,
    double size = 8.5,
  }) {
    final halfH = size * 0.44;
    final wingW = size * 0.34;
    final wingH = size * 0.32;
    final topY = center.dy - halfH;
    final bottomY = center.dy + halfH;

    final paint = Paint()
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.65
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round
      ..color = color;

    canvas.drawLine(Offset(center.dx, topY), Offset(center.dx, bottomY), paint);

    final head = Path();
    if (downward) {
      head
        ..moveTo(center.dx - wingW, bottomY - wingH)
        ..lineTo(center.dx, bottomY)
        ..lineTo(center.dx + wingW, bottomY - wingH);
    } else {
      head
        ..moveTo(center.dx - wingW, topY + wingH)
        ..lineTo(center.dx, topY)
        ..lineTo(center.dx + wingW, topY + wingH);
    }
    canvas.drawPath(head, paint);
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
        (oldDelegate.scale >= kTopologyEdgeBadgeSecondaryMinScale) !=
            (scale >= kTopologyEdgeBadgeSecondaryMinScale) ||
        oldDelegate.edgeLatencyMs != edgeLatencyMs ||
        oldDelegate.linkTestOverlay != linkTestOverlay ||
        oldDelegate.colorScheme != colorScheme ||
        oldDelegate.semanticColors != semanticColors;
  }
}

class _PreparedEdgeBadge {
  _PreparedEdgeBadge({
    required this.edge,
    required this.spec,
    required this.pair,
    required this.naturalCenter,
    required this.center,
    required this.size,
    required this.dimmed,
    required this.emphasis,
    required this.isSelected,
    required this.hasUtilBar,
    required this.line1Width,
    required this.line1Height,
    required this.hPad,
    required this.topPad,
    required this.lineGap,
    this.primaryPainter,
    this.rxPainter,
    this.sepPainter,
    this.txPainter,
    this.secondaryPainter,
  });

  final TopologyEdgeModel edge;
  final TopologyEdgeBadgeSpec spec;
  final SemanticColorPair pair;
  final Offset naturalCenter;
  Offset center;
  final Size size;
  final bool dimmed;
  final bool emphasis;
  final bool isSelected;
  final bool hasUtilBar;
  final double line1Width;
  final double line1Height;
  final double hPad;
  final double topPad;
  final double lineGap;
  final TextPainter? primaryPainter;
  final TextPainter? rxPainter;
  final TextPainter? sepPainter;
  final TextPainter? txPainter;
  final TextPainter? secondaryPainter;

  Rect get rect =>
      Rect.fromCenter(center: center, width: size.width, height: size.height);

  void shift({
    required double dx,
    required double dy,
    required double maxDx,
    required double maxDy,
  }) {
    final nextX = (center.dx + dx).clamp(
      naturalCenter.dx - maxDx,
      naturalCenter.dx + maxDx,
    );
    final nextY = (center.dy + dy).clamp(
      naturalCenter.dy - maxDy,
      naturalCenter.dy + maxDy,
    );
    center = Offset(nextX, nextY);
  }
}
