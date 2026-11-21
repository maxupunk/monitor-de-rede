import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/alert_models.dart';
import 'package:netmonitor_app/features/core/models/dashboard_models.dart';
import 'package:netmonitor_app/features/core/models/event_models.dart';
import 'package:netmonitor_app/features/core/providers/alerts_provider.dart';
import 'package:netmonitor_app/features/core/providers/events_provider.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Os 11 widgets do Núcleo registrados em `coreArea.widgets` (ARQ-04).
Map<String, DashboardWidgetContribution> buildCoreDashboardWidgets() {
  return <String, DashboardWidgetContribution>{
    'stat_cards': DashboardWidgetContribution(
      type: 'stat_cards',
      builder: (_, config) => const _StatCardsWidget(),
    ),
    'health_gauge': DashboardWidgetContribution(
      type: 'health_gauge',
      builder: (_, config) => const _HealthGaugeWidget(),
    ),
    'active_alerts': DashboardWidgetContribution(
      type: 'active_alerts',
      builder: (_, config) => const _ActiveAlertsWidget(),
    ),
    'latency_time_series': DashboardWidgetContribution(
      type: 'latency_time_series',
      builder: (_, config) => _LatencyTimeSeriesWidget(config: config),
    ),
    'event_distribution': DashboardWidgetContribution(
      type: 'event_distribution',
      builder: (_, config) => _EventDistributionWidget(config: config),
    ),
    'unstable_targets': DashboardWidgetContribution(
      type: 'unstable_targets',
      builder: (_, config) => const _UnstableTargetsWidget(),
    ),
    'events_feed': DashboardWidgetContribution(
      type: 'events_feed',
      builder: (_, config) => const _EventsFeedWidget(),
    ),
    'network_monitors': DashboardWidgetContribution(
      type: 'network_monitors',
      builder: (_, config) => const _NetworkMonitorsWidget(),
    ),
    'cpu_usage': DashboardWidgetContribution(
      type: 'cpu_usage',
      builder: (_, config) =>
          _ResourceUsageWidget(metric: 'cpu', config: config),
    ),
    'ram_usage': DashboardWidgetContribution(
      type: 'ram_usage',
      builder: (_, config) =>
          _ResourceUsageWidget(metric: 'ram', config: config),
    ),
    'binary_status': DashboardWidgetContribution(
      type: 'binary_status',
      builder: (_, config) => _BinaryStatusWidget(config: config),
    ),
  };
}

// 1. StatCardsWidget
class _StatCardsWidget extends ConsumerWidget {
  const _StatCardsWidget();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final monitors = ref.watch(monitorsProvider);
    final alerts = ref.watch(alertsProvider);
    final totalDevices = monitors.devices.length;
    final onlineDevices = monitors.devices
        .where((d) => (d.status ?? '').toLowerCase() == 'online')
        .length;

    return ResponsiveGrid(
      xs: 1,
      sm: 2,
      md: 4,
      lg: 4,
      xl: 4,
      children: <Widget>[
        KpiCard(
          label: 'Dispositivos',
          value: formatInteger(totalDevices),
          subtitle: '$onlineDevices online',
          icon: Icons.devices_outlined,
          tone: SemanticTone.primary,
          onTap: () => context.go('/devices'),
        ),
        KpiCard(
          label: 'Monitores ativos',
          value: formatInteger(monitors.upCount),
          subtitle: 'de ${formatInteger(monitors.totalCount)} monitorados',
          icon: Icons.monitor_heart_outlined,
          tone: SemanticTone.success,
          onTap: () => context.go('/monitors?status=up'),
        ),
        KpiCard(
          label: 'Indisponíveis',
          value: formatInteger(monitors.downCount),
          subtitle: monitors.warningCount > 0
              ? '${monitors.warningCount} em atenção'
              : 'Sem degradação',
          icon: Icons.error_outline,
          tone: monitors.downCount > 0 ? SemanticTone.error : SemanticTone.info,
          onTap: () => context.go('/monitors?status=down'),
        ),
        KpiCard(
          label: 'Alertas abertos',
          value: formatInteger(alerts.activeAlerts.length),
          subtitle: alerts.criticalCount > 0
              ? '${alerts.criticalCount} críticos'
              : 'Operação estável',
          icon: Icons.notifications_active_outlined,
          tone: alerts.criticalCount > 0
              ? SemanticTone.error
              : (alerts.activeAlerts.isNotEmpty
                    ? SemanticTone.warning
                    : SemanticTone.success),
          onTap: () => context.go('/alerts'),
        ),
      ],
    );
  }
}

// 2. HealthGaugeWidget
class _HealthGaugeWidget extends ConsumerWidget {
  const _HealthGaugeWidget();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final monitors = ref.watch(monitorsProvider);
    final active = monitors.monitors
        .where((m) => m.isEnabled)
        .toList(growable: false);
    final total = active.length;
    final up = active.where((m) => m.normalizedStatus == 'up').length;
    final warning = active.where((m) => m.normalizedStatus == 'warning').length;
    final down = active.where((m) => m.normalizedStatus == 'down').length;
    final unknown = (total - up - warning - down).clamp(0, total);
    final healthPct = total > 0 ? (up / total) * 100.0 : 100.0;

    final scheme = context.colorScheme;
    final sem = context.semanticColors;
    final healthTone = healthPct >= 95
        ? SemanticTone.success
        : (healthPct >= 80 ? SemanticTone.warning : SemanticTone.error);

    return SectionCard(
      title: 'Saúde geral',
      subtitle: total > 0
          ? '$up de $total monitores ativos saudáveis'
          : 'Nenhum monitor ativo cadastrado',
      icon: Icons.favorite_outline,
      iconTone: healthTone,
      trailing: SemanticChip(
        label: formatPercent(healthPct, digits: 1),
        tone: healthTone,
        filled: true,
        compact: true,
      ),
      child: total == 0
          ? const EmptyStateView(
              title: 'Sem monitores ativos',
              description: 'Crie monitores para acompanhar a saúde da rede.',
              icon: Icons.monitor_heart_outlined,
            )
          : Row(
              children: <Widget>[
                SizedBox(
                  width: 140,
                  height: 140,
                  child: Stack(
                    alignment: Alignment.center,
                    children: <Widget>[
                      PieChart(
                        PieChartData(
                          sectionsSpace: 2,
                          centerSpaceRadius: 42,
                          sections: <PieChartSectionData>[
                            if (up > 0)
                              PieChartSectionData(
                                value: up.toDouble(),
                                color: sem.success,
                                radius: 18,
                                showTitle: false,
                              ),
                            if (warning > 0)
                              PieChartSectionData(
                                value: warning.toDouble(),
                                color: sem.warning,
                                radius: 18,
                                showTitle: false,
                              ),
                            if (down > 0)
                              PieChartSectionData(
                                value: down.toDouble(),
                                color: sem.error,
                                radius: 18,
                                showTitle: false,
                              ),
                            if (unknown > 0)
                              PieChartSectionData(
                                value: unknown.toDouble(),
                                color: scheme.outline,
                                radius: 18,
                                showTitle: false,
                              ),
                          ],
                        ),
                      ),
                      Column(
                        mainAxisSize: MainAxisSize.min,
                        children: <Widget>[
                          Text(
                            formatPercent(healthPct, digits: 0),
                            style: context.textTheme.titleLarge?.copyWith(
                              fontWeight: FontWeight.w800,
                              color: scheme.onSurface,
                            ),
                          ),
                          Text(
                            'saúde',
                            style: context.textTheme.labelSmall?.copyWith(
                              color: scheme.onSurfaceVariant,
                            ),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 16),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      _LegendRow(
                        label: 'Online',
                        count: up,
                        tone: SemanticTone.success,
                      ),
                      const SizedBox(height: 8),
                      _LegendRow(
                        label: 'Atenção',
                        count: warning,
                        tone: SemanticTone.warning,
                      ),
                      const SizedBox(height: 8),
                      _LegendRow(
                        label: 'Indisponível',
                        count: down,
                        tone: SemanticTone.error,
                      ),
                      if (unknown > 0) ...<Widget>[
                        const SizedBox(height: 8),
                        _LegendRow(
                          label: 'Desconhecido',
                          count: unknown,
                          tone: SemanticTone.secondary,
                        ),
                      ],
                    ],
                  ),
                ),
              ],
            ),
    );
  }
}

class _LegendRow extends StatelessWidget {
  const _LegendRow({
    required this.label,
    required this.count,
    required this.tone,
  });

  final String label;
  final int count;
  final SemanticTone tone;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: <Widget>[
        Expanded(
          child: StatusIndicator(label: label, tone: tone, compact: true),
        ),
        Text(
          formatInteger(count),
          style: context.textTheme.titleSmall?.copyWith(
            fontWeight: FontWeight.w700,
          ),
        ),
      ],
    );
  }
}

// 3. ActiveAlertsWidget
class _ActiveAlertsWidget extends ConsumerWidget {
  const _ActiveAlertsWidget();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final alerts = ref.watch(alertsProvider);
    final auth = ref.watch(authProvider);
    final active = alerts.activeAlerts.take(6).toList(growable: false);

    return SectionCard(
      title: 'Alertas ativos',
      subtitle: active.isEmpty
          ? 'Nenhum incidente aberto no momento'
          : '${alerts.activeAlerts.length} incidentes requerem atenção',
      icon: Icons.notifications_active_outlined,
      iconTone: alerts.criticalCount > 0
          ? SemanticTone.error
          : (active.isNotEmpty ? SemanticTone.warning : SemanticTone.success),
      trailing: TextButton(
        onPressed: () => context.go('/alerts'),
        child: const Text('Ver todos'),
      ),
      child: active.isEmpty
          ? const EmptyStateView(
              title: 'Tudo tranquilo',
              description: 'Sem alertas ativos agora.',
              icon: Icons.check_circle_outline,
              tone: SemanticTone.success,
            )
          : Column(
              children: <Widget>[
                for (final alert in active) ...<Widget>[
                  NestedPanel(
                    padding: const EdgeInsets.all(10),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: <Widget>[
                              Wrap(
                                spacing: 6,
                                runSpacing: 4,
                                crossAxisAlignment: WrapCrossAlignment.center,
                                children: <Widget>[
                                  SemanticChip(
                                    label: alert.severity.toUpperCase(),
                                    tone:
                                        alert.severity == 'critical' ||
                                            alert.severity == 'error'
                                        ? SemanticTone.error
                                        : (alert.severity == 'warning'
                                              ? SemanticTone.warning
                                              : SemanticTone.info),
                                    compact: true,
                                  ),
                                  Text(
                                    alert.title,
                                    style: context.textTheme.titleSmall
                                        ?.copyWith(fontWeight: FontWeight.w700),
                                  ),
                                ],
                              ),
                              if (alert.message.isNotEmpty) ...<Widget>[
                                const SizedBox(height: 4),
                                Text(
                                  alert.message,
                                  maxLines: 2,
                                  overflow: TextOverflow.ellipsis,
                                  style: context.textTheme.bodySmall?.copyWith(
                                    color: context.colorScheme.onSurfaceVariant,
                                  ),
                                ),
                              ],
                              const SizedBox(height: 4),
                              Text(
                                formatRelativeTime(
                                  alert.startedAt ?? alert.createdAt,
                                ),
                                style: context.textTheme.labelSmall?.copyWith(
                                  color: context.colorScheme.onSurfaceVariant,
                                ),
                              ),
                            ],
                          ),
                        ),
                        if (auth.canWrite && alert.status == 'active')
                          IconButton(
                            tooltip: 'Reconhecer alerta',
                            icon: const Icon(Icons.done_all, size: 20),
                            onPressed: () => ref
                                .read(alertsProvider.notifier)
                                .acknowledgeAlert(alert.id),
                          ),
                      ],
                    ),
                  ),
                  const SizedBox(height: 8),
                ],
              ],
            ),
    );
  }
}

// 4. LatencyTimeSeriesWidget
class _LatencyTimeSeriesWidget extends ConsumerStatefulWidget {
  const _LatencyTimeSeriesWidget({required this.config});

  final Map<String, dynamic> config;

  @override
  ConsumerState<_LatencyTimeSeriesWidget> createState() =>
      _LatencyTimeSeriesWidgetState();
}

class _LatencyTimeSeriesWidgetState
    extends ConsumerState<_LatencyTimeSeriesWidget> {
  String _timeframe = '1h';
  MonitorTimeSeriesResponse? _data;
  bool _loading = true;
  VoidCallback? _unsub;

  @override
  void initState() {
    super.initState();
    _timeframe = (widget.config['timeframe'] ?? '1h').toString();
    _load();
    _unsub = ref.read(sseServiceProvider.notifier).onEvent(
      <String>['monitor:result'],
      (_, _) {
        if (mounted) _load(silent: true);
      },
    );
  }

  @override
  void dispose() {
    _unsub?.call();
    super.dispose();
  }

  Future<void> _load({bool silent = false}) async {
    if (!silent) setState(() => _loading = true);
    final rawMonitorId = widget.config['monitorId'];
    final monitorId = rawMonitorId is num ? rawMonitorId.toInt() : null;
    final res = await ref
        .read(monitorsProvider.notifier)
        .fetchTimeSeries(monitorId: monitorId, timeframe: _timeframe);
    if (!mounted) return;
    setState(() {
      _data = res;
      _loading = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    final samples = _data?.samples ?? const <MonitorTimeSeriesPoint>[];
    final points = <MetricSeriesPoint>[
      for (var i = 0; i < samples.length; i++)
        MetricSeriesPoint(
          x: i.toDouble(),
          y: samples[i].latency,
          label: samples[i].time,
        ),
    ];

    return SectionCard(
      title: 'Latência média',
      subtitle: _data != null
          ? 'Média ${formatLatency(_data!.avgLatency)} · Perda ${formatPercent(_data!.packetLossPct)}'
          : 'Tendência de resposta dos monitores',
      icon: Icons.show_chart,
      trailing: SegmentedButton<String>(
        showSelectedIcon: false,
        style: const ButtonStyle(visualDensity: VisualDensity.compact),
        segments: const <ButtonSegment<String>>[
          ButtonSegment<String>(value: '15m', label: Text('15m')),
          ButtonSegment<String>(value: '1h', label: Text('1h')),
          ButtonSegment<String>(value: '24h', label: Text('24h')),
        ],
        selected: <String>{_timeframe},
        onSelectionChanged: (next) {
          setState(() => _timeframe = next.first);
          _load();
        },
      ),
      child: _loading && _data == null
          ? const SizedBox(
              height: 180,
              child: LoadingStateView(message: 'Carregando série...'),
            )
          : MetricLineChart(
              height: 190,
              series: <MetricLineSeries>[
                MetricLineSeries(
                  name: 'Latência',
                  points: points,
                  tone: SemanticTone.primary,
                ),
              ],
              yFormatter: (v) => formatLatency(v),
              xFormatter: (x) {
                final idx = x.round();
                if (idx < 0 || idx >= samples.length) return '';
                if (samples.length > 6 && idx % (samples.length ~/ 4) != 0) {
                  return '';
                }
                return samples[idx].time;
              },
            ),
    );
  }
}

// 5. EventDistributionWidget
class _EventDistributionWidget extends ConsumerStatefulWidget {
  const _EventDistributionWidget({required this.config});

  final Map<String, dynamic> config;

  @override
  ConsumerState<_EventDistributionWidget> createState() =>
      _EventDistributionWidgetState();
}

class _EventDistributionWidgetState
    extends ConsumerState<_EventDistributionWidget> {
  List<HourlyEventBucket> _buckets = const <HourlyEventBucket>[];
  bool _loading = true;
  VoidCallback? _unsub;

  @override
  void initState() {
    super.initState();
    _load();
    _unsub = ref.read(sseServiceProvider.notifier).onEvent(
      <String>['alert:triggered', 'alert:updated', 'alert:resolved'],
      (_, _) {
        if (mounted) _load(silent: true);
      },
    );
  }

  @override
  void dispose() {
    _unsub?.call();
    super.dispose();
  }

  Future<void> _load({bool silent = false}) async {
    if (!silent) setState(() => _loading = true);
    final buckets = await ref
        .read(eventsHistoryProvider.notifier)
        .fetchHourlyDistribution(hours: 24);
    if (!mounted) return;
    setState(() {
      _buckets = buckets;
      _loading = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final sem = context.semanticColors;

    return SectionCard(
      title: 'Distribuição de eventos (24h)',
      subtitle: 'Volume por hora nas últimas 24 horas',
      icon: Icons.bar_chart,
      iconTone: SemanticTone.info,
      child: _loading && _buckets.isEmpty
          ? const SizedBox(
              height: 180,
              child: LoadingStateView(message: 'Carregando...'),
            )
          : _buckets.isEmpty
          ? const EmptyStateView(
              title: 'Sem eventos no período',
              description: 'A distribuição horária aparecerá conforme eventos forem gravados.',
              icon: Icons.bar_chart,
            )
          : SizedBox(
              height: 190,
              child: BarChart(
                BarChartData(
                  gridData: FlGridData(
                    show: true,
                    drawVerticalLine: false,
                    getDrawingHorizontalLine: (_) => FlLine(
                      color: scheme.outlineVariant.withValues(alpha: 0.6),
                      strokeWidth: 1,
                    ),
                  ),
                  borderData: FlBorderData(show: false),
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
                        reservedSize: 32,
                        getTitlesWidget: (v, _) => Text(
                          v.toInt().toString(),
                          style: TextStyle(
                            fontSize: 10,
                            color: scheme.onSurfaceVariant,
                          ),
                        ),
                      ),
                    ),
                    bottomTitles: AxisTitles(
                      sideTitles: SideTitles(
                        showTitles: true,
                        getTitlesWidget: (v, _) {
                          final idx = v.toInt();
                          if (idx < 0 || idx >= _buckets.length) {
                            return const SizedBox.shrink();
                          }
                          if (_buckets.length > 8 && idx % 4 != 0) {
                            return const SizedBox.shrink();
                          }
                          return Padding(
                            padding: const EdgeInsets.only(top: 4),
                            child: Text(
                              _buckets[idx].hour,
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
                  barGroups: <BarChartGroupData>[
                    for (var i = 0; i < _buckets.length; i++)
                      BarChartGroupData(
                        x: i,
                        barRods: <BarChartRodData>[
                          BarChartRodData(
                            toY: _buckets[i].total.toDouble(),
                            color: _buckets[i].critical > 0
                                ? sem.error
                                : (_buckets[i].warning > 0
                                      ? sem.warning
                                      : sem.info),
                            width: 8,
                            borderRadius: const BorderRadius.vertical(
                              top: Radius.circular(4),
                            ),
                          ),
                        ],
                      ),
                  ],
                ),
              ),
            ),
    );
  }
}

// 6. UnstableTargetsWidget
class _UnstableTargetsWidget extends ConsumerStatefulWidget {
  const _UnstableTargetsWidget();

  @override
  ConsumerState<_UnstableTargetsWidget> createState() =>
      _UnstableTargetsWidgetState();
}

class _UnstableTargetsWidgetState
    extends ConsumerState<_UnstableTargetsWidget> {
  List<ScopeInstability> _items = const <ScopeInstability>[];
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final res = await ref
        .read(alertsProvider.notifier)
        .fetchInstability(hours: 24, limit: 6);
    if (!mounted) return;
    setState(() {
      _items = res;
      _loading = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    return SectionCard(
      title: 'Alvos instáveis (24h)',
      subtitle: 'Equipamentos ou monitores com maior oscilação',
      icon: Icons.waves,
      iconTone: _items.any((i) => i.flapping)
          ? SemanticTone.warning
          : SemanticTone.info,
      child: _loading
          ? const LoadingStateView(message: 'Analisando oscilações...')
          : _items.isEmpty
          ? const EmptyStateView(
              title: 'Nenhuma oscilação detectada',
              description:
                  'Todos os alvos permaneceram estáveis nas últimas 24h.',
              icon: Icons.verified_outlined,
              tone: SemanticTone.success,
            )
          : Column(
              children: <Widget>[
                for (final item in _items) ...<Widget>[
                  ListTile(
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    title: Text(
                      item.scopeKey,
                      style: const TextStyle(fontWeight: FontWeight.w700),
                    ),
                    subtitle: Text(
                      '${countNoun(item.episodes, 'episódio')} · última: ${formatRelativeTime(item.lastProblemAt)}',
                    ),
                    trailing: SemanticChip(
                      label: '${item.oscillations}x',
                      tone: item.flapping
                          ? SemanticTone.error
                          : SemanticTone.warning,
                      compact: true,
                    ),
                  ),
                  const Divider(height: 1),
                ],
              ],
            ),
    );
  }
}

// 7. EventsFeedWidget
class _EventsFeedWidget extends ConsumerWidget {
  const _EventsFeedWidget();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final realtime = ref.watch(sseServiceProvider);
    final recent = realtime.recentEvents.take(8).toList(growable: false);

    return SectionCard(
      title: 'Feed de eventos ao vivo',
      subtitle: realtime.isConnected
          ? 'Conectado via SSE'
          : 'Aguardando conexão SSE...',
      icon: Icons.bolt_outlined,
      iconTone: realtime.isConnected
          ? SemanticTone.success
          : SemanticTone.warning,
      trailing: TextButton(
        onPressed: () => context.go('/events'),
        child: const Text('Abrir feed'),
      ),
      child: recent.isEmpty
          ? const EmptyStateView(
              title: 'Aguardando eventos',
              description: 'Novos eventos em tempo real aparecerão aqui automaticamente.',
              icon: Icons.history,
            )
          : Column(
              children: <Widget>[
                for (final ev in recent) ...<Widget>[
                  Builder(
                    builder: (context) {
                      final p = EventPresentation.describe(ev.type, ev.data);
                      return ListTile(
                        dense: true,
                        contentPadding: EdgeInsets.zero,
                        leading: Icon(
                          p.icon,
                          size: 20,
                          color: context.semanticColors
                              .resolve(p.tone, context.colorScheme)
                              .foreground,
                        ),
                        title: Text(
                          p.label,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(fontWeight: FontWeight.w600),
                        ),
                        subtitle: Text(
                          p.summary,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                        trailing: Text(
                          formatClockTime(ev.timestamp),
                          style: context.textTheme.labelSmall?.copyWith(
                            color: context.colorScheme.onSurfaceVariant,
                          ),
                        ),
                      );
                    },
                  ),
                  const Divider(height: 1),
                ],
              ],
            ),
    );
  }
}

// 8. NetworkMonitorsWidget
class _NetworkMonitorsWidget extends ConsumerWidget {
  const _NetworkMonitorsWidget();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final monitors = ref.watch(monitorsProvider);
    final auth = ref.watch(authProvider);
    final preview = monitors.monitors.take(8).toList(growable: false);

    return SectionCard(
      title: 'Monitores da rede',
      subtitle: '${monitors.upCount} ativos · ${monitors.downCount} fora do ar',
      icon: Icons.monitor_heart_outlined,
      trailing: TextButton(
        onPressed: () => context.go('/monitors'),
        child: const Text('Gerenciar'),
      ),
      child: preview.isEmpty
          ? const EmptyStateView(
              title: 'Nenhum monitor configurado',
              description: 'Adicione monitores Ping, HTTP, TCP, DNS ou SNMP.',
              icon: Icons.monitor_heart_outlined,
            )
          : Column(
              children: <Widget>[
                for (final m in preview) ...<Widget>[
                  ListTile(
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    onTap: () => context.go('/monitors/${m.id}'),
                    leading: StatusIndicator.fromStatus(
                      m.normalizedStatus,
                      compact: true,
                    ),
                    title: Text(
                      m.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(fontWeight: FontWeight.w700),
                    ),
                    subtitle: Text(
                      '${m.type.toUpperCase()} · ${m.target}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                    trailing: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        Text(
                          formatLatency(m.lastLatencyMs),
                          style: context.textTheme.labelMedium?.copyWith(
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                        if (auth.canWrite)
                          IconButton(
                            tooltip: 'Testar agora',
                            icon: monitors.runningId == m.id
                                ? const SizedBox(
                                    width: 16,
                                    height: 16,
                                    child: CircularProgressIndicator(
                                      strokeWidth: 2,
                                    ),
                                  )
                                : const Icon(
                                    Icons.play_arrow_outlined,
                                    size: 20,
                                  ),
                            onPressed: monitors.runningId == m.id
                                ? null
                                : () => ref
                                      .read(monitorsProvider.notifier)
                                      .runMonitor(m.id),
                          ),
                      ],
                    ),
                  ),
                  const Divider(height: 1),
                ],
              ],
            ),
    );
  }
}

// 9 & 10. ResourceUsageWidget (cpu_usage & ram_usage)
class _ResourceUsageWidget extends ConsumerStatefulWidget {
  const _ResourceUsageWidget({required this.metric, required this.config});

  final String metric; // 'cpu' | 'ram'
  final Map<String, dynamic> config;

  @override
  ConsumerState<_ResourceUsageWidget> createState() =>
      _ResourceUsageWidgetState();
}

class _ResourceUsageWidgetState extends ConsumerState<_ResourceUsageWidget> {
  List<MetricSeriesPoint> _points = const <MetricSeriesPoint>[];
  double? _latestValue;
  bool _loading = false;
  VoidCallback? _unsub;

  int? get _deviceId {
    final raw = widget.config['deviceId'];
    return raw is num ? raw.toInt() : null;
  }

  String get _metricName =>
      widget.metric == 'cpu' ? 'cpu_usage' : 'memory_usage';

  @override
  void initState() {
    super.initState();
    _load();
    _unsub = ref.read(sseServiceProvider.notifier).onEvent(
      <String>['metric:recorded'],
      (data, _) {
        final devId = (data['deviceId'] as num?)?.toInt();
        if (devId == null || (_deviceId != null && devId != _deviceId)) return;
        final rawMetrics = data['metrics'];
        if (rawMetrics is! List) return;
        for (final item in rawMetrics) {
          if (item is Map && item['name'] == _metricName) {
            final val = (item['value'] as num?)?.toDouble();
            if (val != null && mounted) {
              setState(() {
                _latestValue = val;
                final next = <MetricSeriesPoint>[
                  ..._points,
                  MetricSeriesPoint(x: _points.length.toDouble(), y: val),
                ];
                _points = next.length > 30
                    ? next.sublist(next.length - 30)
                    : next;
              });
            }
          }
        }
      },
    );
  }

  @override
  void dispose() {
    _unsub?.call();
    super.dispose();
  }

  Future<void> _load() async {
    final devId = _deviceId;
    if (devId == null) return;
    setState(() => _loading = true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>(
            '/devices/$devId/metrics',
            queryParameters: <String, dynamic>{'name': _metricName},
          );
      final list = raw is List
          ? raw
          : (raw is Map && raw['data'] is List
                ? raw['data'] as List
                : const []);
      final pts = <MetricSeriesPoint>[];
      for (final item in list) {
        if (item is Map) {
          final val = (item['value'] as num?)?.toDouble();
          if (val != null) {
            pts.add(MetricSeriesPoint(x: pts.length.toDouble(), y: val));
          }
        }
      }
      if (!mounted) return;
      setState(() {
        _points = pts;
        _latestValue = pts.isNotEmpty ? pts.last.y : null;
        _loading = false;
      });
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final warn =
        (widget.config['warningThreshold'] as num?)?.toDouble() ?? 75.0;
    final crit =
        (widget.config['criticalThreshold'] as num?)?.toDouble() ?? 90.0;
    final tone = _latestValue == null
        ? SemanticTone.primary
        : (_latestValue! >= crit
              ? SemanticTone.error
              : (_latestValue! >= warn
                    ? SemanticTone.warning
                    : SemanticTone.success));

    return SectionCard(
      title: widget.metric == 'cpu' ? 'Uso de CPU' : 'Uso de memória RAM',
      subtitle: _deviceId == null
          ? 'Selecione um dispositivo na configuração do card'
          : 'Dispositivo #$_deviceId',
      icon: widget.metric == 'cpu' ? Icons.memory : Icons.storage_outlined,
      iconTone: tone,
      trailing: _latestValue != null
          ? SemanticChip(
              label: formatPercent(_latestValue),
              tone: tone,
              filled: true,
              compact: true,
            )
          : null,
      child: _deviceId == null
          ? const EmptyStateView(
              title: 'Dispositivo não configurado',
              description: 'Use o modo Personalizar para escolher o dispositivo monitorado.',
              icon: Icons.tune,
            )
          : _loading
          ? const SizedBox(height: 160, child: LoadingStateView())
          : MetricLineChart(
              height: 170,
              minY: 0,
              maxY: 100,
              series: <MetricLineSeries>[
                MetricLineSeries(
                  name: widget.metric == 'cpu' ? 'CPU' : 'RAM',
                  points: _points,
                  tone: tone,
                ),
              ],
              yFormatter: (v) => formatPercent(v, digits: 0),
            ),
    );
  }
}

// 11. BinaryStatusWidget
class _BinaryStatusWidget extends ConsumerWidget {
  const _BinaryStatusWidget({required this.config});

  final Map<String, dynamic> config;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final monitors = ref.watch(monitorsProvider);
    final auth = ref.watch(authProvider);
    final monitorId = (config['monitorId'] as num?)?.toInt();
    final monitor = monitorId != null
        ? monitors.monitors.cast<dynamic>().firstWhere(
            (m) => m.id == monitorId,
            orElse: () => null,
          )
        : null;

    return SectionCard(
      title: monitor?.name ?? 'Status de monitor',
      subtitle: monitor != null
          ? '${monitor.type.toUpperCase()} · ${monitor.target}'
          : 'Selecione um monitor',
      icon: Icons.power_settings_new,
      iconTone: monitor == null
          ? SemanticTone.secondary
          : (monitor.normalizedStatus == 'up'
                ? SemanticTone.success
                : (monitor.normalizedStatus == 'down'
                      ? SemanticTone.error
                      : SemanticTone.warning)),
      child: monitor == null
          ? const EmptyStateView(
              title: 'Monitor não selecionado',
              description:
                  'Configure este card para acompanhar um monitor específico.',
              icon: Icons.monitor_heart_outlined,
            )
          : Row(
              children: <Widget>[
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      StatusIndicator.fromStatus(monitor.normalizedStatus),
                      const SizedBox(height: 6),
                      Text(
                        'Latência: ${formatLatency(monitor.lastLatencyMs)} · '
                        'Última: ${formatRelativeTime(monitor.lastCheckedAt)}',
                        style: context.textTheme.bodySmall?.copyWith(
                          color: context.colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ),
                ),
                if (auth.canWrite)
                  FilledButton.tonalIcon(
                    onPressed: monitors.runningId == monitor.id
                        ? null
                        : () => ref
                              .read(monitorsProvider.notifier)
                              .runMonitor(monitor.id),
                    icon: const Icon(Icons.play_arrow, size: 18),
                    label: const Text('Testar'),
                  ),
              ],
            ),
    );
  }
}
