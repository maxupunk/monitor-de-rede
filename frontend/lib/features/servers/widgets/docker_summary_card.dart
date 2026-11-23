import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Card de dashboard `docker_summary` (registrado em `serversArea.widgets`).
///
/// Mostra sempre o Docker desta central (`localView`) e declara interesse SSE
/// `docker` enquanto montado — ZERO polling!
class DockerSummaryCard extends ConsumerStatefulWidget {
  const DockerSummaryCard({super.key, this.config = const <String, dynamic>{}});

  final Map<String, dynamic> config;

  @override
  ConsumerState<DockerSummaryCard> createState() => _DockerSummaryCardState();
}

class _DockerSummaryCardState extends ConsumerState<DockerSummaryCard> {
  VoidCallback? _releaseInterest;

  @override
  void initState() {
    super.initState();
    _releaseInterest = ref
        .read(sseServiceProvider.notifier)
        .acquireInterest('docker');
    Future<void>.microtask(() {
      if (!mounted) return;
      ref.read(dockerProvider.notifier).fetchHosts();
    });
  }

  @override
  void dispose() {
    _releaseInterest?.call();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final docker = ref.watch(dockerProvider);
    final local = docker.localView;
    final editMode = widget.config['editMode'] == true;

    if (!local.available && !editMode) {
      return const SizedBox.shrink();
    }

    final history = local.aggregateHistory;
    final latest = history.isNotEmpty ? history.last : null;
    final cpuPoints = <MetricSeriesPoint>[
      for (var i = 0; i < history.length; i++)
        MetricSeriesPoint(x: i.toDouble(), y: history[i].cpuPercent),
    ];
    final memPoints = <MetricSeriesPoint>[
      for (var i = 0; i < history.length; i++)
        MetricSeriesPoint(
          x: i.toDouble(),
          y: history[i].memoryUsageBytes.toDouble(),
        ),
    ];

    return SectionCard(
      title: 'Docker desta central',
      subtitle: local.available
          ? 'Engine ${local.status?.engineVersion ?? ''}'
          : 'Docker Engine indisponível',
      icon: Icons.view_in_ar_outlined,
      iconTone: local.available ? SemanticTone.primary : SemanticTone.warning,
      trailing: TextButton.icon(
        onPressed: () => context.go('/docker'),
        icon: const Icon(Icons.open_in_new, size: 16),
        label: const Text('Abrir Docker'),
      ),
      child: !local.available
          ? const Text(
              'Ative o agente Docker local para visualizar métricas em tempo real.',
            )
          : Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: <Widget>[
                    SemanticChip(
                      label: '${local.runningContainers} rodando',
                      tone: SemanticTone.success,
                      icon: Icons.play_arrow,
                      compact: true,
                    ),
                    SemanticChip(
                      label: '${local.stoppedContainers} parados',
                      tone: local.stoppedContainers > 0
                          ? SemanticTone.warning
                          : SemanticTone.secondary,
                      icon: Icons.stop,
                      compact: true,
                    ),
                    if (latest != null) ...<Widget>[
                      SemanticChip(
                        label: 'CPU ${formatPercent(latest.cpuPercent)}',
                        tone: SemanticTone.primary,
                        compact: true,
                      ),
                      SemanticChip(
                        label:
                            'RAM ${formatBinaryBytes(latest.memoryUsageBytes)} / ${formatBinaryBytes(local.status?.memoryTotalBytes)}',
                        tone: SemanticTone.info,
                        compact: true,
                      ),
                    ],
                  ],
                ),
                const SizedBox(height: 12),
                ResponsiveGrid(
                  xs: 1,
                  sm: 2,
                  md: 2,
                  lg: 2,
                  xl: 2,
                  children: <Widget>[
                    NestedPanel(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            'CPU agregada (%)',
                            style: context.textTheme.labelMedium?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                          const SizedBox(height: 6),
                          MetricLineChart(
                            height: 110,
                            series: <MetricLineSeries>[
                              MetricLineSeries(
                                name: 'CPU',
                                points: cpuPoints,
                                tone: SemanticTone.primary,
                              ),
                            ],
                            yFormatter: (v) => formatPercent(v, digits: 0),
                            emptyText: 'Aguardando amostras SSE...',
                          ),
                        ],
                      ),
                    ),
                    NestedPanel(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            'RAM dos containers',
                            style: context.textTheme.labelMedium?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                          const SizedBox(height: 6),
                          MetricLineChart(
                            height: 110,
                            series: <MetricLineSeries>[
                              MetricLineSeries(
                                name: 'RAM',
                                points: memPoints,
                                tone: SemanticTone.info,
                              ),
                            ],
                            yFormatter: (v) =>
                                formatBinaryBytes(v, fractionDigits: 0),
                            emptyText: 'Aguardando amostras SSE...',
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
                if (latest != null) ...<Widget>[
                  const SizedBox(height: 10),
                  Wrap(
                    spacing: 16,
                    runSpacing: 4,
                    children: <Widget>[
                      Text(
                        'Rede RX: ${formatDecimalBytes(latest.networkReceivedBytes)}',
                        style: context.textTheme.bodySmall,
                      ),
                      Text(
                        'Rede TX: ${formatDecimalBytes(latest.networkTransmittedBytes)}',
                        style: context.textTheme.bodySmall,
                      ),
                    ],
                  ),
                ],
              ],
            ),
    );
  }
}
