import 'dart:math' as math;

import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

Color _seriesColor(BuildContext context, AiChartSeries series, int index) {
  final semantics = context.semanticColors;
  final scheme = context.colorScheme;
  if (series.id == 'inBps') return semantics.success;
  if (series.id == 'outBps') return semantics.info;
  final palette = <Color>[
    scheme.primary,
    semantics.warning,
    semantics.ai,
    semantics.info,
    semantics.error,
    scheme.secondary,
  ];
  return palette[index % palette.length];
}

String _formatChartValue(double value, String unit) {
  return switch (unit) {
    'latency' => formatLatency(value),
    'bandwidth' => formatBps(value, fractionDigits: 1),
    'percentage' => formatPercent(value, digits: 1),
    'bytes' => formatBytes(value, fractionDigits: 1),
    _ => formatDecimal(value, maxDigits: value.abs() >= 100 ? 0 : 1),
  };
}

/// Gráfico `fl_chart` embutido em respostas da IA (`AiChart`), com suporte a linha/área e barras.
class AiToolChart extends StatefulWidget {
  const AiToolChart({super.key, required this.chart});

  final AiChart chart;

  @override
  State<AiToolChart> createState() => _AiToolChartState();
}

class _AiToolChartState extends State<AiToolChart> {
  bool _barMode = false;

  @override
  Widget build(BuildContext context) {
    final chart = widget.chart;
    final nonEmpty = chart.series
        .where((s) => s.points.isNotEmpty)
        .toList(growable: false);
    if (nonEmpty.isEmpty) return const SizedBox.shrink();

    final scheme = context.colorScheme;
    final fillArea = nonEmpty.length == 1;

    final timestamps = <DateTime>[];
    for (final pt in nonEmpty.first.points) {
      final parsed = parseDateOrNull(pt.time);
      if (parsed != null) timestamps.add(parsed);
    }
    final spanMs = timestamps.length >= 2
        ? timestamps.last.difference(timestamps.first).inMilliseconds.abs()
        : 0;

    return NestedPanel(
      padding: const EdgeInsets.all(12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Row(
            children: <Widget>[
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      chart.title,
                      style: context.textTheme.titleSmall?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    if (chart.subtitle != null &&
                        chart.subtitle!.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 2),
                      Text(
                        chart.subtitle!,
                        style: context.textTheme.bodySmall?.copyWith(
                          color: scheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              IconButton(
                tooltip: _barMode ? 'Ver como linha' : 'Ver como barras',
                visualDensity: VisualDensity.compact,
                icon: Icon(
                  _barMode ? Icons.show_chart : Icons.bar_chart,
                  size: 18,
                ),
                onPressed: () => setState(() => _barMode = !_barMode),
              ),
            ],
          ),
          if (nonEmpty.length > 1 || chart.avgValue != null) ...<Widget>[
            const SizedBox(height: 8),
            Wrap(
              spacing: 12,
              runSpacing: 6,
              children: <Widget>[
                for (var i = 0; i < nonEmpty.length; i++)
                  Row(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      Container(
                        width: 10,
                        height: 10,
                        decoration: BoxDecoration(
                          color: _seriesColor(context, nonEmpty[i], i),
                          shape: BoxShape.circle,
                        ),
                      ),
                      const SizedBox(width: 6),
                      Text(
                        nonEmpty[i].label,
                        style: context.textTheme.labelSmall?.copyWith(
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ],
                  ),
                if (chart.avgValue != null)
                  Text(
                    'Média: ${_formatChartValue(chart.avgValue!, chart.unit)}',
                    style: context.textTheme.labelSmall?.copyWith(
                      color: scheme.primary,
                      fontWeight: FontWeight.w700,
                    ),
                  ),
              ],
            ),
          ],
          const SizedBox(height: 12),
          SizedBox(
            height: 200,
            child: _barMode
                ? _buildBarChart(context, nonEmpty, spanMs)
                : _buildLineChart(context, nonEmpty, fillArea, spanMs),
          ),
        ],
      ),
    );
  }

  Widget _buildLineChart(
    BuildContext context,
    List<AiChartSeries> seriesList,
    bool fillArea,
    int spanMs,
  ) {
    final scheme = context.colorScheme;
    var minY = double.infinity;
    var maxY = double.negativeInfinity;
    for (final s in seriesList) {
      for (final p in s.points) {
        if (p.value < minY) minY = p.value;
        if (p.value > maxY) maxY = p.value;
      }
    }
    if (widget.chart.avgValue != null) {
      minY = math.min(minY, widget.chart.avgValue!);
      maxY = math.max(maxY, widget.chart.avgValue!);
    }
    final effMinY = math.min(0.0, minY);
    final effMaxY = maxY <= effMinY ? effMinY + 1 : maxY * 1.15;
    final refPoints = seriesList.first.points;

    return LineChart(
      LineChartData(
        minY: effMinY,
        maxY: effMaxY,
        gridData: FlGridData(
          show: true,
          drawVerticalLine: false,
          getDrawingHorizontalLine: (_) => FlLine(
            color: scheme.outlineVariant.withValues(alpha: 0.5),
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
        extraLinesData: widget.chart.avgValue != null
            ? ExtraLinesData(
                horizontalLines: <HorizontalLine>[
                  HorizontalLine(
                    y: widget.chart.avgValue!,
                    color: scheme.primary,
                    strokeWidth: 1.5,
                    dashArray: const <int>[6, 4],
                  ),
                ],
              )
            : null,
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
              reservedSize: 56,
              getTitlesWidget: (value, meta) {
                if (value == meta.min || value == meta.max) {
                  return const SizedBox.shrink();
                }
                return Padding(
                  padding: const EdgeInsets.only(right: 6),
                  child: Text(
                    _formatChartValue(value, widget.chart.unit),
                    textAlign: TextAlign.right,
                    style: TextStyle(
                      fontSize: 10.5,
                      color: scheme.onSurfaceVariant,
                    ),
                  ),
                );
              },
            ),
          ),
          bottomTitles: AxisTitles(
            sideTitles: SideTitles(
              showTitles: refPoints.length > 1,
              reservedSize: 24,
              interval: math.max(1, (refPoints.length / 4).ceil()).toDouble(),
              getTitlesWidget: (value, _) {
                final idx = value.round();
                if (idx < 0 || idx >= refPoints.length) {
                  return const SizedBox.shrink();
                }
                final raw = refPoints[idx].label ?? refPoints[idx].time;
                final label = parseDateOrNull(raw) != null
                    ? formatAxisTime(raw, spanMs)
                    : raw;
                return Padding(
                  padding: const EdgeInsets.only(top: 6),
                  child: Text(
                    label,
                    style: TextStyle(
                      fontSize: 10,
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
                    final s = seriesList[spot.barIndex];
                    return LineTooltipItem(
                      '${s.label}: ${_formatChartValue(spot.y, widget.chart.unit)}',
                      TextStyle(
                        color: scheme.onInverseSurface,
                        fontWeight: FontWeight.w700,
                        fontSize: 11.5,
                      ),
                    );
                  })
                  .toList(growable: false);
            },
          ),
        ),
        lineBarsData: <LineChartBarData>[
          for (var i = 0; i < seriesList.length; i++)
            _buildLineBarData(context, seriesList[i], i, fillArea),
        ],
      ),
    );
  }

  LineChartBarData _buildLineBarData(
    BuildContext context,
    AiChartSeries series,
    int index,
    bool fillArea,
  ) {
    final color = _seriesColor(context, series, index);
    return LineChartBarData(
      spots: <FlSpot>[
        for (var j = 0; j < series.points.length; j++)
          FlSpot(j.toDouble(), series.points[j].value),
      ],
      isCurved: true,
      preventCurveOverShooting: true,
      barWidth: 2.2,
      color: color,
      dotData: FlDotData(show: series.points.length <= 2),
      belowBarData: BarAreaData(
        show: fillArea,
        gradient: LinearGradient(
          begin: Alignment.topCenter,
          end: Alignment.bottomCenter,
          colors: <Color>[
            color.withValues(alpha: 0.25),
            color.withValues(alpha: 0.02),
          ],
        ),
      ),
    );
  }

  Widget _buildBarChart(
    BuildContext context,
    List<AiChartSeries> seriesList,
    int spanMs,
  ) {
    final scheme = context.colorScheme;
    final primary = seriesList.first;
    final barColor = _seriesColor(context, primary, 0);
    final sampleCount = math.min(primary.points.length, 24);
    final step = primary.points.length > sampleCount
        ? (primary.points.length / sampleCount).ceil()
        : 1;
    final sampled = <AiChartPoint>[];
    for (var i = 0; i < primary.points.length; i += step) {
      sampled.add(primary.points[i]);
    }

    return BarChart(
      BarChartData(
        gridData: FlGridData(
          show: true,
          drawVerticalLine: false,
          getDrawingHorizontalLine: (_) => FlLine(
            color: scheme.outlineVariant.withValues(alpha: 0.5),
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
              reservedSize: 56,
              getTitlesWidget: (value, meta) {
                if (value == meta.min || value == meta.max) {
                  return const SizedBox.shrink();
                }
                return Text(
                  _formatChartValue(value, widget.chart.unit),
                  style: TextStyle(
                    fontSize: 10.5,
                    color: scheme.onSurfaceVariant,
                  ),
                );
              },
            ),
          ),
          bottomTitles: AxisTitles(
            sideTitles: SideTitles(
              showTitles: true,
              reservedSize: 22,
              getTitlesWidget: (value, _) {
                final idx = value.round();
                if (idx < 0 || idx >= sampled.length || idx % 3 != 0) {
                  return const SizedBox.shrink();
                }
                final raw = sampled[idx].label ?? sampled[idx].time;
                final label = parseDateOrNull(raw) != null
                    ? formatAxisTime(raw, spanMs)
                    : raw;
                return Text(
                  label,
                  style: TextStyle(
                    fontSize: 10,
                    color: scheme.onSurfaceVariant,
                  ),
                );
              },
            ),
          ),
        ),
        barGroups: <BarChartGroupData>[
          for (var i = 0; i < sampled.length; i++)
            BarChartGroupData(
              x: i,
              barRods: <BarChartRodData>[
                BarChartRodData(
                  toY: sampled[i].value,
                  color: barColor,
                  width: 10,
                  borderRadius: const BorderRadius.vertical(
                    top: Radius.circular(4),
                  ),
                ),
              ],
            ),
        ],
      ),
    );
  }
}
