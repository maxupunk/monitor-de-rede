import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

// ── 1. SaasServicesWidget (`saas_services`) + SaasPresetsDialog ───────────────

class SaasServicesWidget extends ConsumerStatefulWidget {
  const SaasServicesWidget({
    super.key,
    this.config = const <String, dynamic>{},
  });

  final Map<String, dynamic> config;

  @override
  ConsumerState<SaasServicesWidget> createState() => _SaasServicesWidgetState();
}

class _SaasServicesWidgetState extends ConsumerState<SaasServicesWidget> {
  List<DeviceMonitorSummary> _monitors = const <DeviceMonitorSummary>[];
  bool _loading = true;
  VoidCallback? _unsub;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
    _unsub = ref.read(sseServiceProvider.notifier).onEvent(
      const <String>['monitor:result'],
      (data, _) {
        final id = parseIntOrNull(data['monitorId'] ?? data['id']);
        if (id == null || !mounted) return;
        setState(() {
          _monitors = _monitors
              .map((m) {
                if (m.id != id) return m;
                return m.copyWith(
                  status: data['status'] as String?,
                  latencyMs: parseDoubleOrNull(data['latencyMs']),
                  lastCheckedAt: data['finishedAt'] as String?,
                );
              })
              .toList(growable: false);
        });
      },
    );
  }

  @override
  void dispose() {
    _unsub?.call();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    try {
      final raw = await ref.read(apiClientProvider).get<dynamic>('/monitors');
      final all = asJsonMapList(raw);
      final saas = all
          .where(
            (m) =>
                m['isSaas'] == true ||
                (m['tags'] is List && (m['tags'] as List).contains('saas')),
          )
          .map(DeviceMonitorSummary.fromJson)
          .toList(growable: false);
      if (mounted) {
        setState(() {
          _monitors = saas.isNotEmpty
              ? saas
              : all
                    .take(6)
                    .map(DeviceMonitorSummary.fromJson)
                    .toList(growable: false);
          _loading = false;
        });
      }
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return SectionCard(
      title: 'Serviços SaaS e Nuvem',
      subtitle: 'Monitoramento de experiência (QoE) de serviços externos',
      icon: Icons.cloud_done_outlined,
      iconTone: SemanticTone.secondary,
      trailing: FilledButton.tonalIcon(
        onPressed: () async {
          await showSaasPresetsDialog(context);
          _load();
        },
        icon: const Icon(Icons.add, size: 18),
        label: const Text('Catálogo SaaS'),
      ),
      child: _loading
          ? const LoadingStateView(message: 'Carregando serviços SaaS...')
          : _monitors.isEmpty
          ? EmptyStateView(
              title: 'Nenhum serviço SaaS provisionado',
              description: 'Adicione bancos, nuvens e ferramentas de produtividade em um clique.',
              icon: Icons.cloud_outlined,
              action: FilledButton.icon(
                onPressed: () async {
                  await showSaasPresetsDialog(context);
                  _load();
                },
                icon: const Icon(Icons.add),
                label: const Text('Abrir catálogo SaaS'),
              ),
            )
          : Column(
              children: _monitors
                  .map((m) {
                    return ListTile(
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      leading: StatusIndicator.fromStatus(
                        m.status,
                        compact: true,
                      ),
                      title: Text(
                        m.name,
                        style: const TextStyle(fontWeight: FontWeight.w700),
                      ),
                      subtitle: Text(m.target ?? m.type),
                      trailing: Text(
                        formatLatency(m.latencyMs),
                        style: context.textTheme.labelLarge?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    );
                  })
                  .toList(growable: false),
            ),
    );
  }
}

Future<void> showSaasPresetsDialog(BuildContext context) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Catálogo de Serviços SaaS',
    subtitle: 'Provisione monitores prontos por categoria',
    maxWidth: 680,
    builder: (ctx) => _SaasPresetsBody(onDone: () => Navigator.of(ctx).pop()),
  );
}

class _SaasPresetsBody extends ConsumerStatefulWidget {
  const _SaasPresetsBody({required this.onDone});

  final VoidCallback onDone;

  @override
  ConsumerState<_SaasPresetsBody> createState() => _SaasPresetsBodyState();
}

class _SaasPresetsBodyState extends ConsumerState<_SaasPresetsBody> {
  static const List<(String, String)> _categories = <(String, String)>[
    ('all', 'Todos'),
    ('finance', 'Bancos'),
    ('productivity', 'Produtividade'),
    ('cloud', 'Nuvem'),
    ('communication', 'Comunicação'),
    ('developer', 'Dev'),
    ('streaming', 'Streaming'),
    ('government', 'Governo'),
  ];

  List<SaasPresetItem> _presets = const <SaasPresetItem>[];
  final Set<String> _selected = <String>{};
  String _category = 'all';
  String _search = '';
  bool _loading = true;
  bool _provisioning = false;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/monitors/saas/presets');
      final listRaw = raw is Map ? (raw['presets'] ?? raw['items']) : raw;
      if (mounted) {
        setState(() {
          _presets = asJsonMapList(listRaw)
              .map(SaasPresetItem.fromJson)
              .toList(growable: false);
          _loading = false;
        });
      }
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  Future<void> _provisionIds(List<String> ids) async {
    if (ids.isEmpty) return;
    setState(() => _provisioning = true);
    try {
      await ref
          .read(apiClientProvider)
          .post<dynamic>(
            '/monitors/saas/provision',
            data: <String, dynamic>{'presetIds': ids},
          );
      if (mounted) widget.onDone();
    } finally {
      if (mounted) setState(() => _provisioning = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const LoadingStateView(message: 'Carregando catálogo SaaS...');
    }
    final filtered = _presets
        .where((p) {
          if (_category != 'all' && p.category != _category) return false;
          if (_search.isNotEmpty) {
            final q = _search.toLowerCase();
            return p.name.toLowerCase().contains(q) ||
                p.target.toLowerCase().contains(q);
          }
          return true;
        })
        .toList(growable: false);

    final recommended = _presets
        .where((p) => p.recommended && !p.alreadyProvisioned)
        .take(6)
        .map((p) => p.id)
        .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          decoration: const InputDecoration(
            hintText: 'Buscar serviço SaaS...',
            prefixIcon: Icon(Icons.search),
          ),
          onChanged: (v) => setState(() => _search = v),
        ),
        const SizedBox(height: 8),
        SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Row(
            children: _categories
                .map((cat) {
                  return Padding(
                    padding: const EdgeInsets.only(right: 6),
                    child: ChoiceChip(
                      label: Text(cat.$2),
                      selected: _category == cat.$1,
                      onSelected: (_) => setState(() => _category = cat.$1),
                    ),
                  );
                })
                .toList(growable: false),
          ),
        ),
        const SizedBox(height: 10),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            if (recommended.isNotEmpty)
              OutlinedButton.icon(
                onPressed: _provisioning
                    ? null
                    : () => _provisionIds(recommended),
                icon: const Icon(Icons.auto_awesome, size: 18),
                label: Text('Provisionar Recomendados (${recommended.length})'),
              ),
            FilledButton.icon(
              onPressed: _provisioning || _selected.isEmpty
                  ? null
                  : () => _provisionIds(_selected.toList(growable: false)),
              icon: const Icon(Icons.check, size: 18),
              label: Text('Provisionar selecionados (${_selected.length})'),
            ),
          ],
        ),
        const SizedBox(height: 10),
        for (final preset in filtered)
          CheckboxListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            title: Text(preset.name),
            subtitle: Text('${preset.target} · ${preset.category}'),
            secondary: preset.alreadyProvisioned
                ? const SemanticChip(
                    label: 'Ativo',
                    tone: SemanticTone.success,
                    compact: true,
                  )
                : null,
            value: preset.alreadyProvisioned || _selected.contains(preset.id),
            onChanged: preset.alreadyProvisioned
                ? null
                : (v) {
                    setState(() {
                      if (v == true) {
                        _selected.add(preset.id);
                      } else {
                        _selected.remove(preset.id);
                      }
                    });
                  },
          ),
      ],
    );
  }
}

// ── 2. SaasLatencyHeatmapWidget (`saas_heatmap`) ──────────────────────────────

class SaasLatencyHeatmapWidget extends ConsumerStatefulWidget {
  const SaasLatencyHeatmapWidget({
    super.key,
    this.config = const <String, dynamic>{},
  });

  final Map<String, dynamic> config;

  @override
  ConsumerState<SaasLatencyHeatmapWidget> createState() =>
      _SaasLatencyHeatmapWidgetState();
}

class _SaasLatencyHeatmapWidgetState
    extends ConsumerState<SaasLatencyHeatmapWidget> {
  int _days = 7;
  bool _loading = true;
  List<HeatmapCell> _cells = const <HeatmapCell>[];

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    try {
      final monitorId = parseIntOrNull(widget.config['monitorId']);
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>(
            '/monitors/hourly-heatmap',
            queryParameters: <String, dynamic>{
              if (monitorId != null) 'monitorId': monitorId else 'isSaas': true,
              'days': _days,
            },
          );
      final map = asJsonMap(raw);
      final cellsRaw = map['cells'] ?? map['buckets'] ?? map['data'] ?? raw;
      if (mounted) {
        setState(() {
          _cells = asJsonMapList(cellsRaw)
              .map(HeatmapCell.fromJson)
              .toList(growable: false);
          _loading = false;
        });
      }
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  Color _cellColor(BuildContext context, HeatmapCell? cell) {
    final scheme = context.colorScheme;
    final sem = context.semanticColors;
    if (cell == null || cell.checks == 0) return scheme.surfaceContainerHighest;
    if (cell.downChecks > 0) return sem.error;
    final ms = cell.avgLatencyMs ?? 0;
    if (ms < 50) return sem.success;
    if (ms < 150) return sem.info;
    if (ms < 300) return sem.warning;
    return sem.caution;
  }

  @override
  Widget build(BuildContext context) {
    final days = _cells.map((c) => c.day).toSet().toList()..sort();
    final lookup = <String, HeatmapCell>{
      for (final c in _cells) '${c.day}:${c.hour}': c,
    };

    return SectionCard(
      title: 'Mapa de Calor · Latência por Hora',
      subtitle: 'Média horária dos últimos $_days dias',
      icon: Icons.grid_on_outlined,
      iconTone: SemanticTone.warning,
      trailing: SegmentedButton<int>(
        segments: const <ButtonSegment<int>>[
          ButtonSegment(value: 7, label: Text('7d')),
          ButtonSegment(value: 14, label: Text('14d')),
        ],
        selected: <int>{_days},
        onSelectionChanged: (s) {
          setState(() => _days = s.first);
          _load();
        },
      ),
      child: _loading
          ? const LoadingStateView(message: 'Gerando mapa de calor...')
          : days.isEmpty
          ? const EmptyStateView(
              title: 'Sem histórico horário suficiente',
              icon: Icons.grid_off_outlined,
            )
          : SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Row(
                    children: <Widget>[
                      const SizedBox(width: 72),
                      for (var h = 0; h < 24; h += 3)
                        SizedBox(
                          width: 48,
                          child: Text(
                            '${h.toString().padLeft(2, '0')}h',
                            style: context.textTheme.labelSmall,
                          ),
                        ),
                    ],
                  ),
                  const SizedBox(height: 4),
                  for (final day in days)
                    Padding(
                      padding: const EdgeInsets.only(bottom: 4),
                      child: Row(
                        children: <Widget>[
                          SizedBox(
                            width: 72,
                            child: Text(
                              day.length >= 10 ? day.substring(5, 10) : day,
                              style: context.textTheme.labelSmall,
                            ),
                          ),
                          for (var h = 0; h < 24; h++)
                            Tooltip(
                              message: () {
                                final c = lookup['$day:$h'];
                                if (c == null || c.checks == 0) {
                                  return '$day ${formatHourOfDay(h)}: sem medições';
                                }
                                return '$day ${formatHourOfDay(h)} · Média ${formatLatency(c.avgLatencyMs)}'
                                    '${c.p95LatencyMs != null ? ' · P95 ${formatLatency(c.p95LatencyMs)}' : ''}'
                                    ' (${c.checks} checks)';
                              }(),
                              child: Container(
                                width: 14,
                                height: 14,
                                margin: const EdgeInsets.only(right: 2),
                                decoration: BoxDecoration(
                                  color: _cellColor(context, lookup['$day:$h']),
                                  borderRadius: BorderRadius.circular(3),
                                ),
                              ),
                            ),
                        ],
                      ),
                    ),
                ],
              ),
            ),
    );
  }
}

// ── 3. EtherBandwidthWidget (`ether_bandwidth`) ───────────────────────────────

class EtherBandwidthWidget extends ConsumerStatefulWidget {
  const EtherBandwidthWidget({
    super.key,
    this.config = const <String, dynamic>{},
  });

  final Map<String, dynamic> config;

  @override
  ConsumerState<EtherBandwidthWidget> createState() =>
      _EtherBandwidthWidgetState();
}

class _EtherBandwidthWidgetState extends ConsumerState<EtherBandwidthWidget> {
  int? _deviceId;
  String? _interfaceName;
  final List<MetricSeriesPoint> _inPoints = <MetricSeriesPoint>[];
  final List<MetricSeriesPoint> _outPoints = <MetricSeriesPoint>[];
  bool _loading = true;
  VoidCallback? _unsub;

  @override
  void initState() {
    super.initState();
    _deviceId = parseIntOrNull(widget.config['deviceId']);
    _interfaceName = widget.config['interfaceName'] as String?;
    Future<void>.microtask(_load);
    _unsub = ref.read(sseServiceProvider.notifier).onEvent(const <String>[
      'metric:recorded',
      'interface:traffic',
    ], (data, _) => _onLiveEvent(data));
  }

  @override
  void dispose() {
    _unsub?.call();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    try {
      if (_deviceId == null) {
        await ref.read(devicesProvider.notifier).fetchDevices();
        final devs = ref.read(devicesProvider).items;
        final snmpDev =
            devs.where((d) => d.snmpEnabled).firstOrNull ?? devs.firstOrNull;
        _deviceId = snmpDev?.id;
      }
      if (_deviceId != null) {
        final raw = await ref
            .read(apiClientProvider)
            .get<dynamic>('/devices/$_deviceId/metrics');
        final listRaw = raw is Map ? raw['data'] : raw;
        final metrics = asJsonMapList(listRaw)
            .map(DeviceMetricModel.fromJson)
            .toList();
        _inPoints.clear();
        _outPoints.clear();
        for (final m in metrics) {
          if (_interfaceName != null &&
              m.interfaceName != null &&
              m.interfaceName != _interfaceName) {
            continue;
          }
          if (m.metricName == 'inBps' || m.metricName == 'interface_in_bps') {
            _inPoints.add(
              MetricSeriesPoint(
                x: _inPoints.length.toDouble(),
                y: m.metricValue,
              ),
            );
          } else if (m.metricName == 'outBps' ||
              m.metricName == 'interface_out_bps') {
            _outPoints.add(
              MetricSeriesPoint(
                x: _outPoints.length.toDouble(),
                y: m.metricValue,
              ),
            );
          }
        }
      }
    } catch (_) {}
    if (mounted) setState(() => _loading = false);
  }

  void _onLiveEvent(Map<String, dynamic> data) {
    if (_deviceId == null ||
        parseIntOrNull(data['deviceId']) != _deviceId ||
        !mounted) {
      return;
    }
    final metrics = asJsonMapList(data['metrics']);
    var changed = false;
    for (final m in metrics) {
      final name = m['name']?.toString();
      final val = parseDoubleOrNull(m['value']);
      if (val == null) continue;
      if (name == 'inBps' || name == 'interface_in_bps') {
        _inPoints.add(
          MetricSeriesPoint(x: _inPoints.length.toDouble(), y: val),
        );
        if (_inPoints.length > 60) _inPoints.removeAt(0);
        changed = true;
      } else if (name == 'outBps' || name == 'interface_out_bps') {
        _outPoints.add(
          MetricSeriesPoint(x: _outPoints.length.toDouble(), y: val),
        );
        if (_outPoints.length > 60) _outPoints.removeAt(0);
        changed = true;
      }
    }
    if (changed) setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final lastIn = _inPoints.isNotEmpty ? _inPoints.last.y : null;
    final lastOut = _outPoints.isNotEmpty ? _outPoints.last.y : null;

    return SectionCard(
      title: 'Largura de Banda da Interface',
      subtitle:
          _interfaceName ??
          (_deviceId != null ? 'Dispositivo #$_deviceId' : 'Ao vivo via SSE'),
      icon: Icons.swap_vert,
      iconTone: SemanticTone.primary,
      trailing: Wrap(
        spacing: 6,
        children: <Widget>[
          if (lastIn != null)
            SemanticChip(
              label: 'RX ${formatBps(lastIn)}',
              tone: SemanticTone.success,
              compact: true,
            ),
          if (lastOut != null)
            SemanticChip(
              label: 'TX ${formatBps(lastOut)}',
              tone: SemanticTone.info,
              compact: true,
            ),
        ],
      ),
      child: _loading
          ? const LoadingStateView(message: 'Carregando tráfego...')
          : MetricLineChart(
              height: 190,
              yFormatter: (v) => formatBps(v),
              series: <MetricLineSeries>[
                MetricLineSeries(
                  name: 'Download (RX)',
                  points: _inPoints,
                  tone: SemanticTone.success,
                ),
                MetricLineSeries(
                  name: 'Upload (TX)',
                  points: _outPoints,
                  tone: SemanticTone.info,
                ),
              ],
            ),
    );
  }
}

// ── 4. BandwidthVsLatencyWidget (`bandwidth_vs_latency`) ──────────────────────

class BandwidthVsLatencyWidget extends ConsumerStatefulWidget {
  const BandwidthVsLatencyWidget({
    super.key,
    this.config = const <String, dynamic>{},
  });

  final Map<String, dynamic> config;

  @override
  ConsumerState<BandwidthVsLatencyWidget> createState() =>
      _BandwidthVsLatencyWidgetState();
}

class _BandwidthVsLatencyWidgetState
    extends ConsumerState<BandwidthVsLatencyWidget> {
  String _timeframe = '1h';
  BandwidthLatencyResponse? _data;
  bool _loading = true;
  VoidCallback? _unsub;

  @override
  void initState() {
    super.initState();
    _timeframe = (widget.config['timeframe'] as String?) ?? '1h';
    Future<void>.microtask(_load);
    _unsub = ref.read(sseServiceProvider.notifier).onEvent(
      const <String>['metric:recorded', 'monitor:result'],
      (data, _) {
        if (!mounted || _data == null) return;
        final lat = parseDoubleOrNull(data['latencyMs']);
        if (lat != null) {
          final nextSamples = <BandwidthLatencyPoint>[
            ..._data!.samples,
            BandwidthLatencyPoint(
              time: formatClockTime(),
              timestamp: DateTime.now().millisecondsSinceEpoch,
              bwBps: _data!.currentBw,
              latency: lat,
            ),
          ];
          if (nextSamples.length > 90) nextSamples.removeAt(0);
          setState(() {
            _data = BandwidthLatencyResponse(
              timeframe: _data!.timeframe,
              samples: nextSamples,
              currentBw: _data!.currentBw,
              peakBw: _data!.peakBw,
              currentLatency: lat,
              avgLatency: _data!.avgLatency,
              correlationScore: _data!.correlationScore,
              hasSaturationCorrelation: _data!.hasSaturationCorrelation,
            );
          });
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
    setState(() => _loading = true);
    final res = await ref
        .read(devicesProvider.notifier)
        .fetchBandwidthLatencySeries(
          deviceId: widget.config['deviceId'],
          pingTarget: widget.config['pingTarget'],
          timeframe: _timeframe,
        );
    if (mounted) {
      setState(() {
        _data = res;
        _loading = false;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final samples = _data?.samples ?? const <BandwidthLatencyPoint>[];
    final bwMbps = <MetricSeriesPoint>[];
    final latMs = <MetricSeriesPoint>[];
    for (var i = 0; i < samples.length; i++) {
      bwMbps.add(MetricSeriesPoint(x: i.toDouble(), y: samples[i].bwBps / 1e6));
      latMs.add(MetricSeriesPoint(x: i.toDouble(), y: samples[i].latency));
    }

    return SectionCard(
      title: 'Banda × Latência (Bufferbloat)',
      subtitle: 'Correlação entre tráfego (Mbps) e latência ICMP (ms)',
      icon: Icons.show_chart,
      iconTone: _data?.hasSaturationCorrelation == true
          ? SemanticTone.warning
          : SemanticTone.info,
      trailing: SegmentedButton<String>(
        segments: const <ButtonSegment<String>>[
          ButtonSegment(value: '15m', label: Text('15m')),
          ButtonSegment(value: '1h', label: Text('1h')),
          ButtonSegment(value: '24h', label: Text('24h')),
        ],
        selected: <String>{_timeframe},
        onSelectionChanged: (s) {
          setState(() => _timeframe = s.first);
          _load();
        },
      ),
      child: _loading
          ? const LoadingStateView(message: 'Analisando correlação...')
          : Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                if (_data != null) ...<Widget>[
                  Wrap(
                    spacing: 8,
                    runSpacing: 6,
                    children: <Widget>[
                      SemanticChip(
                        label: 'Banda atual: ${formatBps(_data!.currentBw)}',
                        tone: SemanticTone.info,
                        compact: true,
                      ),
                      SemanticChip(
                        label:
                            'Latência média: ${formatLatency(_data!.avgLatency)}',
                        tone: SemanticTone.primary,
                        compact: true,
                      ),
                      SemanticChip(
                        label: _data!.hasSaturationCorrelation
                            ? 'Saturação detectada (r=${_data!.correlationScore.toStringAsFixed(2)})'
                            : 'Sem saturação (r=${_data!.correlationScore.toStringAsFixed(2)})',
                        tone: _data!.hasSaturationCorrelation
                            ? SemanticTone.warning
                            : SemanticTone.success,
                        compact: true,
                      ),
                    ],
                  ),
                  const SizedBox(height: 12),
                ],
                MetricLineChart(
                  height: 190,
                  series: <MetricLineSeries>[
                    MetricLineSeries(
                      name: 'Tráfego (Mbps)',
                      points: bwMbps,
                      tone: SemanticTone.info,
                    ),
                    MetricLineSeries(
                      name: 'Latência (ms)',
                      points: latMs,
                      tone: SemanticTone.warning,
                      showArea: false,
                    ),
                  ],
                ),
              ],
            ),
    );
  }
}
