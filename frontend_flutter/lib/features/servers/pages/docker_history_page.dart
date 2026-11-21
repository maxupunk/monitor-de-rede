import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Aba "Histórico" do Docker (`/docker/history`).
/// Consulta sob demanda (não-live), disponível mesmo quando o agente está offline.
class DockerHistoryPage extends ConsumerStatefulWidget {
  const DockerHistoryPage({super.key});

  @override
  ConsumerState<DockerHistoryPage> createState() => _DockerHistoryPageState();
}

class _DockerHistoryPageState extends ConsumerState<DockerHistoryPage> {
  String _range = '24h';
  HostHistoryResponse? _history;
  bool _loading = false;
  String? _error;
  String _loadedHostKey = '';

  String? _selectedContainerName;
  ContainerHistoryResponse? _containerHistory;
  bool _loadingContainer = false;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_loadHistory);
  }

  Future<void> _loadHistory() async {
    final hostKey = ref.read(dockerProvider).selectedHostKey;
    if (hostKey.isEmpty) return;
    setState(() {
      _loading = true;
      _error = null;
      _loadedHostKey = hostKey;
    });
    try {
      final resp = await ref
          .read(dockerProvider.notifier)
          .apiFor(hostKey)
          .hostHistory(_range);
      if (!mounted) return;
      setState(() {
        _history = resp;
        _loading = false;
      });
      if (_selectedContainerName != null) {
        await _loadContainerHistory(_selectedContainerName!);
      }
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = err.toString();
        _loading = false;
      });
    }
  }

  Future<void> _loadContainerHistory(String name) async {
    final hostKey = ref.read(dockerProvider).selectedHostKey;
    if (hostKey.isEmpty) return;
    setState(() {
      _selectedContainerName = name;
      _loadingContainer = true;
    });
    try {
      final resp = await ref
          .read(dockerProvider.notifier)
          .apiFor(hostKey)
          .containerHistory(name, _range);
      if (!mounted) return;
      setState(() {
        _containerHistory = resp;
        _loadingContainer = false;
      });
    } catch (_) {
      if (!mounted) return;
      setState(() => _loadingContainer = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final hostKey = ref.watch(dockerProvider).selectedHostKey;
    if (hostKey.isNotEmpty && hostKey != _loadedHostKey && !_loading) {
      WidgetsBinding.instance.addPostFrameCallback((_) => _loadHistory());
    }

    final hostPoints = _history?.host ?? const <HostHistoryPoint>[];
    final latestDisks = hostPoints.isNotEmpty
        ? hostPoints.last.disks
        : const <HostDiskPoint>[];
    final latestMemTotal = hostPoints.isNotEmpty
        ? hostPoints.last.memoryTotal
        : 0;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Wrap(
          spacing: 12,
          runSpacing: 8,
          alignment: WrapAlignment.spaceBetween,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            SegmentedButton<String>(
              segments: const <ButtonSegment<String>>[
                ButtonSegment(value: '1h', label: Text('1h')),
                ButtonSegment(value: '6h', label: Text('6h')),
                ButtonSegment(value: '24h', label: Text('24h')),
                ButtonSegment(value: '7d', label: Text('7d')),
                ButtonSegment(value: '30d', label: Text('30d')),
              ],
              selected: <String>{_range},
              onSelectionChanged: (set) {
                setState(() => _range = set.first);
                _loadHistory();
              },
            ),
            OutlinedButton.icon(
              onPressed: _loading ? null : _loadHistory,
              icon: const Icon(Icons.refresh, size: 18),
              label: const Text('Recarregar'),
            ),
          ],
        ),
        const SizedBox(height: 16),

        if (_error != null)
          ErrorStateView(message: _error!, onRetry: _loadHistory)
        else if (_loading && _history == null)
          const LoadingStateView(message: 'Carregando histórico do host...')
        else ...<Widget>[
          ResponsiveGrid(
            xs: 1,
            sm: 1,
            md: 2,
            lg: 2,
            xl: 2,
            children: <Widget>[
              SectionCard(
                title: 'CPU do host',
                icon: Icons.memory,
                iconTone: SemanticTone.primary,
                child: MetricLineChart(
                  height: 190,
                  series: <MetricLineSeries>[
                    MetricLineSeries(
                      name: 'Média',
                      tone: SemanticTone.primary,
                      points: <MetricSeriesPoint>[
                        for (var i = 0; i < hostPoints.length; i++)
                          MetricSeriesPoint(
                            x: i.toDouble(),
                            y: hostPoints[i].cpuAvg,
                          ),
                      ],
                    ),
                    MetricLineSeries(
                      name: 'Pico',
                      tone: SemanticTone.warning,
                      showArea: false,
                      points: <MetricSeriesPoint>[
                        for (var i = 0; i < hostPoints.length; i++)
                          MetricSeriesPoint(
                            x: i.toDouble(),
                            y: hostPoints[i].cpuMax,
                          ),
                      ],
                    ),
                  ],
                  yFormatter: (v) => formatPercent(v, digits: 0),
                ),
              ),
              SectionCard(
                title: 'Memória usada',
                subtitle: latestMemTotal > 0
                    ? 'de ${formatBinaryBytes(latestMemTotal)}'
                    : null,
                icon: Icons.sd_storage_outlined,
                iconTone: SemanticTone.info,
                child: MetricLineChart(
                  height: 190,
                  series: <MetricLineSeries>[
                    MetricLineSeries(
                      name: 'Usada',
                      tone: SemanticTone.info,
                      points: <MetricSeriesPoint>[
                        for (var i = 0; i < hostPoints.length; i++)
                          MetricSeriesPoint(
                            x: i.toDouble(),
                            y: hostPoints[i].memoryUsed.toDouble(),
                          ),
                      ],
                    ),
                  ],
                  yFormatter: (v) => formatBinaryBytes(v, fractionDigits: 0),
                ),
              ),
              SectionCard(
                title: 'Tráfego de rede',
                icon: Icons.swap_vert,
                iconTone: SemanticTone.success,
                child: MetricLineChart(
                  height: 190,
                  series: <MetricLineSeries>[
                    MetricLineSeries(
                      name: 'Recebido',
                      tone: SemanticTone.success,
                      points: <MetricSeriesPoint>[
                        for (var i = 0; i < hostPoints.length; i++)
                          MetricSeriesPoint(
                            x: i.toDouble(),
                            y: hostPoints[i].netRxBps * 8,
                          ),
                      ],
                    ),
                    MetricLineSeries(
                      name: 'Enviado',
                      tone: SemanticTone.secondary,
                      showArea: false,
                      points: <MetricSeriesPoint>[
                        for (var i = 0; i < hostPoints.length; i++)
                          MetricSeriesPoint(
                            x: i.toDouble(),
                            y: hostPoints[i].netTxBps * 8,
                          ),
                      ],
                    ),
                  ],
                  yFormatter: (v) => formatBps(v, fractionDigits: 0),
                ),
              ),
              SectionCard(
                title: 'Discos',
                icon: Icons.disc_full_outlined,
                iconTone: SemanticTone.secondary,
                child: latestDisks.isEmpty
                    ? const EmptyStateView(
                        title: 'Sem dados de disco no período',
                      )
                    : Column(
                        children: latestDisks
                            .map((disk) {
                              final pct = disk.usedPercent.clamp(0.0, 100.0);
                              final barColor = pct > 90
                                  ? context.semanticColors.error
                                  : (pct > 75
                                        ? context.semanticColors.warning
                                        : context.colorScheme.primary);
                              return Padding(
                                padding: const EdgeInsets.only(bottom: 12),
                                child: Column(
                                  crossAxisAlignment:
                                      CrossAxisAlignment.stretch,
                                  children: <Widget>[
                                    Row(
                                      mainAxisAlignment:
                                          MainAxisAlignment.spaceBetween,
                                      children: <Widget>[
                                        Text(
                                          disk.mount,
                                          style: const TextStyle(
                                            fontWeight: FontWeight.w700,
                                            fontFamily: 'monospace',
                                          ),
                                        ),
                                        Text(
                                          '${formatBinaryBytes(disk.usedBytes)} / ${formatBinaryBytes(disk.totalBytes)} (${formatPercent(pct)})',
                                          style: context.textTheme.bodySmall,
                                        ),
                                      ],
                                    ),
                                    const SizedBox(height: 6),
                                    ClipRRect(
                                      borderRadius: BorderRadius.circular(6),
                                      child: LinearProgressIndicator(
                                        value: pct / 100,
                                        minHeight: 8,
                                        color: barColor,
                                      ),
                                    ),
                                  ],
                                ),
                              );
                            })
                            .toList(growable: false),
                      ),
              ),
            ],
          ),
          const SizedBox(height: 16),

          SectionCard(
            title: 'Consumo por container no período',
            subtitle:
                'Toque em um container para ver sua série temporal detalhada.',
            icon: Icons.view_in_ar_outlined,
            child: (_history?.containers.isEmpty ?? true)
                ? const EmptyStateView(
                    title: 'Sem histórico de containers no período',
                  )
                : Column(
                    children: _history!.containers
                        .map((c) {
                          final selected = c.name == _selectedContainerName;
                          return Padding(
                            padding: const EdgeInsets.only(bottom: 8),
                            child: ChoiceCard(
                              title: c.name,
                              subtitle:
                                  '${c.project?.isNotEmpty == true ? '${c.project} · ' : ''}'
                                  'CPU méd ${formatPercent(c.cpuAvg)} (pico ${formatPercent(c.cpuMax)}) · '
                                  'RAM pico ${formatBinaryBytes(c.memoryMax)} · '
                                  'Net ↓${formatDecimalBytes(c.netRxBytes)} ↑${formatDecimalBytes(c.netTxBytes)}',
                              selected: selected,
                              onTap: () => _loadContainerHistory(c.name),
                            ),
                          );
                        })
                        .toList(growable: false),
                  ),
          ),

          if (_selectedContainerName != null) ...<Widget>[
            const SizedBox(height: 16),
            SectionCard(
              title: 'Série histórica · $_selectedContainerName',
              subtitle: 'Período $_range',
              icon: Icons.insights,
              iconTone: SemanticTone.info,
              trailing: IconButton(
                tooltip: 'Fechar detalhe',
                icon: const Icon(Icons.close),
                onPressed: () => setState(() {
                  _selectedContainerName = null;
                  _containerHistory = null;
                }),
              ),
              child: _loadingContainer
                  ? const LoadingStateView(
                      message: 'Carregando série do container...',
                    )
                  : ResponsiveGrid(
                      xs: 1,
                      sm: 1,
                      md: 2,
                      lg: 2,
                      xl: 2,
                      children: <Widget>[
                        MetricLineChart(
                          height: 180,
                          series: <MetricLineSeries>[
                            MetricLineSeries(
                              name: 'CPU média',
                              tone: SemanticTone.primary,
                              points: <MetricSeriesPoint>[
                                for (
                                  var i = 0;
                                  i < (_containerHistory?.points.length ?? 0);
                                  i++
                                )
                                  MetricSeriesPoint(
                                    x: i.toDouble(),
                                    y: _containerHistory!.points[i].cpuAvg,
                                  ),
                              ],
                            ),
                            MetricLineSeries(
                              name: 'CPU pico',
                              tone: SemanticTone.warning,
                              showArea: false,
                              points: <MetricSeriesPoint>[
                                for (
                                  var i = 0;
                                  i < (_containerHistory?.points.length ?? 0);
                                  i++
                                )
                                  MetricSeriesPoint(
                                    x: i.toDouble(),
                                    y: _containerHistory!.points[i].cpuMax,
                                  ),
                              ],
                            ),
                          ],
                          yFormatter: (v) => formatPercent(v, digits: 0),
                        ),
                        MetricLineChart(
                          height: 180,
                          series: <MetricLineSeries>[
                            MetricLineSeries(
                              name: 'RAM pico',
                              tone: SemanticTone.info,
                              points: <MetricSeriesPoint>[
                                for (
                                  var i = 0;
                                  i < (_containerHistory?.points.length ?? 0);
                                  i++
                                )
                                  MetricSeriesPoint(
                                    x: i.toDouble(),
                                    y: _containerHistory!.points[i].memoryMax
                                        .toDouble(),
                                  ),
                              ],
                            ),
                          ],
                          yFormatter: (v) =>
                              formatBinaryBytes(v, fractionDigits: 0),
                        ),
                      ],
                    ),
            ),
          ],
        ],
      ],
    );
  }
}
