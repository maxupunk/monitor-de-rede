import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

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
      final devicesNotifier = ref.read(devicesProvider.notifier);
      if (_deviceId == null) {
        await devicesNotifier.fetchDevices();
        final devs = ref.read(devicesProvider).items;
        final snmpDev =
            devs.where((d) => d.snmpEnabled).firstOrNull ?? devs.firstOrNull;
        _deviceId = snmpDev?.id;
      }
      if (_deviceId != null) {
        final metrics = await devicesNotifier.fetchDeviceMetrics(_deviceId!);
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
      _inSeq = _inPoints.length;
      _outSeq = _outPoints.length;
    } catch (_) {}
    if (mounted) setState(() => _loading = false);
  }

  int _inSeq = 0;
  int _outSeq = 0;

  void _appendIn(double val) {
    _inPoints.add(MetricSeriesPoint(x: (_inSeq++).toDouble(), y: val));
    if (_inPoints.length > 60) _inPoints.removeAt(0);
  }

  void _appendOut(double val) {
    _outPoints.add(MetricSeriesPoint(x: (_outSeq++).toDouble(), y: val));
    if (_outPoints.length > 60) _outPoints.removeAt(0);
  }

  void _onLiveEvent(Map<String, dynamic> data) {
    if (_deviceId == null ||
        parseIntOrNull(data['deviceId']) != _deviceId ||
        !mounted) {
      return;
    }
    var changed = false;
    final metrics = asJsonMapList(data['metrics']);
    for (final m in metrics) {
      final name = m['name']?.toString();
      final val = parseDoubleOrNull(m['value']);
      if (val == null) continue;
      if (name == 'inBps' || name == 'interface_in_bps') {
        _appendIn(val);
        changed = true;
      } else if (name == 'outBps' || name == 'interface_out_bps') {
        _appendOut(val);
        changed = true;
      }
    }
    // Eventos `interface:traffic` trazem os contadores sob `data['interfaces']`.
    final ifaces = asJsonMapList(data['interfaces']);
    if (ifaces.isNotEmpty) {
      final wantedName = (widget.config['interfaceName'] as String?)?.trim();
      final chosen = wantedName != null && wantedName.isNotEmpty
          ? ifaces
                .where(
                  (i) => (i['ifName'] ?? i['name'])?.toString() == wantedName,
                )
                .firstOrNull
          : ifaces.first;
      if (chosen != null) {
        final inVal = parseDoubleOrNull(chosen['inBps']);
        final outVal = parseDoubleOrNull(chosen['outBps']);
        if (inVal != null) {
          _appendIn(inVal);
          changed = true;
        }
        if (outVal != null) {
          _appendOut(outVal);
          changed = true;
        }
      }
    }
    if (changed) setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final lastIn = _inPoints.isNotEmpty ? _inPoints.last.y : null;
    final lastOut = _outPoints.isNotEmpty ? _outPoints.last.y : null;

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 520;
        final chips = (lastIn != null || lastOut != null)
            ? Wrap(
                spacing: 6,
                runSpacing: 6,
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
              )
            : null;
        return SectionCard(
          title: 'Largura de Banda da Interface',
          subtitle:
              _interfaceName ??
              (_deviceId != null
                  ? 'Dispositivo #$_deviceId'
                  : 'Ao vivo via SSE'),
          icon: Icons.swap_vert,
          iconTone: SemanticTone.primary,
          trailing: isCompact ? null : chips,
          child: _loading
              ? const LoadingStateView(message: 'Carregando tráfego...')
              : Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    if (isCompact && chips != null) ...<Widget>[
                      chips,
                      const SizedBox(height: 10),
                    ],
                    MetricLineChart(
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
                  ],
                ),
        );
      },
    );
  }
}

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

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 520;
        final timeframeToggle = SegmentedButton<String>(
          showSelectedIcon: !isCompact,
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
        );
        final corrLabel = _data != null
            ? formatDecimal(_data!.correlationScore, maxDigits: 2, minDigits: 2)
            : null;
        return SectionCard(
          title: 'Banda × Latência (Bufferbloat)',
          subtitle: 'Correlação entre tráfego (Mbps) e latência ICMP (ms)',
          icon: Icons.show_chart,
          iconTone: _data?.hasSaturationCorrelation == true
              ? SemanticTone.warning
              : SemanticTone.info,
          trailing: isCompact ? null : timeframeToggle,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              if (isCompact) ...<Widget>[
                Align(alignment: Alignment.centerLeft, child: timeframeToggle),
                const SizedBox(height: 10),
              ],
              if (_loading)
                const LoadingStateView(message: 'Analisando correlação...')
              else ...<Widget>[
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
                            ? 'Saturação detectada (r=$corrLabel)'
                            : 'Sem saturação (r=$corrLabel)',
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
            ],
          ),
        );
      },
    );
  }
}
