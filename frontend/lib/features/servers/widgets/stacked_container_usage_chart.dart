import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/utils/stacked_usage.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Gráfico de área empilhada por container (CPU/RAM) sobre `fl_chart`,
/// com tooltip Material 3 sensível ao tema, eixo Y com marcas humanas limpas
/// e legenda interativa clicável para isolar/destacar um container.
class StackedContainerUsageChart extends StatefulWidget {
  const StackedContainerUsageChart({
    super.key,
    required this.samples,
    required this.labels,
    required this.valueFormatter,
    required this.totalLabel,
    this.axisFormatter,
    this.scaleBase = 10,
    this.minCeiling = 1,
    this.height = 200,
    this.maxSeries = 6,
    this.emptyText = 'Aguardando métricas ao vivo...',
  });

  final List<UsageSample> samples;
  final Map<String, String> labels;
  final String Function(double value) valueFormatter;
  final String Function(double value, double interval)? axisFormatter;
  final String totalLabel;
  final int scaleBase;
  final double minCeiling;
  final double height;
  final int maxSeries;
  final String emptyText;

  @override
  State<StackedContainerUsageChart> createState() =>
      _StackedContainerUsageChartState();
}

class _StackedContainerUsageChartState
    extends State<StackedContainerUsageChart> {
  String? _isolatedLayerId;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final stack = buildStackedUsage(
      widget.samples,
      widget.labels,
      maxSeries: widget.maxSeries,
    );

    if (stack.isEmpty || stack.layers.isEmpty) {
      return SizedBox(
        height: widget.height,
        child: Center(
          child: Text(
            widget.emptyText,
            style: context.textTheme.bodyMedium?.copyWith(
              color: scheme.onSurfaceVariant,
            ),
          ),
        ),
      );
    }

    // Limpa o isolamento se o container selecionado saiu da janela.
    final isolatedId = stack.layers.any((l) => l.id == _isolatedLayerId)
        ? _isolatedLayerId
        : null;

    final scale = niceAxisScale(
      stack.peak,
      base: widget.scaleBase,
      minCeiling: widget.minCeiling,
    );

    final sampleCount = stack.times.length;
    final singlePoint = sampleCount == 1;
    // fl_chart precisa de minX < maxX mesmo quando há apenas 1 amostra SSE.
    final maxX = singlePoint ? 1.0 : (sampleCount - 1).toDouble();

    final lineBars = <LineChartBarData>[
      // Índice 0: linha de base y = 0 (invisível) para a primeira faixa empilhada.
      LineChartBarData(
        spots: singlePoint
            ? const <FlSpot>[FlSpot(0, 0), FlSpot(1, 0)]
            : <FlSpot>[
                for (var i = 0; i < sampleCount; i++) FlSpot(i.toDouble(), 0),
              ],
        barWidth: 0,
        color: Colors.transparent,
        dotData: const FlDotData(show: false),
      ),
      // Índices 1..N: topo acumulado de cada camada.
      for (var layerIdx = 0; layerIdx < stack.layers.length; layerIdx++)
        _buildLayerBar(
          layer: stack.layers[layerIdx],
          isTop: layerIdx == stack.layers.length - 1,
          isIsolated: isolatedId == stack.layers[layerIdx].id,
          isDimmed:
              isolatedId != null && isolatedId != stack.layers[layerIdx].id,
          singlePoint: singlePoint,
          totalStrokeColor: scheme.onSurface.withValues(alpha: 0.72),
        ),
    ];

    final betweenBars = <BetweenBarsData>[
      for (var layerIdx = 0; layerIdx < stack.layers.length; layerIdx++)
        BetweenBarsData(
          fromIndex: layerIdx,
          toIndex: layerIdx + 1,
          color: stack.layers[layerIdx].color.withValues(
            alpha: isolatedId == null
                ? 0.62
                : (isolatedId == stack.layers[layerIdx].id ? 0.88 : 0.12),
          ),
        ),
    ];

    // Ordena a legenda por consumo atual (maior primeiro), mantendo "Outros" por último.
    final legendLayers = List<StackedLayer>.from(stack.layers)
      ..sort((a, b) {
        if (a.id == kOthersLayerId) return 1;
        if (b.id == kOthersLayerId) return -1;
        return b.latestValue.compareTo(a.latestValue);
      });

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        SizedBox(
          height: widget.height,
          child: LineChart(
            LineChartData(
              minX: 0,
              maxX: maxX,
              minY: 0,
              maxY: scale.maxY,
              clipData: const FlClipData.all(),
              gridData: FlGridData(
                show: true,
                drawVerticalLine: false,
                horizontalInterval: scale.interval,
                getDrawingHorizontalLine: (_) => FlLine(
                  color: scheme.outlineVariant.withValues(alpha: 0.45),
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
                    reservedSize: 58,
                    interval: scale.interval,
                    getTitlesWidget: (value, meta) {
                      if (value < 0) return const SizedBox.shrink();
                      // Evita marcas fora da grade exata por erro de ponto flutuante.
                      final ratio = value / scale.interval;
                      if ((ratio - ratio.round()).abs() > 0.05) {
                        return const SizedBox.shrink();
                      }
                      final label = widget.axisFormatter != null
                          ? widget.axisFormatter!(value, scale.interval)
                          : widget.valueFormatter(value);
                      return Padding(
                        padding: const EdgeInsets.only(right: 6),
                        child: Text(
                          label,
                          textAlign: TextAlign.right,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
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
                    showTitles: true,
                    reservedSize: 22,
                    interval: singlePoint ? 1 : (maxX / 2).clamp(1, maxX),
                    getTitlesWidget: (value, meta) {
                      final idx = value.round();
                      if (singlePoint) {
                        if (idx != 0) return const SizedBox.shrink();
                        return _buildTimeTick(
                          formatClockTime(stack.times.first),
                          scheme.onSurfaceVariant,
                        );
                      }
                      final isStart = idx == 0;
                      final isEnd = idx == sampleCount - 1;
                      final midIdx = (sampleCount - 1) ~/ 2;
                      final isMid = sampleCount >= 5 && idx == midIdx;
                      if (!isStart && !isEnd && !isMid) {
                        return const SizedBox.shrink();
                      }
                      return _buildTimeTick(
                        formatClockTime(stack.times[idx]),
                        scheme.onSurfaceVariant,
                      );
                    },
                  ),
                ),
              ),
              betweenBarsData: betweenBars,
              lineBarsData: lineBars,
              lineTouchData: LineTouchData(
                handleBuiltInTouches: true,
                getTouchedSpotIndicator: (barData, spotIndexes) {
                  return spotIndexes
                      .map((_) {
                        return TouchedSpotIndicatorData(
                          FlLine(
                            color: scheme.outline.withValues(alpha: 0.7),
                            strokeWidth: 1,
                            dashArray: const <int>[4, 3],
                          ),
                          const FlDotData(show: false),
                        );
                      })
                      .toList(growable: false);
                },
                touchTooltipData: LineTouchTooltipData(
                  maxContentWidth: 240,
                  tooltipRoundedRadius: 10,
                  tooltipPadding: const EdgeInsets.symmetric(
                    horizontal: 10,
                    vertical: 8,
                  ),
                  tooltipBorder: BorderSide(color: scheme.outlineVariant),
                  fitInsideHorizontally: true,
                  fitInsideVertically: true,
                  getTooltipColor: (_) => scheme.surfaceContainerHighest,
                  getTooltipItems: (touchedSpots) =>
                      _buildTooltipItems(touchedSpots, stack, scheme),
                ),
              ),
            ),
            duration: const Duration(milliseconds: 180),
          ),
        ),
        const SizedBox(height: 10),
        Wrap(
          spacing: 6,
          runSpacing: 6,
          children: <Widget>[
            for (final layer in legendLayers)
              _LegendChip(
                color: layer.color,
                label: layer.label,
                value: widget.valueFormatter(layer.latestValue),
                selected: isolatedId == layer.id,
                dimmed: isolatedId != null && isolatedId != layer.id,
                onTap: () {
                  setState(() {
                    _isolatedLayerId = _isolatedLayerId == layer.id
                        ? null
                        : layer.id;
                  });
                },
              ),
          ],
        ),
      ],
    );
  }

  LineChartBarData _buildLayerBar({
    required StackedLayer layer,
    required bool isTop,
    required bool isIsolated,
    required bool isDimmed,
    required bool singlePoint,
    required Color totalStrokeColor,
  }) {
    final spots = singlePoint
        ? <FlSpot>[FlSpot(0, layer.upper.first), FlSpot(1, layer.upper.first)]
        : <FlSpot>[
            for (var i = 0; i < layer.upper.length; i++)
              FlSpot(i.toDouble(), layer.upper[i]),
          ];
    return LineChartBarData(
      spots: spots,
      isCurved: false,
      barWidth: isIsolated ? 2.0 : (isTop ? 1.6 : 0.7),
      color: isDimmed
          ? layer.color.withValues(alpha: 0.25)
          : (isTop && !isIsolated ? totalStrokeColor : layer.color),
      dotData: const FlDotData(show: false),
    );
  }

  Widget _buildTimeTick(String label, Color color) {
    return Padding(
      padding: const EdgeInsets.only(top: 5),
      child: Text(
        label,
        maxLines: 1,
        style: TextStyle(fontSize: 10, color: color),
      ),
    );
  }

  List<LineTooltipItem?> _buildTooltipItems(
    List<LineBarSpot> touchedSpots,
    StackedUsage stack,
    ColorScheme scheme,
  ) {
    if (touchedSpots.isEmpty) return const <LineTooltipItem?>[];
    final rawIndex = touchedSpots.first.x.round();
    final idx = rawIndex.clamp(0, stack.times.length - 1);
    final clock = formatClockTime(stack.times[idx]);
    final total = stack.totals[idx];

    // Ordena os containers presentes nesse instante do maior para o menor.
    final activeLayers =
        stack.layers.where((l) => l.values[idx] > 0).toList(growable: false)
          ..sort((a, b) => b.values[idx].compareTo(a.values[idx]));
    const maxRows = 5;
    final visible = activeLayers.take(maxRows).toList(growable: false);
    final hiddenCount = activeLayers.length - visible.length;

    final spans = <TextSpan>[
      TextSpan(
        text: '$clock · ${widget.totalLabel}: ${widget.valueFormatter(total)}',
        style: TextStyle(
          color: scheme.onSurface,
          fontWeight: FontWeight.w800,
          fontSize: 11.5,
        ),
      ),
      for (final layer in visible) ...<TextSpan>[
        const TextSpan(text: '\n'),
        TextSpan(
          text: '● ',
          style: TextStyle(
            color: layer.color,
            fontSize: 11,
            fontWeight: FontWeight.w900,
          ),
        ),
        TextSpan(
          text: '${_truncateLabel(layer.label)}: ',
          style: TextStyle(color: scheme.onSurfaceVariant, fontSize: 11),
        ),
        TextSpan(
          text: widget.valueFormatter(layer.values[idx]),
          style: TextStyle(
            color: scheme.onSurface,
            fontWeight: FontWeight.w700,
            fontSize: 11,
          ),
        ),
      ],
      if (hiddenCount > 0)
        TextSpan(
          text: '\n+$hiddenCount outros',
          style: TextStyle(
            color: scheme.onSurfaceVariant,
            fontSize: 10.5,
            fontStyle: FontStyle.italic,
          ),
        ),
    ];

    // Retorna um único bloco consolidado no primeiro spot para evitar bolhas duplicadas.
    return List<LineTooltipItem?>.generate(touchedSpots.length, (i) {
      if (i != 0) return null;
      return LineTooltipItem(
        '',
        TextStyle(color: scheme.onSurface, fontSize: 11),
        textAlign: TextAlign.left,
        children: spans,
      );
    }, growable: false);
  }

  String _truncateLabel(String text) {
    if (text.length <= 22) return text;
    return '${text.substring(0, 21)}…';
  }
}

class _LegendChip extends StatelessWidget {
  const _LegendChip({
    required this.color,
    required this.label,
    required this.value,
    required this.selected,
    required this.dimmed,
    required this.onTap,
  });

  final Color color;
  final String label;
  final String value;
  final bool selected;
  final bool dimmed;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    return Material(
      color: selected
          ? color.withValues(alpha: 0.16)
          : scheme.surfaceContainerLow,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(8),
        side: BorderSide(
          color: selected
              ? color
              : scheme.outlineVariant.withValues(alpha: 0.7),
          width: selected ? 1.4 : 1,
        ),
      ),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onTap,
        child: Opacity(
          opacity: dimmed ? 0.45 : 1.0,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Container(
                  width: 8,
                  height: 8,
                  decoration: BoxDecoration(
                    color: color,
                    shape: BoxShape.circle,
                  ),
                ),
                const SizedBox(width: 6),
                ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 132),
                  child: Text(
                    label,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: context.textTheme.labelSmall?.copyWith(
                      color: scheme.onSurface,
                      fontWeight: selected ? FontWeight.w700 : FontWeight.w500,
                    ),
                  ),
                ),
                const SizedBox(width: 6),
                Text(
                  value,
                  style: context.textTheme.labelSmall?.copyWith(
                    color: scheme.onSurface,
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
