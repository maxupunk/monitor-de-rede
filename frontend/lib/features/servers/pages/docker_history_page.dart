import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Aba "Histórico" do Docker (`/docker/history`).
/// Consulta sob demanda (não-live), disponível mesmo quando o agente está offline.
///
/// - Mobile (`< 840px`): alternador compacto `Host` | `Containers (N)`; tocar em
///   um container abre imediatamente um Bottom Sheet Material 3 com seletor rápido
///   no topo (`AppDropdownFormField`), KPIs e séries de CPU, RAM, Rede e Block I/O.
/// - Desktop (`>= 840px`): visão do host no topo + Master-Detail lado a lado
///   para os containers sem precisar rolar até o fim da página.
class DockerHistoryPage extends ConsumerStatefulWidget {
  const DockerHistoryPage({super.key});

  @override
  ConsumerState<DockerHistoryPage> createState() => _DockerHistoryPageState();
}

class _DockerHistoryPageState extends ConsumerState<DockerHistoryPage> {
  String _range = '24h';
  String _mobileSection = 'host'; // 'host' | 'containers'
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
      final containers = resp.containers;
      String? nextSelected = _selectedContainerName;
      if (nextSelected != null &&
          !containers.any((c) => c.name == nextSelected)) {
        nextSelected = null;
      }
      if (nextSelected == null && containers.isNotEmpty) {
        nextSelected = containers.first.name;
      }
      setState(() {
        _history = resp;
        _selectedContainerName = nextSelected;
        _loading = false;
      });
      if (nextSelected != null) {
        await _loadContainerHistory(nextSelected);
      }
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = err.toString();
        _loading = false;
      });
    }
  }

  Future<ContainerHistoryResponse?> _fetchContainerSeries(
    String name, {
    String? rangeOverride,
  }) async {
    final hostKey = ref.read(dockerProvider).selectedHostKey;
    if (hostKey.isEmpty) return null;
    return ref
        .read(dockerProvider.notifier)
        .apiFor(hostKey)
        .containerHistory(name, rangeOverride ?? _range);
  }

  Future<void> _loadContainerHistory(String name) async {
    final hostKey = ref.read(dockerProvider).selectedHostKey;
    if (hostKey.isEmpty) return;
    setState(() {
      _selectedContainerName = name;
      _loadingContainer = true;
    });
    try {
      final resp = await _fetchContainerSeries(name);
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

  Future<void> _openContainerSheet(
    ContainerUsageSummary initialContainer,
    List<ContainerUsageSummary> allContainers,
  ) async {
    setState(() => _selectedContainerName = initialContainer.name);
    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Histórico do container',
      subtitle: 'Período $_range',
      maxWidth: 680,
      builder: (_) => _ContainerHistorySheetBody(
        initialContainerName: initialContainer.name,
        containers: allContainers,
        range: _range,
        loader: (name) => _fetchContainerSeries(name),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final hostKey = ref.watch(dockerProvider).selectedHostKey;
    if (hostKey.isNotEmpty && hostKey != _loadedHostKey && !_loading) {
      WidgetsBinding.instance.addPostFrameCallback((_) => _loadHistory());
    }

    final isDesktop = MediaQuery.sizeOf(context).width >= 840;
    final hostPoints = _history?.host ?? const <HostHistoryPoint>[];
    final containers = _history?.containers ?? const <ContainerUsageSummary>[];
    final latestDisks = hostPoints.isNotEmpty
        ? hostPoints.last.disks
        : const <HostDiskPoint>[];
    final latestMemTotal = hostPoints.isNotEmpty
        ? hostPoints.last.memoryTotal
        : 0;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        // Barra compacta de período + atualização na mesma linha
        Row(
          children: <Widget>[
            Expanded(
              child: SegmentedButton<String>(
                showSelectedIcon: false,
                style: SegmentedButton.styleFrom(
                  visualDensity: VisualDensity.compact,
                  padding: const EdgeInsets.symmetric(horizontal: 6),
                ),
                segments: const <ButtonSegment<String>>[
                  ButtonSegment(
                    value: '1h',
                    label: Text('1h', maxLines: 1, softWrap: false),
                  ),
                  ButtonSegment(
                    value: '6h',
                    label: Text('6h', maxLines: 1, softWrap: false),
                  ),
                  ButtonSegment(
                    value: '24h',
                    label: Text('24h', maxLines: 1, softWrap: false),
                  ),
                  ButtonSegment(
                    value: '7d',
                    label: Text('7d', maxLines: 1, softWrap: false),
                  ),
                  ButtonSegment(
                    value: '30d',
                    label: Text('30d', maxLines: 1, softWrap: false),
                  ),
                ],
                selected: <String>{_range},
                onSelectionChanged: (set) {
                  setState(() => _range = set.first);
                  _loadHistory();
                },
              ),
            ),
            const SizedBox(width: 8),
            IconButton.filledTonal(
              tooltip: 'Recarregar histórico',
              onPressed: _loading ? null : _loadHistory,
              icon: const Icon(Icons.refresh, size: 19),
            ),
          ],
        ),
        if (!isDesktop) ...<Widget>[
          const SizedBox(height: 8),
          SegmentedButton<String>(
            showSelectedIcon: false,
            style: SegmentedButton.styleFrom(
              visualDensity: VisualDensity.compact,
              padding: const EdgeInsets.symmetric(horizontal: 10),
            ),
            segments: <ButtonSegment<String>>[
              const ButtonSegment<String>(
                value: 'host',
                icon: Icon(Icons.dns_outlined, size: 16),
                label: Text('Host', maxLines: 1, softWrap: false),
              ),
              ButtonSegment<String>(
                value: 'containers',
                icon: const Icon(Icons.view_in_ar_outlined, size: 16),
                label: Text(
                  containers.isNotEmpty
                      ? 'Containers (${containers.length})'
                      : 'Containers',
                  maxLines: 1,
                  softWrap: false,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
            selected: <String>{_mobileSection},
            onSelectionChanged: (set) =>
                setState(() => _mobileSection = set.first),
          ),
        ],
        const SizedBox(height: 12),

        if (_error != null)
          ErrorStateView(message: _error!, onRetry: _loadHistory)
        else if (_loading && _history == null)
          const LoadingStateView(message: 'Carregando histórico do host...')
        else if (!isDesktop) ...<Widget>[
          if (_mobileSection == 'host')
            _buildHostGrid(
              context,
              hostPoints: hostPoints,
              latestDisks: latestDisks,
              latestMemTotal: latestMemTotal,
            )
          else
            _buildMobileContainersSection(context, containers),
        ] else ...<Widget>[
          _buildHostGrid(
            context,
            hostPoints: hostPoints,
            latestDisks: latestDisks,
            latestMemTotal: latestMemTotal,
          ),
          const SizedBox(height: 14),
          _buildDesktopMasterDetail(context, containers),
        ],
      ],
    );
  }

  Widget _buildHostGrid(
    BuildContext context, {
    required List<HostHistoryPoint> hostPoints,
    required List<HostDiskPoint> latestDisks,
    required int latestMemTotal,
  }) {
    return ResponsiveGrid(
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
                    MetricSeriesPoint(x: i.toDouble(), y: hostPoints[i].cpuAvg),
                ],
              ),
              MetricLineSeries(
                name: 'Pico',
                tone: SemanticTone.warning,
                showArea: false,
                points: <MetricSeriesPoint>[
                  for (var i = 0; i < hostPoints.length; i++)
                    MetricSeriesPoint(x: i.toDouble(), y: hostPoints[i].cpuMax),
                ],
              ),
            ],
            yFormatter: (v) => formatPercent(v, digits: v < 10 ? 1 : 0),
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
              ? const EmptyStateView(title: 'Sem dados de disco no período')
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
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: <Widget>[
                              Row(
                                children: <Widget>[
                                  Expanded(
                                    child: Text(
                                      disk.mount,
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: const TextStyle(
                                        fontWeight: FontWeight.w700,
                                        fontFamily: 'monospace',
                                      ),
                                    ),
                                  ),
                                  const SizedBox(width: 8),
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
    );
  }

  Widget _buildMobileContainersSection(
    BuildContext context,
    List<ContainerUsageSummary> containers,
  ) {
    return SectionCard(
      title: 'Consumo por container',
      subtitle: 'Toque em um container para abrir os gráficos detalhados.',
      icon: Icons.view_in_ar_outlined,
      child: containers.isEmpty
          ? const EmptyStateView(
              title: 'Sem histórico de containers no período',
            )
          : Column(
              children: containers
                  .map((c) {
                    return Padding(
                      padding: const EdgeInsets.only(bottom: 8),
                      child: ChoiceCard(
                        title: c.name,
                        subtitle: _containerSummarySubtitle(c),
                        selected: c.name == _selectedContainerName,
                        onTap: () => _openContainerSheet(c, containers),
                      ),
                    );
                  })
                  .toList(growable: false),
            ),
    );
  }

  Widget _buildDesktopMasterDetail(
    BuildContext context,
    List<ContainerUsageSummary> containers,
  ) {
    if (containers.isEmpty) {
      return const SectionCard(
        title: 'Consumo por container no período',
        icon: Icons.view_in_ar_outlined,
        child: EmptyStateView(title: 'Sem histórico de containers no período'),
      );
    }

    final selectedSummary =
        containers.where((c) => c.name == _selectedContainerName).firstOrNull ??
        containers.first;

    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Expanded(
          flex: 4,
          child: SectionCard(
            title: 'Containers (${containers.length})',
            subtitle: 'Selecione para inspecionar ao lado',
            icon: Icons.view_in_ar_outlined,
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxHeight: 560),
              child: ListView.separated(
                shrinkWrap: true,
                itemCount: containers.length,
                separatorBuilder: (_, _) => const SizedBox(height: 8),
                itemBuilder: (_, idx) {
                  final c = containers[idx];
                  return ChoiceCard(
                    title: c.name,
                    subtitle: _containerSummarySubtitle(c),
                    selected: c.name == selectedSummary.name,
                    onTap: () => _loadContainerHistory(c.name),
                  );
                },
              ),
            ),
          ),
        ),
        const SizedBox(width: 14),
        Expanded(
          flex: 6,
          child: SectionCard(
            title: selectedSummary.name,
            subtitle:
                '${selectedSummary.project?.isNotEmpty == true ? '${selectedSummary.project} · ' : ''}Período $_range',
            icon: Icons.insights,
            iconTone: SemanticTone.info,
            child: _loadingContainer
                ? const LoadingStateView(
                    message: 'Carregando série do container...',
                  )
                : _ContainerHistoryDetailContent(
                    summary: selectedSummary,
                    history: _containerHistory,
                  ),
          ),
        ),
      ],
    );
  }

  String _containerSummarySubtitle(ContainerUsageSummary c) {
    return '${c.project?.isNotEmpty == true ? '${c.project} · ' : ''}'
        'CPU ${formatPercent(c.cpuAvg)} (pico ${formatPercent(c.cpuMax)}) · '
        'RAM ${formatBinaryBytes(c.memoryMax)} · '
        'Net ↓${formatDecimalBytes(c.netRxBytes)} ↑${formatDecimalBytes(c.netTxBytes)}';
  }
}

/// Conteúdo do Bottom Sheet mobile com seletor rápido de container no topo
/// (permite alternar entre containers sem fechar o sheet nem rolar a lista).
class _ContainerHistorySheetBody extends StatefulWidget {
  const _ContainerHistorySheetBody({
    required this.initialContainerName,
    required this.containers,
    required this.range,
    required this.loader,
  });

  final String initialContainerName;
  final List<ContainerUsageSummary> containers;
  final String range;
  final Future<ContainerHistoryResponse?> Function(String name) loader;

  @override
  State<_ContainerHistorySheetBody> createState() =>
      _ContainerHistorySheetBodyState();
}

class _ContainerHistorySheetBodyState
    extends State<_ContainerHistorySheetBody> {
  late String _selectedName;
  ContainerHistoryResponse? _history;
  bool _loading = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    _selectedName = widget.initialContainerName;
    _load(_selectedName);
  }

  Future<void> _load(String name) async {
    setState(() {
      _selectedName = name;
      _loading = true;
      _error = null;
    });
    try {
      final resp = await widget.loader(name);
      if (!mounted || _selectedName != name) return;
      setState(() {
        _history = resp;
        _loading = false;
      });
    } catch (err) {
      if (!mounted || _selectedName != name) return;
      setState(() {
        _error = err.toString();
        _loading = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final summary =
        widget.containers.where((c) => c.name == _selectedName).firstOrNull ??
        widget.containers.first;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        AppDropdownFormField<String>(
          initialValue: _selectedName,
          isDense: true,
          decoration: const InputDecoration(
            labelText: 'Container',
            prefixIcon: Icon(Icons.view_in_ar_outlined, size: 18),
          ),
          items: widget.containers
              .map(
                (c) => DropdownMenuItem<String>(
                  value: c.name,
                  child: Text(
                    c.project?.isNotEmpty == true
                        ? '${c.name} (${c.project})'
                        : c.name,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              )
              .toList(growable: false),
          onChanged: (next) {
            if (next != null && next != _selectedName) {
              _load(next);
            }
          },
        ),
        const SizedBox(height: 12),
        if (_error != null)
          ErrorStateView(message: _error!, onRetry: () => _load(_selectedName))
        else if (_loading)
          const LoadingStateView(message: 'Carregando gráficos do container...')
        else
          _ContainerHistoryDetailContent(summary: summary, history: _history),
      ],
    );
  }
}

/// Bloco reutilizável (DRY) de KPIs + 4 gráficos históricos de um container
/// (CPU, RAM, Rede RX/TX e Block I/O), compartilhado entre o Bottom Sheet mobile
/// e o painel Detail no desktop.
class _ContainerHistoryDetailContent extends StatelessWidget {
  const _ContainerHistoryDetailContent({
    required this.summary,
    required this.history,
  });

  final ContainerUsageSummary summary;
  final ContainerHistoryResponse? history;

  @override
  Widget build(BuildContext context) {
    final points = history?.points ?? const <ContainerHistoryPoint>[];

    var totalBlockRead = 0;
    var totalBlockWrite = 0;
    for (final pt in points) {
      totalBlockRead += math.max(0, pt.blockReadBytes);
      totalBlockWrite += math.max(0, pt.blockWriteBytes);
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        ResponsiveGrid(
          xs: 2,
          sm: 2,
          md: 4,
          lg: 4,
          xl: 4,
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            KpiCard(
              label: 'CPU média',
              value: formatPercent(summary.cpuAvg, digits: 1),
              subtitle: 'Pico ${formatPercent(summary.cpuMax, digits: 1)}',
              icon: Icons.memory,
              tone: SemanticTone.primary,
            ),
            KpiCard(
              label: 'RAM pico',
              value: formatBinaryBytes(summary.memoryMax, fractionDigits: 0),
              icon: Icons.sd_storage_outlined,
              tone: SemanticTone.info,
            ),
            KpiCard(
              label: 'Rede RX / TX',
              value: formatDecimalBytes(summary.netRxBytes, fractionDigits: 0),
              subtitle:
                  '↑ ${formatDecimalBytes(summary.netTxBytes, fractionDigits: 0)}',
              icon: Icons.swap_vert,
              tone: SemanticTone.success,
            ),
            KpiCard(
              label: 'Disco R / W',
              value: formatDecimalBytes(totalBlockRead, fractionDigits: 0),
              subtitle:
                  'W ${formatDecimalBytes(totalBlockWrite, fractionDigits: 0)}',
              icon: Icons.storage_outlined,
              tone: SemanticTone.secondary,
            ),
          ],
        ),
        const SizedBox(height: 12),
        ResponsiveGrid(
          xs: 1,
          sm: 1,
          md: 2,
          lg: 2,
          xl: 2,
          children: <Widget>[
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
                    height: 165,
                    series: <MetricLineSeries>[
                      MetricLineSeries(
                        name: 'Média',
                        tone: SemanticTone.primary,
                        points: <MetricSeriesPoint>[
                          for (var i = 0; i < points.length; i++)
                            MetricSeriesPoint(
                              x: i.toDouble(),
                              y: points[i].cpuAvg,
                            ),
                        ],
                      ),
                      MetricLineSeries(
                        name: 'Pico',
                        tone: SemanticTone.warning,
                        showArea: false,
                        points: <MetricSeriesPoint>[
                          for (var i = 0; i < points.length; i++)
                            MetricSeriesPoint(
                              x: i.toDouble(),
                              y: points[i].cpuMax,
                            ),
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
                    height: 165,
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
                    height: 165,
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
                    height: 165,
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
        ),
      ],
    );
  }
}
