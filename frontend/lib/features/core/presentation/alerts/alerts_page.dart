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

  Future<void> _verifyAll() async {
    final res = await ref.read(alertsProvider.notifier).verifyAllAlerts();
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          res.message ??
              'Verificação concluída (${res.resolvedCount} resolvido(s)).',
        ),
      ),
    );
  }

  Widget _buildHeader(
    BuildContext context,
    AuthState auth, {
    required bool isCompact,
  }) {
    final scheme = context.colorScheme;
    final severityChips = <Widget>[
      for (final (v, label) in const <(String, String)>[
        ('all', 'Todas'),
        ('critical', 'Crítico'),
        ('warning', 'Aviso'),
        ('info', 'Info'),
      ])
        FilterChip(
          selected: _severityFilter == v,
          label: Text(label),
          visualDensity: VisualDensity.compact,
          materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
          onSelected: (_) => setState(() => _severityFilter = v),
        ),
    ];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (isCompact)
          Row(
            children: <Widget>[
              Expanded(
                child: Text(
                  'Alertas',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: context.textTheme.headlineSmall?.copyWith(
                    fontWeight: FontWeight.w800,
                    color: scheme.onSurface,
                  ),
                ),
              ),
              if (auth.canWrite)
                FilledButton.icon(
                  style: FilledButton.styleFrom(
                    visualDensity: VisualDensity.compact,
                    padding: const EdgeInsets.symmetric(
                      horizontal: 12,
                      vertical: 8,
                    ),
                  ),
                  onPressed: () => showAlertRuleFormSheet(
                    context: context,
                    ref: ref,
                    presetDeviceId: widget.deviceId,
                  ),
                  icon: const Icon(Icons.add, size: 18),
                  label: const Text('Nova regra'),
                ),
              PopupMenuButton<String>(
                tooltip: 'Mais ações',
                icon: const Icon(Icons.more_vert),
                onSelected: (action) {
                  switch (action) {
                    case 'verify':
                      _verifyAll();
                    case 'catalog':
                      showAlertRuleCatalogSheet(
                        context: context,
                        ref: ref,
                        deviceId: widget.deviceId,
                      );
                    case 'push':
                      context.go('/settings?tab=notificacoes');
                  }
                },
                itemBuilder: (_) => <PopupMenuEntry<String>>[
                  if (auth.canWrite) ...<PopupMenuEntry<String>>[
                    const PopupMenuItem<String>(
                      value: 'verify',
                      child: ListTile(
                        dense: true,
                        contentPadding: EdgeInsets.zero,
                        leading: Icon(Icons.fact_check_outlined, size: 18),
                        title: Text('Verificar todos'),
                      ),
                    ),
                    const PopupMenuItem<String>(
                      value: 'catalog',
                      child: ListTile(
                        dense: true,
                        contentPadding: EdgeInsets.zero,
                        leading: Icon(Icons.auto_awesome_motion, size: 18),
                        title: Text('Catálogo de regras'),
                      ),
                    ),
                    const PopupMenuDivider(),
                  ],
                  const PopupMenuItem<String>(
                    value: 'push',
                    child: ListTile(
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      leading: Icon(
                        Icons.notifications_active_outlined,
                        size: 18,
                      ),
                      title: Text('Configurar notificações Push'),
                    ),
                  ),
                ],
              ),
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
                    'Alertas',
                    style: context.textTheme.headlineSmall?.copyWith(
                      fontWeight: FontWeight.w800,
                      color: scheme.onSurface,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    'Incidentes em tempo real, análise de causa raiz e regras',
                    style: context.textTheme.bodyMedium?.copyWith(
                      color: scheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[
                  if (auth.canWrite) ...<Widget>[
                    OutlinedButton.icon(
                      onPressed: _verifyAll,
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
              ),
            ],
          ),
        const SizedBox(height: 10),
        UnifiedFilterBar(
          searchController: _searchCtrl,
          searchHint: 'Filtrar por título, mensagem ou dispositivo...',
          onSearchChanged: (_) => setState(() {}),
          quickChips: severityChips,
        ),
      ],
    );
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(alertsProvider);
    final auth = ref.watch(authProvider);

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 680;

        return NestedScrollView(
          headerSliverBuilder: (context, innerBoxIsScrolled) => <Widget>[
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.only(bottom: 8),
                child: _buildHeader(context, auth, isCompact: isCompact),
              ),
            ),
            SliverPersistentHeader(
              pinned: true,
              delegate: PinnedSliverHeaderDelegate(
                height: 48,
                bottomBorder: true,
                child: TabBar(
                  controller: _tabController,
                  isScrollable: isCompact,
                  tabAlignment: isCompact
                      ? TabAlignment.start
                      : TabAlignment.fill,
                  tabs: <Widget>[
                    Tab(text: 'Ativos (${state.activeAlerts.length})'),
                    Tab(text: 'Resolvidos (${state.resolvedAlerts.length})'),
                    Tab(text: 'Regras (${state.alertRules.length})'),
                    const Tab(text: 'Histórico'),
                  ],
                ),
              ),
            ),
          ],
          body: TabBarView(
            controller: _tabController,
            children: <Widget>[
              _buildActiveTab(state, auth),
              _buildResolvedTab(state),
              _buildRulesTab(state, auth),
              _buildHistoryTab(state),
            ],
          ),
        );
      },
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
        padding: const EdgeInsets.only(top: 10, bottom: 24),
        physics: const AlwaysScrollableScrollPhysics(),
        children: <Widget>[
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              children: <Widget>[
                for (final (i, item) in const <(String, String)>[
                  ('all', 'Todos status'),
                  ('active', 'Abertos'),
                  ('acknowledged', 'Reconhecidos'),
                  ('silenced', 'Silenciados'),
                ].indexed) ...<Widget>[
                  if (i > 0) const SizedBox(width: 6),
                  FilterChip(
                    visualDensity: VisualDensity.compact,
                    selected: _statusFilter == item.$1,
                    label: Text(item.$2),
                    onSelected: (_) => setState(() => _statusFilter = item.$1),
                  ),
                ],
              ],
            ),
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
              description:
                  'Todos os dispositivos e monitores estão operando dentro do esperado.',
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
      return ListView(
        padding: const EdgeInsets.only(top: 12),
        physics: const AlwaysScrollableScrollPhysics(),
        children: const <Widget>[
          EmptyStateView(
            title: 'Nenhum alerta resolvido recentemente',
            icon: Icons.history,
          ),
        ],
      );
    }
    return ListView.separated(
      padding: const EdgeInsets.only(top: 12, bottom: 24),
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
      return ListView(
        padding: const EdgeInsets.only(top: 12),
        physics: const AlwaysScrollableScrollPhysics(),
        children: <Widget>[
          EmptyStateView(
            title: 'Nenhuma regra configurada',
            description:
                'Crie uma regra personalizada ou aplique regras prontas do catálogo.',
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
          ),
        ],
      );
    }

    return ListView.separated(
      padding: const EdgeInsets.only(top: 12, bottom: 24),
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
      return ListView(
        padding: const EdgeInsets.only(top: 12),
        physics: const AlwaysScrollableScrollPhysics(),
        children: const <Widget>[
          LoadingStateView(message: 'Carregando histórico...'),
        ],
      );
    }
    if (state.historyItems.isEmpty) {
      return ListView(
        padding: const EdgeInsets.only(top: 12),
        physics: const AlwaysScrollableScrollPhysics(),
        children: const <Widget>[
          EmptyStateView(title: 'Histórico vazio', icon: Icons.history),
        ],
      );
    }
    return ListView(
      padding: const EdgeInsets.only(top: 8, bottom: 24),
      children: <Widget>[
        for (final item in state.historyItems) ...<Widget>[
          ListTile(
            leading: StatusIndicator.fromStatus(item.status, compact: true),
            title: Text(
              item.title,
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
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
    final (
      String statusLabel,
      SemanticTone statusTone,
    ) = switch (alert.status) {
      'acknowledged' => ('Reconhecido', SemanticTone.info),
      'silenced' => ('Silenciado', SemanticTone.caution),
      'resolved' => ('Resolvido', SemanticTone.success),
      _ => ('Em alerta', tone),
    };
    const compactBtnPadding = EdgeInsets.symmetric(
      horizontal: 10,
      vertical: 10,
    );

    return SectionCard(
      title: alert.title,
      subtitle:
          '${alert.deviceName ?? alert.monitorName ?? 'Geral'} · ${formatRelativeTime(alert.startedAt ?? alert.createdAt)}',
      icon: Icons.notifications_active_outlined,
      iconTone: tone,
      trailing: SemanticChip(
        label: statusLabel,
        tone: statusTone,
        compact: true,
      ),
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
          SymmetricalActionBar(
            spacing: 8,
            runSpacing: 8,
            primaryFullWidthWhenOdd: true,
            children: <Widget>[
              if (canWrite && alert.status == 'active')
                FilledButton.tonalIcon(
                  style: FilledButton.styleFrom(padding: compactBtnPadding),
                  onPressed: onAck,
                  icon: const Icon(Icons.done_all, size: 16),
                  label: const Text(
                    'Reconhecer',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              if (canWrite) ...<Widget>[
                OutlinedButton.icon(
                  style: OutlinedButton.styleFrom(padding: compactBtnPadding),
                  onPressed: onVerify,
                  icon: const Icon(Icons.fact_check_outlined, size: 16),
                  label: const Text(
                    'Verificar agora',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                OutlinedButton.icon(
                  style: OutlinedButton.styleFrom(padding: compactBtnPadding),
                  onPressed: onSilence,
                  icon: const Icon(Icons.notifications_off_outlined, size: 16),
                  label: const Text(
                    'Silenciar',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ],
              OutlinedButton.icon(
                style: OutlinedButton.styleFrom(padding: compactBtnPadding),
                onPressed: onCorrelation,
                icon: const Icon(Icons.hub_outlined, size: 16),
                label: const Text(
                  'Correlação',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              OutlinedButton.icon(
                style: OutlinedButton.styleFrom(padding: compactBtnPadding),
                onPressed: onAiSummary,
                icon: Icon(
                  Icons.auto_awesome,
                  size: 16,
                  color: context.semanticColors.ai,
                ),
                label: Text(
                  'Resumo IA',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
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
