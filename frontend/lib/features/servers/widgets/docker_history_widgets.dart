import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Seletor compacto reutilizável de janela histórica Docker (`1h`, `6h`, `24h`, `7d`, `30d`).
class DockerHistoryRangeSelector extends StatelessWidget {
  const DockerHistoryRangeSelector({
    super.key,
    required this.selectedRange,
    required this.onChanged,
    this.trailing,
  });

  final String selectedRange;
  final ValueChanged<String> onChanged;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    final segmented = SegmentedButton<String>(
      showSelectedIcon: false,
      style: SegmentedButton.styleFrom(
        visualDensity: VisualDensity.compact,
        padding: const EdgeInsets.symmetric(horizontal: 6),
      ),
      segments: const <ButtonSegment<String>>[
        ButtonSegment<String>(
          value: '1h',
          label: Text('1h', maxLines: 1, softWrap: false),
        ),
        ButtonSegment<String>(
          value: '6h',
          label: Text('6h', maxLines: 1, softWrap: false),
        ),
        ButtonSegment<String>(
          value: '24h',
          label: Text('24h', maxLines: 1, softWrap: false),
        ),
        ButtonSegment<String>(
          value: '7d',
          label: Text('7d', maxLines: 1, softWrap: false),
        ),
        ButtonSegment<String>(
          value: '30d',
          label: Text('30d', maxLines: 1, softWrap: false),
        ),
      ],
      selected: <String>{selectedRange},
      onSelectionChanged: (set) => onChanged(set.first),
    );

    if (trailing == null) return segmented;
    return Row(
      children: <Widget>[
        Expanded(child: segmented),
        const SizedBox(width: 8),
        trailing!,
      ],
    );
  }
}

/// Gráficos históricos reutilizáveis de um container Docker (CPU, RAM, Rede RX/TX e Block I/O),
/// compartilhados entre `DockerHistoryPage` (desktop + sheet mobile) e `DockerContainerDetailSheet`.
class ContainerHistoryCharts extends StatelessWidget {
  const ContainerHistoryCharts({
    super.key,
    required this.points,
    this.showNetworkAndDisk = true,
    this.chartHeight = 165,
    this.useGrid = true,
  });

  final List<ContainerHistoryPoint> points;
  final bool showNetworkAndDisk;
  final double chartHeight;
  final bool useGrid;

  @override
  Widget build(BuildContext context) {
    final cards = <Widget>[
      NestedPanel(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Text(
              'CPU (%)',
              style: context.textTheme.titleSmall?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            const SizedBox(height: 8),
            MetricLineChart(
              height: chartHeight,
              series: <MetricLineSeries>[
                MetricLineSeries(
                  name: 'Média',
                  tone: SemanticTone.primary,
                  points: <MetricSeriesPoint>[
                    for (var i = 0; i < points.length; i++)
                      MetricSeriesPoint(x: i.toDouble(), y: points[i].cpuAvg),
                  ],
                ),
                MetricLineSeries(
                  name: 'Pico',
                  tone: SemanticTone.warning,
                  showArea: false,
                  points: <MetricSeriesPoint>[
                    for (var i = 0; i < points.length; i++)
                      MetricSeriesPoint(x: i.toDouble(), y: points[i].cpuMax),
                  ],
                ),
              ],
              yFormatter: (v) => formatPercent(v, digits: v < 10 ? 1 : 0),
            ),
          ],
        ),
      ),
      NestedPanel(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Text(
              'Memória (RAM)',
              style: context.textTheme.titleSmall?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            const SizedBox(height: 8),
            MetricLineChart(
              height: chartHeight,
              series: <MetricLineSeries>[
                MetricLineSeries(
                  name: 'Média',
                  tone: SemanticTone.info,
                  points: <MetricSeriesPoint>[
                    for (var i = 0; i < points.length; i++)
                      MetricSeriesPoint(
                        x: i.toDouble(),
                        y: points[i].memoryAvg.toDouble(),
                      ),
                  ],
                ),
                MetricLineSeries(
                  name: 'Pico',
                  tone: SemanticTone.primary,
                  showArea: false,
                  points: <MetricSeriesPoint>[
                    for (var i = 0; i < points.length; i++)
                      MetricSeriesPoint(
                        x: i.toDouble(),
                        y: points[i].memoryMax.toDouble(),
                      ),
                  ],
                ),
              ],
              yFormatter: (v) => formatBinaryBytes(v, fractionDigits: 0),
            ),
          ],
        ),
      ),
      if (showNetworkAndDisk) ...<Widget>[
        NestedPanel(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text(
                'Rede (RX / TX)',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              const SizedBox(height: 8),
              MetricLineChart(
                height: chartHeight,
                series: <MetricLineSeries>[
                  MetricLineSeries(
                    name: 'Recebido (RX)',
                    tone: SemanticTone.success,
                    points: <MetricSeriesPoint>[
                      for (var i = 0; i < points.length; i++)
                        MetricSeriesPoint(
                          x: i.toDouble(),
                          y: points[i].netRxBytes.toDouble(),
                        ),
                    ],
                  ),
                  MetricLineSeries(
                    name: 'Enviado (TX)',
                    tone: SemanticTone.secondary,
                    showArea: false,
                    points: <MetricSeriesPoint>[
                      for (var i = 0; i < points.length; i++)
                        MetricSeriesPoint(
                          x: i.toDouble(),
                          y: points[i].netTxBytes.toDouble(),
                        ),
                    ],
                  ),
                ],
                yFormatter: (v) => formatDecimalBytes(v, fractionDigits: 0),
              ),
            ],
          ),
        ),
        NestedPanel(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Text(
                'Disco (Block I/O)',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              const SizedBox(height: 8),
              MetricLineChart(
                height: chartHeight,
                series: <MetricLineSeries>[
                  MetricLineSeries(
                    name: 'Leitura',
                    tone: SemanticTone.info,
                    points: <MetricSeriesPoint>[
                      for (var i = 0; i < points.length; i++)
                        MetricSeriesPoint(
                          x: i.toDouble(),
                          y: points[i].blockReadBytes.toDouble(),
                        ),
                    ],
                  ),
                  MetricLineSeries(
                    name: 'Escrita',
                    tone: SemanticTone.warning,
                    showArea: false,
                    points: <MetricSeriesPoint>[
                      for (var i = 0; i < points.length; i++)
                        MetricSeriesPoint(
                          x: i.toDouble(),
                          y: points[i].blockWriteBytes.toDouble(),
                        ),
                    ],
                  ),
                ],
                yFormatter: (v) => formatDecimalBytes(v, fractionDigits: 0),
              ),
            ],
          ),
        ),
      ],
    ];

    if (useGrid) {
      return ResponsiveGrid(xs: 1, sm: 1, md: 2, lg: 2, xl: 2, children: cards);
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        for (var i = 0; i < cards.length; i++) ...<Widget>[
          if (i > 0) const SizedBox(height: 12),
          cards[i],
        ],
      ],
    );
  }
}

/// Linha responsiva campo + botão de ação (empilha verticalmente abaixo de [breakpoint]).
class InlineFieldActionRow extends StatelessWidget {
  const InlineFieldActionRow({
    super.key,
    required this.field,
    required this.action,
    this.breakpoint = 420,
  });

  final Widget field;
  final Widget action;
  final double breakpoint;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        if (constraints.maxWidth < breakpoint) {
          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              field,
              const SizedBox(height: 8),
              Align(alignment: Alignment.centerRight, child: action),
            ],
          );
        }
        return Row(
          children: <Widget>[
            Expanded(child: field),
            const SizedBox(width: 8),
            action,
          ],
        );
      },
    );
  }
}
