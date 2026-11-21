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
    final monitorsState = ref.read(monitorsProvider);
    final nameCtrl = TextEditingController(text: existing?.name ?? '');
    final descCtrl = TextEditingController(text: existing?.description ?? '');
    DateTime startsAt = existing != null
        ? (DateTime.tryParse(existing.startsAt)?.toLocal() ?? DateTime.now())
        : DateTime.now().add(const Duration(minutes: 15));
    DateTime endsAt = existing != null
        ? (DateTime.tryParse(existing.endsAt)?.toLocal() ??
              DateTime.now().add(const Duration(hours: 2)))
        : DateTime.now().add(const Duration(hours: 2));
    String scope = existing?.deviceId != null
        ? 'device'
        : (existing?.siteId != null ? 'site' : 'global');
    int? siteId = existing?.siteId;
    int? deviceId = existing?.deviceId;
    final formKey = GlobalKey<FormState>();
    var saving = false;

    Future<DateTime?> pickDateTime(BuildContext ctx, DateTime initial) async {
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

    await showAdaptiveFormSheet<void>(
      context: context,
      title: existing == null
          ? 'Nova janela de manutenção'
          : 'Editar janela de manutenção',
      subtitle:
          'Alertas ficam suprimidos para o escopo escolhido durante o período.',
      maxWidth: 600,
      builder: (sheetCtx) {
        return StatefulBuilder(
          builder: (context, setModalState) {
            return Form(
              key: formKey,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  TextFormField(
                    controller: nameCtrl,
                    decoration: const InputDecoration(
                      labelText: 'Nome da janela',
                    ),
                    validator: FormValidators.required('Informe um nome.'),
                  ),
                  const SizedBox(height: 12),
                  TextFormField(
                    controller: descCtrl,
                    maxLines: 2,
                    decoration: const InputDecoration(
                      labelText: 'Descrição (opcional)',
                    ),
                  ),
                  const SizedBox(height: 12),
                  SegmentedButton<String>(
                    segments: const <ButtonSegment<String>>[
                      ButtonSegment<String>(
                        value: 'global',
                        label: Text('Global'),
                      ),
                      ButtonSegment<String>(value: 'site', label: Text('Site')),
                      ButtonSegment<String>(
                        value: 'device',
                        label: Text('Dispositivo'),
                      ),
                    ],
                    selected: <String>{scope},
                    onSelectionChanged: (s) => setModalState(() {
                      scope = s.first;
                      if (scope != 'site') siteId = null;
                      if (scope != 'device') deviceId = null;
                    }),
                  ),
                  if (scope == 'site') ...<Widget>[
                    const SizedBox(height: 12),
                    DropdownButtonFormField<int?>(
                      initialValue: siteId,
                      decoration: const InputDecoration(labelText: 'Site'),
                      items: <DropdownMenuItem<int?>>[
                        const DropdownMenuItem<int?>(
                          value: null,
                          child: Text('Selecione o site'),
                        ),
                        for (final s in monitorsState.sites)
                          DropdownMenuItem<int?>(
                            value: s.id,
                            child: Text(s.name),
                          ),
                      ],
                      onChanged: (v) => setModalState(() => siteId = v),
                    ),
                  ],
                  if (scope == 'device') ...<Widget>[
                    const SizedBox(height: 12),
                    DropdownButtonFormField<int?>(
                      initialValue: deviceId,
                      decoration: const InputDecoration(
                        labelText: 'Dispositivo',
                      ),
                      items: <DropdownMenuItem<int?>>[
                        const DropdownMenuItem<int?>(
                          value: null,
                          child: Text('Selecione o dispositivo'),
                        ),
                        for (final d in monitorsState.devices)
                          DropdownMenuItem<int?>(
                            value: d.id,
                            child: Text(d.name),
                          ),
                      ],
                      onChanged: (v) => setModalState(() => deviceId = v),
                    ),
                  ],
                  const SizedBox(height: 12),
                  ListTile(
                    contentPadding: EdgeInsets.zero,
                    leading: const Icon(Icons.schedule),
                    title: const Text('Início'),
                    subtitle: Text(formatDateTime(startsAt)),
                    trailing: TextButton(
                      onPressed: () async {
                        final picked = await pickDateTime(context, startsAt);
                        if (picked != null) {
                          setModalState(() => startsAt = picked);
                        }
                      },
                      child: const Text('Alterar'),
                    ),
                  ),
                  ListTile(
                    contentPadding: EdgeInsets.zero,
                    leading: const Icon(Icons.event_available),
                    title: const Text('Término'),
                    subtitle: Text(formatDateTime(endsAt)),
                    trailing: TextButton(
                      onPressed: () async {
                        final picked = await pickDateTime(context, endsAt);
                        if (picked != null) {
                          setModalState(() => endsAt = picked);
                        }
                      },
                      child: const Text('Alterar'),
                    ),
                  ),
                  const SizedBox(height: 14),
                  FilledButton.icon(
                    onPressed: saving
                        ? null
                        : () async {
                            if (!formKey.currentState!.validate()) return;
                            if (!endsAt.isAfter(startsAt)) {
                              ScaffoldMessenger.of(context).showSnackBar(
                                const SnackBar(
                                  content: Text(
                                    'O término deve ser posterior ao início.',
                                  ),
                                ),
                              );
                              return;
                            }
                            setModalState(() => saving = true);
                            final payload = <String, dynamic>{
                              'name': nameCtrl.text.trim(),
                              'description': descCtrl.text.trim().isEmpty
                                  ? null
                                  : descCtrl.text.trim(),
                              'siteId': scope == 'site' ? siteId : null,
                              'deviceId': scope == 'device' ? deviceId : null,
                              'startsAt': startsAt.toUtc().toIso8601String(),
                              'endsAt': endsAt.toUtc().toIso8601String(),
                            };
                            final notifier = ref.read(
                              maintenanceWindowsProvider.notifier,
                            );
                            final ok = existing == null
                                ? await notifier.createWindow(payload)
                                : await notifier.updateWindow(
                                    existing.id,
                                    payload,
                                  );
                            if (sheetCtx.mounted && ok) {
                              Navigator.of(sheetCtx).pop();
                            }
                          },
                    icon: saving
                        ? const SizedBox(
                            width: 18,
                            height: 18,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : const Icon(Icons.save_outlined),
                    label: Text(
                      existing == null ? 'Criar janela' : 'Salvar alterações',
                    ),
                  ),
                ],
              ),
            );
          },
        );
      },
    );

    nameCtrl.dispose();
    descCtrl.dispose();
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

    return RefreshIndicator(
      onRefresh: () =>
          ref.read(maintenanceWindowsProvider.notifier).fetchWindows(),
      child: ListView(
        children: <Widget>[
          SearchFilterHeader(
            title: 'Janelas de Manutenção',
            subtitle: 'Programe paradas planejadas sem gerar alertas falsos',
            searchController: _searchCtrl,
            searchHint: 'Buscar janela...',
            onSearchChanged: (_) => setState(() {}),
            actions: <Widget>[
              if (auth.canWrite)
                FilledButton.icon(
                  onPressed: () => _openForm(),
                  icon: const Icon(Icons.add, size: 18),
                  label: const Text('Nova janela'),
                ),
            ],
            filters: <Widget>[
              for (final (v, label) in const <(String, String)>[
                ('all', 'Todas'),
                ('active', 'Ativas agora'),
                ('scheduled', 'Agendadas'),
                ('ended', 'Encerradas'),
              ])
                FilterChip(
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
              onRetry: () =>
                  ref.read(maintenanceWindowsProvider.notifier).fetchWindows(),
            )
          else if (filtered.isEmpty)
            EmptyStateView(
              title: 'Nenhuma janela de manutenção',
              description: 'Crie uma janela para silenciar alertas durante intervenções programadas.',
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
                        '${formatDateTime(w.startsAt)} → ${formatDateTime(w.endsAt)} (${formatTimeSpan(w.startsAt, w.endsAt)})',
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
                        Wrap(
                          spacing: 8,
                          runSpacing: 8,
                          alignment: WrapAlignment.spaceBetween,
                          crossAxisAlignment: WrapCrossAlignment.center,
                          children: <Widget>[
                            SemanticChip(
                              label: 'Escopo: $scopeLabel',
                              tone: SemanticTone.primary,
                              compact: true,
                            ),
                            if (auth.canWrite)
                              Row(
                                mainAxisSize: MainAxisSize.min,
                                children: <Widget>[
                                  TextButton.icon(
                                    onPressed: () => _openForm(w),
                                    icon: const Icon(
                                      Icons.edit_outlined,
                                      size: 18,
                                    ),
                                    label: const Text('Editar'),
                                  ),
                                  TextButton.icon(
                                    onPressed: () async {
                                      final ok = await showConfirmActionDialog(
                                        context: context,
                                        title: 'Excluir janela?',
                                        message:
                                            'A janela "${w.name}" será removida.',
                                        confirmLabel: 'Excluir',
                                      );
                                      if (ok) {
                                        await ref
                                            .read(
                                              maintenanceWindowsProvider
                                                  .notifier,
                                            )
                                            .deleteWindow(w.id);
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
                        ),
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
  }
}
