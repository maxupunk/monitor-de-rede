import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/form_validators.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/operations_models.dart';
import 'package:netmonitor_app/features/core/providers/maintenance_windows_provider.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Gestão de Janelas de Manutenção (paridade com `MaintenanceWindowsPage.vue`, ZERO polling).
class MaintenanceWindowsPage extends ConsumerStatefulWidget {
  const MaintenanceWindowsPage({super.key});

  @override
  ConsumerState<MaintenanceWindowsPage> createState() =>
      _MaintenanceWindowsPageState();
}

class _MaintenanceWindowsPageState
    extends ConsumerState<MaintenanceWindowsPage> {
  final TextEditingController _searchCtrl = TextEditingController();
  String _statusFilter = 'all';

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      ref.read(maintenanceWindowsProvider.notifier).fetchWindows();
      ref.read(monitorsProvider.notifier).fetchReferenceCatalogs();
    });
  }

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  Future<void> _openForm([MaintenanceWindow? existing]) async {
    await showAdaptiveFormSheet<void>(
      context: context,
      title: existing == null
          ? 'Nova janela de manutenção'
          : 'Editar janela de manutenção',
      subtitle:
          'Alertas ficam suprimidos para o escopo escolhido durante o período.',
      maxWidth: 600,
      builder: (_) => _MaintenanceWindowFormSheetBody(existing: existing),
    );
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(maintenanceWindowsProvider);
    final auth = ref.watch(authProvider);
    final now = DateTime.now();
    final q = _searchCtrl.text.trim().toLowerCase();

    final filtered = state.windows
        .where((w) {
          final st = w.statusAt(now);
          if (_statusFilter != 'all' && st != _statusFilter) return false;
          if (q.isNotEmpty) {
            final match =
                w.name.toLowerCase().contains(q) ||
                (w.description?.toLowerCase().contains(q) ?? false);
            if (!match) return false;
          }
          return true;
        })
        .toList(growable: false);

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 680;
        final scheme = context.colorScheme;

        return RefreshIndicator(
          onRefresh: () =>
              ref.read(maintenanceWindowsProvider.notifier).fetchWindows(),
          child: ListView(
            children: <Widget>[
              if (isCompact)
                Row(
                  children: <Widget>[
                    Expanded(
                      child: Text(
                        'Janelas de Manutenção',
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
                        onPressed: () => _openForm(),
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
                          'Janelas de Manutenção',
                          style: context.textTheme.headlineSmall?.copyWith(
                            fontWeight: FontWeight.w800,
                            color: scheme.onSurface,
                          ),
                        ),
                        const SizedBox(height: 2),
                        Text(
                          'Programe paradas planejadas sem gerar alertas falsos',
                          style: context.textTheme.bodyMedium?.copyWith(
                            color: scheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                    if (auth.canWrite)
                      FilledButton.icon(
                        onPressed: () => _openForm(),
                        icon: const Icon(Icons.add, size: 18),
                        label: const Text('Nova janela'),
                      ),
                  ],
                ),
              const SizedBox(height: 10),
              UnifiedFilterBar(
                searchController: _searchCtrl,
                searchHint: 'Buscar janela...',
                onSearchChanged: (_) => setState(() {}),
                quickChips: <Widget>[
                  for (final (v, label) in const <(String, String)>[
                    ('all', 'Todas'),
                    ('active', 'Ativas agora'),
                    ('scheduled', 'Agendadas'),
                    ('ended', 'Encerradas'),
                  ])
                    FilterChip(
                      visualDensity: VisualDensity.compact,
                      materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                      selected: _statusFilter == v,
                      label: Text(label),
                      onSelected: (_) => setState(() => _statusFilter = v),
                    ),
                ],
              ),
              const SizedBox(height: 14),
              if (state.loading && state.windows.isEmpty)
                const LoadingStateView(
                  message: 'Carregando janelas de manutenção...',
                )
              else if (state.error != null && state.windows.isEmpty)
                ErrorStateView(
                  message: state.error!,
                  onRetry: () => ref
                      .read(maintenanceWindowsProvider.notifier)
                      .fetchWindows(),
                )
              else if (filtered.isEmpty)
                EmptyStateView(
                  title: 'Nenhuma janela de manutenção',
                  description:
                      'Crie uma janela para silenciar alertas durante intervenções programadas.',
                  icon: Icons.build_circle_outlined,
                  action: auth.canWrite
                      ? FilledButton.icon(
                          onPressed: () => _openForm(),
                          icon: const Icon(Icons.add),
                          label: const Text('Nova janela'),
                        )
                      : null,
                )
              else
                for (final w in filtered) ...<Widget>[
                  Builder(
                    builder: (context) {
                      final st = w.statusAt(now);
                      final (String stLabel, SemanticTone tone) = switch (st) {
                        'active' => ('Ativa agora', SemanticTone.warning),
                        'scheduled' => ('Agendada', SemanticTone.info),
                        _ => ('Encerrada', SemanticTone.secondary),
                      };
                      final scopeLabel = w.deviceId != null
                          ? 'Dispositivo #${w.deviceId}'
                          : (w.siteId != null ? 'Site #${w.siteId}' : 'Global');

                      return SectionCard(
                        title: w.name,
                        subtitle:
                            '${formatDateTime(w.startsAt)} até ${formatDateTime(w.endsAt)} (${formatTimeSpan(w.startsAt, w.endsAt)})',
                        icon: Icons.build_circle_outlined,
                        iconTone: tone,
                        trailing: SemanticChip(
                          label: stLabel,
                          tone: tone,
                          compact: true,
                        ),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: <Widget>[
                            if (w.description != null &&
                                w.description!.isNotEmpty) ...<Widget>[
                              Text(w.description!),
                              const SizedBox(height: 8),
                            ],
                            Align(
                              alignment: Alignment.centerLeft,
                              child: SemanticChip(
                                label: 'Escopo: $scopeLabel',
                                tone: SemanticTone.primary,
                                compact: true,
                              ),
                            ),
                            if (auth.canWrite) ...<Widget>[
                              const SizedBox(height: 10),
                              SymmetricalActionBar(
                                spacing: 8,
                                runSpacing: 8,
                                children: <Widget>[
                                  OutlinedButton.icon(
                                    onPressed: () => _openForm(w),
                                    icon: const Icon(
                                      Icons.edit_outlined,
                                      size: 18,
                                    ),
                                    label: const Text('Editar'),
                                  ),
                                  OutlinedButton.icon(
                                    onPressed: () async {
                                      final ok = await showConfirmActionDialog(
                                        context: context,
                                        title: 'Excluir janela?',
                                        message:
                                            'A janela "${w.name}" será removida.',
                                        confirmLabel: 'Excluir',
                                      );
                                      if (ok) {
                                        final deleted = await ref
                                            .read(
                                              maintenanceWindowsProvider
                                                  .notifier,
                                            )
                                            .deleteWindow(w.id);
                                        if (!deleted && context.mounted) {
                                          final err = ref
                                              .read(maintenanceWindowsProvider)
                                              .error;
                                          ScaffoldMessenger.of(
                                            context,
                                          ).showSnackBar(
                                            SnackBar(
                                              content: Text(
                                                err ??
                                                    'Erro ao excluir janela de manutenção.',
                                              ),
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
                              ),
                            ],
                          ],
                        ),
                      );
                    },
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

class _MaintenanceWindowFormSheetBody extends ConsumerStatefulWidget {
  const _MaintenanceWindowFormSheetBody({this.existing});

  final MaintenanceWindow? existing;

  @override
  ConsumerState<_MaintenanceWindowFormSheetBody> createState() =>
      _MaintenanceWindowFormSheetBodyState();
}

class _MaintenanceWindowFormSheetBodyState
    extends ConsumerState<_MaintenanceWindowFormSheetBody> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();
  late final TextEditingController _nameCtrl;
  late final TextEditingController _descCtrl;
  late DateTime _startsAt;
  late DateTime _endsAt;
  late String _scope;
  int? _siteId;
  int? _deviceId;
  bool _saving = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    final existing = widget.existing;
    _nameCtrl = TextEditingController(text: existing?.name ?? '');
    _descCtrl = TextEditingController(text: existing?.description ?? '');
    _startsAt = existing != null
        ? (parseDateOrNull(existing.startsAt) ?? DateTime.now())
        : DateTime.now().add(const Duration(minutes: 15));
    _endsAt = existing != null
        ? (parseDateOrNull(existing.endsAt) ??
              DateTime.now().add(const Duration(hours: 2)))
        : DateTime.now().add(const Duration(hours: 2));
    _scope = existing?.deviceId != null ? 'device' : 'site';
    _siteId = existing?.siteId;
    _deviceId = existing?.deviceId;
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _descCtrl.dispose();
    super.dispose();
  }

  Future<DateTime?> _pickDateTime(BuildContext ctx, DateTime initial) async {
    final date = await showDatePicker(
      context: ctx,
      initialDate: initial,
      firstDate: DateTime(2024),
      lastDate: DateTime(2035),
    );
    if (date == null || !ctx.mounted) return null;
    final time = await showTimePicker(
      context: ctx,
      initialTime: TimeOfDay.fromDateTime(initial),
    );
    if (time == null) return null;
    return DateTime(date.year, date.month, date.day, time.hour, time.minute);
  }

  Future<void> _save() async {
    if (_saving) return;
    if (!_formKey.currentState!.validate()) return;
    if (_scope == 'site' && _siteId == null) {
      setState(() => _error = 'Selecione o site para a janela de manutenção.');
      return;
    }
    if (_scope == 'device' && _deviceId == null) {
      setState(
        () => _error = 'Selecione o dispositivo para a janela de manutenção.',
      );
      return;
    }
    if (!_endsAt.isAfter(_startsAt)) {
      setState(() => _error = 'O término deve ser posterior ao início.');
      return;
    }
    setState(() {
      _saving = true;
      _error = null;
    });
    final payload = <String, dynamic>{
      'name': _nameCtrl.text.trim(),
      'description': _descCtrl.text.trim().isEmpty
          ? null
          : _descCtrl.text.trim(),
      'siteId': _scope == 'site' ? _siteId : null,
      'deviceId': _scope == 'device' ? _deviceId : null,
      'startsAt': _startsAt.toUtc().toIso8601String(),
      'endsAt': _endsAt.toUtc().toIso8601String(),
    };
    final notifier = ref.read(maintenanceWindowsProvider.notifier);
    final existing = widget.existing;
    final ok = existing == null
        ? await notifier.createWindow(payload)
        : await notifier.updateWindow(existing.id, payload);
    if (!mounted) return;
    if (ok) {
      Navigator.of(context).pop();
    } else {
      setState(() {
        _saving = false;
        _error =
            ref.read(maintenanceWindowsProvider).error ??
            'Erro ao salvar janela de manutenção.';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final monitorsState = ref.watch(monitorsProvider);
    final safeSiteId = monitorsState.sites.any((s) => s.id == _siteId)
        ? _siteId
        : null;
    final safeDeviceId = monitorsState.devices.any((d) => d.id == _deviceId)
        ? _deviceId
        : null;

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
            decoration: const InputDecoration(labelText: 'Nome da janela'),
            validator: FormValidators.required('Informe um nome.'),
          ),
          const SizedBox(height: 12),
          TextFormField(
            controller: _descCtrl,
            maxLines: 2,
            decoration: const InputDecoration(
              labelText: 'Descrição (opcional)',
            ),
          ),
          const SizedBox(height: 12),
          SegmentedButton<String>(
            showSelectedIcon: false,
            style: const ButtonStyle(visualDensity: VisualDensity.compact),
            segments: const <ButtonSegment<String>>[
              ButtonSegment<String>(
                value: 'site',
                label: Text('Site', overflow: TextOverflow.ellipsis),
              ),
              ButtonSegment<String>(
                value: 'device',
                label: Text('Dispositivo', overflow: TextOverflow.ellipsis),
              ),
            ],
            selected: <String>{_scope},
            onSelectionChanged: (s) => setState(() {
              _scope = s.first;
              _error = null;
              if (_scope != 'site') _siteId = null;
              if (_scope != 'device') _deviceId = null;
            }),
          ),
          if (_scope == 'site') ...<Widget>[
            const SizedBox(height: 12),
            AppDropdownFormField<int?>(
              initialValue: safeSiteId,
              decoration: const InputDecoration(labelText: 'Site'),
              items: <DropdownMenuItem<int?>>[
                const DropdownMenuItem<int?>(
                  value: null,
                  child: Text(
                    'Selecione o site',
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                for (final s in monitorsState.sites)
                  DropdownMenuItem<int?>(
                    value: s.id,
                    child: Text(s.name, overflow: TextOverflow.ellipsis),
                  ),
              ],
              onChanged: (v) => setState(() => _siteId = v),
            ),
          ],
          if (_scope == 'device') ...<Widget>[
            const SizedBox(height: 12),
            AppDropdownFormField<int?>(
              initialValue: safeDeviceId,
              decoration: const InputDecoration(labelText: 'Dispositivo'),
              items: <DropdownMenuItem<int?>>[
                const DropdownMenuItem<int?>(
                  value: null,
                  child: Text(
                    'Selecione o dispositivo',
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                for (final d in monitorsState.devices)
                  DropdownMenuItem<int?>(
                    value: d.id,
                    child: Text(d.name, overflow: TextOverflow.ellipsis),
                  ),
              ],
              onChanged: (v) => setState(() => _deviceId = v),
            ),
          ],
          const SizedBox(height: 12),
          ListTile(
            contentPadding: EdgeInsets.zero,
            leading: const Icon(Icons.schedule),
            title: const Text('Início'),
            subtitle: Text(formatDateTime(_startsAt)),
            trailing: TextButton(
              onPressed: () async {
                final picked = await _pickDateTime(context, _startsAt);
                if (picked != null && mounted) {
                  setState(() => _startsAt = picked);
                }
              },
              child: const Text('Alterar'),
            ),
          ),
          ListTile(
            contentPadding: EdgeInsets.zero,
            leading: const Icon(Icons.event_available),
            title: const Text('Término'),
            subtitle: Text(formatDateTime(_endsAt)),
            trailing: TextButton(
              onPressed: () async {
                final picked = await _pickDateTime(context, _endsAt);
                if (picked != null && mounted) {
                  setState(() => _endsAt = picked);
                }
              },
              child: const Text('Alterar'),
            ),
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
              widget.existing == null ? 'Criar janela' : 'Salvar alterações',
            ),
          ),
        ],
      ),
    );
  }
}
