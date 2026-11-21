import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/alert_models.dart';
import 'package:netmonitor_app/features/core/presentation/alerts/alert_dialogs.dart';
import 'package:netmonitor_app/features/core/providers/alerts_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Central de Alertas e Regras (paridade com `AlertsPage.vue`).
class AlertsPage extends ConsumerStatefulWidget {
  const AlertsPage({
    super.key,
    this.initialTab,
    this.deviceId,
    this.ruleId,
    this.catalogKey,
  });

  final String? initialTab;
  final int? deviceId;
  final int? ruleId;
  final String? catalogKey;

  @override
  ConsumerState<AlertsPage> createState() => _AlertsPageState();
}

class _AlertsPageState extends ConsumerState<AlertsPage>
    with SingleTickerProviderStateMixin {
  late final TabController _tabController;
  final TextEditingController _searchCtrl = TextEditingController();
  String _severityFilter = 'all';
  String _statusFilter = 'all';
  bool _catalogOpened = false;

  static const List<String> _tabs = <String>[
    'active',
    'resolved',
    'rules',
    'history',
  ];

  @override
  void initState() {
    super.initState();
    final initialIndex = _tabs.indexOf(widget.initialTab ?? 'active');
    _tabController = TabController(
      length: _tabs.length,
      vsync: this,
      initialIndex: initialIndex >= 0 ? initialIndex : 0,
    );
    _tabController.addListener(() {
      if (!_tabController.indexIsChanging && _tabController.index == 3) {
        final history = ref.read(alertsProvider).historyItems;
        if (history.isEmpty) {
          ref.read(alertsProvider.notifier).fetchHistory(page: 1);
        }
      }
    });

    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted) return;
      await Future.wait<void>(<Future<void>>[
        ref.read(alertsProvider.notifier).fetchActiveAlerts(),
        ref
            .read(alertsProvider.notifier)
            .fetchAlertRules(deviceId: widget.deviceId),
        ref.read(alertsProvider.notifier).fetchRootCauseAnalysis(),
      ]);
      if (mounted &&
          widget.catalogKey != null &&
          widget.catalogKey!.isNotEmpty &&
          !_catalogOpened) {
        _catalogOpened = true;
        await showAlertRuleCatalogSheet(
          context: context,
          ref: ref,
          deviceId: widget.deviceId,
          preselectKey: widget.catalogKey,
        );
      }
    });
  }

  @override
  void dispose() {
    _tabController.dispose();
    _searchCtrl.dispose();
    super.dispose();
  }

  SemanticTone _severityTone(String sev) => switch (sev.toLowerCase()) {
    'critical' || 'error' => SemanticTone.error,
    'warning' => SemanticTone.warning,
    _ => SemanticTone.info,
  };

  List<AlertEvent> _filterEvents(List<AlertEvent> list) {
    final q = _searchCtrl.text.trim().toLowerCase();
    return list
        .where((a) {
          if (widget.deviceId != null && a.deviceId != widget.deviceId) {
            return false;
          }
          if (_severityFilter != 'all' && a.severity != _severityFilter) {
            return false;
          }
          if (_statusFilter != 'all' && a.status != _statusFilter) return false;
          if (q.isNotEmpty) {
            final match =
                a.title.toLowerCase().contains(q) ||
                a.message.toLowerCase().contains(q) ||
                (a.deviceName?.toLowerCase().contains(q) ?? false);
            if (!match) return false;
          }
          return true;
        })
        .toList(growable: false);
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(alertsProvider);
    final auth = ref.watch(authProvider);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SearchFilterHeader(
          title: 'Alertas',
          subtitle: 'Incidentes em tempo real, análise de causa raiz e regras',
          searchController: _searchCtrl,
          searchHint: 'Filtrar por título, mensagem ou dispositivo...',
          onSearchChanged: (_) => setState(() {}),
          actions: <Widget>[
            if (auth.canWrite) ...<Widget>[
              OutlinedButton.icon(
                onPressed: () async {
                  final res = await ref
                      .read(alertsProvider.notifier)
                      .verifyAllAlerts();
                  if (!context.mounted) return;
                  ScaffoldMessenger.of(context).showSnackBar(
                    SnackBar(
                      content: Text(
                        res.message ??
                            'Verificação concluída (${res.resolvedCount} resolvido(s)).',
                      ),
                    ),
                  );
                },
                icon: const Icon(Icons.fact_check_outlined, size: 18),
                label: const Text('Verificar todos'),
              ),
              OutlinedButton.icon(
                onPressed: () => showAlertRuleCatalogSheet(
                  context: context,
                  ref: ref,
                  deviceId: widget.deviceId,
                ),
                icon: const Icon(Icons.auto_awesome_motion, size: 18),
                label: const Text('Catálogo'),
              ),
              FilledButton.icon(
                onPressed: () => showAlertRuleFormSheet(
                  context: context,
                  ref: ref,
                  presetDeviceId: widget.deviceId,
                ),
                icon: const Icon(Icons.add, size: 18),
                label: const Text('Nova regra'),
              ),
            ],
            IconButton(
              tooltip: 'Configurar notificações Push',
              icon: const Icon(Icons.notifications_active_outlined),
              onPressed: () => context.go('/settings?tab=notificacoes'),
            ),
          ],
          filters: <Widget>[
            for (final (v, label) in const <(String, String)>[
              ('all', 'Todas severidades'),
              ('critical', 'Crítico'),
              ('warning', 'Aviso'),
              ('info', 'Info'),
            ])
              FilterChip(
                selected: _severityFilter == v,
                label: Text(label),
                onSelected: (_) => setState(() => _severityFilter = v),
              ),
          ],
        ),
        const SizedBox(height: 10),
        TabBar(
          controller: _tabController,
          isScrollable: true,
          tabAlignment: TabAlignment.start,
          tabs: <Widget>[
            Tab(text: 'Ativos (${state.activeAlerts.length})'),
            Tab(text: 'Resolvidos (${state.resolvedAlerts.length})'),
            Tab(text: 'Regras (${state.alertRules.length})'),
            const Tab(text: 'Histórico'),
          ],
        ),
        const SizedBox(height: 12),
        Expanded(
          child: TabBarView(
            controller: _tabController,
            children: <Widget>[
              _buildActiveTab(state, auth),
              _buildResolvedTab(state),
              _buildRulesTab(state, auth),
              _buildHistoryTab(state),
            ],
          ),
        ),
      ],
    );
  }

  Widget _buildActiveTab(AlertsState state, AuthState auth) {
    final active = _filterEvents(state.activeAlerts);
    final clusters =
        state.rcaSummary?.activeClusters ?? const <IncidentCluster>[];

    return RefreshIndicator(
      onRefresh: () async {
        await ref.read(alertsProvider.notifier).fetchActiveAlerts();
        await ref.read(alertsProvider.notifier).fetchRootCauseAnalysis();
      },
      child: ListView(
        children: <Widget>[
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: <Widget>[
              for (final (v, label) in const <(String, String)>[
                ('all', 'Todos status'),
                ('active', 'Abertos'),
                ('acknowledged', 'Reconhecidos'),
                ('silenced', 'Silenciados'),
              ])
                FilterChip(
                  selected: _statusFilter == v,
                  label: Text(label),
                  onSelected: (_) => setState(() => _statusFilter = v),
                ),
            ],
          ),
          const SizedBox(height: 12),
          if (clusters.isNotEmpty) ...<Widget>[
            for (final c in clusters) ...<Widget>[
              NestedPanel(
                tone: SemanticTone.warning,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Wrap(
                      spacing: 8,
                      runSpacing: 4,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: <Widget>[
                        const Icon(Icons.hub_outlined, size: 18),
                        Text(
                          'Causa raiz provável: ${c.rootCauseDeviceName ?? c.causalCategoryLabel}',
                          style: const TextStyle(fontWeight: FontWeight.w800),
                        ),
                        SemanticChip(
                          label:
                              '${c.totalAlertsCount} alertas · ${c.impactedDevicesCount} alvos',
                          tone: SemanticTone.warning,
                          compact: true,
                        ),
                      ],
                    ),
                    if (c.explanation.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 6),
                      Text(c.explanation, style: context.textTheme.bodySmall),
                    ],
                  ],
                ),
              ),
              const SizedBox(height: 10),
            ],
          ],
          if (state.loading && state.alertEvents.isEmpty)
            const LoadingStateView(message: 'Carregando alertas...')
          else if (active.isEmpty)
            const EmptyStateView(
              title: 'Nenhum alerta ativo',
              description: 'Todos os dispositivos e monitores estão operando dentro do esperado.',
              icon: Icons.check_circle_outline,
              tone: SemanticTone.success,
            )
          else
            for (final alert in active) ...<Widget>[
              _AlertEventCard(
                alert: alert,
                tone: _severityTone(alert.severity),
                canWrite: auth.canWrite,
                onAck: () => ref
                    .read(alertsProvider.notifier)
                    .acknowledgeAlert(alert.id),
                onVerify: () async {
                  final res = await ref
                      .read(alertsProvider.notifier)
                      .verifyAlert(alert.id);
                  if (!mounted) return;
                  ScaffoldMessenger.of(context).showSnackBar(
                    SnackBar(
                      content: Text(
                        res.message ??
                            (res.resolved
                                ? 'Alerta resolvido após verificação.'
                                : 'Condição ainda ativa.'),
                      ),
                    ),
                  );
                },
                onSilence: () => showAlertSilenceSheet(
                  context: context,
                  ref: ref,
                  alert: alert,
                ),
                onCorrelation: () => showAlertCorrelationSheet(
                  context: context,
                  ref: ref,
                  alert: alert,
                ),
                onAiSummary: () async {
                  final err = await ref
                      .read(alertsProvider.notifier)
                      .requestAiSummary(alert.id);
                  if (!mounted) return;
                  ScaffoldMessenger.of(context).showSnackBar(
                    SnackBar(
                      content: Text(
                        err ?? 'Resumo de incidente solicitado à IA.',
                      ),
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

  Widget _buildResolvedTab(AlertsState state) {
    final resolved = _filterEvents(state.resolvedAlerts);
    if (resolved.isEmpty) {
      return const EmptyStateView(
        title: 'Nenhum alerta resolvido recentemente',
        icon: Icons.history,
      );
    }
    return ListView.separated(
      itemCount: resolved.length,
      separatorBuilder: (_, _) => const SizedBox(height: 10),
      itemBuilder: (context, index) {
        final alert = resolved[index];
        return SectionCard(
          title: alert.title,
          subtitle:
              'Resolvido ${formatRelativeTime(alert.resolvedAt)} · Duração: ${formatTimeSpan(alert.startedAt ?? alert.createdAt, alert.resolvedAt)}',
          icon: Icons.check_circle_outline,
          iconTone: SemanticTone.success,
          child: Text(alert.message),
        );
      },
    );
  }

  Widget _buildRulesTab(AlertsState state, AuthState auth) {
    final rules = state.alertRules
        .where((r) {
          if (widget.ruleId != null && r.id != widget.ruleId) return false;
          final q = _searchCtrl.text.trim().toLowerCase();
          if (q.isNotEmpty && !r.name.toLowerCase().contains(q)) return false;
          return true;
        })
        .toList(growable: false);

    if (rules.isEmpty) {
      return EmptyStateView(
        title: 'Nenhuma regra configurada',
        description: 'Crie uma regra personalizada ou aplique regras prontas do catálogo.',
        icon: Icons.rule_outlined,
        action: auth.canWrite
            ? FilledButton.icon(
                onPressed: () => showAlertRuleCatalogSheet(
                  context: context,
                  ref: ref,
                  deviceId: widget.deviceId,
                ),
                icon: const Icon(Icons.auto_awesome_motion),
                label: const Text('Abrir catálogo'),
              )
            : null,
      );
    }

    return ListView.separated(
      itemCount: rules.length,
      separatorBuilder: (_, _) => const SizedBox(height: 10),
      itemBuilder: (context, index) {
        final rule = rules[index];
        return SectionCard(
          title: rule.name,
          subtitle:
              '${rule.condition.field} ${rule.condition.operator} ${rule.condition.value} · ${rule.durationSeconds}s',
          icon: Icons.rule_outlined,
          iconTone: _severityTone(rule.severity),
          trailing: auth.canWrite
              ? Switch(
                  value: rule.isEnabled,
                  onChanged: (v) => ref
                      .read(alertsProvider.notifier)
                      .updateAlertRule(rule.id, <String, dynamic>{
                        'name': rule.name,
                        'type': rule.type,
                        'severity': rule.severity,
                        'condition': rule.condition.toJson(),
                        'durationSeconds': rule.durationSeconds,
                        'enabled': v,
                        'isEnabled': v,
                      }),
                )
              : SemanticChip(
                  label: rule.isEnabled ? 'Ativa' : 'Inativa',
                  tone: rule.isEnabled
                      ? SemanticTone.success
                      : SemanticTone.secondary,
                  compact: true,
                ),
          child: Wrap(
            spacing: 8,
            runSpacing: 8,
            alignment: WrapAlignment.spaceBetween,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              Wrap(
                spacing: 6,
                children: <Widget>[
                  SemanticChip(
                    label: rule.severity.toUpperCase(),
                    tone: _severityTone(rule.severity),
                    compact: true,
                  ),
                  SemanticChip(
                    label: rule.deviceId != null
                        ? 'Dispositivo #${rule.deviceId}'
                        : 'Global',
                    tone: SemanticTone.secondary,
                    compact: true,
                  ),
                ],
              ),
              if (auth.canWrite)
                Row(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    TextButton.icon(
                      onPressed: () => showAlertRuleFormSheet(
                        context: context,
                        ref: ref,
                        existing: rule,
                      ),
                      icon: const Icon(Icons.edit_outlined, size: 18),
                      label: const Text('Editar'),
                    ),
                    TextButton.icon(
                      onPressed: () async {
                        final ok = await showConfirmActionDialog(
                          context: context,
                          title: 'Excluir regra?',
                          message: 'A regra "${rule.name}" será removida.',
                          confirmLabel: 'Excluir',
                        );
                        if (ok) {
                          await ref
                              .read(alertsProvider.notifier)
                              .deleteAlertRule(rule.id);
                        }
                      },
                      icon: Icon(
                        Icons.delete_outline,
                        size: 18,
                        color: context.colorScheme.error,
                      ),
                      label: Text(
                        'Excluir',
                        style: TextStyle(color: context.colorScheme.error),
                      ),
                    ),
                  ],
                ),
            ],
          ),
        );
      },
    );
  }

  Widget _buildHistoryTab(AlertsState state) {
    if (state.historyLoading && state.historyItems.isEmpty) {
      return const LoadingStateView(message: 'Carregando histórico...');
    }
    if (state.historyItems.isEmpty) {
      return const EmptyStateView(
        title: 'Histórico vazio',
        icon: Icons.history,
      );
    }
    return ListView(
      children: <Widget>[
        for (final item in state.historyItems) ...<Widget>[
          ListTile(
            leading: StatusIndicator.fromStatus(item.status, compact: true),
            title: Text(
              item.title,
              style: const TextStyle(fontWeight: FontWeight.w700),
            ),
            subtitle: Text(
              '${formatDateTime(item.startedAt ?? item.createdAt)} · ${item.message}',
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
            trailing: SemanticChip(
              label: item.severity.toUpperCase(),
              tone: _severityTone(item.severity),
              compact: true,
            ),
          ),
          const Divider(height: 1),
        ],
        if (state.historyMeta?.hasNext == true)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 12),
            child: OutlinedButton(
              onPressed: state.historyLoading
                  ? null
                  : () => ref
                        .read(alertsProvider.notifier)
                        .fetchHistory(
                          page: state.historyMeta!.currentPage + 1,
                          append: true,
                        ),
              child: const Text('Carregar mais'),
            ),
          ),
      ],
    );
  }
}

class _AlertEventCard extends StatelessWidget {
  const _AlertEventCard({
    required this.alert,
    required this.tone,
    required this.canWrite,
    required this.onAck,
    required this.onVerify,
    required this.onSilence,
    required this.onCorrelation,
    required this.onAiSummary,
  });

  final AlertEvent alert;
  final SemanticTone tone;
  final bool canWrite;
  final VoidCallback onAck;
  final VoidCallback onVerify;
  final VoidCallback onSilence;
  final VoidCallback onCorrelation;
  final VoidCallback onAiSummary;

  @override
  Widget build(BuildContext context) {
    final aiSummary = alert.aiSummary;
    return SectionCard(
      title: alert.title,
      subtitle:
          '${alert.deviceName ?? alert.monitorName ?? 'Geral'} · ${formatRelativeTime(alert.startedAt ?? alert.createdAt)}',
      icon: Icons.notifications_active_outlined,
      iconTone: tone,
      trailing: StatusIndicator.fromStatus(alert.status, compact: true),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (alert.message.isNotEmpty) Text(alert.message),
          if (aiSummary != null) ...<Widget>[
            const SizedBox(height: 10),
            NestedPanel(
              tone: SemanticTone.ai,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  const Row(
                    children: <Widget>[
                      Icon(Icons.auto_awesome, size: 16),
                      SizedBox(width: 6),
                      Text(
                        'Resumo IA',
                        style: TextStyle(fontWeight: FontWeight.w700),
                      ),
                    ],
                  ),
                  const SizedBox(height: 4),
                  Text(
                    (aiSummary['summary'] ?? aiSummary['text'] ?? '')
                        .toString(),
                  ),
                ],
              ),
            ),
          ],
          const SizedBox(height: 10),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: <Widget>[
              if (canWrite && alert.status == 'active')
                FilledButton.tonalIcon(
                  onPressed: onAck,
                  icon: const Icon(Icons.done_all, size: 16),
                  label: const Text('Reconhecer'),
                ),
              if (canWrite) ...<Widget>[
                OutlinedButton.icon(
                  onPressed: onVerify,
                  icon: const Icon(Icons.fact_check_outlined, size: 16),
                  label: const Text('Verificar agora'),
                ),
                OutlinedButton.icon(
                  onPressed: onSilence,
                  icon: const Icon(Icons.notifications_off_outlined, size: 16),
                  label: const Text('Silenciar'),
                ),
              ],
              TextButton.icon(
                onPressed: onCorrelation,
                icon: const Icon(Icons.hub_outlined, size: 16),
                label: const Text('Correlação'),
              ),
              TextButton.icon(
                onPressed: onAiSummary,
                icon: Icon(
                  Icons.auto_awesome,
                  size: 16,
                  color: context.semanticColors.ai,
                ),
                label: Text(
                  'Resumo IA',
                  style: TextStyle(color: context.semanticColors.ai),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
