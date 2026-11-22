import 'dart:math' as math;

import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

@immutable
class MetricSeriesPoint {
  const MetricSeriesPoint({required this.x, required this.y, this.label});

  final double x;
  final double y;
  final String? label;
}

@immutable
class MetricLineSeries {
  const MetricLineSeries({
    required this.name,
    required this.points,
    this.tone = SemanticTone.primary,
    this.showArea = true,
  });

  final String name;
  final List<MetricSeriesPoint> points;
  final SemanticTone tone;
  final bool showArea;
}

/// Gráfico de linha reutilizável sobre `fl_chart` com cores semânticas e tooltip formatado.
class MetricLineChart extends StatelessWidget {
  const MetricLineChart({
    super.key,
    required this.series,
    this.height = 220,
    this.yFormatter,
    this.xFormatter,
    this.minY,
    this.maxY,
    this.emptyText = 'Sem dados no período selecionado.',
  });

  final List<MetricLineSeries> series;
  final double height;
  final String Function(double value)? yFormatter;
  final String Function(double value)? xFormatter;
  final double? minY;
  final double? maxY;
  final String emptyText;

  @override
  Widget build(BuildContext context) {
    final nonEmpty = series.where((s) => s.points.isNotEmpty).toList();
    final scheme = context.colorScheme;
    final semantics = context.semanticColors;

    if (nonEmpty.isEmpty) {
      return SizedBox(
        height: height,
        child: Center(
          child: Text(
            emptyText,
            style: context.textTheme.bodyMedium?.copyWith(
              color: scheme.onSurfaceVariant,
            ),
          ),
        ),
      );
    }

    var computedMinY = double.infinity;
    var computedMaxY = double.negativeInfinity;
    for (final s in nonEmpty) {
      for (final pt in s.points) {
        if (pt.y < computedMinY) computedMinY = pt.y;
        if (pt.y > computedMaxY) computedMaxY = pt.y;
      }
    }

    final effectiveMinY = minY ?? math.min(0.0, computedMinY);
    final rawMaxY =
        maxY ??
        (computedMaxY == effectiveMinY
            ? computedMaxY + 1
            : computedMaxY * 1.12);
    final effectiveMaxY = rawMaxY <= effectiveMinY
        ? effectiveMinY + 1
        : rawMaxY;

    final bars = nonEmpty
        .map((s) {
          final colors = semantics.resolve(s.tone, scheme);
          return LineChartBarData(
            spots: s.points
                .map((pt) => FlSpot(pt.x, pt.y))
                .toList(growable: false),
            isCurved: true,
            preventCurveOverShooting: true,
            barWidth: 2.5,
            color: colors.foreground,
            dotData: FlDotData(show: s.points.length <= 2),
            belowBarData: BarAreaData(
              show: s.showArea,
              gradient: LinearGradient(
                begin: Alignment.topCenter,
                end: Alignment.bottomCenter,
                colors: <Color>[
                  colors.foreground.withValues(alpha: 0.25),
                  colors.foreground.withValues(alpha: 0.02),
                ],
              ),
            ),
          );
        })
        .toList(growable: false);

    return SizedBox(
      height: height,
      child: LineChart(
        LineChartData(
          minY: effectiveMinY,
          maxY: effectiveMaxY,
          gridData: FlGridData(
            show: true,
            drawVerticalLine: false,
            getDrawingHorizontalLine: (_) => FlLine(
              color: scheme.outlineVariant.withValues(alpha: 0.6),
              strokeWidth: 1,
            ),
          ),
          borderData: FlBorderData(
            show: true,
            border: Border(
              bottom: BorderSide(color: scheme.outlineVariant),
              left: BorderSide(color: scheme.outlineVariant),
            ),
          ),
          titlesData: FlTitlesData(
            topTitles: const AxisTitles(
              sideTitles: SideTitles(showTitles: false),
            ),
            rightTitles: const AxisTitles(
              sideTitles: SideTitles(showTitles: false),
            ),
            leftTitles: AxisTitles(
              sideTitles: SideTitles(
                showTitles: true,
                reservedSize: 52,
                getTitlesWidget: (value, meta) {
                  if (value == meta.min || value == meta.max) {
                    return const SizedBox.shrink();
                  }
                  final text = yFormatter != null
                      ? yFormatter!(value)
                      : formatDecimal(
                          value,
                          maxDigits: value.abs() >= 100 ? 0 : 1,
                        );
                  return Padding(
                    padding: const EdgeInsets.only(right: 6),
                    child: Text(
                      text,
                      textAlign: TextAlign.right,
                      style: TextStyle(
                        fontSize: 11,
                        color: scheme.onSurfaceVariant,
                      ),
                    ),
                  );
                },
              ),
            ),
            bottomTitles: AxisTitles(
              sideTitles: SideTitles(
                showTitles: xFormatter != null,
                reservedSize: 26,
                getTitlesWidget: (value, meta) {
                  if (xFormatter == null) return const SizedBox.shrink();
                  return Padding(
                    padding: const EdgeInsets.only(top: 6),
                    child: Text(
                      xFormatter!(value),
                      style: TextStyle(
                        fontSize: 11,
                        color: scheme.onSurfaceVariant,
                      ),
                    ),
                  );
                },
              ),
            ),
          ),
          lineTouchData: LineTouchData(
            touchTooltipData: LineTouchTooltipData(
              getTooltipColor: (_) => scheme.inverseSurface,
              getTooltipItems: (spots) {
                return spots
                    .map((spot) {
                      final seriesItem = nonEmpty[spot.barIndex];
                      final formattedY = yFormatter != null
                          ? yFormatter!(spot.y)
                          : formatDecimal(spot.y, maxDigits: 1, minDigits: 1);
                      return LineTooltipItem(
                        '${seriesItem.name}: $formattedY',
                        TextStyle(
                          color: scheme.onInverseSurface,
                          fontWeight: FontWeight.w700,
                          fontSize: 12,
                        ),
                      );
                    })
                    .toList(growable: false);
              },
            ),
          ),
          lineBarsData: bars,
        ),
      ),
    );
  }
}
