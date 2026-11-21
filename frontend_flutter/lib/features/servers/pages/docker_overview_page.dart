import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Aba "Visão geral" do Docker (`/docker`).
class DockerOverviewPage extends ConsumerWidget {
  const DockerOverviewPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final docker = ref.watch(dockerProvider);
    final current = docker.current;
    final status = current.status;
    final suffix = docker.hostQuerySuffix;

    if (docker.loading && status == null) {
      return const LoadingStateView(
        message: 'Aguardando snapshot da Docker Engine...',
      );
    }

    final history = current.aggregateHistory;
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

    final metricsList =
        List<DockerContainerMetrics>.from(
          current.metrics?.containers ?? const <DockerContainerMetrics>[],
        )..sort(
          (a, b) => (b.cpu.usagePercent + b.memory.usagePercent).compareTo(
            a.cpu.usagePercent + a.memory.usagePercent,
          ),
        );
    final top8 = metricsList.take(8).toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        // 4 KPI Cards clicáveis
        ResponsiveGrid(
          xs: 2,
          sm: 2,
          md: 4,
          lg: 4,
          xl: 4,
          children: <Widget>[
            KpiCard(
              label: 'Containers ativos',
              value: formatInteger(current.runningContainers),
              icon: Icons.play_circle_outline,
              tone: SemanticTone.success,
              onTap: () => context.go(
                '/docker/containers${suffix.isEmpty ? '?state=running' : '$suffix&state=running'}',
              ),
            ),
            KpiCard(
              label: 'Containers parados',
              value: formatInteger(current.stoppedContainers),
              icon: Icons.stop_circle_outlined,
              tone: current.stoppedContainers > 0
                  ? SemanticTone.warning
                  : SemanticTone.secondary,
              onTap: () => context.go(
                '/docker/containers${suffix.isEmpty ? '?state=stopped' : '$suffix&state=stopped'}',
              ),
            ),
            KpiCard(
              label: 'Volumes',
              value: formatInteger(current.volumes.length),
              icon: Icons.storage_outlined,
              tone: SemanticTone.info,
              onTap: () => context.go('/docker/volumes$suffix'),
            ),
            KpiCard(
              label: 'Imagens',
              value: formatInteger(current.images.length),
              icon: Icons.layers_outlined,
              tone: SemanticTone.primary,
              onTap: () => context.go('/docker/images$suffix'),
            ),
          ],
        ),
        const SizedBox(height: 16),

        if ((current.metrics?.failedContainerCount ?? 0) > 0) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.warning,
            child: Text(
              'Métricas parciais: ${current.metrics!.failedContainerCount} container(s) não responderam na última coleta.',
            ),
          ),
          const SizedBox(height: 12),
        ],

        // Gráficos ao vivo do buffer circular SSE (60 amostras)
        ResponsiveGrid(
          xs: 1,
          sm: 1,
          md: 2,
          lg: 2,
          xl: 2,
          children: <Widget>[
            SectionCard(
              title: 'CPU total dos containers',
              subtitle: history.isNotEmpty
                  ? 'Atual: ${formatPercent(history.last.cpuPercent)} (últimas ${history.length} amostras SSE)'
                  : 'Recebendo amostras em tempo real via SSE',
              icon: Icons.memory,
              iconTone: SemanticTone.primary,
              child: MetricLineChart(
                height: 200,
                series: <MetricLineSeries>[
                  MetricLineSeries(
                    name: 'CPU total',
                    points: cpuPoints,
                    tone: SemanticTone.primary,
                  ),
                ],
                yFormatter: (v) => formatPercent(v, digits: 0),
                emptyText: 'Aguardando métricas ao vivo...',
              ),
            ),
            SectionCard(
              title: 'RAM usada pelos containers',
              subtitle: history.isNotEmpty
                  ? '${formatBinaryBytes(history.last.memoryUsageBytes)} (${formatPercent(history.last.memoryPercent)} da Engine)'
                  : 'Recebendo amostras em tempo real via SSE',
              icon: Icons.sd_storage_outlined,
              iconTone: SemanticTone.info,
              child: MetricLineChart(
                height: 200,
                series: <MetricLineSeries>[
                  MetricLineSeries(
                    name: 'RAM usada',
                    points: memPoints,
                    tone: SemanticTone.info,
                  ),
                ],
                yFormatter: (v) => formatBinaryBytes(v, fractionDigits: 0),
                emptyText: 'Aguardando métricas ao vivo...',
              ),
            ),
          ],
        ),
        const SizedBox(height: 16),

        // Informações da Docker Engine + Top 8 Containers
        ResponsiveGrid(
          xs: 1,
          sm: 1,
          md: 1,
          lg: 2,
          xl: 2,
          children: <Widget>[
            SectionCard(
              title: 'Docker Engine',
              icon: Icons.info_outline,
              iconTone: SemanticTone.secondary,
              child: status == null
                  ? const Text('Aguardando status do host...')
                  : Column(
                      children: <Widget>[
                        KeyValueRow(label: 'Host', value: status.name ?? '—'),
                        KeyValueRow(
                          label: 'Engine / API',
                          value:
                              '${status.engineVersion ?? '—'} / ${status.apiVersion ?? '—'}',
                        ),
                        KeyValueRow(
                          label: 'Sistema / Arquitetura',
                          value:
                              '${status.operatingSystem ?? '—'} · ${status.architecture ?? '—'}',
                        ),
                        KeyValueRow(
                          label: 'CPUs / Memória total',
                          value:
                              '${status.cpus} vCPU · ${formatBinaryBytes(status.memoryTotalBytes)}',
                        ),
                        KeyValueRow(
                          label: 'Redes / Imagens',
                          value:
                              '${current.networks.length} redes · ${current.images.length} imagens',
                        ),
                      ],
                    ),
            ),
            SectionCard(
              title: 'Top containers em execução',
              subtitle: 'Ordenados por uso combinado de CPU e RAM',
              icon: Icons.leaderboard_outlined,
              iconTone: SemanticTone.success,
              child: top8.isEmpty
                  ? const EmptyStateView(
                      title: 'Nenhum container com métricas ativas',
                      icon: Icons.view_in_ar_outlined,
                    )
                  : Column(
                      children: top8
                          .map((m) {
                            return Padding(
                              padding: const EdgeInsets.only(bottom: 8),
                              child: NestedPanel(
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 12,
                                  vertical: 10,
                                ),
                                child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: <Widget>[
                                    Row(
                                      children: <Widget>[
                                        Expanded(
                                          child: Text(
                                            m.containerName,
                                            style: context.textTheme.titleSmall
                                                ?.copyWith(
                                                  fontWeight: FontWeight.w700,
                                                ),
                                            overflow: TextOverflow.ellipsis,
                                          ),
                                        ),
                                        SemanticChip(
                                          label:
                                              m.projectName?.isNotEmpty == true
                                              ? m.projectName!
                                              : 'Avulso',
                                          tone: SemanticTone.secondary,
                                          compact: true,
                                        ),
                                      ],
                                    ),
                                    const SizedBox(height: 6),
                                    Wrap(
                                      spacing: 12,
                                      runSpacing: 4,
                                      children: <Widget>[
                                        Text(
                                          'CPU: ${formatPercent(m.cpu.usagePercent)}',
                                          style: context.textTheme.bodySmall
                                              ?.copyWith(
                                                fontWeight: FontWeight.w700,
                                              ),
                                        ),
                                        Text(
                                          'RAM: ${formatBinaryBytes(m.memory.usageBytes)}',
                                          style: context.textTheme.bodySmall,
                                        ),
                                        Text(
                                          'RX/TX: ${formatDecimalBytes(m.network.receivedBytes)} / ${formatDecimalBytes(m.network.transmittedBytes)}',
                                          style: context.textTheme.bodySmall,
                                        ),
                                      ],
                                    ),
                                  ],
                                ),
                              ),
                            );
                          })
                          .toList(growable: false),
                    ),
            ),
          ],
        ),
      ],
    );
  }
}
