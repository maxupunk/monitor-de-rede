import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/form_validators.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/core/models/event_models.dart';
import 'package:netmonitor_app/features/core/models/operations_models.dart';
import 'package:netmonitor_app/features/core/providers/scheduled_tasks_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

const List<(String, String)> _cronPresets = <(String, String)>[
  ('*/15 * * * *', 'A cada 15 min'),
  ('0 * * * *', 'A cada hora'),
  ('0 3 * * *', 'Todo dia às 03:00'),
  ('0 8 * * 1-5', 'Seg–Sex às 08:00'),
  ('0 4 * * 0', 'Domingos às 04:00'),
];

/// Tela de Tarefas Agendadas (paridade 100% com `ScheduledTasksPage.vue`).
class ScheduledTasksPage extends ConsumerStatefulWidget {
  const ScheduledTasksPage({super.key});

  @override
  ConsumerState<ScheduledTasksPage> createState() => _ScheduledTasksPageState();
}

class _ScheduledTasksPageState extends ConsumerState<ScheduledTasksPage> {
  final TextEditingController _searchCtrl = TextEditingController();

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      ref.read(scheduledTasksProvider.notifier).fetchTasks();
      ref.read(scheduledTasksProvider.notifier).fetchCatalog();
    });
  }

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  Future<void> _openTaskForm({
    ScheduledTask? existing,
    String? presetName,
    String? presetTool,
    String? presetCron,
  }) async {
    await ref.read(scheduledTasksProvider.notifier).fetchCatalog();
    if (!mounted) return;

    await showAdaptiveFormSheet<void>(
      context: context,
      title: existing == null
          ? 'Nova tarefa agendada'
          : 'Editar tarefa agendada',
      subtitle:
          'Configure a ferramenta, parâmetros JSON Schema e agendamento Cron.',
      maxWidth: 660,
      builder: (_) => _ScheduledTaskFormSheetBody(
        existing: existing,
        presetName: presetName,
        presetTool: presetTool,
        presetCron: presetCron,
      ),
    );
  }

  Future<void> _openRunsSheet(ScheduledTask task) async {
    await showAdaptiveFormSheet<void>(
      context: context,
      title: 'Histórico de execuções',
      subtitle: task.name,
      maxWidth: 680,
      builder: (_) => _TaskRunsBody(task: task),
    );
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(scheduledTasksProvider);
    final auth = ref.watch(authProvider);
    final q = _searchCtrl.text.trim().toLowerCase();
    final filtered = state.tasks
        .where((t) {
          if (q.isEmpty) return true;
          return t.name.toLowerCase().contains(q) ||
              t.tool.toLowerCase().contains(q) ||
              t.scheduleLabel.toLowerCase().contains(q);
        })
        .toList(growable: false);

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 680;
        final scheme = context.colorScheme;

        return RefreshIndicator(
          onRefresh: () =>
              ref.read(scheduledTasksProvider.notifier).fetchTasks(),
          child: ListView(
            children: <Widget>[
              if (isCompact)
                Row(
                  children: <Widget>[
                    Expanded(
                      child: Text(
                        'Tarefas Agendadas',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.headlineSmall?.copyWith(
                          fontWeight: FontWeight.w800,
                          color: scheme.onSurface,
                        ),
                      ),
                    ),
                    if (auth.canWrite) ...<Widget>[
                      const SizedBox(width: 8),
                      FilledButton.icon(
                        style: FilledButton.styleFrom(
                          visualDensity: VisualDensity.compact,
                          padding: const EdgeInsets.symmetric(
                            horizontal: 12,
                            vertical: 8,
                          ),
                        ),
                        onPressed: () => _openTaskForm(),
                        icon: const Icon(Icons.add, size: 18),
                        label: const Text('Nova'),
                      ),
                    ],
                  ],
                )
              else
                Wrap(
                  spacing: 12,
                  runSpacing: 10,
                  alignment: WrapAlignment.spaceBetween,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: <Widget>[
                    Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          'Tarefas Agendadas',
                          style: context.textTheme.headlineSmall?.copyWith(
                            fontWeight: FontWeight.w800,
                            color: scheme.onSurface,
                          ),
                        ),
                        const SizedBox(height: 2),
                        Text(
                          'Automação periódica com prévia de risco e histórico por execução',
                          style: context.textTheme.bodyMedium?.copyWith(
                            color: scheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                    if (auth.canWrite)
                      FilledButton.icon(
                        onPressed: () => _openTaskForm(),
                        icon: const Icon(Icons.add, size: 18),
                        label: const Text('Nova tarefa'),
                      ),
                  ],
                ),
              const SizedBox(height: 10),
              UnifiedFilterBar(
                searchController: _searchCtrl,
                searchHint: 'Buscar tarefa ou ferramenta...',
                onSearchChanged: (_) => setState(() {}),
                quickChips: auth.canWrite
                    ? <Widget>[
                        ActionChip(
                          visualDensity: VisualDensity.compact,
                          materialTapTargetSize:
                              MaterialTapTargetSize.shrinkWrap,
                          avatar: const Icon(Icons.radar, size: 16),
                          label: const Text('Modelo: Checagem diária'),
                          onPressed: () => _openTaskForm(
                            presetName: 'Checagem diária de rede',
                            presetCron: '0 7 * * *',
                          ),
                        ),
                        if (auth.isAdmin)
                          ActionChip(
                            visualDensity: VisualDensity.compact,
                            materialTapTargetSize:
                                MaterialTapTargetSize.shrinkWrap,
                            avatar: const Icon(
                              Icons.view_in_ar_outlined,
                              size: 16,
                            ),
                            label: const Text('Modelo: Rotina Docker (Admin)'),
                            onPressed: () => _openTaskForm(
                              presetName: 'Rotina programada de container',
                              presetCron: '0 4 * * 0',
                            ),
                          ),
                      ]
                    : const <Widget>[],
              ),
              const SizedBox(height: 14),
              if (state.loading && state.tasks.isEmpty)
                const LoadingStateView(
                  message: 'Carregando tarefas agendadas...',
                )
              else if (state.error != null && state.tasks.isEmpty)
                ErrorStateView(
                  message: state.error!,
                  onRetry: () =>
                      ref.read(scheduledTasksProvider.notifier).fetchTasks(),
                )
              else if (filtered.isEmpty)
                EmptyStateView(
                  title: 'Nenhuma tarefa agendada',
                  description:
                      'Crie automações recorrentes usando as ferramentas do catálogo.',
                  icon: Icons.event_repeat_outlined,
                  action: auth.canWrite
                      ? FilledButton.icon(
                          onPressed: () => _openTaskForm(),
                          icon: const Icon(Icons.add),
                          label: const Text('Nova tarefa'),
                        )
                      : null,
                )
              else
                for (final task in filtered) ...<Widget>[
                  SectionCard(
                    title: task.name,
                    subtitle: task.cron.trim().isNotEmpty
                        ? '${task.scheduleLabel} (${task.cron})'
                        : task.scheduleLabel,
                    icon: Icons.event_repeat_outlined,
                    iconTone: task.lastStatus == 'failed'
                        ? SemanticTone.error
                        : (task.enabled
                              ? SemanticTone.primary
                              : SemanticTone.secondary),
                    trailing: auth.canWrite
                        ? Switch(
                            value: task.enabled,
                            onChanged: (v) async {
                              try {
                                await ref
                                    .read(scheduledTasksProvider.notifier)
                                    .setEnabled(task.id, v);
                              } catch (err) {
                                if (!context.mounted) return;
                                ScaffoldMessenger.of(context).showSnackBar(
                                  SnackBar(content: Text(describeError(err))),
                                );
                              }
                            },
                          )
                        : SemanticChip(
                            label: task.enabled ? 'Ativa' : 'Pausada',
                            tone: task.enabled
                                ? SemanticTone.success
                                : SemanticTone.secondary,
                            compact: true,
                          ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: <Widget>[
                        Wrap(
                          spacing: 6,
                          runSpacing: 6,
                          children: <Widget>[
                            if (task.tool.trim().isNotEmpty)
                              SemanticChip(
                                label: task.tool,
                                tone: SemanticTone.info,
                                compact: true,
                              ),
                            if (task.origin == 'ai')
                              const SemanticChip(
                                label: 'Criada pela IA',
                                tone: SemanticTone.ai,
                                compact: true,
                              ),
                            if (task.running)
                              const SemanticChip(
                                label: 'Executando...',
                                tone: SemanticTone.warning,
                                compact: true,
                              )
                            else if (task.lastStatus != null)
                              SemanticChip(
                                label: switch (task.lastStatus) {
                                  'success' ||
                                  'succeeded' ||
                                  'ok' => 'Concluída',
                                  'failed' || 'error' => 'Falhou',
                                  'running' => 'Executando',
                                  _ => task.lastStatus!,
                                },
                                tone: switch (task.lastStatus) {
                                  'success' ||
                                  'succeeded' ||
                                  'ok' => SemanticTone.success,
                                  'failed' || 'error' => SemanticTone.error,
                                  _ => SemanticTone.info,
                                },
                                compact: true,
                              ),
                            if (task.ownerName != null)
                              SemanticChip(
                                label: 'Por ${task.ownerName!}',
                                tone: SemanticTone.secondary,
                                compact: true,
                              ),
                          ],
                        ),
                        const SizedBox(height: 8),
                        Text(
                          'Próxima: ${task.nextRunAt != null ? formatShortDateTime(task.nextRunAt) : '—'} · '
                          'Última: ${formatRelativeTime(task.lastRunAt)}',
                          style: context.textTheme.bodySmall?.copyWith(
                            color: context.colorScheme.onSurfaceVariant,
                          ),
                        ),
                        if (task.lastMessage != null &&
                            task.lastMessage!.isNotEmpty) ...<Widget>[
                          const SizedBox(height: 4),
                          Text(
                            task.lastMessage!,
                            style: context.textTheme.bodySmall,
                          ),
                        ],
                        const SizedBox(height: 10),
                        SymmetricalActionBar(
                          spacing: 8,
                          runSpacing: 8,
                          children: <Widget>[
                            if (auth.canWrite)
                              FilledButton.tonalIcon(
                                onPressed: task.running
                                    ? null
                                    : () async {
                                        try {
                                          await ref
                                              .read(
                                                scheduledTasksProvider.notifier,
                                              )
                                              .runNow(task.id);
                                          if (!context.mounted) return;
                                          ScaffoldMessenger.of(
                                            context,
                                          ).showSnackBar(
                                            const SnackBar(
                                              content: Text(
                                                'Execução disparada. Aguarde o retorno via SSE.',
                                              ),
                                            ),
                                          );
                                        } catch (err) {
                                          if (!context.mounted) return;
                                          ScaffoldMessenger.of(
                                            context,
                                          ).showSnackBar(
                                            SnackBar(
                                              content: Text(describeError(err)),
                                            ),
                                          );
                                        }
                                      },
                                icon: const Icon(Icons.play_arrow, size: 18),
                                label: const Text('Rodar agora'),
                              ),
                            OutlinedButton.icon(
                              onPressed: () => _openRunsSheet(task),
                              icon: const Icon(Icons.history, size: 18),
                              label: const Text('Histórico'),
                            ),
                            if (auth.canWrite) ...<Widget>[
                              OutlinedButton.icon(
                                onPressed: () => _openTaskForm(existing: task),
                                icon: const Icon(Icons.edit_outlined, size: 18),
                                label: const Text('Editar'),
                              ),
                              OutlinedButton.icon(
                                onPressed: () async {
                                  final ok = await showConfirmActionDialog(
                                    context: context,
                                    title: 'Excluir tarefa?',
                                    message:
                                        'A tarefa "${task.name}" será removida permanentemente.',
                                    confirmLabel: 'Excluir',
                                  );
                                  if (ok) {
                                    try {
                                      await ref
                                          .read(scheduledTasksProvider.notifier)
                                          .remove(task.id);
                                    } catch (err) {
                                      if (!context.mounted) return;
                                      ScaffoldMessenger.of(
                                        context,
                                      ).showSnackBar(
                                        SnackBar(
                                          content: Text(describeError(err)),
                                        ),
                                      );
                                    }
                                  }
                                },
                                icon: Icon(
                                  Icons.delete_outline,
                                  size: 18,
                                  color: context.colorScheme.error,
                                ),
                                label: Text(
                                  'Excluir',
                                  style: TextStyle(
                                    color: context.colorScheme.error,
                                  ),
                                ),
                              ),
                            ],
                          ],
                        ),
                      ],
                    ),
                  ),
                  const SizedBox(height: 10),
                ],
            ],
          ),
        );
      },
    );
  }
}

class _ScheduledTaskFormSheetBody extends ConsumerStatefulWidget {
  const _ScheduledTaskFormSheetBody({
    this.existing,
    this.presetName,
    this.presetTool,
    this.presetCron,
  });

  final ScheduledTask? existing;
  final String? presetName;
  final String? presetTool;
  final String? presetCron;

  @override
  ConsumerState<_ScheduledTaskFormSheetBody> createState() =>
      _ScheduledTaskFormSheetBodyState();
}

class _ScheduledTaskFormSheetBodyState
    extends ConsumerState<_ScheduledTaskFormSheetBody> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();
  late final TextEditingController _nameCtrl;
  late final TextEditingController _descCtrl;
  late final TextEditingController _cronCtrl;
  late String _selectedTool;
  late final Map<String, dynamic> _args;
  late bool _enabled;
  ScheduledTaskPreview? _preview;
  bool _previewLoading = false;
  bool _saving = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    final existing = widget.existing;
    final auth = ref.read(authProvider);
    final allTools = ref
        .read(scheduledTasksProvider)
        .catalog
        .where((t) => auth.isAdmin || !t.adminOnly)
        .toList(growable: false);

    _nameCtrl = TextEditingController(
      text: existing?.name ?? widget.presetName ?? '',
    );
    _descCtrl = TextEditingController(text: existing?.description ?? '');
    _cronCtrl = TextEditingController(
      text: existing?.cron ?? widget.presetCron ?? '0 3 * * *',
    );
    _selectedTool =
        existing?.tool ??
        (widget.presetTool != null &&
                allTools.any((t) => t.name == widget.presetTool)
            ? widget.presetTool!
            : (allTools.isNotEmpty ? allTools.first.name : ''));
    _args = Map<String, dynamic>.from(
      existing?.arguments ?? const <String, dynamic>{},
    );
    _enabled = existing?.enabled ?? true;
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _descCtrl.dispose();
    _cronCtrl.dispose();
    super.dispose();
  }

  Future<void> _runPreview() async {
    if (_selectedTool.isEmpty || _cronCtrl.text.trim().isEmpty) return;
    setState(() {
      _previewLoading = true;
      _error = null;
    });
    try {
      final res = await ref
          .read(scheduledTasksProvider.notifier)
          .preview(
            tool: _selectedTool,
            arguments: _args,
            cron: _cronCtrl.text.trim(),
          );
      if (!mounted) return;
      setState(() {
        _preview = res;
        _previewLoading = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err);
        _previewLoading = false;
      });
    }
  }

  Future<void> _save() async {
    if (!_formKey.currentState!.validate()) return;
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      final input = <String, dynamic>{
        'name': _nameCtrl.text.trim(),
        'description': _descCtrl.text.trim().isEmpty
            ? null
            : _descCtrl.text.trim(),
        'tool': _selectedTool,
        'arguments': _args,
        'cron': _cronCtrl.text.trim(),
        'enabled': _enabled,
      };
      final notifier = ref.read(scheduledTasksProvider.notifier);
      final existing = widget.existing;
      if (existing == null) {
        await notifier.create(input);
      } else {
        await notifier.update(existing.id, input);
      }
      if (mounted) {
        Navigator.of(context).pop();
      }
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _saving = false;
        _error = describeError(err);
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final auth = ref.watch(authProvider);
    final allTools = ref
        .watch(scheduledTasksProvider)
        .catalog
        .where((t) => auth.isAdmin || !t.adminOnly)
        .toList(growable: false);

    final toolObj = allTools.cast<SchedulableTool?>().firstWhere(
      (t) => t?.name == _selectedTool,
      orElse: () => null,
    );
    final props = asJsonMap(toolObj?.parameters['properties']);

    return Form(
      key: _formKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (_error != null) ...<Widget>[
            NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
            const SizedBox(height: 10),
          ],
          TextFormField(
            controller: _nameCtrl,
            decoration: const InputDecoration(labelText: 'Nome da tarefa'),
            validator: FormValidators.required('Informe um nome.'),
          ),
          const SizedBox(height: 12),
          TextFormField(
            controller: _descCtrl,
            decoration: const InputDecoration(
              labelText: 'Descrição (opcional)',
            ),
          ),
          const SizedBox(height: 12),
          if (allTools.isNotEmpty)
            AppDropdownFormField<String>(
              initialValue: allTools.any((t) => t.name == _selectedTool)
                  ? _selectedTool
                  : allTools.first.name,
              decoration: const InputDecoration(labelText: 'Ferramenta / Ação'),
              items: <DropdownMenuItem<String>>[
                for (final t in allTools)
                  DropdownMenuItem<String>(
                    value: t.name,
                    child: Text(
                      '${t.groupLabel} · ${t.name}',
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
              ],
              onChanged: (v) {
                if (v != null) {
                  setState(() {
                    _selectedTool = v;
                    _args.clear();
                    _preview = null;
                  });
                }
              },
            ),
          if (toolObj != null && toolObj.description.isNotEmpty) ...<Widget>[
            const SizedBox(height: 6),
            Text(
              toolObj.description,
              style: context.textTheme.bodySmall?.copyWith(
                color: context.colorScheme.onSurfaceVariant,
              ),
            ),
          ],
          if (props.isNotEmpty) ...<Widget>[
            const SizedBox(height: 12),
            NestedPanel(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  Text(
                    'Parâmetros da ferramenta',
                    style: context.textTheme.labelLarge?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  const SizedBox(height: 8),
                  for (final entry in props.entries) ...<Widget>[
                    _SchemaFieldInput(
                      key: ValueKey<String>('$_selectedTool:${entry.key}'),
                      fieldName: entry.key,
                      schema: asJsonMap(entry.value),
                      value: _args[entry.key],
                      onChanged: (val) {
                        setState(() {
                          if (val == null || (val is String && val.isEmpty)) {
                            _args.remove(entry.key);
                          } else {
                            _args[entry.key] = val;
                          }
                        });
                      },
                    ),
                    const SizedBox(height: 8),
                  ],
                ],
              ),
            ),
          ],
          const SizedBox(height: 12),
          Text(
            'Agenda (Cron de 5 campos)',
            style: context.textTheme.labelLarge?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 6),
          Wrap(
            spacing: 6,
            runSpacing: 6,
            children: <Widget>[
              for (final (expr, label) in _cronPresets)
                ChoiceChip(
                  selected: _cronCtrl.text.trim() == expr,
                  label: Text(label),
                  onSelected: (_) {
                    setState(() {
                      _cronCtrl.text = expr;
                      _preview = null;
                    });
                  },
                ),
            ],
          ),
          const SizedBox(height: 10),
          LayoutBuilder(
            builder: (context, constraints) {
              final cronField = TextFormField(
                controller: _cronCtrl,
                style: const TextStyle(fontFamily: 'monospace'),
                decoration: const InputDecoration(
                  labelText: 'Expressão Cron',
                  hintText: '0 3 * * *',
                ),
                validator: FormValidators.required('Informe a expressão Cron.'),
              );
              final previewBtn = OutlinedButton.icon(
                onPressed: _previewLoading ? null : _runPreview,
                icon: _previewLoading
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.preview_outlined, size: 18),
                label: const Text('Conferir'),
              );
              if (constraints.maxWidth < 440) {
                return Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    cronField,
                    const SizedBox(height: 8),
                    previewBtn,
                  ],
                );
              }
              return Row(
                children: <Widget>[
                  Expanded(child: cronField),
                  const SizedBox(width: 8),
                  previewBtn,
                ],
              );
            },
          ),
          if (_preview != null) ...<Widget>[
            const SizedBox(height: 12),
            NestedPanel(
              tone: _preview!.valid ? SemanticTone.success : SemanticTone.error,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text(
                    _preview!.valid
                        ? (_preview!.scheduleLabel ?? 'Agenda válida')
                        : 'Existem pendências na configuração:',
                    style: const TextStyle(fontWeight: FontWeight.w700),
                  ),
                  if (_preview!.summary != null) ...<Widget>[
                    const SizedBox(height: 4),
                    Text(_preview!.summary!),
                  ],
                  if (_preview!.riskReason != null) ...<Widget>[
                    const SizedBox(height: 6),
                    SemanticChip(
                      label: 'Risco: ${_preview!.riskReason!}',
                      tone: SemanticTone.warning,
                      compact: true,
                    ),
                  ],
                  for (final errMsg in _preview!.errors)
                    Text(
                      '• $errMsg',
                      style: TextStyle(color: context.colorScheme.error),
                    ),
                  if (_preview!.nextRuns.isNotEmpty) ...<Widget>[
                    const SizedBox(height: 6),
                    Text(
                      'Próximas execuções: ${_preview!.nextRuns.take(3).map(formatShortDateTime).join(' · ')}',
                      style: context.textTheme.bodySmall,
                    ),
                  ],
                ],
              ),
            ),
          ],
          const SizedBox(height: 8),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            value: _enabled,
            onChanged: (v) => setState(() => _enabled = v),
            title: const Text('Tarefa ativa'),
          ),
          const SizedBox(height: 14),
          FilledButton.icon(
            onPressed: _saving ? null : _save,
            icon: _saving
                ? const SizedBox(
                    width: 18,
                    height: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.save_outlined),
            label: Text(
              widget.existing == null ? 'Criar tarefa' : 'Salvar alterações',
            ),
          ),
        ],
      ),
    );
  }
}

class _SchemaFieldInput extends StatelessWidget {
  const _SchemaFieldInput({
    super.key,
    required this.fieldName,
    required this.schema,
    required this.value,
    required this.onChanged,
  });

  final String fieldName;
  final Map<String, dynamic> schema;
  final Object? value;
  final ValueChanged<Object?> onChanged;

  @override
  Widget build(BuildContext context) {
    final type = (schema['type'] ?? 'string').toString();
    final desc = schema['description']?.toString();
    final enumValues = schema['enum'] is List ? schema['enum'] as List : null;

    if (type == 'boolean') {
      return SwitchListTile(
        contentPadding: EdgeInsets.zero,
        dense: true,
        value: value == true,
        onChanged: onChanged,
        title: Text(fieldName),
        subtitle: desc != null ? Text(desc) : null,
      );
    }

    if (enumValues != null && enumValues.isNotEmpty) {
      return AppDropdownFormField<Object?>(
        initialValue: enumValues.contains(value) ? value : null,
        decoration: InputDecoration(
          labelText: fieldName,
          helperText: desc,
          isDense: true,
        ),
        items: <DropdownMenuItem<Object?>>[
          for (final item in enumValues)
            DropdownMenuItem<Object?>(
              value: item,
              child: Text(item.toString(), overflow: TextOverflow.ellipsis),
            ),
        ],
        onChanged: onChanged,
      );
    }

    return TextFormField(
      initialValue: value?.toString() ?? '',
      keyboardType: (type == 'integer' || type == 'number')
          ? TextInputType.number
          : TextInputType.text,
      decoration: InputDecoration(
        labelText: fieldName,
        helperText: desc,
        isDense: true,
      ),
      onChanged: (raw) {
        if (type == 'integer') {
          onChanged(int.tryParse(raw.trim()));
        } else if (type == 'number') {
          onChanged(double.tryParse(raw.trim()));
        } else {
          onChanged(raw);
        }
      },
    );
  }
}

class _TaskRunsBody extends ConsumerStatefulWidget {
  const _TaskRunsBody({required this.task});

  final ScheduledTask task;

  @override
  ConsumerState<_TaskRunsBody> createState() => _TaskRunsBodyState();
}

class _TaskRunsBodyState extends ConsumerState<_TaskRunsBody> {
  List<ScheduledTaskRun> _runs = const <ScheduledTaskRun>[];
  PaginationMeta? _meta;
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load(page: 1);
  }

  Future<void> _load({int page = 1, bool append = false}) async {
    setState(() => _loading = true);
    try {
      final res = await ref
          .read(scheduledTasksProvider.notifier)
          .fetchRuns(widget.task.id, page: page);
      if (!mounted) return;
      setState(() {
        _runs = append ? <ScheduledTaskRun>[..._runs, ...res.runs] : res.runs;
        _meta = res.meta;
        _loading = false;
      });
    } catch (_) {
      if (mounted) setState(() => _loading = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    ref.listen<ScheduledTasksState>(scheduledTasksProvider, (prev, next) {
      if (prev?.lastEventSeq != next.lastEventSeq &&
          next.lastEventTaskId == widget.task.id) {
        _load(page: 1);
      }
    });

    if (_loading && _runs.isEmpty) {
      return const LoadingStateView(message: 'Carregando execuções...');
    }
    if (_runs.isEmpty) {
      return const EmptyStateView(
        title: 'Nenhuma execução registrada ainda',
        icon: Icons.history,
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        for (final run in _runs) ...<Widget>[
          ExpansionTile(
            tilePadding: EdgeInsets.zero,
            leading: StatusIndicator.fromStatus(run.status, compact: true),
            title: Text(formatDateTime(run.startedAt)),
            subtitle: Text(
              '${run.trigger} · ${formatElapsedMs(run.durationMs)}'
              '${run.message != null ? ' · ${run.message}' : ''}',
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
            children: <Widget>[
              if (run.result != null)
                Padding(
                  padding: const EdgeInsets.only(bottom: 10),
                  child: CodeViewer(
                    title: 'Resultado',
                    code: const JsonEncoder.withIndent(
                      '  ',
                    ).convert(run.result),
                    maxHeight: 220,
                  ),
                ),
            ],
          ),
          const Divider(height: 1),
        ],
        if (_meta?.hasNext == true) ...<Widget>[
          const SizedBox(height: 10),
          OutlinedButton(
            onPressed: _loading
                ? null
                : () => _load(page: _meta!.currentPage + 1, append: true),
            child: const Text('Carregar mais'),
          ),
        ],
      ],
    );
  }
}
