import 'dart:math' as math;

import 'package:flutter/material.dart' show Offset, Path, Rect, Size, immutable;
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_layouts.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_presentation.dart';

/// Estilo de roteamento geométrico dos enlaces no canvas.
enum TopologyEdgeStyle {
  curved(label: 'Curvas suaves'),
  orthogonal(label: 'Ortogonais');

  const TopologyEdgeStyle({required this.label});

  final String label;

  static TopologyEdgeStyle fromStorageKey(String? raw) {
    for (final style in values) {
      if (style.name == raw) return style;
    }
    return TopologyEdgeStyle.curved;
  }
}

/// Geometria pré-calculada de um enlace no espaço do mundo.
@immutable
class TopologyEdgePath {
  const TopologyEdgePath({
    required this.edge,
    required this.sourceCenter,
    required this.targetCenter,
    required this.start,
    required this.end,
    required this.controlPoints,
    required this.style,
    required this.parallelIndex,
    required this.parallelCount,
    required this.midpoint,
    required this.labelTangent,
    required this.polyline,
    required this.totalLength,
  });

  final TopologyEdgeModel edge;
  final Offset sourceCenter;
  final Offset targetCenter;
  final Offset start;
  final Offset end;

  /// Para `curved`: `[cp1, cp2]` (Bezier cúbica).
  /// Para `orthogonal`: pontos intermediários de dobra `[b1, b2, ...]`.
  final List<Offset> controlPoints;
  final TopologyEdgeStyle style;

  /// Índice `0 .. parallelCount - 1` entre enlaces paralelos no mesmo par de nós.
  final int parallelIndex;
  final int parallelCount;

  /// Ponto exato no meio do comprimento do enlace (ideal para badge/pílula).
  final Offset midpoint;

  /// Vetor unitário tangente no ponto médio.
  final Offset labelTangent;

  /// Polilinha amostrada usada para hit-test, comprimento e animação de fluxo.
  final List<Offset> polyline;

  /// Comprimento total em unidades do mundo.
  final double totalLength;

  /// Constrói o `Path` vetorial nítido para pintura em `Canvas`.
  Path toFlutterPath() {
    final path = Path()..moveTo(start.dx, start.dy);
    if (style == TopologyEdgeStyle.curved && controlPoints.length == 2) {
      final cp1 = controlPoints[0];
      final cp2 = controlPoints[1];
      path.cubicTo(cp1.dx, cp1.dy, cp2.dx, cp2.dy, end.dx, end.dy);
      return path;
    }

    if (controlPoints.isEmpty) {
      path.lineTo(end.dx, end.dy);
      return path;
    }

    // Ortogonal com cantos suavemente arredondados.
    final pts = <Offset>[start, ...controlPoints, end];
    const cornerRadius = 12.0;
    for (var i = 1; i < pts.length - 1; i++) {
      final prev = pts[i - 1];
      final curr = pts[i];
      final next = pts[i + 1];
      final v1 = curr - prev;
      final v2 = next - curr;
      final len1 = v1.distance;
      final len2 = v2.distance;
      if (len1 < 0.5 || len2 < 0.5) {
        path.lineTo(curr.dx, curr.dy);
        continue;
      }
      final r = math.min(cornerRadius, math.min(len1, len2) / 2);
      final before = curr - Offset(v1.dx / len1 * r, v1.dy / len1 * r);
      final after = curr + Offset(v2.dx / len2 * r, v2.dy / len2 * r);
      path.lineTo(before.dx, before.dy);
      path.quadraticBezierTo(curr.dx, curr.dy, after.dx, after.dy);
    }
    path.lineTo(end.dx, end.dy);
    return path;
  }
}

/// Ponto animado de fluxo de tráfego ("formiga" / pulso) ao longo de um enlace.
@immutable
class TopologyAntDot {
  const TopologyAntDot({
    required this.position,
    required this.tangent,
    required this.t,
    required this.reverse,
  });

  final Offset position;
  final Offset tangent;
  final double t;
  final bool reverse;
}

/// Interseção exata do raio que sai do centro de `rect` em direção a `targetPoint`
/// com a borda do retângulo (opcionalmente expandido por `margin`).
Offset intersectRectBorder(
  Rect rect,
  Offset targetPoint, {
  double margin = 0.0,
}) {
  final effective = margin != 0.0 ? rect.inflate(margin) : rect;
  final center = effective.center;
  final dx = targetPoint.dx - center.dx;
  final dy = targetPoint.dy - center.dy;
  if (dx.abs() < 1e-6 && dy.abs() < 1e-6) {
    return Offset(effective.right, center.dy);
  }
  final halfW = math.max(0.5, effective.width / 2);
  final halfH = math.max(0.5, effective.height / 2);
  final scaleX = dx.abs() > 1e-6 ? halfW / dx.abs() : double.infinity;
  final scaleY = dy.abs() > 1e-6 ? halfH / dy.abs() : double.infinity;
  final t = math.min(scaleX, scaleY);
  return Offset(center.dx + dx * t, center.dy + dy * t);
}

/// Calcula todas as geometrias de enlaces entre nós posicionados, aplicando
/// automaticamente abertura em leque (fan-out) para enlaces paralelos entre o
/// mesmo par de equipamentos.
List<TopologyEdgePath> buildTopologyEdgePaths({
  required List<TopologyEdgeModel> edges,
  required Map<int, Offset> positions,
  Size nodeSize = kDefaultTopologyNodeSize,
  TopologyEdgeStyle style = TopologyEdgeStyle.curved,
  double parallelSpacing = 28.0,
}) {
  if (edges.isEmpty || positions.isEmpty) return const <TopologyEdgePath>[];

  // Agrupa enlaces pelo par não-ordenado (minId, maxId) para fan-out simétrico.
  final grouped = <(int, int), List<TopologyEdgeModel>>{};
  for (final edge in edges) {
    if (!positions.containsKey(edge.sourceDeviceId) ||
        !positions.containsKey(edge.targetDeviceId)) {
      continue;
    }
    final lo = math.min(edge.sourceDeviceId, edge.targetDeviceId);
    final hi = math.max(edge.sourceDeviceId, edge.targetDeviceId);
    grouped.putIfAbsent((lo, hi), () => <TopologyEdgeModel>[]).add(edge);
  }

  final paths = <TopologyEdgePath>[];
  for (final entry in grouped.entries) {
    final pairEdges = List<TopologyEdgeModel>.from(entry.value)
      ..sort((a, b) => a.id.compareTo(b.id));
    final count = pairEdges.length;

    for (var idx = 0; idx < count; idx++) {
      final edge = pairEdges[idx];
      final srcPos = positions[edge.sourceDeviceId]!;
      final tgtPos = positions[edge.targetDeviceId]!;
      final srcRect = topologyNodeRect(srcPos, nodeSize: nodeSize);
      final tgtRect = topologyNodeRect(tgtPos, nodeSize: nodeSize);

      if (edge.sourceDeviceId == edge.targetDeviceId) {
        paths.add(_buildSelfLoopPath(edge, srcRect, idx, count));
        continue;
      }

      // Sinal canônico baseado na ordem (minId -> maxId) para que links A->B e B->A
      // abram em lados distintos em vez de sobrepor o mesmo traçado.
      final canonicalForward = edge.sourceDeviceId <= edge.targetDeviceId;
      final centeredIdx = idx - (count - 1) / 2.0;
      final canonicalOffset = centeredIdx * parallelSpacing;
      final curvedLaneOffset =
          canonicalOffset * (canonicalForward ? 1.0 : -1.0);

      if (style == TopologyEdgeStyle.orthogonal) {
        paths.add(
          _buildOrthogonalPath(
            edge: edge,
            srcRect: srcRect,
            tgtRect: tgtRect,
            parallelIndex: idx,
            parallelCount: count,
            laneOffset: canonicalOffset,
          ),
        );
      } else {
        paths.add(
          _buildCurvedPath(
            edge: edge,
            srcRect: srcRect,
            tgtRect: tgtRect,
            parallelIndex: idx,
            parallelCount: count,
            laneOffset: curvedLaneOffset,
          ),
        );
      }
    }
  }
  return List<TopologyEdgePath>.unmodifiable(paths);
}

TopologyEdgePath _buildSelfLoopPath(
  TopologyEdgeModel edge,
  Rect rect,
  int index,
  int count,
) {
  final spread = 34.0 + index * 18.0;
  final start = Offset(rect.center.dx - 16.0, rect.top);
  final end = Offset(rect.center.dx + 16.0, rect.top);
  final cp1 = Offset(rect.center.dx - spread, rect.top - spread * 1.4);
  final cp2 = Offset(rect.center.dx + spread, rect.top - spread * 1.4);
  final polyline = _sampleCubic(start, cp1, cp2, end, steps: 20);
  final len = _polylineLength(polyline);
  final mid = _samplePolylineAt(polyline, len, 0.5);
  return TopologyEdgePath(
    edge: edge,
    sourceCenter: rect.center,
    targetCenter: rect.center,
    start: start,
    end: end,
    controlPoints: <Offset>[cp1, cp2],
    style: TopologyEdgeStyle.curved,
    parallelIndex: index,
    parallelCount: count,
    midpoint: mid.$1,
    labelTangent: mid.$2,
    polyline: polyline,
    totalLength: len,
  );
}

TopologyEdgePath _buildCurvedPath({
  required TopologyEdgeModel edge,
  required Rect srcRect,
  required Rect tgtRect,
  required int parallelIndex,
  required int parallelCount,
  required double laneOffset,
}) {
  final c1 = srcRect.center;
  final c2 = tgtRect.center;
  final delta = c2 - c1;
  final dist = math.max(1.0, delta.distance);
  final dir = Offset(delta.dx / dist, delta.dy / dist);
  final normal = Offset(-dir.dy, dir.dx);

  // Desloca levemente os pontos de mira na borda quando há múltiplos enlaces paralelos.
  final anchorShift = parallelCount > 1
      ? Offset(normal.dx * (laneOffset * 0.42), normal.dy * (laneOffset * 0.42))
      : Offset.zero;
  final start = intersectRectBorder(srcRect, c2 + anchorShift);
  final end = intersectRectBorder(tgtRect, c1 + anchorShift);

  final chord = end - start;
  final chordLen = math.max(1.0, chord.distance);
  final bendMagnitude = parallelCount > 1
      ? laneOffset * 1.6
      : math.min(24.0, chordLen * 0.08);

  final Offset cp1;
  final Offset cp2;
  if (parallelCount > 1) {
    // Arco limpo afastado lateralmente para que cada enlace paralelo fique nítido.
    final p1 = Offset.lerp(start, end, 0.30)!;
    final p2 = Offset.lerp(start, end, 0.70)!;
    cp1 = p1 + Offset(normal.dx * bendMagnitude, normal.dy * bendMagnitude);
    cp2 = p2 + Offset(normal.dx * bendMagnitude, normal.dy * bendMagnitude);
  } else if (delta.dy.abs() >= delta.dx.abs() * 0.7) {
    // Fluxo predominantemente vertical: curva S suave entre camadas hierárquicas.
    final pull = math.min(chordLen * 0.38, 110.0);
    final signY = delta.dy >= 0 ? 1.0 : -1.0;
    cp1 = Offset(start.dx, start.dy + signY * pull);
    cp2 = Offset(end.dx, end.dy - signY * pull);
  } else {
    // Fluxo predominantemente horizontal: curva S suave esquerda → direita.
    final pull = math.min(chordLen * 0.38, 110.0);
    final signX = delta.dx >= 0 ? 1.0 : -1.0;
    cp1 = Offset(start.dx + signX * pull, start.dy);
    cp2 = Offset(end.dx - signX * pull, end.dy);
  }

  final polyline = _sampleCubic(start, cp1, cp2, end, steps: 24);
  final len = _polylineLength(polyline);
  final mid = _samplePolylineAt(polyline, len, 0.5);

  return TopologyEdgePath(
    edge: edge,
    sourceCenter: c1,
    targetCenter: c2,
    start: start,
    end: end,
    controlPoints: <Offset>[cp1, cp2],
    style: TopologyEdgeStyle.curved,
    parallelIndex: parallelIndex,
    parallelCount: parallelCount,
    midpoint: mid.$1,
    labelTangent: mid.$2,
    polyline: polyline,
    totalLength: len,
  );
}

TopologyEdgePath _buildOrthogonalPath({
  required TopologyEdgeModel edge,
  required Rect srcRect,
  required Rect tgtRect,
  required int parallelIndex,
  required int parallelCount,
  required double laneOffset,
}) {
  final c1 = srcRect.center;
  final c2 = tgtRect.center;
  final dx = c2.dx - c1.dx;
  final dy = c2.dy - c1.dy;
  final verticalFlow = dy.abs() >= dx.abs();

  final Offset start;
  final Offset end;
  final List<Offset> bends;

  if (verticalFlow) {
    final goingDown = dy >= 0;
    final maxAnchorOffset = math.max(8.0, srcRect.width * 0.36);
    final anchorShift = (laneOffset * 0.65).clamp(
      -maxAnchorOffset,
      maxAnchorOffset,
    );
    start = Offset(
      c1.dx + anchorShift,
      goingDown ? srcRect.bottom : srcRect.top,
    );
    end = Offset(c2.dx + anchorShift, goingDown ? tgtRect.top : tgtRect.bottom);
    final midY = (start.dy + end.dy) / 2 + laneOffset * 0.45;
    if ((start.dx - end.dx).abs() < 1.0 && parallelCount == 1) {
      bends = const <Offset>[];
    } else {
      bends = <Offset>[Offset(start.dx, midY), Offset(end.dx, midY)];
    }
  } else {
    final goingRight = dx >= 0;
    final maxAnchorOffset = math.max(6.0, srcRect.height * 0.34);
    final anchorShift = (laneOffset * 0.65).clamp(
      -maxAnchorOffset,
      maxAnchorOffset,
    );
    start = Offset(
      goingRight ? srcRect.right : srcRect.left,
      c1.dy + anchorShift,
    );
    end = Offset(
      goingRight ? tgtRect.left : tgtRect.right,
      c2.dy + anchorShift,
    );
    final midX = (start.dx + end.dx) / 2 + laneOffset * 0.45;
    if ((start.dy - end.dy).abs() < 1.0 && parallelCount == 1) {
      bends = const <Offset>[];
    } else {
      bends = <Offset>[Offset(midX, start.dy), Offset(midX, end.dy)];
    }
  }

  final polyline = List<Offset>.unmodifiable(<Offset>[start, ...bends, end]);
  final len = _polylineLength(polyline);
  final mid = _samplePolylineAt(polyline, len, 0.5);

  return TopologyEdgePath(
    edge: edge,
    sourceCenter: c1,
    targetCenter: c2,
    start: start,
    end: end,
    controlPoints: List<Offset>.unmodifiable(bends),
    style: TopologyEdgeStyle.orthogonal,
    parallelIndex: parallelIndex,
    parallelCount: parallelCount,
    midpoint: mid.$1,
    labelTangent: mid.$2,
    polyline: polyline,
    totalLength: len,
  );
}

List<Offset> _sampleCubic(
  Offset p0,
  Offset p1,
  Offset p2,
  Offset p3, {
  int steps = 24,
}) {
  final pts = <Offset>[];
  for (var i = 0; i <= steps; i++) {
    final t = i / steps;
    final u = 1.0 - t;
    final tt = t * t;
    final uu = u * u;
    final uuu = uu * u;
    final ttt = tt * t;
    final x =
        uuu * p0.dx + 3 * uu * t * p1.dx + 3 * u * tt * p2.dx + ttt * p3.dx;
    final y =
        uuu * p0.dy + 3 * uu * t * p1.dy + 3 * u * tt * p2.dy + ttt * p3.dy;
    pts.add(Offset(x, y));
  }
  return List<Offset>.unmodifiable(pts);
}

double _polylineLength(List<Offset> pts) {
  var total = 0.0;
  for (var i = 1; i < pts.length; i++) {
    total += (pts[i] - pts[i - 1]).distance;
  }
  return total;
}

(Offset, Offset) _samplePolylineAt(
  List<Offset> pts,
  double totalLength,
  double t,
) {
  if (pts.isEmpty) return (Offset.zero, const Offset(1, 0));
  if (pts.length == 1 || totalLength <= 1e-6) {
    return (pts.first, const Offset(1, 0));
  }
  final clamped = t.clamp(0.0, 1.0);
  final targetDist = totalLength * clamped;
  var traversed = 0.0;

  for (var i = 1; i < pts.length; i++) {
    final a = pts[i - 1];
    final b = pts[i];
    final segVec = b - a;
    final segLen = segVec.distance;
    if (segLen <= 1e-6) continue;
    if (traversed + segLen >= targetDist || i == pts.length - 1) {
      final localT = ((targetDist - traversed) / segLen).clamp(0.0, 1.0);
      final pos = Offset(a.dx + segVec.dx * localT, a.dy + segVec.dy * localT);
      final tangent = Offset(segVec.dx / segLen, segVec.dy / segLen);
      return (pos, tangent);
    }
    traversed += segLen;
  }
  final lastSeg = pts.last - pts[pts.length - 2];
  final lastLen = math.max(1e-6, lastSeg.distance);
  return (pts.last, Offset(lastSeg.dx / lastLen, lastSeg.dy / lastLen));
}

/// Menor distância euclidiana de `point` até o segmento `[a, b]`.
double _distancePointToSegment(Offset point, Offset a, Offset b) {
  final ab = b - a;
  final lenSq = ab.dx * ab.dx + ab.dy * ab.dy;
  if (lenSq <= 1e-9) return (point - a).distance;
  final ap = point - a;
  final proj = ((ap.dx * ab.dx + ap.dy * ab.dy) / lenSq).clamp(0.0, 1.0);
  final closest = Offset(a.dx + ab.dx * proj, a.dy + ab.dy * proj);
  return (point - closest).distance;
}

/// Distância mínima entre um ponto do mundo e a polilinha de um `TopologyEdgePath`.
double distanceToEdgePath(Offset worldPoint, TopologyEdgePath path) {
  final pts = path.polyline;
  if (pts.isEmpty) return double.infinity;
  if (pts.length == 1) return (worldPoint - pts.first).distance;
  var best = double.infinity;
  for (var i = 1; i < pts.length; i++) {
    final d = _distancePointToSegment(worldPoint, pts[i - 1], pts[i]);
    if (d < best) best = d;
  }
  return best;
}

/// Testa se um toque em `worldPoint` atingiu algum enlace dentro de `tolerance`.
/// Retorna o enlace mais próximo (ideal para toque confortável em telas mobile).
TopologyEdgeModel? hitTestEdge(
  Offset worldPoint,
  List<TopologyEdgePath> paths, {
  double tolerance = 22.0,
}) {
  TopologyEdgeModel? bestEdge;
  var bestDist = tolerance;
  for (final path in paths) {
    final dist = distanceToEdgePath(worldPoint, path);
    if (dist <= bestDist) {
      bestDist = dist;
      bestEdge = path.edge;
    }
  }
  return bestEdge;
}

/// Amostra um ponto ao longo de `path` parametrizado por comprimento (`0.0..1.0`).
Offset sampleEdgePoint(TopologyEdgePath path, double t) {
  return _samplePolylineAt(path.polyline, path.totalLength, t).$1;
}

/// Amostra o vetor tangente unitário ao longo de `path` em `t` (`0.0..1.0`).
Offset sampleEdgeTangent(TopologyEdgePath path, double t) {
  return _samplePolylineAt(path.polyline, path.totalLength, t).$2;
}

/// Calcula multiplicador de velocidade visual das "formigas" de fluxo segundo
/// utilização ou volume de tráfego do enlace (`0.0` se link estiver fora do ar).
double computeTrafficFlowSpeedFactor(TopologyEdgeModel edge) {
  if (classifyEdgeStatus(edge) == TopologyEdgeStatus.down) return 0.0;
  final util = resolveEdgeUtilization(edge);
  if (util != null) {
    return (0.45 + util * 2.05).clamp(0.45, 2.5);
  }
  final bps = edge.effectiveTrafficBps ?? 0.0;
  if (bps > 0) {
    final logScale = (math.log(bps + 1.0) / math.ln10) / 10.0;
    return (0.5 + logScale * 1.6).clamp(0.5, 2.2);
  }
  return 0.6;
}

/// Calcula posições dos pontos animados ("formigas") ao longo de um enlace.
///
/// `phase` é o progresso normalizado (`0.0..1.0`) de um `AnimationController`.
/// O número de pontos escala suavemente com o comprimento do enlace.
List<TopologyAntDot> computeAntDotPositions(
  TopologyEdgePath path,
  double phase, {
  double spacing = 56.0,
  int minDots = 1,
  int maxDots = 8,
  double speedMultiplier = 1.0,
  bool reverse = false,
}) {
  if (path.totalLength <= 8.0 || speedMultiplier <= 0.0) {
    return const <TopologyAntDot>[];
  }
  final safeSpacing = math.max(16.0, spacing);
  final rawCount = (path.totalLength / safeSpacing).floor();
  final count = rawCount.clamp(minDots, math.max(minDots, maxDots));

  final wrappedPhase = (phase * speedMultiplier) % 1.0;
  final positivePhase = wrappedPhase < 0 ? wrappedPhase + 1.0 : wrappedPhase;
  final dots = <TopologyAntDot>[];

  for (var i = 0; i < count; i++) {
    final baseT = (i + positivePhase) / count;
    final t = reverse ? (1.0 - baseT) : baseT;
    final sample = _samplePolylineAt(path.polyline, path.totalLength, t);
    final tangent = reverse ? Offset(-sample.$2.dx, -sample.$2.dy) : sample.$2;
    dots.add(
      TopologyAntDot(
        position: sample.$1,
        tangent: tangent,
        t: t,
        reverse: reverse,
      ),
    );
  }
  return List<TopologyAntDot>.unmodifiable(dots);
}
