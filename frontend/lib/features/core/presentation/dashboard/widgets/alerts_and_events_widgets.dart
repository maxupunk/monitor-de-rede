import 'package:fl_chart/fl_chart.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/alert_models.dart';
import 'package:netmonitor_app/features/core/models/dashboard_models.dart';
import 'package:netmonitor_app/features/core/models/event_models.dart';
import 'package:netmonitor_app/features/core/providers/alerts_provider.dart';
import 'package:netmonitor_app/features/core/providers/events_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// 3. Lista compacta de incidentes ativos (`active_alerts`).
class ActiveAlertsWidget extends ConsumerWidget {
  const ActiveAlertsWidget({super.key});

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
                                    label: switch (alert.severity) {
                                      'critical' || 'error' => 'Crítico',
                                      'warning' => 'Aviso',
                                      _ => 'Info',
                                    },
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

/// 5. Histograma horário de eventos nas últimas 24h (`event_distribution`).
class EventDistributionWidget extends ConsumerStatefulWidget {
  const EventDistributionWidget({super.key, required this.config});

  final Map<String, dynamic> config;

  @override
  ConsumerState<EventDistributionWidget> createState() =>
      _EventDistributionWidgetState();
}

class _EventDistributionWidgetState
    extends ConsumerState<EventDistributionWidget> {
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

  int get _hours {
    return switch ((widget.config['timeframe'] ?? '24h').toString()) {
      '5m' || '15m' || '1h' => 1,
      '6h' => 6,
      '12h' => 12,
      '7d' => 168,
      _ => 24,
    };
  }

  @override
  void didUpdateWidget(covariant EventDistributionWidget oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.config['timeframe'] != widget.config['timeframe']) {
      _load();
    }
  }

  Future<void> _load({bool silent = false}) async {
    if (!silent) setState(() => _loading = true);
    final buckets = await ref
        .read(eventsHistoryProvider.notifier)
        .fetchHourlyDistribution(hours: _hours);
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
              description:
                  'A distribuição horária aparecerá conforme eventos forem gravados.',
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
                          formatInteger(v),
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

/// 6. Alvos com maior oscilação / flapping (`unstable_targets`).
class UnstableTargetsWidget extends ConsumerStatefulWidget {
  const UnstableTargetsWidget({super.key});

  @override
  ConsumerState<UnstableTargetsWidget> createState() =>
      _UnstableTargetsWidgetState();
}

class _UnstableTargetsWidgetState extends ConsumerState<UnstableTargetsWidget> {
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
              children: withDividers(<Widget>[
                for (final item in _items)
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
              ]),
            ),
    );
  }
}

/// 7. Feed em tempo real de eventos SSE (`events_feed`).
class EventsFeedWidget extends ConsumerWidget {
  const EventsFeedWidget({super.key});

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
              description:
                  'Novos eventos em tempo real aparecerão aqui automaticamente.',
              icon: Icons.history,
            )
          : Column(
              children: withDividers(<Widget>[
                for (final ev in recent)
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
              ]),
            ),
    );
  }
}
