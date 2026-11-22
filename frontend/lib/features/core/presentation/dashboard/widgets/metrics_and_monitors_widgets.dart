import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/core/models/dashboard_models.dart';
import 'package:netmonitor_app/features/core/models/monitor_models.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// 4. Série temporal de latência dos monitores (`latency_time_series`).
class LatencyTimeSeriesWidget extends ConsumerStatefulWidget {
  const LatencyTimeSeriesWidget({super.key, required this.config});

  final Map<String, dynamic> config;

  @override
  ConsumerState<LatencyTimeSeriesWidget> createState() =>
      _LatencyTimeSeriesWidgetState();
}

class _LatencyTimeSeriesWidgetState
    extends ConsumerState<LatencyTimeSeriesWidget> {
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
    final monitorId = parseIntOrNull(widget.config['monitorId']);
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
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Align(
            alignment: Alignment.centerRight,
            child: SegmentedButton<String>(
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
          ),
          const SizedBox(height: 10),
          if (_loading && _data == null)
            const SizedBox(
              height: 180,
              child: LoadingStateView(message: 'Carregando série...'),
            )
          else
            MetricLineChart(
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
        ],
      ),
    );
  }
}

/// 8. Lista de monitores da rede com ação rápida (`network_monitors`).
class NetworkMonitorsWidget extends ConsumerWidget {
  const NetworkMonitorsWidget({super.key});

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
              children: withDividers(<Widget>[
                for (final m in preview)
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
              ]),
            ),
    );
  }
}

/// 9 & 10. Uso de CPU ou RAM de dispositivo em tempo real (`cpu_usage` / `ram_usage`).
class ResourceUsageWidget extends ConsumerStatefulWidget {
  const ResourceUsageWidget({
    super.key,
    required this.metric,
    required this.config,
  });

  final String metric; // 'cpu' | 'ram'
  final Map<String, dynamic> config;

  @override
  ConsumerState<ResourceUsageWidget> createState() =>
      _ResourceUsageWidgetState();
}

class _ResourceUsageWidgetState extends ConsumerState<ResourceUsageWidget> {
  List<MetricSeriesPoint> _points = const <MetricSeriesPoint>[];
  double? _latestValue;
  bool _loading = false;
  VoidCallback? _unsub;

  int? get _deviceId => parseIntOrNull(widget.config['deviceId']);

  String get _metricName =>
      widget.metric == 'cpu' ? 'cpu_usage' : 'memory_usage';

  @override
  void initState() {
    super.initState();
    _load();
    _unsub = ref.read(sseServiceProvider.notifier).onEvent(
      <String>['metric:recorded'],
      (data, _) {
        final devId = parseIntOrNull(data['deviceId']);
        if (devId == null || (_deviceId != null && devId != _deviceId)) return;
        for (final item in asJsonMapList(data['metrics'])) {
          if (item['name'] == _metricName) {
            final val = parseDoubleOrNull(item['value']);
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
    final values = await ref
        .read(monitorsProvider.notifier)
        .fetchDeviceMetricSeries(devId, _metricName);
    if (!mounted) return;
    final pts = <MetricSeriesPoint>[
      for (var i = 0; i < values.length; i++)
        MetricSeriesPoint(x: i.toDouble(), y: values[i]),
    ];
    setState(() {
      _points = pts;
      _latestValue = pts.isNotEmpty ? pts.last.y : null;
      _loading = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    final warn = parseDoubleOrNull(widget.config['warningThreshold']) ?? 75.0;
    final crit = parseDoubleOrNull(widget.config['criticalThreshold']) ?? 90.0;
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
              description:
                  'Use o modo Personalizar para escolher o dispositivo monitorado.',
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

/// 11. Indicador binário dedicado para um monitor específico (`binary_status`).
class BinaryStatusWidget extends ConsumerWidget {
  const BinaryStatusWidget({super.key, required this.config});

  final Map<String, dynamic> config;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final monitors = ref.watch(monitorsProvider);
    final auth = ref.watch(authProvider);
    final monitorId = parseIntOrNull(config['monitorId']);
    MonitorModel? monitor;
    if (monitorId != null) {
      for (final m in monitors.monitors) {
        if (m.id == monitorId) {
          monitor = m;
          break;
        }
      }
    }

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
                if (auth.canWrite) ...<Widget>[
                  const SizedBox(width: 8),
                  FilledButton.tonalIcon(
                    onPressed: monitors.runningId == monitor.id
                        ? null
                        : () => ref
                              .read(monitorsProvider.notifier)
                              .runMonitor(monitor!.id),
                    icon: const Icon(Icons.play_arrow, size: 18),
                    label: const Text('Testar'),
                  ),
                ],
              ],
            ),
    );
  }
}
