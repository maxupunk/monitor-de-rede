import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/monitor_models.dart';
import 'package:netmonitor_app/features/core/presentation/monitors/monitor_form_sheet.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Tela de gestão de Monitores (paridade com `MonitorsPage.vue`).
class MonitorsPage extends ConsumerStatefulWidget {
  const MonitorsPage({super.key, this.initialStatus});

  final String? initialStatus;

  @override
  ConsumerState<MonitorsPage> createState() => _MonitorsPageState();
}

class _MonitorsPageState extends ConsumerState<MonitorsPage> {
  final TextEditingController _searchCtrl = TextEditingController();
  String _statusFilter = 'all';
  String? _typeFilter;
  int? _deviceFilter;

  @override
  void initState() {
    super.initState();
    if (widget.initialStatus != null && widget.initialStatus!.isNotEmpty) {
      _statusFilter = widget.initialStatus!;
    }
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      ref.read(monitorsProvider.notifier).fetchMonitors();
      ref.read(monitorsProvider.notifier).fetchReferenceCatalogs();
    });
  }

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  List<MonitorModel> _filtered(List<MonitorModel> all) {
    final q = _searchCtrl.text.trim().toLowerCase();
    return all
        .where((m) {
          if (_statusFilter != 'all' && m.normalizedStatus != _statusFilter) {
            return false;
          }
          if (_typeFilter != null && m.type != _typeFilter) return false;
          if (_deviceFilter != null && m.deviceId != _deviceFilter) {
            return false;
          }
          if (q.isNotEmpty) {
            final match =
                m.name.toLowerCase().contains(q) ||
                m.target.toLowerCase().contains(q) ||
                (m.deviceName?.toLowerCase().contains(q) ?? false);
            if (!match) return false;
          }
          return true;
        })
        .toList(growable: false);
  }

  Future<void> _deleteMonitor(MonitorModel monitor) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Excluir monitor?',
      message:
          'O monitor "${monitor.name}" e o histórico associado serão removidos.',
      confirmLabel: 'Excluir',
    );
    if (!confirmed) return;
    await ref.read(monitorsProvider.notifier).deleteMonitor(monitor.id);
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(monitorsProvider);
    final auth = ref.watch(authProvider);
    final filtered = _filtered(state.monitors);

    return RefreshIndicator(
      onRefresh: () => ref.read(monitorsProvider.notifier).fetchMonitors(),
      child: ListView(
        children: <Widget>[
          SearchFilterHeader(
            title: 'Monitores',
            subtitle: 'Acompanhamento em tempo real de disponibilidade, latência e SNMP',
            searchController: _searchCtrl,
            searchHint: 'Buscar por nome, alvo ou dispositivo...',
            onSearchChanged: (_) => setState(() {}),
            actions: <Widget>[
              if (auth.canWrite) ...<Widget>[
                OutlinedButton.icon(
                  onPressed: () => showSaasPresetsSheet(context, ref),
                  icon: const Icon(Icons.cloud_outlined, size: 18),
                  label: const Text('Serviços SaaS'),
                ),
                FilledButton.icon(
                  onPressed: () =>
                      showMonitorFormSheet(context: context, ref: ref),
                  icon: const Icon(Icons.add, size: 18),
                  label: const Text('Novo monitor'),
                ),
              ],
            ],
            filters: <Widget>[
              for (final (value, label) in const <(String, String)>[
                ('all', 'Todos'),
                ('up', 'Online'),
                ('warning', 'Atenção'),
                ('down', 'Fora do ar'),
                ('disabled', 'Desativados'),
              ])
                FilterChip(
                  selected: _statusFilter == value,
                  label: Text(label),
                  onSelected: (_) => setState(() => _statusFilter = value),
                ),
              DropdownButton<String?>(
                value: _typeFilter,
                hint: const Text('Tipo'),
                underline: const SizedBox.shrink(),
                items: <DropdownMenuItem<String?>>[
                  const DropdownMenuItem<String?>(
                    value: null,
                    child: Text('Todos os tipos'),
                  ),
                  for (final (v, label) in kMonitorKinds)
                    DropdownMenuItem<String?>(value: v, child: Text(label)),
                ],
                onChanged: (v) => setState(() => _typeFilter = v),
              ),
              if (state.devices.isNotEmpty)
                DropdownButton<int?>(
                  value: _deviceFilter,
                  hint: const Text('Dispositivo'),
                  underline: const SizedBox.shrink(),
                  items: <DropdownMenuItem<int?>>[
                    const DropdownMenuItem<int?>(
                      value: null,
                      child: Text('Todos os dispositivos'),
                    ),
                    for (final d in state.devices)
                      DropdownMenuItem<int?>(value: d.id, child: Text(d.name)),
                  ],
                  onChanged: (v) => setState(() => _deviceFilter = v),
                ),
            ],
          ),
          const SizedBox(height: 14),
          ResponsiveGrid(
            xs: 2,
            sm: 2,
            md: 5,
            lg: 5,
            xl: 5,
            children: <Widget>[
              KpiCard(
                label: 'Total',
                value: formatInteger(state.totalCount),
                icon: Icons.monitor_heart_outlined,
                tone: SemanticTone.primary,
                onTap: () => setState(() => _statusFilter = 'all'),
              ),
              KpiCard(
                label: 'Online',
                value: formatInteger(state.upCount),
                icon: Icons.check_circle_outline,
                tone: SemanticTone.success,
                onTap: () => setState(() => _statusFilter = 'up'),
              ),
              KpiCard(
                label: 'Atenção',
                value: formatInteger(state.warningCount),
                icon: Icons.warning_amber_rounded,
                tone: SemanticTone.warning,
                onTap: () => setState(() => _statusFilter = 'warning'),
              ),
              KpiCard(
                label: 'Fora do ar',
                value: formatInteger(state.downCount),
                icon: Icons.error_outline,
                tone: SemanticTone.error,
                onTap: () => setState(() => _statusFilter = 'down'),
              ),
              KpiCard(
                label: 'Desativados',
                value: formatInteger(state.disabledCount),
                icon: Icons.pause_circle_outline,
                tone: SemanticTone.secondary,
                onTap: () => setState(() => _statusFilter = 'disabled'),
              ),
            ],
          ),
          const SizedBox(height: 14),
          if (state.loading && state.monitors.isEmpty)
            const LoadingStateView(message: 'Carregando monitores...')
          else if (state.error != null && state.monitors.isEmpty)
            ErrorStateView(
              message: state.error!,
              onRetry: () =>
                  ref.read(monitorsProvider.notifier).fetchMonitors(),
            )
          else if (filtered.isEmpty)
            EmptyStateView(
              title: 'Nenhum monitor encontrado',
              description: state.monitors.isEmpty
                  ? 'Cadastre seu primeiro monitor para iniciar o acompanhamento.'
                  : 'Ajuste os filtros ou a busca para visualizar outros monitores.',
              icon: Icons.monitor_heart_outlined,
              action: auth.canWrite && state.monitors.isEmpty
                  ? FilledButton.icon(
                      onPressed: () =>
                          showMonitorFormSheet(context: context, ref: ref),
                      icon: const Icon(Icons.add),
                      label: const Text('Novo monitor'),
                    )
                  : null,
            )
          else
            for (final monitor in filtered) ...<Widget>[
              _MonitorListCard(
                monitor: monitor,
                canWrite: auth.canWrite,
                running: state.runningId == monitor.id,
                onOpenDetail: () => context.go('/monitors/${monitor.id}'),
                onRunNow: () =>
                    ref.read(monitorsProvider.notifier).runMonitor(monitor.id),
                onToggleEnabled: () => ref
                    .read(monitorsProvider.notifier)
                    .toggleEnabled(monitor.id, !monitor.isEnabled),
                onEdit: () => showMonitorFormSheet(
                  context: context,
                  ref: ref,
                  existing: monitor,
                ),
                onDelete: () => _deleteMonitor(monitor),
              ),
              const SizedBox(height: 10),
            ],
          const SizedBox(height: 24),
        ],
      ),
    );
  }
}

class _MonitorListCard extends StatelessWidget {
  const _MonitorListCard({
    required this.monitor,
    required this.canWrite,
    required this.running,
    required this.onOpenDetail,
    required this.onRunNow,
    required this.onToggleEnabled,
    required this.onEdit,
    required this.onDelete,
  });

  final MonitorModel monitor;
  final bool canWrite;
  final bool running;
  final VoidCallback onOpenDetail;
  final VoidCallback onRunNow;
  final VoidCallback onToggleEnabled;
  final VoidCallback onEdit;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final metricText = monitor.isTrafficMonitor
        ? '↓ ${formatBps(monitor.inBps)} · ↑ ${formatBps(monitor.outBps)}'
        : (monitor.gaugeMetric != null
              ? formatMeasuredValue(
                  monitor.gaugeMetric!.value,
                  unit: monitor.gaugeMetric!.unit,
                )
              : formatLatency(monitor.lastLatencyMs));

    return Card(
      elevation: 2,
      shape: RoundedRectangleBorder(borderRadius: AppTheme.borderRadius),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onOpenDetail,
        child: Padding(
          padding: const EdgeInsets.all(14),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Wrap(
                          spacing: 8,
                          runSpacing: 4,
                          crossAxisAlignment: WrapCrossAlignment.center,
                          children: <Widget>[
                            StatusIndicator.fromStatus(
                              monitor.normalizedStatus,
                              compact: true,
                            ),
                            Text(
                              monitor.name,
                              style: context.textTheme.titleSmall?.copyWith(
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                            SemanticChip(
                              label: monitor.type.toUpperCase(),
                              tone: SemanticTone.primary,
                              compact: true,
                            ),
                            if (monitor.inMaintenance)
                              const SemanticChip(
                                label: 'Manutenção',
                                tone: SemanticTone.info,
                                icon: Icons.build_circle_outlined,
                                compact: true,
                              ),
                          ],
                        ),
                        const SizedBox(height: 6),
                        Text(
                          monitor.target,
                          style: context.textTheme.bodySmall?.copyWith(
                            fontFamily: 'monospace',
                            color: scheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                  ),
                  Column(
                    crossAxisAlignment: CrossAxisAlignment.end,
                    children: <Widget>[
                      Text(
                        metricText,
                        style: context.textTheme.titleSmall?.copyWith(
                          fontWeight: FontWeight.w800,
                        ),
                      ),
                      const SizedBox(height: 2),
                      Text(
                        '${monitor.intervalSeconds}s · ${formatRelativeTime(monitor.lastCheckedAt)}',
                        style: context.textTheme.labelSmall?.copyWith(
                          color: scheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ),
                ],
              ),
              const SizedBox(height: 10),
              Wrap(
                spacing: 8,
                runSpacing: 6,
                alignment: WrapAlignment.spaceBetween,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[
                  Wrap(
                    spacing: 8,
                    children: <Widget>[
                      if (monitor.deviceName != null)
                        SemanticChip(
                          label: monitor.deviceName!,
                          tone: SemanticTone.secondary,
                          icon: Icons.devices_outlined,
                          compact: true,
                        ),
                      if (monitor.probeName != null)
                        SemanticChip(
                          label: monitor.probeName!,
                          tone: SemanticTone.info,
                          icon: Icons.sensors,
                          compact: true,
                        ),
                    ],
                  ),
                  Wrap(
                    spacing: 4,
                    children: <Widget>[
                      if (canWrite) ...<Widget>[
                        IconButton(
                          tooltip: 'Testar agora',
                          icon: running
                              ? const SizedBox(
                                  width: 18,
                                  height: 18,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : const Icon(Icons.play_arrow_outlined),
                          onPressed: running ? null : onRunNow,
                        ),
                        IconButton(
                          tooltip: monitor.isEnabled
                              ? 'Pausar monitor'
                              : 'Reativar monitor',
                          icon: Icon(
                            monitor.isEnabled
                                ? Icons.pause_circle_outline
                                : Icons.play_circle_outline,
                          ),
                          onPressed: onToggleEnabled,
                        ),
                        IconButton(
                          tooltip: 'Editar monitor',
                          icon: const Icon(Icons.edit_outlined),
                          onPressed: onEdit,
                        ),
                        IconButton(
                          tooltip: 'Excluir monitor',
                          icon: Icon(Icons.delete_outline, color: scheme.error),
                          onPressed: onDelete,
                        ),
                      ],
                      TextButton(
                        onPressed: onOpenDetail,
                        child: const Text('Detalhes'),
                      ),
                    ],
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}
