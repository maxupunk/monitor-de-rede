import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/alert_models.dart';
import 'package:netmonitor_app/features/core/models/monitor_models.dart';
import 'package:netmonitor_app/features/core/presentation/alerts/alert_dialogs.dart';
import 'package:netmonitor_app/features/core/presentation/logs/logs_page.dart';
import 'package:netmonitor_app/features/core/presentation/monitors/monitor_form_sheet.dart';
import 'package:netmonitor_app/features/core/providers/alerts_provider.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Contribuições do Núcleo para `DeviceDetailPage` (`monitors`, `rules`, `logs`).
List<DeviceDetailTabContribution> buildCoreDeviceDetailTabs() {
  return <DeviceDetailTabContribution>[
    DeviceDetailTabContribution(
      id: 'monitors',
      title: 'Monitores',
      icon: Icons.monitor_heart_outlined,
      order: 30,
      area: ProductArea.core,
      builder: (_, deviceId) => _DeviceMonitorsTab(deviceId: deviceId),
    ),
    DeviceDetailTabContribution(
      id: 'rules',
      title: 'Regras de alerta',
      icon: Icons.rule_outlined,
      order: 40,
      area: ProductArea.core,
      builder: (_, deviceId) => _DeviceAlertRulesTab(deviceId: deviceId),
    ),
    DeviceDetailTabContribution(
      id: 'logs',
      title: 'Logs',
      icon: Icons.article_outlined,
      order: 50,
      area: ProductArea.core,
      builder: (_, deviceId) => LogsPage(deviceId: deviceId, embedded: true),
    ),
  ];
}

class _DeviceMonitorsTab extends ConsumerStatefulWidget {
  const _DeviceMonitorsTab({required this.deviceId});

  final int deviceId;

  @override
  ConsumerState<_DeviceMonitorsTab> createState() => _DeviceMonitorsTabState();
}

class _DeviceMonitorsTabState extends ConsumerState<_DeviceMonitorsTab> {
  List<MonitorModel> _items = const <MonitorModel>[];
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    final list = await ref
        .read(monitorsProvider.notifier)
        .fetchMonitors(deviceId: widget.deviceId);
    if (!mounted) return;
    setState(() {
      _items = list;
      _loading = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    final auth = ref.watch(authProvider);

    return RefreshIndicator(
      onRefresh: _load,
      child: ListView(
        children: <Widget>[
          if (auth.canWrite)
            Align(
              alignment: Alignment.centerRight,
              child: FilledButton.icon(
                onPressed: () async {
                  final ok = await showMonitorFormSheet(
                    context: context,
                    ref: ref,
                    presetDeviceId: widget.deviceId,
                  );
                  if (ok) await _load();
                },
                icon: const Icon(Icons.add, size: 18),
                label: const Text('Novo monitor'),
              ),
            ),
          const SizedBox(height: 10),
          if (_loading)
            const LoadingStateView(
              message: 'Carregando monitores do dispositivo...',
            )
          else if (_items.isEmpty)
            const EmptyStateView(
              title: 'Nenhum monitor vinculado',
              description:
                  'Adicione um monitor Ping, SNMP, TCP ou HTTP para este equipamento.',
              icon: Icons.monitor_heart_outlined,
            )
          else
            for (final m in _items) ...<Widget>[
              Card(
                elevation: 1,
                shape: RoundedRectangleBorder(
                  borderRadius: AppTheme.borderRadius,
                ),
                child: ListTile(
                  onTap: () => context.go('/monitors/${m.id}'),
                  leading: StatusIndicator.fromStatus(
                    m.normalizedStatus,
                    compact: true,
                  ),
                  title: Text(
                    m.name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(fontWeight: FontWeight.w700),
                  ),
                  subtitle: Text(
                    '${m.type.toUpperCase()} · ${m.target}',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  trailing: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      Text(formatLatency(m.lastLatencyMs)),
                      if (auth.canWrite)
                        IconButton(
                          tooltip: 'Testar agora',
                          icon: const Icon(Icons.play_arrow_outlined),
                          onPressed: () async {
                            await ref
                                .read(monitorsProvider.notifier)
                                .runMonitor(m.id);
                            await _load();
                          },
                        ),
                    ],
                  ),
                ),
              ),
              const SizedBox(height: 8),
            ],
        ],
      ),
    );
  }
}

class _DeviceAlertRulesTab extends ConsumerStatefulWidget {
  const _DeviceAlertRulesTab({required this.deviceId});

  final int deviceId;

  @override
  ConsumerState<_DeviceAlertRulesTab> createState() =>
      _DeviceAlertRulesTabState();
}

class _DeviceAlertRulesTabState extends ConsumerState<_DeviceAlertRulesTab> {
  List<AlertRule> _rules = const <AlertRule>[];
  bool _loading = true;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    setState(() => _loading = true);
    final list = await ref
        .read(alertsProvider.notifier)
        .fetchAlertRules(deviceId: widget.deviceId, includeGlobal: true);
    if (!mounted) return;
    setState(() {
      _rules = list;
      _loading = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    final auth = ref.watch(authProvider);

    return RefreshIndicator(
      onRefresh: _load,
      child: ListView(
        children: <Widget>[
          if (auth.canWrite)
            Wrap(
              alignment: WrapAlignment.end,
              spacing: 8,
              runSpacing: 8,
              children: <Widget>[
                OutlinedButton.icon(
                  onPressed: () async {
                    await showAlertRuleCatalogSheet(
                      context: context,
                      ref: ref,
                      deviceId: widget.deviceId,
                    );
                    await _load();
                  },
                  icon: const Icon(Icons.auto_awesome_motion, size: 18),
                  label: const Text('Catálogo'),
                ),
                FilledButton.icon(
                  onPressed: () async {
                    final ok = await showAlertRuleFormSheet(
                      context: context,
                      ref: ref,
                      presetDeviceId: widget.deviceId,
                    );
                    if (ok) await _load();
                  },
                  icon: const Icon(Icons.add, size: 18),
                  label: const Text('Nova regra'),
                ),
              ],
            ),
          const SizedBox(height: 10),
          if (_loading)
            const LoadingStateView(message: 'Carregando regras...')
          else if (_rules.isEmpty)
            const EmptyStateView(
              title: 'Nenhuma regra de alerta',
              description:
                  'Aplique templates do catálogo ou crie uma regra personalizada.',
              icon: Icons.rule_outlined,
            )
          else
            for (final r in _rules) ...<Widget>[
              Card(
                elevation: 1,
                shape: RoundedRectangleBorder(
                  borderRadius: AppTheme.borderRadius,
                ),
                child: ListTile(
                  title: Text(
                    r.name,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                    style: const TextStyle(fontWeight: FontWeight.w700),
                  ),
                  subtitle: Text(
                    '${r.condition.field} ${r.condition.operator} ${r.condition.value} · '
                    '${r.deviceId == null ? 'Global' : 'Este dispositivo'}',
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                  trailing: SemanticChip(
                    label: r.isEnabled ? 'Ativa' : 'Inativa',
                    tone: r.isEnabled
                        ? SemanticTone.success
                        : SemanticTone.secondary,
                    compact: true,
                  ),
                ),
              ),
              const SizedBox(height: 8),
            ],
        ],
      ),
    );
  }
}
