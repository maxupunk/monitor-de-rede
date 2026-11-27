import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/file_download.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/features/servers/widgets/docker_history_widgets.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Modal / bottom sheet de inspeção completa de container Docker (5 abas:
/// Consumo, Detalhes, Volumes, Redes e Logs ao vivo via SSE).
Future<void> showDockerContainerDetailSheet(
  BuildContext context, {
  required DockerContainerSummary container,
  required Future<void> Function(DockerContainerActionMeta action) onAction,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: containerDisplayName(container),
    subtitle: container.image,
    maxWidth: 860,
    builder: (_) =>
        _ContainerDetailBody(initialContainer: container, onAction: onAction),
  );
}

class _ContainerDetailBody extends ConsumerStatefulWidget {
  const _ContainerDetailBody({
    required this.initialContainer,
    required this.onAction,
  });

  final DockerContainerSummary initialContainer;
  final Future<void> Function(DockerContainerActionMeta action) onAction;

  @override
  ConsumerState<_ContainerDetailBody> createState() =>
      _ContainerDetailBodyState();
}

class _ContainerDetailBodyState extends ConsumerState<_ContainerDetailBody>
    with SingleTickerProviderStateMixin {
  late final TabController _tabController;
  DockerContainerDetail? _detail;
  bool _loadingDetail = true;
  String? _detailError;

  // Consumo histórico
  String _historyRange = '24h';
  ContainerHistoryResponse? _history;
  bool _loadingHistory = false;

  // Logs
  String _tail = '100';
  List<DockerLogEntry> _logs = const <DockerLogEntry>[];
  bool _loadingLogs = false;
  String? _logStreamId;
  String _logSearch = '';

  // Redes
  String? _selectedConnectNetworkId;
  DockerNotifier? _dockerNotifier;

  @override
  void initState() {
    super.initState();
    _dockerNotifier = ref.read(dockerProvider.notifier);
    _tabController = TabController(length: 5, vsync: this);
    _tabController.addListener(_onTabChanged);
    Future<void>.microtask(() {
      if (!mounted) return;
      _loadDetail();
      _loadHistory();
    });
  }

  void _onTabChanged() {
    if (_tabController.indexIsChanging) return;
    if (_tabController.index == 4 && _logs.isEmpty && _logStreamId == null) {
      _loadLogs();
    }
  }

  @override
  void dispose() {
    final streamId = _logStreamId;
    final notifier = _dockerNotifier;
    if (streamId != null && notifier != null) {
      Future<void>.microtask(() => notifier.stopLogStream(streamId));
    }
    _tabController.removeListener(_onTabChanged);
    _tabController.dispose();
    super.dispose();
  }

  Future<void> _loadDetail() async {
    setState(() {
      _loadingDetail = true;
      _detailError = null;
    });
    try {
      final detail = await ref
          .read(dockerProvider.notifier)
          .fetchContainerDetail(widget.initialContainer.id);
      if (!mounted) return;
      setState(() {
        _detail = detail;
        _loadingDetail = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _detailError = describeError(
          err,
          fallback: 'Erro ao inspecionar container.',
        );
        _loadingDetail = false;
      });
    }
  }

  Future<void> _loadHistory() async {
    setState(() => _loadingHistory = true);
    try {
      final resp = await ref
          .read(dockerProvider.notifier)
          .fetchContainerHistory(
            containerDisplayName(widget.initialContainer),
            _historyRange,
          );
      if (!mounted) return;
      setState(() {
        _history = resp;
        _loadingHistory = false;
      });
    } catch (_) {
      if (!mounted) return;
      setState(() => _loadingHistory = false);
    }
  }

  Future<void> _loadLogs() async {
    setState(() => _loadingLogs = true);
    try {
      final entries = await ref
          .read(dockerProvider.notifier)
          .fetchContainerLogs(
            widget.initialContainer.id,
            tail: _tail,
            timestamps: true,
          );
      if (!mounted) return;
      setState(() {
        _logs = entries;
        _loadingLogs = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() => _loadingLogs = false);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Erro ao carregar logs: ${describeError(err)}')),
      );
    }
  }

  Future<void> _toggleLiveLogs() async {
    final notifier = ref.read(dockerProvider.notifier);
    if (_logStreamId != null) {
      final id = _logStreamId!;
      setState(() => _logStreamId = null);
      await notifier.stopLogStream(id);
      return;
    }
    try {
      final streamId = await notifier.startLogStream(
        widget.initialContainer.id,
        tail: _tail,
      );
      if (!mounted) {
        unawaited(notifier.stopLogStream(streamId));
        return;
      }
      setState(() => _logStreamId = streamId);
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            'Falha ao acompanhar logs ao vivo: ${describeError(err)}',
          ),
        ),
      );
    }
  }

  Future<void> _downloadLogs() async {
    try {
      final all = await ref
          .read(dockerProvider.notifier)
          .fetchContainerLogs(
            widget.initialContainer.id,
            tail: 'all',
            timestamps: true,
          );
      if (!mounted) return;
      final name = sanitizeFileSlug(
        containerDisplayName(widget.initialContainer),
        fallback: 'container',
      );
      await saveOrShareText(
        context: context,
        content: all.map((e) => e.formattedLine).join('\n'),
        filename: '$name-logs.txt',
      );
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Erro ao baixar logs: ${describeError(err)}')),
      );
    }
  }

  Future<void> _clearLogs() async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Limpar logs do container?',
      message:
          'Para limpar o arquivo de logs, o container pode ser brevemente parado e iniciado.',
      confirmLabel: 'Limpar logs',
      tone: SemanticTone.warning,
    );
    if (!ok || !mounted) return;
    try {
      await ref
          .read(dockerProvider.notifier)
          .clearContainerLogs(widget.initialContainer.id);
      if (!mounted) return;
      setState(() => _logs = const <DockerLogEntry>[]);
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(const SnackBar(content: Text('Logs limpos com sucesso.')));
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Erro ao limpar logs: ${describeError(err)}')),
      );
    }
  }

  Future<void> _connectNetwork() async {
    if (_selectedConnectNetworkId == null) return;
    final netId = _selectedConnectNetworkId!;
    final ok = await ref
        .read(dockerProvider.notifier)
        .connectNetwork(netId, widget.initialContainer.id);
    if (ok && mounted) {
      setState(() => _selectedConnectNetworkId = null);
      await _loadDetail();
    }
  }

  Future<void> _disconnectNetwork(DockerNetworkEndpoint ep) async {
    final docker = ref.read(dockerProvider);
    String? networkId = ep.networkId;
    if (networkId == null || networkId.isEmpty) {
      for (final n in docker.current.networks) {
        if (n.name == ep.networkName) {
          networkId = n.id;
          break;
        }
      }
    }
    if (networkId == null) return;
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Desconectar de ${ep.networkName}?',
      message: 'O container perderá comunicação nesta rede Docker.',
      confirmLabel: 'Desconectar',
      tone: SemanticTone.warning,
    );
    if (!confirmed || !mounted) return;
    final ok = await ref
        .read(dockerProvider.notifier)
        .disconnectNetwork(networkId, widget.initialContainer.id);
    if (ok && mounted) await _loadDetail();
  }

  @override
  Widget build(BuildContext context) {
    final docker = ref.watch(dockerProvider);
    final isAdmin = ref.watch(authProvider).isAdmin;
    final liveSummary = docker.current.containers.firstWhere(
      (c) => c.id == widget.initialContainer.id,
      orElse: () => widget.initialContainer,
    );
    final metric = docker.current.metricFor(liveSummary.id);
    final stateMeta = containerStateMeta(liveSummary.state);
    final actions = allowedContainerActions(
      container: liveSummary,
      canLifecycle: isAdmin && docker.allows('lifecycle'),
      canUpdate: isAdmin && docker.allows('update'),
    );

    final activeStream = _logStreamId != null
        ? docker.logStreams[_logStreamId!]
        : null;
    final displayLogs = activeStream != null ? activeStream.entries : _logs;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Wrap(
          spacing: 8,
          runSpacing: 8,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            SemanticChip(
              label: stateMeta.label,
              tone: stateMeta.tone,
              icon: stateMeta.icon,
              compact: true,
            ),
            if (liveSummary.isProtected)
              const Tooltip(
                message:
                    'Container protegido (com.netmonitor.protected=true): não pode ser parado, recriado ou removido.',
                child: SemanticChip(
                  label: 'Protegido',
                  tone: SemanticTone.info,
                  icon: Icons.shield,
                  compact: true,
                ),
              ),
            ...actions.map(
              (a) => OutlinedButton.icon(
                onPressed: docker.actionLoading
                    ? null
                    : () => widget.onAction(a),
                icon: Icon(a.icon, size: 16),
                label: Text(a.label),
              ),
            ),
          ],
        ),
        const SizedBox(height: 12),

        TabBar(
          controller: _tabController,
          isScrollable: true,
          tabAlignment: TabAlignment.start,
          tabs: const <Tab>[
            Tab(text: 'Consumo'),
            Tab(text: 'Detalhes'),
            Tab(text: 'Volumes'),
            Tab(text: 'Redes'),
            Tab(text: 'Logs'),
          ],
        ),
        const SizedBox(height: 12),

        SizedBox(
          height: math.min(440.0, MediaQuery.sizeOf(context).height * 0.56),
          child: TabBarView(
            controller: _tabController,
            children: <Widget>[
              _buildUsageTab(metric),
              _buildOverviewTab(),
              _buildMountsTab(),
              _buildNetworksTab(isAdmin, docker.current.networks),
              _buildLogsTab(isAdmin, displayLogs, activeStream?.ended),
            ],
          ),
        ),
      ],
    );
  }

  Widget _buildUsageTab(DockerContainerMetrics? metric) {
    final points = _history?.points ?? const <ContainerHistoryPoint>[];
    return ListView(
      children: <Widget>[
        if (metric != null) ...<Widget>[
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: <Widget>[
              SemanticChip(
                label: 'CPU atual: ${formatPercent(metric.cpu.usagePercent)}',
                tone: SemanticTone.primary,
              ),
              SemanticChip(
                label:
                    'RAM atual: ${formatBinaryBytes(metric.memory.usageBytes)}',
                tone: SemanticTone.info,
              ),
              SemanticChip(
                label:
                    'Rede: RX ${formatDecimalBytes(metric.network.receivedBytes)} · TX ${formatDecimalBytes(metric.network.transmittedBytes)}',
                tone: SemanticTone.secondary,
              ),
            ],
          ),
          const SizedBox(height: 12),
        ],
        DockerHistoryRangeSelector(
          selectedRange: _historyRange,
          onChanged: (nextRange) {
            setState(() => _historyRange = nextRange);
            _loadHistory();
          },
        ),
        const SizedBox(height: 12),
        if (_loadingHistory)
          const LoadingStateView(message: 'Carregando histórico...')
        else
          ContainerHistoryCharts(
            points: points,
            showNetworkAndDisk: false,
            chartHeight: 150,
            useGrid: false,
          ),
      ],
    );
  }

  Widget _buildOverviewTab() {
    if (_loadingDetail) {
      return const LoadingStateView(message: 'Inspecionando container...');
    }
    if (_detailError != null) {
      return ErrorStateView(message: _detailError!, onRetry: _loadDetail);
    }
    final d = _detail!;
    return ListView(
      children: <Widget>[
        KeyValueRow(
          label: 'ID',
          value: shortDockerId(d.id),
          monospace: true,
          copyable: true,
        ),
        KeyValueRow(label: 'Imagem', value: d.image, monospace: true),
        KeyValueRow(label: 'Hostname', value: d.hostname ?? '—'),
        KeyValueRow(
          label: 'Usuário',
          value: d.user?.isNotEmpty == true ? d.user! : 'root (padrão)',
        ),
        KeyValueRow(
          label: 'Estado / PID',
          value: '${d.status} · PID ${d.pid ?? '—'}',
        ),
        KeyValueRow(label: 'Restart policy', value: d.restartPolicy ?? '—'),
        KeyValueRow(label: 'Modo de rede', value: d.networkMode ?? '—'),
        KeyValueRow(label: 'Criado em', value: formatDateTime(d.created)),
        if (d.command.isNotEmpty) ...<Widget>[
          const SizedBox(height: 10),
          CodeViewer(title: 'Comando', code: d.command.join(' ')),
        ],
        if (d.environment.isNotEmpty) ...<Widget>[
          const SizedBox(height: 10),
          CodeViewer(
            title: 'Variáveis de ambiente (segredos mascarados no servidor)',
            code: d.environment.join('\n'),
            maxHeight: 180,
          ),
        ],
      ],
    );
  }

  Widget _buildMountsTab() {
    if (_loadingDetail) {
      return const LoadingStateView(message: 'Carregando mounts...');
    }
    final mounts = _detail?.mounts ?? const <DockerMount>[];
    if (mounts.isEmpty) {
      return const EmptyStateView(
        title: 'Nenhum volume montado',
        icon: Icons.storage_outlined,
      );
    }
    return ListView.separated(
      itemCount: mounts.length,
      separatorBuilder: (_, _) => const SizedBox(height: 8),
      itemBuilder: (_, idx) {
        final m = mounts[idx];
        return NestedPanel(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Row(
                children: <Widget>[
                  SemanticChip(
                    label: m.mountType,
                    tone: SemanticTone.info,
                    compact: true,
                  ),
                  const SizedBox(width: 8),
                  SemanticChip(
                    label: m.rw ? 'RW' : 'RO',
                    tone: m.rw ? SemanticTone.success : SemanticTone.warning,
                    compact: true,
                  ),
                ],
              ),
              const SizedBox(height: 6),
              KeyValueRow(
                label: 'Origem',
                value: (m.name != null && m.name!.isNotEmpty)
                    ? m.name!
                    : m.source,
                monospace: true,
              ),
              KeyValueRow(
                label: 'Destino',
                value: m.destination,
                monospace: true,
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _buildNetworksTab(
    bool isAdmin,
    List<DockerNetworkSummary> allNetworks,
  ) {
    if (_loadingDetail) {
      return const LoadingStateView(message: 'Carregando redes...');
    }
    final connected = _detail?.networks ?? const <DockerNetworkEndpoint>[];
    final connectedNames = connected.map((e) => e.networkName).toSet();
    final availableToConnect = allNetworks
        .where((n) => !connectedNames.contains(n.name))
        .toList(growable: false);

    return ListView(
      children: <Widget>[
        if (isAdmin && availableToConnect.isNotEmpty) ...<Widget>[
          InlineFieldActionRow(
            field: AppDropdownFormField<String>(
              initialValue:
                  availableToConnect.any(
                    (n) => n.id == _selectedConnectNetworkId,
                  )
                  ? _selectedConnectNetworkId
                  : null,
              decoration: const InputDecoration(
                labelText: 'Conectar a uma rede',
                isDense: true,
              ),
              items: availableToConnect
                  .map(
                    (n) => DropdownMenuItem<String>(
                      value: n.id,
                      child: Text(n.name, overflow: TextOverflow.ellipsis),
                    ),
                  )
                  .toList(growable: false),
              onChanged: (v) => setState(() => _selectedConnectNetworkId = v),
            ),
            action: FilledButton(
              onPressed: _selectedConnectNetworkId == null
                  ? null
                  : _connectNetwork,
              child: const Text('Adicionar'),
            ),
          ),
          const SizedBox(height: 12),
        ],
        if (connected.isEmpty)
          const EmptyStateView(
            title: 'Sem redes conectadas',
            icon: Icons.hub_outlined,
          )
        else
          ...connected.map(
            (ep) => Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: NestedPanel(
                child: Row(
                  children: <Widget>[
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Text(
                            ep.networkName,
                            style: context.textTheme.titleSmall?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                          const SizedBox(height: 4),
                          Text(
                            'IP: ${ep.ipAddress ?? '—'} · Gateway: ${ep.gateway ?? '—'}',
                            style: context.textTheme.bodySmall,
                          ),
                        ],
                      ),
                    ),
                    if (isAdmin)
                      Tooltip(
                        message: connected.length <= 1
                            ? 'O container precisa permanecer em pelo menos uma rede.'
                            : 'Desconectar desta rede',
                        child: IconButton(
                          icon: const Icon(Icons.link_off),
                          onPressed: connected.length <= 1
                              ? null
                              : () => _disconnectNetwork(ep),
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
      ],
    );
  }

  Widget _buildLogsTab(
    bool isAdmin,
    List<DockerLogEntry> entries,
    String? endedReason,
  ) {
    final term = _logSearch.trim().toLowerCase();
    final visible = term.isEmpty
        ? entries
        : entries
              .where((e) => e.formattedLine.toLowerCase().contains(term))
              .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        LayoutBuilder(
          builder: (context, constraints) {
            final isCompact = constraints.maxWidth < 520;
            final linesField = SizedBox(
              width: 104,
              child: AppDropdownFormField<String>(
                initialValue: _tail,
                isDense: true,
                decoration: const InputDecoration(labelText: 'Linhas'),
                items: const <DropdownMenuItem<String>>[
                  DropdownMenuItem(value: '100', child: Text('100')),
                  DropdownMenuItem(value: '500', child: Text('500')),
                  DropdownMenuItem(value: '1000', child: Text('1000')),
                  DropdownMenuItem(value: '5000', child: Text('5000')),
                  DropdownMenuItem(value: 'all', child: Text('Todos')),
                ],
                onChanged: _logStreamId != null
                    ? null
                    : (v) {
                        if (v != null) {
                          setState(() => _tail = v);
                          _loadLogs();
                        }
                      },
              ),
            );
            final liveBtn = FilledButton.tonalIcon(
              onPressed: _toggleLiveLogs,
              icon: Icon(
                _logStreamId != null ? Icons.pause : Icons.play_arrow,
                size: 16,
              ),
              label: Text(_logStreamId != null ? 'Pausar' : 'Ao vivo'),
            );

            if (isCompact) {
              return Row(
                children: <Widget>[
                  linesField,
                  const SizedBox(width: 8),
                  if (isAdmin) Expanded(child: liveBtn) else const Spacer(),
                  PopupMenuButton<String>(
                    tooltip: 'Mais ações de log',
                    onSelected: (cmd) {
                      if (cmd == 'refresh' &&
                          _logStreamId == null &&
                          !_loadingLogs) {
                        _loadLogs();
                      } else if (cmd == 'download') {
                        _downloadLogs();
                      } else if (cmd == 'clear') {
                        _clearLogs();
                      }
                    },
                    itemBuilder: (_) => <PopupMenuEntry<String>>[
                      PopupMenuItem<String>(
                        value: 'refresh',
                        enabled: _logStreamId == null && !_loadingLogs,
                        child: const ListTile(
                          dense: true,
                          leading: Icon(Icons.refresh, size: 18),
                          title: Text('Atualizar'),
                        ),
                      ),
                      const PopupMenuItem<String>(
                        value: 'download',
                        child: ListTile(
                          dense: true,
                          leading: Icon(Icons.download, size: 18),
                          title: Text('Baixar'),
                        ),
                      ),
                      if (isAdmin)
                        const PopupMenuItem<String>(
                          value: 'clear',
                          child: ListTile(
                            dense: true,
                            leading: Icon(
                              Icons.delete_sweep_outlined,
                              size: 18,
                            ),
                            title: Text('Limpar'),
                          ),
                        ),
                    ],
                  ),
                ],
              );
            }

            return Wrap(
              spacing: 8,
              runSpacing: 8,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: <Widget>[
                linesField,
                OutlinedButton.icon(
                  onPressed: _logStreamId != null || _loadingLogs
                      ? null
                      : _loadLogs,
                  icon: const Icon(Icons.refresh, size: 16),
                  label: const Text('Atualizar'),
                ),
                if (isAdmin) liveBtn,
                OutlinedButton.icon(
                  onPressed: _downloadLogs,
                  icon: const Icon(Icons.download, size: 16),
                  label: const Text('Baixar'),
                ),
                if (isAdmin)
                  TextButton.icon(
                    onPressed: _clearLogs,
                    icon: const Icon(Icons.delete_sweep_outlined, size: 16),
                    label: const Text('Limpar'),
                  ),
              ],
            );
          },
        ),
        const SizedBox(height: 8),
        TextField(
          decoration: const InputDecoration(
            hintText: 'Filtrar linhas de log...',
            prefixIcon: Icon(Icons.search, size: 18),
            isDense: true,
          ),
          onChanged: (v) => setState(() => _logSearch = v),
        ),
        if (endedReason != null) ...<Widget>[
          const SizedBox(height: 8),
          NestedPanel(
            tone: SemanticTone.warning,
            child: Text('Acompanhamento encerrado: $endedReason'),
          ),
        ],
        const SizedBox(height: 8),
        Expanded(
          child: _loadingLogs
              ? const LoadingStateView(message: 'Lendo logs...')
              : CodeViewer(
                  title:
                      '${visible.length} linhas${_logStreamId != null ? ' · SSE ao vivo' : ''}',
                  code: visible.isEmpty
                      ? '(Nenhuma linha de log encontrada)'
                      : visible.map((e) => e.formattedLine).join('\n'),
                  followTail: true,
                  actions: <Widget>[
                    AiLogAnalysisButton(
                      source:
                          'Logs do container '
                          '${containerDisplayName(widget.initialContainer)}',
                      content: visible.map((e) => e.formattedLine).join('\n'),
                    ),
                  ],
                ),
        ),
      ],
    );
  }
}
