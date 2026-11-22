import 'dart:async';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/providers/diagnostics_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

// ── Speed Test Dialog (WAN NDJSON + LAN Raw Binary) ──────────────────────────

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
        setState(() => _error = describeError(err));
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
        LayoutBuilder(
          builder: (context, constraints) {
            final compact = constraints.maxWidth < 400;
            return SegmentedButton<String>(
              showSelectedIcon: !compact,
              segments: <ButtonSegment<String>>[
                ButtonSegment<String>(
                  value: 'wan',
                  label: Text(compact ? 'WAN' : 'Internet (WAN)'),
                  icon: const Icon(Icons.public, size: 16),
                ),
                ButtonSegment<String>(
                  value: 'lan',
                  label: Text(compact ? 'LAN' : 'Rede Local (LAN)'),
                  icon: const Icon(Icons.lan, size: 16),
                ),
              ],
              selected: <String>{_scope},
              onSelectionChanged: _running
                  ? null
                  : (s) => setState(() => _scope = s.first),
            );
          },
        ),
        const SizedBox(height: 10),
        AppDropdownFormField<String>(
          initialValue: _mode,
          decoration: const InputDecoration(labelText: 'Intensidade do teste'),
          items: const <DropdownMenuItem<String>>[
            DropdownMenuItem(
              value: 'simple',
              child: Text(
                'Rápido (menor tráfego)',
                overflow: TextOverflow.ellipsis,
              ),
            ),
            DropdownMenuItem(
              value: 'medium',
              child: Text(
                'Padrão (equilibrado)',
                overflow: TextOverflow.ellipsis,
              ),
            ),
            DropdownMenuItem(
              value: 'complete',
              child: Text(
                'Completo (alta precisão)',
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
          onChanged: _running
              ? null
              : (v) => setState(() => _mode = v ?? 'medium'),
        ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            FilledButton.icon(
              onPressed: _running ? null : _start,
              icon: const Icon(Icons.speed),
              label: Text(_running ? 'Medindo...' : 'Iniciar teste'),
            ),
            if (_running)
              OutlinedButton.icon(
                onPressed: () {
                  _cancelWan?.call();
                  _cancelLan?.cancel();
                },
                icon: const Icon(Icons.stop),
                label: const Text('Cancelar'),
              ),
          ],
        ),
        if (_progress != null) ...<Widget>[
          const SizedBox(height: 14),
          LinearProgressIndicator(
            value: (_progress!.progressPct / 100.0).clamp(0.0, 1.0),
          ),
          const SizedBox(height: 6),
          Text(
            'Fase: ${_progress!.phase.toUpperCase()} (${formatPercent(_progress!.progressPct, digits: 0)})'
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

// ── Quick Ping Dialog ────────────────────────────────────────────────────────

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
      if (mounted) setState(() => _error = describeError(err));
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
