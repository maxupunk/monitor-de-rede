import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/providers/diagnostics_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

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
      final target = _targetCtrl.text.trim();
      final deviceIp = widget.device?.ipAddress?.trim();
      final useDeviceId =
          widget.device != null &&
          (deviceIp == null || deviceIp.isEmpty || target == deviceIp);
      final stream = ref
          .read(diagnosticsServiceProvider)
          .streamPlaybook(
            playbookType: _playbookType,
            target: target,
            deviceId: useDeviceId ? widget.device?.id : null,
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
      if (mounted) setState(() => _error = describeError(err));
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
        AppDropdownFormField<String>(
          initialValue: _playbookType,
          decoration: const InputDecoration(labelText: 'Tipo de Playbook'),
          items: const <DropdownMenuItem<String>>[
            DropdownMenuItem(
              value: 'internet_health',
              child: Text(
                'Saúde da Internet (DNS + Rota + WAN)',
                overflow: TextOverflow.ellipsis,
              ),
            ),
            DropdownMenuItem(
              value: 'device_reachability',
              child: Text(
                'Alcance e Saúde de Equipamento',
                overflow: TextOverflow.ellipsis,
              ),
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
        Wrap(
          spacing: 8,
          runSpacing: 8,
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
            if (_running)
              OutlinedButton.icon(
                onPressed: () => _cancel?.call(),
                icon: const Icon(Icons.stop),
                label: const Text('Abortar'),
              ),
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
