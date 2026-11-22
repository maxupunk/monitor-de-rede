import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/alert_models.dart';
import 'package:netmonitor_app/features/core/models/event_models.dart';
import 'package:netmonitor_app/features/core/models/monitor_models.dart';
import 'package:netmonitor_app/features/core/presentation/alerts/alert_dialogs.dart';
import 'package:netmonitor_app/features/core/presentation/monitors/monitor_form_sheet.dart';
import 'package:netmonitor_app/features/core/providers/alerts_provider.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Página completa de detalhes do monitor (`/monitors/:id`).
class MonitorDetailPage extends StatelessWidget {
  const MonitorDetailPage({super.key, required this.monitorId});

  final int monitorId;

  @override
  Widget build(BuildContext context) {
    return MonitorDetailView(
      monitorId: monitorId,
      onClose: () {
        if (context.canPop()) {
          context.pop();
        } else {
          context.go('/monitors');
        }
      },
    );
  }
}

/// View reutilizável (página dedicada ou modal embutido — paridade com `MonitorDetailView.vue`).
class MonitorDetailView extends ConsumerStatefulWidget {
  const MonitorDetailView({
    super.key,
    required this.monitorId,
    this.onClose,
    this.embedded = false,
  });

  final int monitorId;
  final VoidCallback? onClose;
  final bool embedded;

  @override
  ConsumerState<MonitorDetailView> createState() => _MonitorDetailViewState();
}

class _MonitorDetailViewState extends ConsumerState<MonitorDetailView> {
  int _uptimeHours = 24;
  MonitorUptimeResponse? _uptime;
  Map<String, dynamic>? _baseline;

  bool _historyExpanded = false;
  bool _historyLoading = false;
  List<MonitorResult> _historyResults = const <MonitorResult>[];
  PaginationMeta? _historyMeta;

  bool _alertsExpanded = false;
  bool _alertsLoading = false;
  List<AlertEvent> _monitorAlerts = const <AlertEvent>[];
  PaginationMeta? _alertsMeta;

  VoidCallback? _unsubAlerts;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _loadAll());
    _unsubAlerts = ref.read(sseServiceProvider.notifier).onEvent(
      <String>[
        'alert:triggered',
        'alert:resolved',
        'alert:acknowledged',
        'alert:silenced',
      ],
      (data, _) {
        final monId = (data['monitorId'] as num?)?.toInt();
        if (monId == widget.monitorId && _alertsExpanded && mounted) {
          _loadAlerts(page: 1, append: false);
        }
      },
    );
  }

  @override
  void dispose() {
    _unsubAlerts?.call();
    super.dispose();
  }

  Future<void> _loadAll() async {
    final notifier = ref.read(monitorsProvider.notifier);
    final mon = await notifier.fetchMonitorById(widget.monitorId);
    final up = await notifier.fetchUptime(
      widget.monitorId,
      hours: _uptimeHours,
    );
    Map<String, dynamic>? base;
    if (mon != null && !mon.isSnmpGauge) {
      base = await notifier.fetchBaseline(widget.monitorId);
    }
    if (!mounted) return;
    setState(() {
      _uptime = up;
      _baseline = base;
    });
  }

  Future<void> _changeUptimeRange(int hours) async {
    setState(() => _uptimeHours = hours);
    final up = await ref
        .read(monitorsProvider.notifier)
        .fetchUptime(widget.monitorId, hours: hours);
    if (!mounted) return;
    setState(() => _uptime = up);
  }

  Future<void> _loadHistory({int page = 1, bool append = false}) async {
    setState(() => _historyLoading = true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>(
            '/monitors/${widget.monitorId}/results',
            queryParameters: <String, dynamic>{'page': page, 'limit': 30},
          );
      List<MonitorResult> items = const <MonitorResult>[];
      PaginationMeta? meta;
      if (raw is Map) {
        final list = raw['data'] is List ? raw['data'] as List : const [];
        items = list
            .whereType<Map>()
            .map((e) => MonitorResult.fromJson(Map<String, dynamic>.from(e)))
            .toList(growable: false);
        if (raw['meta'] is Map) {
          meta = PaginationMeta.fromJson(
            Map<String, dynamic>.from(raw['meta'] as Map),
          );
        }
      } else if (raw is List) {
        items = raw
            .whereType<Map>()
            .map((e) => MonitorResult.fromJson(Map<String, dynamic>.from(e)))
            .toList(growable: false);
      }
      if (!mounted) return;
      setState(() {
        _historyResults = append
            ? <MonitorResult>[..._historyResults, ...items]
            : items;
        _historyMeta = meta;
        _historyLoading = false;
      });
    } catch (_) {
      if (mounted) setState(() => _historyLoading = false);
    }
  }

  Future<void> _loadAlerts({int page = 1, bool append = false}) async {
    setState(() => _alertsLoading = true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>(
            '/monitors/${widget.monitorId}/alerts',
            queryParameters: <String, dynamic>{'page': page, 'limit': 25},
          );
      List<AlertEvent> items = const <AlertEvent>[];
      PaginationMeta? meta;
      if (raw is Map) {
        final list = raw['data'] is List ? raw['data'] as List : const [];
        items = list
            .whereType<Map>()
            .map((e) => AlertEvent.fromJson(Map<String, dynamic>.from(e)))
            .toList(growable: false);
        if (raw['meta'] is Map) {
          meta = PaginationMeta.fromJson(
            Map<String, dynamic>.from(raw['meta'] as Map),
          );
        }
      } else if (raw is List) {
        items = raw
            .whereType<Map>()
            .map((e) => AlertEvent.fromJson(Map<String, dynamic>.from(e)))
            .toList(growable: false);
      }
      if (!mounted) return;
      setState(() {
        _monitorAlerts = append
            ? <AlertEvent>[..._monitorAlerts, ...items]
            : items;
        _alertsMeta = meta;
        _alertsLoading = false;
      });
    } catch (_) {
      if (mounted) setState(() => _alertsLoading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final monitorsState = ref.watch(monitorsProvider);
    final auth = ref.watch(authProvider);
    final monitor = monitorsState.currentMonitor?.id == widget.monitorId
        ? monitorsState.currentMonitor
        : null;

    if (monitorsState.loading && monitor == null) {
      return const LoadingStateView(
        message: 'Carregando detalhes do monitor...',
      );
    }
    if (monitor == null) {
      return ErrorStateView(
        message: monitorsState.error ?? 'Monitor não encontrado.',
        onRetry: _loadAll,
      );
    }

    final recent = monitor.recentResults; // oldest -> newest (do NOT reverse)
    final latencyPoints = <MetricSeriesPoint>[
      for (var i = 0; i < recent.length; i++)
        if (recent[i].latencyMs != null)
          MetricSeriesPoint(x: i.toDouble(), y: recent[i].latencyMs!),
    ];

    final inPoints = <MetricSeriesPoint>[
      for (var i = 0; i < monitor.inHistory.length; i++)
        MetricSeriesPoint(x: i.toDouble(), y: monitor.inHistory[i].value),
    ];
    final outPoints = <MetricSeriesPoint>[
      for (var i = 0; i < monitor.outHistory.length; i++)
        MetricSeriesPoint(x: i.toDouble(), y: monitor.outHistory[i].value),
    ];
    final gaugePoints = <MetricSeriesPoint>[
      for (var i = 0; i < monitor.gaugeHistory.length; i++)
        MetricSeriesPoint(x: i.toDouble(), y: monitor.gaugeHistory[i].value),
    ];

    return RefreshIndicator(
      onRefresh: _loadAll,
      child: ListView(
        children: <Widget>[
          Row(
            children: <Widget>[
              if (!widget.embedded && widget.onClose != null) ...<Widget>[
                IconButton(
                  tooltip: 'Voltar',
                  icon: const Icon(Icons.arrow_back),
                  onPressed: widget.onClose,
                ),
                const SizedBox(width: 4),
              ],
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Wrap(
                      spacing: 8,
                      runSpacing: 6,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: <Widget>[
                        Text(
                          monitor.name,
                          style: context.textTheme.headlineSmall?.copyWith(
                            fontWeight: FontWeight.w800,
                          ),
                        ),
                        StatusIndicator.fromStatus(monitor.normalizedStatus),
                        SemanticChip(
                          label: monitor.type.toUpperCase(),
                          tone: SemanticTone.primary,
                          compact: true,
                        ),
                        if (monitor.inMaintenance)
                          const SemanticChip(
                            label: 'Em manutenção',
                            tone: SemanticTone.info,
                            icon: Icons.build_circle_outlined,
                            compact: true,
                          ),
                      ],
                    ),
                    const SizedBox(height: 4),
                    Text(
                      '${monitor.target} · Intervalo ${monitor.intervalSeconds}s · Timeout ${monitor.timeoutSeconds}s',
                      style: context.textTheme.bodySmall?.copyWith(
                        color: context.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          if (auth.canWrite)
            SymmetricalActionBar(
              spacing: 8,
              runSpacing: 8,
              children: <Widget>[
                FilledButton.icon(
                  onPressed: monitorsState.runningId == monitor.id
                      ? null
                      : () => ref
                            .read(monitorsProvider.notifier)
                            .runMonitor(monitor.id),
                  icon: const Icon(Icons.play_arrow, size: 18),
                  label: const Text('Testar agora'),
                ),
                OutlinedButton.icon(
                  onPressed: () => ref
                      .read(monitorsProvider.notifier)
                      .toggleEnabled(monitor.id, !monitor.isEnabled),
                  icon: Icon(
                    monitor.isEnabled ? Icons.pause : Icons.play_arrow_outlined,
                    size: 18,
                  ),
                  label: Text(monitor.isEnabled ? 'Pausar' : 'Ativar'),
                ),
                OutlinedButton.icon(
                  onPressed: () async {
                    final ok = await showMonitorFormSheet(
                      context: context,
                      ref: ref,
                      existing: monitor,
                    );
                    if (ok) await _loadAll();
                  },
                  icon: const Icon(Icons.edit_outlined, size: 18),
                  label: const Text('Editar'),
                ),
                OutlinedButton.icon(
                  onPressed: () async {
                    final ok = await showConfirmActionDialog(
                      context: context,
                      title: 'Excluir monitor?',
                      message:
                          'Esta ação remove "${monitor.name}" permanentemente.',
                      confirmLabel: 'Excluir',
                    );
                    if (!ok) return;
                    final deleted = await ref
                        .read(monitorsProvider.notifier)
                        .deleteMonitor(monitor.id);
                    if (deleted && mounted) widget.onClose?.call();
                  },
                  icon: Icon(
                    Icons.delete_outline,
                    size: 18,
                    color: context.colorScheme.error,
                  ),
                  label: Text(
                    'Excluir',
                    style: TextStyle(color: context.colorScheme.error),
                  ),
                ),
              ],
            ),
          const SizedBox(height: 14),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            alignment: WrapAlignment.spaceBetween,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              Text(
                'Disponibilidade',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              SegmentedButton<int>(
                showSelectedIcon: false,
                style: const ButtonStyle(visualDensity: VisualDensity.compact),
                segments: const <ButtonSegment<int>>[
                  ButtonSegment<int>(value: 24, label: Text('24h')),
                  ButtonSegment<int>(value: 168, label: Text('7d')),
                  ButtonSegment<int>(value: 720, label: Text('30d')),
                ],
                selected: <int>{_uptimeHours},
                onSelectionChanged: (s) => _changeUptimeRange(s.first),
              ),
            ],
          ),
          const SizedBox(height: 10),
          ResponsiveGrid(
            xs: 2,
            sm: 2,
            md: 4,
            lg: 4,
            xl: 4,
            children: <Widget>[
              KpiCard(
                label:
                    'Uptime (${_uptimeHours == 24
                        ? '24h'
                        : _uptimeHours == 168
                        ? '7d'
                        : '30d'})',
                value: formatPercent(
                  _uptime?.uptimePercentage ?? monitor.stats?.uptimePercentage,
                ),
                icon: Icons.verified_outlined,
                tone: SemanticTone.success,
              ),
              KpiCard(
                label: 'Latência média',
                value: formatLatency(
                  _uptime?.avgLatencyMs ?? monitor.stats?.avgLatency,
                ),
                subtitle: 'Última: ${formatLatency(monitor.lastLatencyMs)}',
                icon: Icons.speed,
                tone: SemanticTone.primary,
              ),
              KpiCard(
                label: 'Verificações OK',
                value: formatInteger(
                  _uptime?.upChecks ?? monitor.stats?.upChecks ?? 0,
                ),
                subtitle:
                    'de ${formatInteger(_uptime?.totalChecks ?? monitor.stats?.totalChecks ?? 0)}',
                icon: Icons.check_circle_outline,
                tone: SemanticTone.info,
              ),
              KpiCard(
                label: 'Quedas no período',
                value: formatInteger(_uptime?.downChecks ?? 0),
                subtitle:
                    'Última: ${formatRelativeTime(monitor.lastCheckedAt)}',
                icon: Icons.error_outline,
                tone: (_uptime?.downChecks ?? 0) > 0
                    ? SemanticTone.error
                    : SemanticTone.success,
              ),
            ],
          ),
          const SizedBox(height: 14),
          if (_baseline != null && _baseline!.isNotEmpty) ...<Widget>[
            SectionCard(
              title: 'Linha de base adaptativa',
              subtitle: 'Comportamento esperado de latência para este alvo',
              icon: Icons.insights,
              iconTone: SemanticTone.info,
              child: Wrap(
                spacing: 16,
                runSpacing: 8,
                children: <Widget>[
                  if (_baseline!['expectedLatencyMs'] is num)
                    SemanticChip(
                      label:
                          'Esperado: ${formatLatency(_baseline!['expectedLatencyMs'] as num)}',
                      tone: SemanticTone.info,
                    ),
                  if (_baseline!['alertThresholdMs'] is num)
                    SemanticChip(
                      label:
                          'Limiar de alerta: ${formatLatency(_baseline!['alertThresholdMs'] as num)}',
                      tone: SemanticTone.warning,
                    ),
                  if (_baseline!['sampleCount'] is num)
                    SemanticChip(
                      label:
                          '${formatInteger(_baseline!['sampleCount'] as num)} amostras',
                      tone: SemanticTone.secondary,
                    ),
                ],
              ),
            ),
            const SizedBox(height: 14),
          ],
          SectionCard(
            title: monitor.isTrafficMonitor
                ? 'Tráfego de interface'
                : (monitor.isSnmpGauge
                      ? 'Histórico de métrica SNMP'
                      : 'Latência recente'),
            subtitle: 'Atualizado automaticamente em tempo real via SSE',
            icon: Icons.show_chart,
            child: monitor.isTrafficMonitor
                ? MetricLineChart(
                    height: 220,
                    series: <MetricLineSeries>[
                      MetricLineSeries(
                        name: 'Entrada (RX)',
                        points: inPoints,
                        tone: SemanticTone.success,
                      ),
                      MetricLineSeries(
                        name: 'Saída (TX)',
                        points: outPoints,
                        tone: SemanticTone.primary,
                      ),
                    ],
                    yFormatter: (v) => formatBps(v),
                  )
                : monitor.isSnmpGauge
                ? MetricLineChart(
                    height: 220,
                    series: <MetricLineSeries>[
                      MetricLineSeries(
                        name: monitor.snmpMode,
                        points: gaugePoints,
                        tone: SemanticTone.primary,
                      ),
                    ],
                    yFormatter: (v) => formatMeasuredValue(
                      v,
                      unit: monitor.gaugeMetric?.unit ?? '%',
                    ),
                  )
                : MetricLineChart(
                    height: 220,
                    series: <MetricLineSeries>[
                      MetricLineSeries(
                        name: 'Latência',
                        points: latencyPoints,
                        tone: SemanticTone.primary,
                      ),
                    ],
                    yFormatter: (v) => formatLatency(v),
                  ),
          ),
          const SizedBox(height: 14),
          SectionCard(
            title: 'Histórico de verificações',
            subtitle: 'Carregado sob demanda',
            icon: Icons.history,
            trailing: TextButton(
              onPressed: () {
                setState(() => _historyExpanded = !_historyExpanded);
                if (_historyExpanded && _historyResults.isEmpty) {
                  _loadHistory(page: 1);
                }
              },
              child: Text(_historyExpanded ? 'Ocultar' : 'Carregar'),
            ),
            child: !_historyExpanded
                ? Text(
                    'Expanda para consultar as verificações gravadas deste monitor.',
                    style: context.textTheme.bodySmall?.copyWith(
                      color: context.colorScheme.onSurfaceVariant,
                    ),
                  )
                : _historyLoading && _historyResults.isEmpty
                ? const LoadingStateView(message: 'Carregando histórico...')
                : Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      for (final item in _historyResults) ...<Widget>[
                        ListTile(
                          dense: true,
                          contentPadding: EdgeInsets.zero,
                          leading: StatusIndicator.fromStatus(
                            item.status,
                            compact: true,
                          ),
                          title: Text(
                            formatDateTime(item.finishedAt),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                          subtitle:
                              item.message != null && item.message!.isNotEmpty
                              ? Text(
                                  item.message!,
                                  maxLines: 2,
                                  overflow: TextOverflow.ellipsis,
                                )
                              : null,
                          trailing: Text(
                            formatLatency(item.latencyMs),
                            style: const TextStyle(fontWeight: FontWeight.w700),
                          ),
                        ),
                        const Divider(height: 1),
                      ],
                      if (_historyMeta?.hasNext == true) ...<Widget>[
                        const SizedBox(height: 8),
                        OutlinedButton(
                          onPressed: _historyLoading
                              ? null
                              : () => _loadHistory(
                                  page: _historyMeta!.currentPage + 1,
                                  append: true,
                                ),
                          child: const Text('Carregar mais'),
                        ),
                      ],
                    ],
                  ),
          ),
          const SizedBox(height: 14),
          SectionCard(
            title: 'Alertas associados',
            subtitle: 'Incidentes registrados para este monitor',
            icon: Icons.notifications_outlined,
            iconTone: SemanticTone.warning,
            trailing: TextButton(
              onPressed: () {
                setState(() => _alertsExpanded = !_alertsExpanded);
                if (_alertsExpanded && _monitorAlerts.isEmpty) {
                  _loadAlerts(page: 1);
                }
              },
              child: Text(_alertsExpanded ? 'Ocultar' : 'Carregar'),
            ),
            child: !_alertsExpanded
                ? Text(
                    'Expanda para ver o histórico de incidentes deste monitor.',
                    style: context.textTheme.bodySmall?.copyWith(
                      color: context.colorScheme.onSurfaceVariant,
                    ),
                  )
                : _alertsLoading && _monitorAlerts.isEmpty
                ? const LoadingStateView(message: 'Carregando alertas...')
                : _monitorAlerts.isEmpty
                ? const EmptyStateView(
                    title: 'Nenhum alerta registrado',
                    icon: Icons.check_circle_outline,
                    tone: SemanticTone.success,
                  )
                : Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      for (final alert in _monitorAlerts) ...<Widget>[
                        ListTile(
                          dense: true,
                          contentPadding: EdgeInsets.zero,
                          title: Text(
                            alert.title,
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(fontWeight: FontWeight.w700),
                          ),
                          subtitle: Text(
                            '${formatDateTime(alert.startedAt ?? alert.createdAt)} · ${alert.message}',
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                          ),
                          trailing: auth.canWrite && !alert.isResolved
                              ? Row(
                                  mainAxisSize: MainAxisSize.min,
                                  children: <Widget>[
                                    IconButton(
                                      tooltip: 'Reconhecer',
                                      icon: const Icon(
                                        Icons.done_all,
                                        size: 18,
                                      ),
                                      onPressed: () async {
                                        await ref
                                            .read(alertsProvider.notifier)
                                            .acknowledgeAlert(alert.id);
                                        await _loadAlerts(page: 1);
                                      },
                                    ),
                                    IconButton(
                                      tooltip: 'Silenciar',
                                      icon: const Icon(
                                        Icons.notifications_off_outlined,
                                        size: 18,
                                      ),
                                      onPressed: () async {
                                        final ok = await showAlertSilenceSheet(
                                          context: context,
                                          ref: ref,
                                          alert: alert,
                                        );
                                        if (ok) await _loadAlerts(page: 1);
                                      },
                                    ),
                                  ],
                                )
                              : StatusIndicator.fromStatus(
                                  alert.status,
                                  compact: true,
                                ),
                        ),
                        const Divider(height: 1),
                      ],
                      if (_alertsMeta?.hasNext == true) ...<Widget>[
                        const SizedBox(height: 8),
                        OutlinedButton(
                          onPressed: _alertsLoading
                              ? null
                              : () => _loadAlerts(
                                  page: _alertsMeta!.currentPage + 1,
                                  append: true,
                                ),
                          child: const Text('Carregar mais'),
                        ),
                      ],
                    ],
                  ),
          ),
          const SizedBox(height: 24),
        ],
      ),
    );
  }
}
