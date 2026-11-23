import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/plugin_presentation.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugin_apps_provider.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/features/plugins/utils/plugin_settings_logic.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_output_view.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_settings_form.dart';
import 'package:netmonitor_app/shared/device_access/device_access.dart'
    hide CredentialView;
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Assistente de ação em lote na frota (`FleetActionFlow.vue`):
/// 4 etapas: `form` -> `review` (prévia) -> `running` (SSE) -> `done` (com apply-patch).
Future<void> showFleetActionFlow({
  required BuildContext context,
  required FleetView fleet,
  required PluginAction action,
  String? previewAction,
  Map<String, dynamic> initialParams = const <String, dynamic>{},
  List<int>? preselectedDeviceIds,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: action.title,
    subtitle: 'Execução em frota · ${fleet.plugin.name}',
    maxWidth: 680,
    builder: (_) => _FleetActionFlowBody(
      fleet: fleet,
      action: action,
      previewAction: previewAction,
      initialParams: initialParams,
      preselectedDeviceIds: preselectedDeviceIds,
    ),
  );
}

enum _FlowStep { form, review, running, done }

class _FleetActionFlowBody extends ConsumerStatefulWidget {
  const _FleetActionFlowBody({
    required this.fleet,
    required this.action,
    this.previewAction,
    this.initialParams = const <String, dynamic>{},
    this.preselectedDeviceIds,
  });

  final FleetView fleet;
  final PluginAction action;
  final String? previewAction;
  final Map<String, dynamic> initialParams;
  final List<int>? preselectedDeviceIds;

  @override
  ConsumerState<_FleetActionFlowBody> createState() =>
      _FleetActionFlowBodyState();
}

class _FleetActionFlowBodyState extends ConsumerState<_FleetActionFlowBody> {
  final _formKey = GlobalKey<FormState>();
  _FlowStep _step = _FlowStep.form;
  late Map<String, dynamic> _params;
  late Set<int> _selectedDevices;
  bool _confirmWrite = false;
  bool _submitting = false;
  int? _previewBatchId;
  int? _runBatchId;
  String? _error;

  @override
  void initState() {
    super.initState();
    _params = <String, dynamic>{
      ...pluginSchemaDefaults(widget.action.paramsSchema),
      ...widget.initialParams,
    };
    _selectedDevices = widget.preselectedDeviceIds != null
        ? Set<int>.from(widget.preselectedDeviceIds!)
        : widget.fleet.members
              .where((m) => m.credentialsReady)
              .map((m) => m.deviceId)
              .toSet();
  }

  Future<void> _startPreviewOrRun() async {
    if (!(_formKey.currentState?.validate() ?? false)) return;
    if (_selectedDevices.isEmpty) {
      setState(() => _error = 'Selecione ao menos um equipamento.');
      return;
    }
    setState(() {
      _submitting = true;
      _error = null;
    });
    final normalized = normalizePluginFormParams(
      widget.action.paramsSchema,
      _params,
    );
    try {
      if (widget.action.isWrite && widget.previewAction != null) {
        final batchId = await ref
            .read(pluginAppsProvider.notifier)
            .runAction(
              pluginId: widget.fleet.plugin.id,
              action: widget.previewAction!,
              deviceIds: _selectedDevices.toList(growable: false),
              confirmWrite: false,
              params: normalized,
            );
        if (mounted) {
          setState(() {
            _previewBatchId = batchId;
            _step = _FlowStep.review;
            _submitting = false;
          });
        }
      } else {
        await _launchFinalRun(normalized, _selectedDevices.toList());
      }
    } catch (err) {
      if (mounted) {
        setState(() {
          _submitting = false;
          _error = describeError(err);
        });
      }
    }
  }

  Future<void> _launchFinalRun(
    Map<String, dynamic> params,
    List<int> deviceIds,
  ) async {
    setState(() {
      _submitting = true;
      _error = null;
    });
    try {
      final batchId = await ref
          .read(pluginAppsProvider.notifier)
          .runAction(
            pluginId: widget.fleet.plugin.id,
            action: widget.action.name,
            deviceIds: deviceIds,
            confirmWrite: widget.action.isWrite,
            params: params,
          );
      if (mounted) {
        setState(() {
          _runBatchId = batchId;
          _step = _FlowStep.running;
          _submitting = false;
        });
      }
    } catch (err) {
      if (mounted) {
        setState(() {
          _submitting = false;
          _error = describeError(err);
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final appsState = ref.watch(pluginAppsProvider);
    final pluginId = widget.fleet.plugin.id;
    final previewBatch = appsState.batchById(pluginId, _previewBatchId);
    final runBatch = appsState.batchById(pluginId, _runBatchId);

    if (_step == _FlowStep.running && runBatch != null && !runBatch.isRunning) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && _step == _FlowStep.running) {
          setState(() => _step = _FlowStep.done);
          if (widget.action.isWrite) {
            ref.read(pluginAppsProvider.notifier).loadFleet(pluginId);
          }
        }
      });
    }

    if (_step == _FlowStep.form) {
      final liveFleet =
          ref.watch(pluginAppsProvider).fleets[pluginId] ?? widget.fleet;
      return Form(
        key: _formKey,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            if (_error != null) ...<Widget>[
              NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
              const SizedBox(height: 10),
            ],
            if (widget.action.paramsSchema != null) ...<Widget>[
              PluginSettingsForm(
                schema: widget.action.paramsSchema,
                values: _params,
                onChanged: (next) => setState(() => _params = next),
                onRevealSecret: (fieldName, revealAction) async {
                  final ready = liveFleet.members
                      .where((m) => m.credentialsReady)
                      .firstOrNull;
                  if (ready == null) return null;
                  try {
                    final out = await ref
                        .read(pluginsProvider.notifier)
                        .revealSecret(
                          deviceId: ready.deviceId,
                          pluginId: pluginId,
                          action: revealAction,
                          params: _params,
                        );
                    return out[fieldName]?.toString() ??
                        out['password']?.toString();
                  } catch (err) {
                    if (mounted) {
                      setState(() => _error = describeError(err));
                    }
                    return null;
                  }
                },
              ),
              const SizedBox(height: 12),
            ],
            Text(
              'Equipamentos (${_selectedDevices.length}/${liveFleet.members.length})',
              style: context.textTheme.titleSmall?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            const SizedBox(height: 6),
            for (final member in liveFleet.members)
              CheckboxListTile(
                contentPadding: EdgeInsets.zero,
                dense: true,
                value: _selectedDevices.contains(member.deviceId),
                onChanged: (v) {
                  setState(() {
                    if (v == true) {
                      _selectedDevices.add(member.deviceId);
                    } else {
                      _selectedDevices.remove(member.deviceId);
                    }
                  });
                },
                title: Text(member.deviceName),
                subtitle: Text(member.ipAddress),
                secondary: !member.credentialsReady
                    ? TextButton(
                        onPressed: () async {
                          final ok = await showDeviceAccessDialog(
                            context: context,
                            ref: ref,
                            deviceId: member.deviceId,
                            lockedTransport: member.missingTransport,
                          );
                          if (ok && mounted) {
                            await ref
                                .read(pluginAppsProvider.notifier)
                                .loadFleet(pluginId);
                          }
                        },
                        child: const Text('Acesso'),
                      )
                    : null,
              ),
            if (widget.action.isWrite &&
                widget.previewAction == null) ...<Widget>[
              const SizedBox(height: 8),
              NestedPanel(
                tone: SemanticTone.error,
                child: Material(
                  type: MaterialType.transparency,
                  child: CheckboxListTile(
                    contentPadding: EdgeInsets.zero,
                    value: _confirmWrite,
                    onChanged: (v) => setState(() => _confirmWrite = v == true),
                    title: const Text(
                      'Confirmo que esta ação altera a configuração real nos equipamentos selecionados.',
                      style: TextStyle(
                        fontSize: 13,
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                  ),
                ),
              ),
            ],
            const SizedBox(height: 14),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: <Widget>[
                TextButton(
                  onPressed: () => Navigator.of(context).pop(),
                  child: const Text('Cancelar'),
                ),
                const SizedBox(width: 8),
                FilledButton.icon(
                  onPressed:
                      _submitting ||
                          (widget.action.isWrite &&
                              widget.previewAction == null &&
                              !_confirmWrite)
                      ? null
                      : _startPreviewOrRun,
                  icon: _submitting
                      ? const SizedBox(
                          width: 16,
                          height: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : Icon(
                          widget.previewAction != null
                              ? Icons.preview_outlined
                              : Icons.play_arrow,
                          size: 18,
                        ),
                  label: Text(
                    widget.previewAction != null
                        ? 'Simular alterações'
                        : 'Executar na frota',
                  ),
                ),
              ],
            ),
          ],
        ),
      );
    }

    if (_step == _FlowStep.review) {
      final succeededIds = (previewBatch?.devices ?? const <BatchDevice>[])
          .where((d) => d.status == 'succeeded')
          .map((d) => d.deviceId)
          .toList(growable: false);
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (previewBatch == null || previewBatch.isRunning)
            const LoadingStateView(
              message: 'Calculando prévia das alterações via SSE...',
            )
          else ...<Widget>[
            for (final d in previewBatch.devices) ...<Widget>[
              NestedPanel(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Row(
                      children: <Widget>[
                        Expanded(
                          child: Text(
                            d.deviceName,
                            style: const TextStyle(fontWeight: FontWeight.w700),
                          ),
                        ),
                        SemanticChip(
                          label: batchStatusPresentation(d.status).label,
                          tone: batchStatusPresentation(d.status).tone,
                          compact: true,
                        ),
                      ],
                    ),
                    if (d.error != null) Text(d.error!),
                    if (d.output != null) ...<Widget>[
                      const SizedBox(height: 6),
                      PluginOutputView(output: d.output),
                    ],
                  ],
                ),
              ),
              const SizedBox(height: 8),
            ],
            const SizedBox(height: 12),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: <Widget>[
                TextButton(
                  onPressed: () => setState(() => _step = _FlowStep.form),
                  child: const Text('Voltar'),
                ),
                const SizedBox(width: 8),
                FilledButton.icon(
                  onPressed: succeededIds.isEmpty || _submitting
                      ? null
                      : () => _launchFinalRun(
                          normalizePluginFormParams(
                            widget.action.paramsSchema,
                            _params,
                          ),
                          succeededIds,
                        ),
                  icon: const Icon(Icons.check, size: 18),
                  label: Text('Aplicar agora (${succeededIds.length})'),
                ),
              ],
            ),
          ],
        ],
      );
    }

    // Running / Done
    final batch = runBatch;
    final doneCount = (batch?.devices ?? const <BatchDevice>[])
        .where((d) => d.status != 'running' && d.status != 'pending')
        .length;
    final total = batch?.devices.length ?? _selectedDevices.length;
    final progress = total > 0 ? doneCount / total : null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        LinearProgressIndicator(value: progress),
        const SizedBox(height: 8),
        Text(
          batch == null
              ? 'Iniciando lote...'
              : '${batchStatusPresentation(batch.status).label} · $doneCount/$total equipamento(s)',
          style: context.textTheme.bodySmall?.copyWith(
            fontWeight: FontWeight.w700,
          ),
        ),
        const SizedBox(height: 12),
        if (batch != null) ...<Widget>[
          for (final d in batch.devices) ...<Widget>[
            Builder(
              builder: (_) {
                final step = d.runId != null
                    ? appsState.lastSteps[d.runId!]
                    : null;
                final pres = batchStatusPresentation(d.status);
                return NestedPanel(
                  padding: const EdgeInsets.all(10),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Row(
                        children: <Widget>[
                          Expanded(
                            child: Text(
                              d.deviceName,
                              style: const TextStyle(
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                          ),
                          SemanticChip(
                            label: pres.label,
                            tone: pres.tone,
                            compact: true,
                          ),
                        ],
                      ),
                      if (step != null && d.status == 'running')
                        Text(step.request, style: context.textTheme.labelSmall),
                      if (d.error != null)
                        Text(
                          d.error!,
                          style: TextStyle(color: context.colorScheme.error),
                        ),
                    ],
                  ),
                );
              },
            ),
            const SizedBox(height: 6),
          ],
          if (batch.result != null) ...<Widget>[
            const SizedBox(height: 8),
            PluginOutputView(output: batch.result),
          ],
        ],
        const SizedBox(height: 12),
        Row(
          mainAxisAlignment: MainAxisAlignment.end,
          children: <Widget>[
            if (batch != null && batch.isRunning)
              OutlinedButton(
                onPressed: () =>
                    ref.read(pluginAppsProvider.notifier).cancelBatch(batch.id),
                child: const Text('Cancelar lote'),
              ),
            if (batch != null &&
                !batch.isRunning &&
                batch.hasPatch) ...<Widget>[
              FilledButton.tonal(
                onPressed: () =>
                    ref.read(pluginAppsProvider.notifier).applyPatch(batch),
                child: const Text('Aceitar sugestão'),
              ),
              const SizedBox(width: 8),
            ],
            FilledButton(
              onPressed: () => Navigator.of(context).pop(),
              child: const Text('Fechar'),
            ),
          ],
        ),
      ],
    );
  }
}
