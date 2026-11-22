import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/providers/diagnostics_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

// ── Traceroute ICMP Dialog (NDJSON) ──────────────────────────────────────────

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
      if (mounted) setState(() => _error = describeError(err));
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
        ResponsiveFieldRow(
          breakpoint: 480,
          children: <Widget>[
            TextField(
              controller: _hostCtrl,
              enabled: !_running,
              decoration: const InputDecoration(
                labelText: 'Host ou IP destino',
                hintText: 'Ex.: 8.8.8.8 ou cloudflare.com',
              ),
            ),
            AppDropdownFormField<int>(
              initialValue: _maxHops,
              decoration: const InputDecoration(labelText: 'Máx. saltos'),
              items: const <DropdownMenuItem<int>>[
                DropdownMenuItem(
                  value: 15,
                  child: Text('15', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 30,
                  child: Text('30', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 45,
                  child: Text('45', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: _running
                  ? null
                  : (v) => setState(() => _maxHops = v ?? 30),
            ),
          ],
        ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 8,
          runSpacing: 8,
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
            if (_running)
              OutlinedButton.icon(
                onPressed: () => _cancel?.call(),
                icon: const Icon(Icons.stop),
                label: const Text('Parar'),
              ),
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

// ── Port Scan Dialog (NDJSON) ────────────────────────────────────────────────

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
      if (mounted) setState(() => _error = describeError(err));
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
        ResponsiveFieldRow(
          breakpoint: 640,
          children: <Widget>[
            TextField(
              controller: _hostCtrl,
              enabled: !_running,
              decoration: const InputDecoration(labelText: 'Host ou IP'),
            ),
            AppDropdownFormField<String>(
              initialValue: _protocol,
              decoration: const InputDecoration(labelText: 'Protocolo'),
              items: const <DropdownMenuItem<String>>[
                DropdownMenuItem(
                  value: 'tcp',
                  child: Text('TCP', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 'udp',
                  child: Text('UDP', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: _running
                  ? null
                  : (v) => setState(() => _protocol = v ?? 'tcp'),
            ),
            AppDropdownFormField<String>(
              initialValue: _profile,
              decoration: const InputDecoration(labelText: 'Perfil'),
              items: const <DropdownMenuItem<String>>[
                DropdownMenuItem(
                  value: 'fast',
                  child: Text('Rápido', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 'reliable',
                  child: Text('Confiável', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 'complete',
                  child: Text('Completo', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: _running
                  ? null
                  : (v) => setState(() => _profile = v ?? 'fast'),
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
                  Expanded(
                    child: Text(
                      item.service ?? 'serviço desconhecido',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
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
