import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/form_validators.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/alert_models.dart';
import 'package:netmonitor_app/features/core/providers/alerts_provider.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

const List<(int, String)> _silencePresets = <(int, String)>[
  (15, '15 min'),
  (30, '30 min'),
  (60, '1 hora'),
  (120, '2 horas'),
  (240, '4 horas'),
  (480, '8 horas'),
  (1440, '24 horas'),
];

/// Diálogo / BottomSheet para silenciar um alerta (`AlertSilenceDialog`).
Future<bool> showAlertSilenceSheet({
  required BuildContext context,
  required WidgetRef ref,
  required AlertEvent alert,
}) async {
  final result = await showAdaptiveFormSheet<bool>(
    context: context,
    title: 'Silenciar alerta',
    subtitle: alert.title,
    builder: (_) => _AlertSilenceSheetBody(alertId: alert.id),
  );
  return result == true;
}

class _AlertSilenceSheetBody extends ConsumerStatefulWidget {
  const _AlertSilenceSheetBody({required this.alertId});

  final int alertId;

  @override
  ConsumerState<_AlertSilenceSheetBody> createState() =>
      _AlertSilenceSheetBodyState();
}

class _AlertSilenceSheetBodyState
    extends ConsumerState<_AlertSilenceSheetBody> {
  int _selectedMinutes = 60;
  final TextEditingController _customCtrl = TextEditingController(text: '60');
  bool _saving = false;

  @override
  void dispose() {
    _customCtrl.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    final mins = int.tryParse(_customCtrl.text.trim()) ?? _selectedMinutes;
    if (mins <= 0) return;
    setState(() => _saving = true);
    final ok = await ref
        .read(alertsProvider.notifier)
        .silenceAlert(widget.alertId, mins);
    if (mounted) Navigator.of(context).pop(ok);
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          'Escolha por quanto tempo suprimir notificações deste incidente:',
          style: context.textTheme.bodyMedium,
        ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            for (final (mins, label) in _silencePresets)
              ChoiceChip(
                selected: _selectedMinutes == mins,
                label: Text(label),
                onSelected: (_) {
                  setState(() {
                    _selectedMinutes = mins;
                    _customCtrl.text = '$mins';
                  });
                },
              ),
          ],
        ),
        const SizedBox(height: 14),
        TextField(
          controller: _customCtrl,
          keyboardType: TextInputType.number,
          decoration: const InputDecoration(
            labelText: 'Duração personalizada (minutos)',
            prefixIcon: Icon(Icons.timer_outlined),
          ),
          onChanged: (v) {
            final parsed = int.tryParse(v.trim());
            if (parsed != null && parsed > 0) {
              setState(() => _selectedMinutes = parsed);
            }
          },
        ),
        const SizedBox(height: 18),
        FilledButton.icon(
          onPressed: _saving ? null : _submit,
          icon: _saving
              ? const SizedBox(
                  width: 18,
                  height: 18,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.notifications_off_outlined),
          label: const Text('Silenciar'),
        ),
      ],
    );
  }
}

/// Diálogo de Correlação & Análise de Causa Raiz (`AlertCorrelationDialog`).
Future<void> showAlertCorrelationSheet({
  required BuildContext context,
  required WidgetRef ref,
  required AlertEvent alert,
}) async {
  await showAdaptiveFormSheet<void>(
    context: context,
    title: 'Correlação e Causa Raiz',
    subtitle: alert.title,
    maxWidth: 640,
    builder: (sheetCtx) {
      return FutureBuilder<AlertCorrelation?>(
        future: ref.read(alertsProvider.notifier).fetchCorrelation(alert.id),
        builder: (context, snapshot) {
          if (snapshot.connectionState == ConnectionState.waiting) {
            return const LoadingStateView(
              message: 'Analisando cadeia de dependência...',
            );
          }
          final data = snapshot.data;
          if (data == null) {
            return const EmptyStateView(
              title: 'Sem correlação disponível',
              description:
                  'Não foram encontrados incidentes correlacionados nesta janela.',
              icon: Icons.hub_outlined,
            );
          }

          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              NestedPanel(
                tone: SemanticTone.info,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Wrap(
                      spacing: 8,
                      runSpacing: 6,
                      children: <Widget>[
                        SemanticChip(
                          label: data.causalCategoryLabel,
                          tone: SemanticTone.primary,
                          compact: true,
                        ),
                        SemanticChip(
                          label:
                              'Confiança ${formatPercent(data.confidence * 100, digits: 0)}',
                          tone: SemanticTone.info,
                          compact: true,
                        ),
                        SemanticChip(
                          label: countNoun(
                            data.impactedDevicesCount,
                            'dispositivo impactado',
                            'dispositivos impactados',
                          ),
                          tone: SemanticTone.warning,
                          compact: true,
                        ),
                      ],
                    ),
                    const SizedBox(height: 10),
                    Text(data.explanation, style: context.textTheme.bodyMedium),
                    if (data.primaryCauseTitle != null) ...<Widget>[
                      const SizedBox(height: 8),
                      Text(
                        'Causa primária: ${data.primaryCauseTitle}',
                        style: context.textTheme.labelLarge?.copyWith(
                          fontWeight: FontWeight.w700,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              if (data.dependencyChain.isNotEmpty) ...<Widget>[
                const SizedBox(height: 14),
                Text(
                  'Cadeia topológica',
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 8),
                for (final node in data.dependencyChain)
                  ListTile(
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    leading: Icon(
                      node['isRootCause'] == true
                          ? Icons.error_outline
                          : Icons.router_outlined,
                      color: node['isRootCause'] == true
                          ? context.colorScheme.error
                          : context.colorScheme.primary,
                    ),
                    title: Text((node['name'] ?? '').toString()),
                    subtitle: Text(
                      (node['ipAddress'] ?? node['type'] ?? '').toString(),
                    ),
                    trailing: node['isRootCause'] == true
                        ? const SemanticChip(
                            label: 'Causa raiz',
                            tone: SemanticTone.error,
                            compact: true,
                          )
                        : null,
                  ),
              ],
              if (data.relatedEvents.isNotEmpty) ...<Widget>[
                const SizedBox(height: 14),
                Text(
                  'Eventos relacionados (${data.relatedEvents.length})',
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 6),
                for (final ev in data.relatedEvents)
                  ListTile(
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    title: Text((ev['title'] ?? '').toString()),
                    subtitle: Text(
                      (ev['message'] ?? '').toString(),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                    trailing: StatusIndicator.fromStatus(
                      (ev['status'] ?? '').toString(),
                      compact: true,
                    ),
                  ),
              ],
            ],
          );
        },
      );
    },
  );
}

/// Formulário completo de Regra de Alerta (`AlertRuleFormDialog`).
Future<bool> showAlertRuleFormSheet({
  required BuildContext context,
  required WidgetRef ref,
  AlertRule? existing,
  int? presetDeviceId,
  String? presetLogRegex,
}) async {
  final monitorsState = ref.read(monitorsProvider);
  if (monitorsState.devices.isEmpty) {
    await ref.read(monitorsProvider.notifier).fetchReferenceCatalogs();
  }
  if (!context.mounted) return false;

  final result = await showAdaptiveFormSheet<bool>(
    context: context,
    title: existing == null ? 'Nova regra de alerta' : 'Editar regra de alerta',
    subtitle:
        'Defina condição, severidade, escopo e controles contra oscilação.',
    maxWidth: 640,
    builder: (_) => _AlertRuleFormBody(
      existing: existing,
      presetDeviceId: presetDeviceId,
      presetLogRegex: presetLogRegex,
    ),
  );
  return result == true;
}

class _AlertRuleFormBody extends ConsumerStatefulWidget {
  const _AlertRuleFormBody({
    this.existing,
    this.presetDeviceId,
    this.presetLogRegex,
  });

  final AlertRule? existing;
  final int? presetDeviceId;
  final String? presetLogRegex;

  @override
  ConsumerState<_AlertRuleFormBody> createState() => _AlertRuleFormBodyState();
}

class _AlertRuleFormBodyState extends ConsumerState<_AlertRuleFormBody> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();
  late final TextEditingController _nameCtrl;
  late final TextEditingController _fieldCtrl;
  late final TextEditingController _valueCtrl;
  late final TextEditingController _durationCtrl;
  late final TextEditingController _recoveryCtrl;
  late final TextEditingController _flapThresholdCtrl;
  late final TextEditingController _cooldownCtrl;

  String _type = 'device_offline';
  String _operator = '==';
  String _severity = 'warning';
  int? _siteId;
  int? _deviceId;
  int? _monitorId;
  bool _inhibitParent = true;
  bool _enabled = true;
  bool _saving = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    final e = widget.existing;
    _type =
        e?.type ??
        (widget.presetLogRegex != null ? 'custom' : 'device_offline');
    _operator =
        e?.condition.operator ??
        (widget.presetLogRegex != null ? 'matches' : '==');
    _severity = e?.severity ?? 'warning';
    _siteId = e?.siteId;
    _deviceId = e?.deviceId ?? widget.presetDeviceId;
    _monitorId = e?.monitorId;
    _inhibitParent = e?.inhibitWhenParentDown ?? true;
    _enabled = e?.isEnabled ?? true;

    _nameCtrl = TextEditingController(text: e?.name ?? '');
    _fieldCtrl = TextEditingController(
      text:
          e?.condition.field ??
          (widget.presetLogRegex != null ? 'log_pattern' : 'status'),
    );
    _valueCtrl = TextEditingController(
      text: e?.condition.value.toString() ?? widget.presetLogRegex ?? 'down',
    );
    _durationCtrl = TextEditingController(
      text: (e?.durationSeconds ?? 60).toString(),
    );
    _recoveryCtrl = TextEditingController(
      text: (e?.recoveryWindowSeconds ?? 0).toString(),
    );
    _flapThresholdCtrl = TextEditingController(
      text: (e?.flapThreshold ?? 0).toString(),
    );
    _cooldownCtrl = TextEditingController(
      text: (e?.notificationCooldownSeconds ?? 0).toString(),
    );
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _fieldCtrl.dispose();
    _valueCtrl.dispose();
    _durationCtrl.dispose();
    _recoveryCtrl.dispose();
    _flapThresholdCtrl.dispose();
    _cooldownCtrl.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    if (!_formKey.currentState!.validate()) return;
    setState(() {
      _saving = true;
      _error = null;
    });

    final rawVal = _valueCtrl.text.trim();
    final numVal = num.tryParse(rawVal);

    final payload = <String, dynamic>{
      'name': _nameCtrl.text.trim(),
      'type': _type,
      'severity': _severity,
      'siteId': _siteId,
      'deviceId': _deviceId,
      'monitorId': _monitorId,
      'condition': <String, dynamic>{
        'field': _fieldCtrl.text.trim(),
        'operator': _operator,
        'value': numVal ?? rawVal,
      },
      'durationSeconds': int.tryParse(_durationCtrl.text.trim()) ?? 60,
      'recoveryWindowSeconds': int.tryParse(_recoveryCtrl.text.trim()) ?? 0,
      'flapThreshold': int.tryParse(_flapThresholdCtrl.text.trim()) ?? 0,
      'notificationCooldownSeconds':
          int.tryParse(_cooldownCtrl.text.trim()) ?? 0,
      'inhibitWhenParentDown': _inhibitParent,
      'enabled': _enabled,
      'isEnabled': _enabled,
    };

    final notifier = ref.read(alertsProvider.notifier);
    final ok = widget.existing == null
        ? await notifier.createAlertRule(payload)
        : await notifier.updateAlertRule(widget.existing!.id, payload);

    if (!mounted) return;
    if (ok) {
      Navigator.of(context).pop(true);
    } else {
      setState(() {
        _saving = false;
        _error = ref.read(alertsProvider).error ?? 'Erro ao salvar regra.';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final monitorsState = ref.watch(monitorsProvider);

    return Form(
      key: _formKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (_error != null) ...<Widget>[
            NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
            const SizedBox(height: 12),
          ],
          TextFormField(
            controller: _nameCtrl,
            decoration: const InputDecoration(labelText: 'Nome da regra'),
            validator: FormValidators.required('Informe o nome da regra.'),
          ),
          const SizedBox(height: 12),
          ResponsiveFieldRow(
            breakpoint: 480,
            children: <Widget>[
              AppDropdownFormField<String>(
                initialValue: _type,
                decoration: const InputDecoration(labelText: 'Tipo'),
                items: const <DropdownMenuItem<String>>[
                  DropdownMenuItem(
                    value: 'device_offline',
                    child: Text(
                      'Dispositivo offline',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  DropdownMenuItem(
                    value: 'latency_high',
                    child: Text(
                      'Latência alta',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                  DropdownMenuItem(
                    value: 'http_failure',
                    child: Text('Falha HTTP', overflow: TextOverflow.ellipsis),
                  ),
                  DropdownMenuItem(
                    value: 'tcp_failure',
                    child: Text('Falha TCP', overflow: TextOverflow.ellipsis),
                  ),
                  DropdownMenuItem(
                    value: 'custom',
                    child: Text(
                      'Personalizada / Log',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ],
                onChanged: (v) {
                  if (v != null) setState(() => _type = v);
                },
              ),
              AppDropdownFormField<String>(
                initialValue: _severity,
                decoration: const InputDecoration(labelText: 'Severidade'),
                items: const <DropdownMenuItem<String>>[
                  DropdownMenuItem(
                    value: 'info',
                    child: Text('Informação', overflow: TextOverflow.ellipsis),
                  ),
                  DropdownMenuItem(
                    value: 'warning',
                    child: Text('Aviso', overflow: TextOverflow.ellipsis),
                  ),
                  DropdownMenuItem(
                    value: 'critical',
                    child: Text('Crítico', overflow: TextOverflow.ellipsis),
                  ),
                ],
                onChanged: (v) {
                  if (v != null) setState(() => _severity = v);
                },
              ),
            ],
          ),
          const SizedBox(height: 12),
          ResponsiveFieldRow(
            breakpoint: 640,
            children: <Widget>[
              TextFormField(
                controller: _fieldCtrl,
                decoration: const InputDecoration(
                  labelText: 'Campo',
                  hintText: 'status, latency_ms, cpu_usage, log_pattern',
                ),
                validator: FormValidators.required('Campo obrigatório.'),
              ),
              AppDropdownFormField<String>(
                initialValue: _operator,
                decoration: const InputDecoration(labelText: 'Operador'),
                items: const <DropdownMenuItem<String>>[
                  DropdownMenuItem(value: '==', child: Text('==')),
                  DropdownMenuItem(value: '!=', child: Text('!=')),
                  DropdownMenuItem(value: '>', child: Text('>')),
                  DropdownMenuItem(value: '>=', child: Text('>=')),
                  DropdownMenuItem(value: '<', child: Text('<')),
                  DropdownMenuItem(value: '<=', child: Text('<=')),
                  DropdownMenuItem(value: 'matches', child: Text('regex')),
                ],
                onChanged: (v) {
                  if (v != null) setState(() => _operator = v);
                },
              ),
              TextFormField(
                controller: _valueCtrl,
                decoration: const InputDecoration(labelText: 'Valor / Regex'),
                validator: FormValidators.required('Informe o valor.'),
              ),
            ],
          ),
          const SizedBox(height: 12),
          AppDropdownFormField<int?>(
            initialValue: _deviceId,
            decoration: const InputDecoration(
              labelText: 'Escopo — Dispositivo (opcional)',
            ),
            items: <DropdownMenuItem<int?>>[
              const DropdownMenuItem<int?>(
                value: null,
                child: Text('Global (todos)', overflow: TextOverflow.ellipsis),
              ),
              for (final d in monitorsState.devices)
                DropdownMenuItem<int?>(
                  value: d.id,
                  child: Text(d.name, overflow: TextOverflow.ellipsis),
                ),
            ],
            onChanged: (v) => setState(() => _deviceId = v),
          ),
          const SizedBox(height: 12),
          ResponsiveFieldRow(
            breakpoint: 480,
            children: <Widget>[
              TextFormField(
                controller: _durationCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(labelText: 'Duração (s)'),
              ),
              TextFormField(
                controller: _recoveryCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(
                  labelText: 'Estabilização (s)',
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          ResponsiveFieldRow(
            breakpoint: 480,
            children: <Widget>[
              TextFormField(
                controller: _flapThresholdCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(labelText: 'Limiar flapping'),
              ),
              TextFormField(
                controller: _cooldownCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(labelText: 'Cooldown (s)'),
              ),
            ],
          ),
          const SizedBox(height: 8),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            value: _inhibitParent,
            onChanged: (v) => setState(() => _inhibitParent = v),
            title: const Text('Suprimir se o equipamento-pai estiver fora'),
          ),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            value: _enabled,
            onChanged: (v) => setState(() => _enabled = v),
            title: const Text('Regra ativa'),
          ),
          const SizedBox(height: 14),
          FilledButton.icon(
            onPressed: _saving ? null : _submit,
            icon: _saving
                ? const SizedBox(
                    width: 18,
                    height: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.save_outlined),
            label: Text(
              widget.existing == null ? 'Criar regra' : 'Salvar regra',
            ),
          ),
        ],
      ),
    );
  }
}

/// Catálogo de templates de regras (`AlertRuleCatalogDialog`).
Future<void> showAlertRuleCatalogSheet({
  required BuildContext context,
  required WidgetRef ref,
  int? deviceId,
  String? preselectKey,
}) async {
  await ref.read(alertsProvider.notifier).fetchRuleCatalog(deviceId: deviceId);
  if (!context.mounted) return;

  final selectedKeys = <String>{
    if (preselectKey != null && preselectKey.isNotEmpty) preselectKey,
  };
  var applying = false;

  await showAdaptiveFormSheet<void>(
    context: context,
    title: 'Catálogo de regras prontas',
    subtitle:
        'Aplique templates validados em lote sem duplicar regras já existentes.',
    maxWidth: 680,
    builder: (sheetCtx) {
      return StatefulBuilder(
        builder: (context, setModalState) {
          final alerts = ref.watch(alertsProvider);
          final templates = alerts.ruleTemplates;
          if (alerts.catalogLoading && templates.isEmpty) {
            return const LoadingStateView(message: 'Carregando catálogo...');
          }
          if (templates.isEmpty) {
            return const EmptyStateView(
              title: 'Nenhum template disponível',
              icon: Icons.rule_folder_outlined,
            );
          }

          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: <Widget>[
                  OutlinedButton(
                    onPressed: () {
                      setModalState(() {
                        for (final t in templates) {
                          if (t.recommended && !t.applied && t.applicable) {
                            selectedKeys.add(t.key);
                          }
                        }
                      });
                    },
                    child: const Text('Selecionar recomendadas'),
                  ),
                  TextButton(
                    onPressed: () => setModalState(selectedKeys.clear),
                    child: const Text('Limpar seleção'),
                  ),
                ],
              ),
              const SizedBox(height: 10),
              for (final tmpl in templates) ...<Widget>[
                CheckboxListTile(
                  contentPadding: EdgeInsets.zero,
                  value: tmpl.applied || selectedKeys.contains(tmpl.key),
                  onChanged: tmpl.applied
                      ? null
                      : (v) {
                          setModalState(() {
                            if (v == true) {
                              selectedKeys.add(tmpl.key);
                            } else {
                              selectedKeys.remove(tmpl.key);
                            }
                          });
                        },
                  title: Wrap(
                    spacing: 6,
                    runSpacing: 4,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: <Widget>[
                      Text(
                        tmpl.name,
                        style: const TextStyle(fontWeight: FontWeight.w700),
                      ),
                      if (tmpl.recommended)
                        const SemanticChip(
                          label: 'Recomendada',
                          tone: SemanticTone.primary,
                          compact: true,
                        ),
                      if (tmpl.applied)
                        const SemanticChip(
                          label: 'Já configurada',
                          tone: SemanticTone.success,
                          compact: true,
                        ),
                    ],
                  ),
                  subtitle: Text(
                    '${alerts.ruleCategories[tmpl.category] ?? tmpl.category} · ${tmpl.description}',
                  ),
                ),
                const Divider(height: 1),
              ],
              const SizedBox(height: 16),
              FilledButton.icon(
                onPressed: selectedKeys.isEmpty || applying
                    ? null
                    : () async {
                        setModalState(() => applying = true);
                        final count = await ref
                            .read(alertsProvider.notifier)
                            .applyCatalogRules(
                              selectedKeys.toList(growable: false),
                              deviceId: deviceId,
                            );
                        if (sheetCtx.mounted) {
                          Navigator.of(sheetCtx).pop();
                          ScaffoldMessenger.of(context).showSnackBar(
                            SnackBar(
                              content: Text(
                                '${countNoun(count, 'regra aplicada', 'regras aplicadas')} com sucesso.',
                              ),
                            ),
                          );
                        }
                      },
                icon: applying
                    ? const SizedBox(
                        width: 18,
                        height: 18,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.playlist_add_check),
                label: Text('Aplicar selecionadas (${selectedKeys.length})'),
              ),
            ],
          );
        },
      );
    },
  );
}
