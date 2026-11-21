import 'dart:async';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/diagnostics_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

// ── 1. Diagnostic Playbook Dialog (NDJSON) ────────────────────────────────────

Future<void> showDiagnosticPlaybookDialog(
  BuildContext context, {
  DeviceModel? device,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Playbooks de Diagnóstico',
    subtitle: device != null
        ? 'Alvo: ${device.name}'
        : 'Diagnóstico automatizado passo a passo',
    maxWidth: 640,
    builder: (_) => _DiagnosticPlaybookBody(device: device),
  );
}

class _DiagnosticPlaybookBody extends ConsumerStatefulWidget {
  const _DiagnosticPlaybookBody({this.device});

  final DeviceModel? device;

  @override
  ConsumerState<_DiagnosticPlaybookBody> createState() =>
      _DiagnosticPlaybookBodyState();
}

class _DiagnosticPlaybookBodyState
    extends ConsumerState<_DiagnosticPlaybookBody> {
  late String _playbookType;
  late final TextEditingController _targetCtrl;
  bool _running = false;
  String? _error;
  final List<PlaybookStepResult> _steps = <PlaybookStepResult>[];
  PlaybookSummary? _summary;
  VoidCallback? _cancel;

  @override
  void initState() {
    super.initState();
    _playbookType = widget.device != null
        ? 'device_reachability'
        : 'internet_health';
    _targetCtrl = TextEditingController(text: widget.device?.ipAddress ?? '');
  }

  @override
  void dispose() {
    _cancel?.call();
    _targetCtrl.dispose();
    super.dispose();
  }

  Future<void> _run() async {
    setState(() {
      _running = true;
      _error = null;
      _steps.clear();
      _summary = null;
    });
    try {
      final stream = ref
          .read(diagnosticsServiceProvider)
          .streamPlaybook(
            playbookType: _playbookType,
            target: _targetCtrl.text.trim(),
            deviceId: widget.device?.id,
            onCancelAvailable: (c) => _cancel = c,
          );
      await for (final evt in stream) {
        if (!mounted) break;
        final type = evt['type'] as String?;
        if (type == 'step' && evt['step'] is Map) {
          final step = PlaybookStepResult.fromJson(asJsonMap(evt['step']));
          setState(() {
            final idx = _steps.indexWhere((s) => s.stepIndex == step.stepIndex);
            if (idx >= 0) {
              _steps[idx] = step;
            } else {
              _steps.add(step);
            }
          });
        } else if (type == 'summary' && evt['summary'] is Map) {
          setState(
            () =>
                _summary = PlaybookSummary.fromJson(asJsonMap(evt['summary'])),
          );
        } else if (type == 'error') {
          setState(
            () => _error =
                (evt['message'] as String?) ?? 'Erro ao executar playbook.',
          );
        }
      }
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      _cancel = null;
      if (mounted) setState(() => _running = false);
    }
  }

  SemanticTone _toneOf(String status) => switch (status) {
    'success' => SemanticTone.success,
    'warning' => SemanticTone.warning,
    'failed' => SemanticTone.error,
    _ => SemanticTone.info,
  };

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        DropdownButtonFormField<String>(
          initialValue: _playbookType,
          decoration: const InputDecoration(labelText: 'Tipo de Playbook'),
          items: const <DropdownMenuItem<String>>[
            DropdownMenuItem(
              value: 'internet_health',
              child: Text('Saúde da Internet (DNS + Rota + WAN)'),
            ),
            DropdownMenuItem(
              value: 'device_reachability',
              child: Text('Alcance e Saúde de Equipamento'),
            ),
          ],
          onChanged: _running
              ? null
              : (v) {
                  if (v != null) setState(() => _playbookType = v);
                },
        ),
        if (_playbookType == 'device_reachability') ...<Widget>[
          const SizedBox(height: 10),
          TextField(
            controller: _targetCtrl,
            enabled: !_running,
            decoration: const InputDecoration(
              labelText: 'IP ou Host alvo',
              hintText: 'Ex.: 192.168.1.1',
            ),
          ),
        ],
        const SizedBox(height: 12),
        Row(
          children: <Widget>[
            FilledButton.icon(
              onPressed: _running ? null : _run,
              icon: _running
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.play_arrow),
              label: Text(_running ? 'Executando...' : 'Executar Playbook'),
            ),
            if (_running) ...<Widget>[
              const SizedBox(width: 8),
              OutlinedButton.icon(
                onPressed: () => _cancel?.call(),
                icon: const Icon(Icons.stop),
                label: const Text('Abortar'),
              ),
            ],
          ],
        ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        if (_steps.isNotEmpty) ...<Widget>[
          const SizedBox(height: 14),
          for (final step in _steps) ...<Widget>[
            NestedPanel(
              tone: _toneOf(step.status),
              padding: const EdgeInsets.all(12),
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  SemanticChip(
                    label: '#${step.stepIndex + 1}',
                    tone: _toneOf(step.status),
                    compact: true,
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          step.stepName,
                          style: context.textTheme.titleSmall?.copyWith(
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                        if (step.description.isNotEmpty)
                          Text(
                            step.description,
                            style: context.textTheme.bodySmall,
                          ),
                        if (step.message != null &&
                            step.message!.isNotEmpty) ...<Widget>[
                          const SizedBox(height: 4),
                          Text(
                            step.message!,
                            style: context.textTheme.bodyMedium,
                          ),
                        ],
                      ],
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 8),
          ],
        ],
        if (_summary != null) ...<Widget>[
          const SizedBox(height: 6),
          NestedPanel(
            tone: _toneOf(_summary!.status),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  'Veredito: ${_summary!.diagnosis}',
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                if (_summary!.recommendations.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 6),
                  for (final rec in _summary!.recommendations)
                    Padding(
                      padding: const EdgeInsets.only(top: 2),
                      child: Text('• $rec', style: context.textTheme.bodySmall),
                    ),
                ],
              ],
            ),
          ),
        ],
      ],
    );
  }
}

// ── 2. Traceroute ICMP Dialog (NDJSON) ────────────────────────────────────────

Future<void> showTracerouteDialog(BuildContext context, {String? initialHost}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Traceroute ICMP',
    subtitle: 'Mapeamento de rota salto a salto em tempo real (NDJSON)',
    maxWidth: 680,
    builder: (_) => _TracerouteBody(initialHost: initialHost),
  );
}

class _TracerouteBody extends ConsumerStatefulWidget {
  const _TracerouteBody({this.initialHost});

  final String? initialHost;

  @override
  ConsumerState<_TracerouteBody> createState() => _TracerouteBodyState();
}

class _TracerouteBodyState extends ConsumerState<_TracerouteBody> {
  late final TextEditingController _hostCtrl;
  int _maxHops = 30;
  bool _running = false;
  String? _error;
  final List<TracerouteHop> _hops = <TracerouteHop>[];
  VoidCallback? _cancel;

  @override
  void initState() {
    super.initState();
    _hostCtrl = TextEditingController(text: widget.initialHost ?? '1.1.1.1');
  }

  @override
  void dispose() {
    _cancel?.call();
    _hostCtrl.dispose();
    super.dispose();
  }

  Future<void> _start() async {
    if (_hostCtrl.text.trim().isEmpty) return;
    setState(() {
      _running = true;
      _error = null;
      _hops.clear();
    });
    try {
      final stream = ref
          .read(diagnosticsServiceProvider)
          .streamTraceroute(
            host: _hostCtrl.text.trim(),
            maxHops: _maxHops,
            onCancelAvailable: (c) => _cancel = c,
          );
      await for (final evt in stream) {
        if (!mounted) break;
        final type = evt['type'] as String?;
        if (type == 'hop') {
          setState(() => _hops.add(TracerouteHop.fromJson(evt)));
        } else if (type == 'error') {
          setState(
            () => _error = (evt['message'] as String?) ?? 'Erro no traceroute.',
          );
        }
      }
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      _cancel = null;
      if (mounted) setState(() => _running = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Row(
          children: <Widget>[
            Expanded(
              flex: 3,
              child: TextField(
                controller: _hostCtrl,
                enabled: !_running,
                decoration: const InputDecoration(
                  labelText: 'Host ou IP destino',
                  hintText: 'Ex.: 8.8.8.8 ou cloudflare.com',
                ),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: DropdownButtonFormField<int>(
                initialValue: _maxHops,
                decoration: const InputDecoration(labelText: 'Máx. saltos'),
                items: const <DropdownMenuItem<int>>[
                  DropdownMenuItem(value: 15, child: Text('15')),
                  DropdownMenuItem(value: 30, child: Text('30')),
                  DropdownMenuItem(value: 45, child: Text('45')),
                ],
                onChanged: _running
                    ? null
                    : (v) => setState(() => _maxHops = v ?? 30),
              ),
            ),
          ],
        ),
        const SizedBox(height: 12),
        Row(
          children: <Widget>[
            FilledButton.icon(
              onPressed: _running ? null : _start,
              icon: _running
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.alt_route),
              label: Text(_running ? 'Traçando...' : 'Iniciar Traceroute'),
            ),
            if (_running) ...<Widget>[
              const SizedBox(width: 8),
              OutlinedButton.icon(
                onPressed: () => _cancel?.call(),
                icon: const Icon(Icons.stop),
                label: const Text('Parar'),
              ),
            ],
          ],
        ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        if (_hops.isNotEmpty) ...<Widget>[
          const SizedBox(height: 14),
          for (final hop in _hops) ...<Widget>[
            NestedPanel(
              tone: hop.status == 'reached'
                  ? SemanticTone.success
                  : hop.status == 'timeout'
                  ? SemanticTone.warning
                  : null,
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
              child: Row(
                children: <Widget>[
                  SizedBox(
                    width: 34,
                    child: Text(
                      '#${hop.hop}',
                      style: context.textTheme.labelLarge?.copyWith(
                        fontWeight: FontWeight.w800,
                      ),
                    ),
                  ),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          hop.ip ?? '* (tempo esgotado)',
                          style: context.textTheme.bodyMedium?.copyWith(
                            fontFamily: 'monospace',
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                        if (hop.hostname != null && hop.hostname != hop.ip)
                          Text(
                            hop.hostname!,
                            style: context.textTheme.bodySmall,
                          ),
                      ],
                    ),
                  ),
                  SemanticChip(
                    label: hop.avgRttMs != null
                        ? formatLatency(hop.avgRttMs)
                        : 'Timeout',
                    tone: hop.avgRttMs == null
                        ? SemanticTone.warning
                        : hop.avgRttMs! < 30
                        ? SemanticTone.success
                        : SemanticTone.info,
                    compact: true,
                  ),
                ],
              ),
            ),
            const SizedBox(height: 6),
          ],
        ],
      ],
    );
  }
}

// ── 3. Port Scan Dialog (NDJSON) ──────────────────────────────────────────────

Future<void> showPortScanDialog(BuildContext context, {String? initialHost}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Varredura de Portas',
    subtitle: 'Scanner TCP/UDP em tempo real (NDJSON)',
    maxWidth: 680,
    builder: (_) => _PortScanBody(initialHost: initialHost),
  );
}

class _PortScanBody extends ConsumerStatefulWidget {
  const _PortScanBody({this.initialHost});

  final String? initialHost;

  @override
  ConsumerState<_PortScanBody> createState() => _PortScanBodyState();
}

class _PortScanBodyState extends ConsumerState<_PortScanBody> {
  static const Map<String, List<int>> _presets = <String, List<int>>{
    'Comuns': <int>[
      21,
      22,
      23,
      25,
      53,
      80,
      110,
      123,
      143,
      161,
      443,
      445,
      3389,
      8080,
      8291,
    ],
    'Web': <int>[80, 443, 8000, 8080, 8443, 3000, 5000],
    'Bancos': <int>[1433, 3306, 5432, 6379, 27017],
    'Infra / Rede': <int>[
      22,
      23,
      53,
      67,
      68,
      123,
      161,
      162,
      179,
      514,
      1723,
      8291,
      8728,
      8729,
    ],
  };

  late final TextEditingController _hostCtrl;
  final TextEditingController _portsCtrl = TextEditingController(
    text: '22, 23, 53, 80, 161, 443, 3389, 8080, 8291',
  );
  String _protocol = 'tcp';
  String _profile = 'fast';
  bool _onlyOpen = false;
  bool _running = false;
  String? _error;
  final List<PortScanItem> _items = <PortScanItem>[];
  VoidCallback? _cancel;

  @override
  void initState() {
    super.initState();
    _hostCtrl = TextEditingController(text: widget.initialHost ?? '');
  }

  @override
  void dispose() {
    _cancel?.call();
    _hostCtrl.dispose();
    _portsCtrl.dispose();
    super.dispose();
  }

  List<int> _parsePorts(String text) {
    final ports = <int>{};
    for (final raw in text.split(RegExp(r'[\s,;]+'))) {
      final token = raw.trim();
      if (token.isEmpty) continue;
      if (token.contains('-')) {
        final parts = token.split('-');
        final start = int.tryParse(parts[0]);
        final end = parts.length > 1 ? int.tryParse(parts[1]) : null;
        if (start != null &&
            end != null &&
            start >= 1 &&
            end <= 65535 &&
            start <= end) {
          for (var p = start; p <= end && ports.length < 1024; p++) {
            ports.add(p);
          }
        }
      } else {
        final p = int.tryParse(token);
        if (p != null && p >= 1 && p <= 65535) ports.add(p);
      }
    }
    final list = ports.toList()..sort();
    return list;
  }

  Future<void> _start() async {
    final host = _hostCtrl.text.trim();
    final ports = _parsePorts(_portsCtrl.text);
    if (host.isEmpty || ports.isEmpty) {
      setState(() => _error = 'Informe um host válido e ao menos uma porta.');
      return;
    }
    setState(() {
      _running = true;
      _error = null;
      _items.clear();
    });
    try {
      final stream = ref
          .read(diagnosticsServiceProvider)
          .streamPortScan(
            host: host,
            protocol: _protocol,
            ports: ports,
            profile: _profile,
            onCancelAvailable: (c) => _cancel = c,
          );
      await for (final evt in stream) {
        if (!mounted) break;
        final type = evt['type'] as String?;
        if (type == 'result') {
          setState(() => _items.add(PortScanItem.fromJson(evt)));
        } else if (type == 'error') {
          setState(
            () => _error = (evt['message'] as String?) ?? 'Erro na varredura.',
          );
        }
      }
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      _cancel = null;
      if (mounted) setState(() => _running = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final visible = _onlyOpen
        ? _items
              .where((i) => i.status == 'open' || i.status == 'open|filtered')
              .toList()
        : _items;
    final openCount = _items.where((i) => i.status == 'open').length;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Row(
          children: <Widget>[
            Expanded(
              flex: 2,
              child: TextField(
                controller: _hostCtrl,
                enabled: !_running,
                decoration: const InputDecoration(labelText: 'Host ou IP'),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: DropdownButtonFormField<String>(
                initialValue: _protocol,
                decoration: const InputDecoration(labelText: 'Protocolo'),
                items: const <DropdownMenuItem<String>>[
                  DropdownMenuItem(value: 'tcp', child: Text('TCP')),
                  DropdownMenuItem(value: 'udp', child: Text('UDP')),
                ],
                onChanged: _running
                    ? null
                    : (v) => setState(() => _protocol = v ?? 'tcp'),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: DropdownButtonFormField<String>(
                initialValue: _profile,
                decoration: const InputDecoration(labelText: 'Perfil'),
                items: const <DropdownMenuItem<String>>[
                  DropdownMenuItem(value: 'fast', child: Text('Rápido')),
                  DropdownMenuItem(value: 'reliable', child: Text('Confiável')),
                  DropdownMenuItem(value: 'complete', child: Text('Completo')),
                ],
                onChanged: _running
                    ? null
                    : (v) => setState(() => _profile = v ?? 'fast'),
              ),
            ),
          ],
        ),
        const SizedBox(height: 10),
        TextField(
          controller: _portsCtrl,
          enabled: !_running,
          decoration: const InputDecoration(
            labelText: 'Portas (lista ou faixa, ex: 22, 80, 8000-8010)',
          ),
        ),
        const SizedBox(height: 8),
        Wrap(
          spacing: 6,
          runSpacing: 6,
          children: _presets.entries
              .map((entry) {
                return ActionChip(
                  label: Text(entry.key),
                  onPressed: _running
                      ? null
                      : () => setState(
                          () => _portsCtrl.text = entry.value.join(', '),
                        ),
                );
              })
              .toList(growable: false),
        ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            FilledButton.icon(
              onPressed: _running ? null : _start,
              icon: const Icon(Icons.radar),
              label: Text(_running ? 'Escaneando...' : 'Iniciar varredura'),
            ),
            if (_running)
              OutlinedButton.icon(
                onPressed: () => _cancel?.call(),
                icon: const Icon(Icons.stop),
                label: const Text('Cancelar'),
              ),
            FilterChip(
              label: Text('Apenas abertas ($openCount)'),
              selected: _onlyOpen,
              onSelected: (v) => setState(() => _onlyOpen = v),
            ),
          ],
        ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        if (visible.isNotEmpty) ...<Widget>[
          const SizedBox(height: 12),
          for (final item in visible) ...<Widget>[
            NestedPanel(
              tone: item.status == 'open' ? SemanticTone.success : null,
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
              child: Row(
                children: <Widget>[
                  SizedBox(
                    width: 82,
                    child: Text(
                      '${item.port}/${item.protocol}',
                      style: const TextStyle(
                        fontFamily: 'monospace',
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                  ),
                  Expanded(child: Text(item.service ?? 'serviço desconhecido')),
                  Text(
                    formatLatency(item.latencyMs),
                    style: context.textTheme.bodySmall,
                  ),
                  const SizedBox(width: 8),
                  SemanticChip(
                    label: item.status,
                    tone: item.status == 'open'
                        ? SemanticTone.success
                        : item.status.contains('filtered')
                        ? SemanticTone.warning
                        : SemanticTone.secondary,
                    compact: true,
                  ),
                ],
              ),
            ),
            const SizedBox(height: 6),
          ],
        ],
      ],
    );
  }
}

// ── 4. Speed Test Dialog (WAN NDJSON + LAN Raw Binary) ────────────────────────

Future<void> showSpeedTestDialog(BuildContext context) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Velocidade do navegador',
    subtitle: 'Teste WAN (CDN) via NDJSON ou LAN (Cliente ↔ Servidor)',
    maxWidth: 620,
    builder: (_) => const _SpeedTestBody(),
  );
}

class _SpeedTestBody extends ConsumerStatefulWidget {
  const _SpeedTestBody();

  @override
  ConsumerState<_SpeedTestBody> createState() => _SpeedTestBodyState();
}

class _SpeedTestBodyState extends ConsumerState<_SpeedTestBody> {
  String _scope = 'wan';
  String _mode = 'medium';
  bool _running = false;
  String? _error;
  SpeedTestProgress? _progress;
  SpeedTestResult? _result;
  VoidCallback? _cancelWan;
  CancelToken? _cancelLan;

  @override
  void dispose() {
    _cancelWan?.call();
    _cancelLan?.cancel();
    super.dispose();
  }

  Future<void> _start() async {
    setState(() {
      _running = true;
      _error = null;
      _progress = null;
      _result = null;
    });
    final service = ref.read(diagnosticsServiceProvider);
    try {
      if (_scope == 'wan') {
        final stream = service.streamWanSpeedTest(
          mode: _mode,
          onCancelAvailable: (c) => _cancelWan = c,
        );
        await for (final evt in stream) {
          if (!mounted) break;
          final type = evt['type'] as String?;
          if (type == 'progress') {
            setState(() => _progress = SpeedTestProgress.fromJson(evt));
          } else if (type == 'complete' && evt['result'] is Map) {
            setState(
              () =>
                  _result = SpeedTestResult.fromJson(asJsonMap(evt['result'])),
            );
          } else if (type == 'error') {
            setState(
              () => _error =
                  (evt['message'] as String?) ?? 'Erro no teste de velocidade.',
            );
          }
        }
      } else {
        final token = CancelToken();
        _cancelLan = token;
        final res = await service.runLanSpeedTest(
          mode: _mode,
          cancelToken: token,
          onProgress: (p) {
            if (mounted) setState(() => _progress = p);
          },
        );
        if (mounted) setState(() => _result = res);
      }
    } catch (err) {
      if (mounted && err is! DioException) {
        setState(() => _error = err.toString());
      }
    } finally {
      _cancelWan = null;
      _cancelLan = null;
      if (mounted) setState(() => _running = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final dl = _result?.downloadMbps ?? _progress?.downloadMbps;
    final ul = _result?.uploadMbps ?? _progress?.uploadMbps;
    final ping = _result?.pingMs ?? _progress?.pingMs;
    final jitter = _result?.jitterMs ?? _progress?.jitterMs;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SegmentedButton<String>(
          segments: const <ButtonSegment<String>>[
            ButtonSegment(
              value: 'wan',
              label: Text('Internet (WAN)'),
              icon: Icon(Icons.public, size: 16),
            ),
            ButtonSegment(
              value: 'lan',
              label: Text('Rede Local (LAN)'),
              icon: Icon(Icons.lan, size: 16),
            ),
          ],
          selected: <String>{_scope},
          onSelectionChanged: _running
              ? null
              : (s) => setState(() => _scope = s.first),
        ),
        const SizedBox(height: 10),
        DropdownButtonFormField<String>(
          initialValue: _mode,
          decoration: const InputDecoration(labelText: 'Intensidade do teste'),
          items: const <DropdownMenuItem<String>>[
            DropdownMenuItem(
              value: 'simple',
              child: Text('Rápido (menor tráfego)'),
            ),
            DropdownMenuItem(
              value: 'medium',
              child: Text('Padrão (equilibrado)'),
            ),
            DropdownMenuItem(
              value: 'complete',
              child: Text('Completo (alta precisão)'),
            ),
          ],
          onChanged: _running
              ? null
              : (v) => setState(() => _mode = v ?? 'medium'),
        ),
        const SizedBox(height: 12),
        Row(
          children: <Widget>[
            FilledButton.icon(
              onPressed: _running ? null : _start,
              icon: const Icon(Icons.speed),
              label: Text(_running ? 'Medindo...' : 'Iniciar teste'),
            ),
            if (_running) ...<Widget>[
              const SizedBox(width: 8),
              OutlinedButton.icon(
                onPressed: () {
                  _cancelWan?.call();
                  _cancelLan?.cancel();
                },
                icon: const Icon(Icons.stop),
                label: const Text('Cancelar'),
              ),
            ],
          ],
        ),
        if (_progress != null) ...<Widget>[
          const SizedBox(height: 14),
          LinearProgressIndicator(
            value: (_progress!.progressPct / 100.0).clamp(0.0, 1.0),
          ),
          const SizedBox(height: 6),
          Text(
            'Fase: ${_progress!.phase.toUpperCase()} (${_progress!.progressPct.round()}%)'
            '${_progress!.serverName != null ? ' · ${_progress!.serverName}' : ''}',
            style: context.textTheme.bodySmall,
          ),
        ],
        if (_error != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        if (dl != null || ul != null || ping != null) ...<Widget>[
          const SizedBox(height: 14),
          ResponsiveGrid(
            xs: 2,
            sm: 2,
            md: 2,
            lg: 2,
            xl: 2,
            children: <Widget>[
              KpiCard(
                label: 'Download',
                value: dl != null
                    ? '${formatSpeedByUnit(dl)} ${getSpeedUnitLabel(SpeedUnit.auto, valueMbps: dl)}'
                    : '—',
                icon: Icons.download_rounded,
                tone: SemanticTone.success,
              ),
              KpiCard(
                label: 'Upload',
                value: ul != null
                    ? '${formatSpeedByUnit(ul)} ${getSpeedUnitLabel(SpeedUnit.auto, valueMbps: ul)}'
                    : '—',
                icon: Icons.upload_rounded,
                tone: SemanticTone.info,
              ),
              KpiCard(
                label: 'Latência (Ping)',
                value: formatLatency(ping),
                icon: Icons.timer_outlined,
                tone: SemanticTone.primary,
              ),
              KpiCard(
                label: 'Jitter',
                value: formatLatency(jitter),
                icon: Icons.graphic_eq,
                tone: SemanticTone.secondary,
              ),
            ],
          ),
        ],
      ],
    );
  }
}

// ── 5. Quick Ping Dialog ──────────────────────────────────────────────────────

Future<void> showQuickPingDialog(
  BuildContext context, {
  required int deviceId,
  required String deviceName,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Ping Rápido · $deviceName',
    subtitle:
        'Latência, perda, fila e ordem de grandeza da capacidade até o alvo',
    maxWidth: 560,
    builder: (_) => _QuickPingBody(deviceId: deviceId),
  );
}

class _QuickPingBody extends ConsumerStatefulWidget {
  const _QuickPingBody({required this.deviceId});

  final int deviceId;

  @override
  ConsumerState<_QuickPingBody> createState() => _QuickPingBodyState();
}

class _QuickPingBodyState extends ConsumerState<_QuickPingBody> {
  bool _loading = true;
  QuickPingReport? _report;
  String? _error;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_run);
  }

  Future<void> _run() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final report = await ref
          .read(diagnosticsServiceProvider)
          .quickPing(widget.deviceId);
      if (mounted) setState(() => _report = report);
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const LoadingStateView(
        message: 'Medindo ecos pequenos e grandes até o equipamento...',
      );
    }
    if (_error != null) {
      return ErrorStateView(message: _error!, onRetry: _run);
    }
    final r = _report!;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        NestedPanel(
          tone: r.lossPct == 0 ? SemanticTone.success : SemanticTone.warning,
          child: Text(
            r.verdict,
            style: context.textTheme.bodyMedium?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
        ),
        const SizedBox(height: 12),
        KeyValueRow(label: 'Alvo', value: r.target, monospace: true),
        KeyValueRow(
          label: 'Pacotes',
          value:
              '${r.samples}/${r.sent} recebidos (${formatPercent(r.lossPct)} perda)',
        ),
        if (r.small != null)
          KeyValueRow(
            label: 'RTT (pacotes pequenos)',
            value:
                '${formatLatency(r.small!.minMs)} / ${formatLatency(r.small!.avgMs)} / ${formatLatency(r.small!.maxMs)}',
          ),
        if (r.large != null)
          KeyValueRow(
            label: 'RTT (pacotes grandes)',
            value:
                '${formatLatency(r.large!.minMs)} / ${formatLatency(r.large!.avgMs)} / ${formatLatency(r.large!.maxMs)}',
          ),
        KeyValueRow(label: 'Jitter', value: formatLatency(r.jitterMs)),
        if (r.capacityLabel != null)
          KeyValueRow(label: 'Capacidade estimada', value: r.capacityLabel!),
        if (r.capacityNote.isNotEmpty) ...<Widget>[
          const SizedBox(height: 8),
          Text(r.capacityNote, style: context.textTheme.bodySmall),
        ],
        const SizedBox(height: 12),
        Align(
          alignment: Alignment.centerRight,
          child: OutlinedButton.icon(
            onPressed: _run,
            icon: const Icon(Icons.refresh),
            label: const Text('Medir novamente'),
          ),
        ),
      ],
    );
  }
}

// ── 6. SNMP Test Dialog ───────────────────────────────────────────────────────

Future<void> showSnmpTestDialog(
  BuildContext context, {
  String? initialHost,
  String initialVersion = 'v2c',
  String initialCommunity = 'public',
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Teste SNMP',
    subtitle: 'Valide conectividade, versão, community e walk de interfaces',
    maxWidth: 600,
    builder: (_) => _SnmpTestBody(
      initialHost: initialHost,
      initialVersion: initialVersion,
      initialCommunity: initialCommunity,
    ),
  );
}

class _SnmpTestBody extends ConsumerStatefulWidget {
  const _SnmpTestBody({
    this.initialHost,
    required this.initialVersion,
    required this.initialCommunity,
  });

  final String? initialHost;
  final String initialVersion;
  final String initialCommunity;

  @override
  ConsumerState<_SnmpTestBody> createState() => _SnmpTestBodyState();
}

class _SnmpTestBodyState extends ConsumerState<_SnmpTestBody> {
  late final TextEditingController _hostCtrl;
  late final TextEditingController _communityCtrl;
  final TextEditingController _portCtrl = TextEditingController(text: '161');
  late String _version;
  bool _testing = false;
  SnmpTestResult? _result;
  List<DeviceInterfaceModel> _interfaces = const <DeviceInterfaceModel>[];
  String? _error;

  @override
  void initState() {
    super.initState();
    _hostCtrl = TextEditingController(text: widget.initialHost ?? '');
    _communityCtrl = TextEditingController(text: widget.initialCommunity);
    _version = widget.initialVersion;
  }

  @override
  void dispose() {
    _hostCtrl.dispose();
    _communityCtrl.dispose();
    _portCtrl.dispose();
    super.dispose();
  }

  Future<void> _test({required bool autoDetect}) async {
    if (_hostCtrl.text.trim().isEmpty) return;
    setState(() {
      _testing = true;
      _error = null;
      _interfaces = const <DeviceInterfaceModel>[];
    });
    try {
      final res = await ref
          .read(snmpProfilesProvider.notifier)
          .testSnmp(
            host: _hostCtrl.text.trim(),
            port: int.tryParse(_portCtrl.text.trim()) ?? 161,
            version: _version,
            community: _communityCtrl.text.trim(),
            autoDetect: autoDetect,
          );
      if (!mounted) return;
      setState(() => _result = res);
      if (res.responded) {
        final ifaces = await ref
            .read(snmpProfilesProvider.notifier)
            .queryInterfaces(
              host: _hostCtrl.text.trim(),
              port: int.tryParse(_portCtrl.text.trim()) ?? 161,
              version: res.version ?? _version,
              community: res.community ?? _communityCtrl.text.trim(),
            );
        if (mounted) setState(() => _interfaces = ifaces);
      }
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _testing = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Row(
          children: <Widget>[
            Expanded(
              flex: 2,
              child: TextField(
                controller: _hostCtrl,
                decoration: const InputDecoration(labelText: 'Host / IP'),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: TextField(
                controller: _portCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(labelText: 'Porta'),
              ),
            ),
          ],
        ),
        const SizedBox(height: 10),
        Row(
          children: <Widget>[
            Expanded(
              child: DropdownButtonFormField<String>(
                initialValue: _version,
                decoration: const InputDecoration(labelText: 'Versão'),
                items: const <DropdownMenuItem<String>>[
                  DropdownMenuItem(value: 'v1', child: Text('v1')),
                  DropdownMenuItem(value: 'v2c', child: Text('v2c')),
                  DropdownMenuItem(value: 'v3', child: Text('v3')),
                ],
                onChanged: (v) => setState(() => _version = v ?? 'v2c'),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              flex: 2,
              child: TextField(
                controller: _communityCtrl,
                decoration: const InputDecoration(labelText: 'Community'),
              ),
            ),
          ],
        ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            FilledButton.icon(
              onPressed: _testing ? null : () => _test(autoDetect: false),
              icon: const Icon(Icons.play_arrow),
              label: const Text('Testar SNMP'),
            ),
            OutlinedButton.icon(
              onPressed: _testing ? null : () => _test(autoDetect: true),
              icon: const Icon(Icons.auto_awesome),
              label: const Text('Auto-detectar'),
            ),
          ],
        ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        if (_result != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            tone: _result!.responded
                ? SemanticTone.success
                : SemanticTone.error,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  _result!.responded
                      ? 'Agente SNMP respondeu'
                      : 'Sem resposta SNMP',
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                if (_result!.sysName != null)
                  KeyValueRow(label: 'sysName', value: _result!.sysName!),
                if (_result!.sysDescr != null)
                  KeyValueRow(label: 'sysDescr', value: _result!.sysDescr!),
                if (_result!.message != null) Text(_result!.message!),
              ],
            ),
          ),
        ],
        if (_interfaces.isNotEmpty) ...<Widget>[
          const SizedBox(height: 12),
          Text(
            'Interfaces descobertas (${_interfaces.length})',
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 6),
          for (final iface in _interfaces.take(20))
            ListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              title: Text(iface.displayName),
              subtitle: Text(formatLinkSpeed(iface.speed)),
              trailing: StatusIndicator.fromStatus(
                iface.operStatus ?? 'unknown',
                compact: true,
              ),
            ),
        ],
      ],
    );
  }
}

// ── 7. SNMP Scan & Apply Monitors Dialog ──────────────────────────────────────

Future<void> showSnmpScanDialog(
  BuildContext context, {
  required int deviceId,
  required String deviceName,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Escanear SNMP · $deviceName',
    subtitle: 'Descubra CPU, memória, interfaces e sensores e aplique monitores em lote',
    maxWidth: 680,
    builder: (ctx) => _SnmpScanBody(
      deviceId: deviceId,
      onApplied: () => Navigator.of(ctx).pop(),
    ),
  );
}

class _SnmpScanBody extends ConsumerStatefulWidget {
  const _SnmpScanBody({required this.deviceId, required this.onApplied});

  final int deviceId;
  final VoidCallback onApplied;

  @override
  ConsumerState<_SnmpScanBody> createState() => _SnmpScanBodyState();
}

class _SnmpScanBodyState extends ConsumerState<_SnmpScanBody> {
  SnmpScanResultModel? _scan;
  bool _scanning = true;
  bool _applying = false;
  bool _enableCpu = false;
  bool _enableMem = false;
  bool _clearRemoved = false;
  final Set<int> _selectedIfIndexes = <int>{};
  final Set<String> _selectedSensors = <String>{};

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_runScan);
  }

  Future<void> _runScan() async {
    setState(() => _scanning = true);
    final res = await ref
        .read(deviceDetailProvider.notifier)
        .scanDeviceSnmp(widget.deviceId);
    if (!mounted) return;
    if (res != null) {
      _enableCpu = res.hasCpuMonitor || res.cpuUsagePercent != null;
      _enableMem = res.hasMemoryMonitor || res.memoryUsedPercent != null;
      _selectedIfIndexes
        ..clear()
        ..addAll(
          res.interfaces.where((i) => i.isMonitored).map((i) => i.ifIndex),
        );
      _selectedSensors
        ..clear()
        ..addAll(res.sensors.where((s) => s.isMonitored).map((s) => s.key));
    }
    setState(() {
      _scan = res;
      _scanning = false;
    });
  }

  Future<void> _apply() async {
    setState(() => _applying = true);
    final ok = await ref
        .read(deviceDetailProvider.notifier)
        .applySnmpMonitors(
          widget.deviceId,
          enableCpuMonitor: _enableCpu,
          enableMemoryMonitor: _enableMem,
          monitoredIfIndexes: _selectedIfIndexes.toList(growable: false),
          monitoredSensors: _selectedSensors.toList(growable: false),
          clearRemovedHistory: _clearRemoved,
        );
    if (!mounted) return;
    setState(() => _applying = false);
    if (ok) widget.onApplied();
  }

  @override
  Widget build(BuildContext context) {
    if (_scanning) {
      return const LoadingStateView(
        message: 'Consultando MIBs SNMP no equipamento (até 120s)...',
      );
    }
    if (_scan == null || !_scan!.snmpResponded) {
      return ErrorStateView(
        message:
            ref.watch(deviceDetailProvider).error ??
            'O equipamento não respondeu via SNMP.',
        onRetry: _runScan,
      );
    }
    final scan = _scan!;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        NestedPanel(
          tone: SemanticTone.success,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                scan.sysName ?? 'SNMP conectado',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              if (scan.sysDescr != null)
                Text(scan.sysDescr!, style: context.textTheme.bodySmall),
              if (scan.matchedProfileName != null)
                Padding(
                  padding: const EdgeInsets.only(top: 6),
                  child: SemanticChip(
                    label: 'Perfil: ${scan.matchedProfileName}',
                    tone: SemanticTone.info,
                    compact: true,
                  ),
                ),
            ],
          ),
        ),
        const SizedBox(height: 12),
        CheckboxListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('Monitorar uso de CPU'),
          subtitle: Text(
            scan.cpuUsagePercent != null
                ? 'Atual: ${formatPercent(scan.cpuUsagePercent)}'
                : 'Não reportado',
          ),
          value: _enableCpu,
          onChanged: (v) => setState(() => _enableCpu = v ?? false),
        ),
        CheckboxListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('Monitorar uso de Memória'),
          subtitle: Text(
            scan.memoryUsedPercent != null
                ? 'Atual: ${formatPercent(scan.memoryUsedPercent)}'
                : 'Não reportado',
          ),
          value: _enableMem,
          onChanged: (v) => setState(() => _enableMem = v ?? false),
        ),
        if (scan.interfaces.isNotEmpty) ...<Widget>[
          const Divider(),
          Text(
            'Interfaces (${_selectedIfIndexes.length}/${scan.interfaces.length} selecionadas)',
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          for (final iface in scan.interfaces)
            CheckboxListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              title: Text(iface.ifName),
              subtitle: Text(
                '${iface.ifOperStatus ?? '—'} · ${formatLinkSpeed(iface.ifSpeed)}'
                '${iface.ifAlias != null && iface.ifAlias!.isNotEmpty ? ' · ${iface.ifAlias}' : ''}',
              ),
              value: _selectedIfIndexes.contains(iface.ifIndex),
              onChanged: (v) {
                setState(() {
                  if (v == true) {
                    _selectedIfIndexes.add(iface.ifIndex);
                  } else {
                    _selectedIfIndexes.remove(iface.ifIndex);
                  }
                });
              },
            ),
        ],
        if (scan.sensors.isNotEmpty) ...<Widget>[
          const Divider(),
          Text(
            'Sensores (${_selectedSensors.length}/${scan.sensors.length})',
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          for (final sensor in scan.sensors)
            CheckboxListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              title: Text(sensor.label),
              subtitle: Text('${sensor.formattedValue} · ${sensor.oid}'),
              value: _selectedSensors.contains(sensor.key),
              onChanged: (v) {
                setState(() {
                  if (v == true) {
                    _selectedSensors.add(sensor.key);
                  } else {
                    _selectedSensors.remove(sensor.key);
                  }
                });
              },
            ),
        ],
        const Divider(),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('Limpar histórico de itens removidos'),
          value: _clearRemoved,
          onChanged: (v) => setState(() => _clearRemoved = v),
        ),
        const SizedBox(height: 12),
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton.icon(
            onPressed: _applying ? null : _apply,
            icon: _applying
                ? const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.check),
            label: const Text('Aplicar monitores SNMP'),
          ),
        ),
      ],
    );
  }
}

// ── 8. SNMP Profiles Dialog ───────────────────────────────────────────────────

Future<void> showSnmpProfilesDialog(BuildContext context) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Perfis SNMP',
    subtitle: 'Catálogo de perfis embutidos e personalizados de sensores SNMP',
    maxWidth: 640,
    builder: (_) => const _SnmpProfilesBody(),
  );
}

class _SnmpProfilesBody extends ConsumerStatefulWidget {
  const _SnmpProfilesBody();

  @override
  ConsumerState<_SnmpProfilesBody> createState() => _SnmpProfilesBodyState();
}

class _SnmpProfilesBodyState extends ConsumerState<_SnmpProfilesBody> {
  final _idCtrl = TextEditingController();
  final _nameCtrl = TextEditingController();
  final _vendorCtrl = TextEditingController();
  final _oidPrefixCtrl = TextEditingController();
  bool _creating = false;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(
      () => ref.read(snmpProfilesProvider.notifier).fetchProfiles(),
    );
  }

  @override
  void dispose() {
    _idCtrl.dispose();
    _nameCtrl.dispose();
    _vendorCtrl.dispose();
    _oidPrefixCtrl.dispose();
    super.dispose();
  }

  Future<void> _saveNew() async {
    if (_idCtrl.text.trim().isEmpty || _nameCtrl.text.trim().isEmpty) return;
    await ref.read(snmpProfilesProvider.notifier).saveProfile(<String, dynamic>{
      'id': _idCtrl.text.trim(),
      'name': _nameCtrl.text.trim(),
      'vendor': _vendorCtrl.text.trim().isEmpty
          ? 'Custom'
          : _vendorCtrl.text.trim(),
      'category': 'network',
      if (_oidPrefixCtrl.text.trim().isNotEmpty)
        'sysObjectIdPrefix': _oidPrefixCtrl.text.trim(),
      'sensors': const <Map<String, dynamic>>[],
    });
    if (!mounted) return;
    _idCtrl.clear();
    _nameCtrl.clear();
    _vendorCtrl.clear();
    _oidPrefixCtrl.clear();
    setState(() => _creating = false);
  }

  @override
  Widget build(BuildContext context) {
    final profiles = ref.watch(snmpProfilesProvider);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: <Widget>[
            Text(
              '${profiles.length} perfis disponíveis',
              style: context.textTheme.labelLarge,
            ),
            OutlinedButton.icon(
              onPressed: () => setState(() => _creating = !_creating),
              icon: Icon(_creating ? Icons.close : Icons.add, size: 18),
              label: Text(_creating ? 'Fechar formulário' : 'Novo perfil'),
            ),
          ],
        ),
        if (_creating) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(
            child: Column(
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Expanded(
                      child: TextField(
                        controller: _idCtrl,
                        decoration: const InputDecoration(
                          labelText: 'ID (slug)',
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: TextField(
                        controller: _nameCtrl,
                        decoration: const InputDecoration(labelText: 'Nome'),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                Row(
                  children: <Widget>[
                    Expanded(
                      child: TextField(
                        controller: _vendorCtrl,
                        decoration: const InputDecoration(
                          labelText: 'Fabricante',
                        ),
                      ),
                    ),
                    const SizedBox(width: 8),
                    Expanded(
                      child: TextField(
                        controller: _oidPrefixCtrl,
                        decoration: const InputDecoration(
                          labelText: 'Prefixo sysObjectID',
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 10),
                Align(
                  alignment: Alignment.centerRight,
                  child: FilledButton(
                    onPressed: _saveNew,
                    child: const Text('Salvar perfil'),
                  ),
                ),
              ],
            ),
          ),
        ],
        const SizedBox(height: 12),
        if (profiles.isEmpty)
          const EmptyStateView(
            title: 'Nenhum perfil SNMP carregado',
            icon: Icons.tune_outlined,
          )
        else
          for (final p in profiles) ...<Widget>[
            NestedPanel(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
              child: Row(
                children: <Widget>[
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          p.name,
                          style: context.textTheme.titleSmall?.copyWith(
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                        Text(
                          '${p.vendor} · ${countNoun(p.sensorCount, 'sensor', 'sensores')}'
                          '${p.sysObjectIdPrefix != null ? ' · ${p.sysObjectIdPrefix}' : ''}',
                          style: context.textTheme.bodySmall,
                        ),
                      ],
                    ),
                  ),
                  SemanticChip(
                    label: p.isBuiltin ? 'Embutido' : 'Personalizado',
                    tone: p.isBuiltin
                        ? SemanticTone.info
                        : SemanticTone.primary,
                    compact: true,
                  ),
                  if (!p.isBuiltin)
                    IconButton(
                      tooltip: 'Excluir perfil',
                      icon: const Icon(Icons.delete_outline, size: 18),
                      onPressed: () => ref
                          .read(snmpProfilesProvider.notifier)
                          .deleteProfile(p.id),
                    ),
                ],
              ),
            ),
            const SizedBox(height: 8),
          ],
      ],
    );
  }
}
