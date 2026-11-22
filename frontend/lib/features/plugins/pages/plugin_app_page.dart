import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/plugin_presentation.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugin_apps_provider.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/features/plugins/utils/plugin_settings_logic.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_app_tabs.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_settings_form.dart';
import 'package:netmonitor_app/shared/device_access/device_access.dart'
    hide CredentialView;
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

export 'package:netmonitor_app/features/plugins/utils/plugin_settings_logic.dart'
    show formatCandidateStatusMessage;
export 'package:netmonitor_app/features/plugins/widgets/plugin_fleet_add_sheet.dart'
    show PluginFleetAddSheet, showPluginFleetAddSheet;

const Duration _staleThreshold = Duration(minutes: 10);

/// Página `/apps/:id` (`area: ProductArea.network`) — Aplicativo de frota de um plugin.
/// Usa obrigatoriamente `SemanticTone.plugin` (teal) em todos os destaques e abas.
class PluginAppPage extends ConsumerStatefulWidget {
  const PluginAppPage({super.key, required this.pluginId});

  final int pluginId;

  @override
  ConsumerState<PluginAppPage> createState() => _PluginAppPageState();
}

class _PluginAppPageState extends ConsumerState<PluginAppPage> {
  Map<String, dynamic> _fleetSettingsDraft = <String, dynamic>{};
  bool _hydratedSettings = false;
  bool _autoReadTriggered = false;
  PluginDetail? _detailForAbout;
  PluginAppsNotifier? _appsNotifier;

  @override
  void initState() {
    super.initState();
    _appsNotifier = ref.read(pluginAppsProvider.notifier);
    WidgetsBinding.instance.addPostFrameCallback((_) => _load());
  }

  @override
  void didUpdateWidget(covariant PluginAppPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.pluginId != widget.pluginId) {
      _appsNotifier?.forgetLaunched(oldWidget.pluginId);
      _hydratedSettings = false;
      _autoReadTriggered = false;
      _detailForAbout = null;
      WidgetsBinding.instance.addPostFrameCallback((_) => _load());
    }
  }

  @override
  void dispose() {
    _appsNotifier?.forgetLaunched(widget.pluginId);
    super.dispose();
  }

  Future<void> _load() async {
    await ref.read(pluginAppsProvider.notifier).loadFleet(widget.pluginId);
    if (!mounted) return;
    final fleet = ref.read(pluginAppsProvider).fleets[widget.pluginId];
    if (fleet == null) return;
    setState(() {
      _fleetSettingsDraft = Map<String, dynamic>.from(
        fleet.settings ?? pluginSchemaDefaults(fleet.plugin.settings?.fleet),
      );
      _hydratedSettings = true;
    });
    _maybeAutoRead(fleet);
    _loadAbout();
  }

  Future<void> _loadAbout() async {
    try {
      final detail = await ref
          .read(pluginsProvider.notifier)
          .fetchDetail(widget.pluginId);
      if (mounted) setState(() => _detailForAbout = detail);
    } catch (_) {}
  }

  Future<void> _runFleetOp(
    Future<void> Function() op, {
    String? successMessage,
  }) async {
    try {
      await op();
      if (mounted && successMessage != null) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text(successMessage)));
      }
    } catch (err) {
      if (!mounted) return;
      if (isDeviceAuthFailure(err)) {
        await showDeviceAuthFailureDialog(context, err);
        return;
      }
      final msg = formatDeviceAccessError(err);
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(msg)));
    }
  }

  void _maybeAutoRead(FleetView fleet) {
    if (_autoReadTriggered) return;
    final isAdmin = ref.read(authProvider).canManagePlugins;
    final statusAction = fleet.plugin.fleet?.statusAction;
    if (!isAdmin || statusAction == null || statusAction.isEmpty) return;
    final readyIds = fleet.members
        .where((m) => m.credentialsReady)
        .map((m) => m.deviceId)
        .toList(growable: false);
    if (readyIds.isEmpty) return;

    final latest = ref
        .read(pluginAppsProvider)
        .latestBatch(widget.pluginId, statusAction);
    final finishedAt = parseDateOrNull(latest?.finishedAt);
    final isStale =
        finishedAt == null ||
        DateTime.now().difference(finishedAt) > _staleThreshold;
    if (!isStale) return;

    _autoReadTriggered = true;
    ref
        .read(pluginAppsProvider.notifier)
        .runAction(
          pluginId: widget.pluginId,
          action: statusAction,
          deviceIds: readyIds,
          confirmWrite: false,
        );
  }

  Future<void> _refreshFleetStatus(FleetView fleet) async {
    final statusAction = fleet.plugin.fleet?.statusAction;
    if (statusAction == null) {
      await ref.read(pluginAppsProvider.notifier).loadFleet(widget.pluginId);
      return;
    }
    final readyIds = fleet.members
        .where((m) => m.credentialsReady)
        .map((m) => m.deviceId)
        .toList(growable: false);
    if (readyIds.isEmpty) return;
    await ref
        .read(pluginAppsProvider.notifier)
        .runAction(
          pluginId: widget.pluginId,
          action: statusAction,
          deviceIds: readyIds,
          confirmWrite: false,
        );
  }

  @override
  Widget build(BuildContext context) {
    final appsState = ref.watch(pluginAppsProvider);
    final isAdmin = ref.watch(authProvider).canManagePlugins;
    final fleet = appsState.fleets[widget.pluginId];
    final pluginColor = context.semanticColors.plugin;

    if (appsState.loading && fleet == null) {
      return const LoadingStateView(message: 'Carregando aplicativo...');
    }
    if (fleet == null) {
      return ErrorStateView(
        message: appsState.error ?? 'Aplicativo não encontrado.',
        onRetry: _load,
      );
    }

    if (!_hydratedSettings) {
      _fleetSettingsDraft = Map<String, dynamic>.from(
        fleet.settings ?? pluginSchemaDefaults(fleet.plugin.settings?.fleet),
      );
      _hydratedSettings = true;
    }

    final plugin = fleet.plugin;
    final fleetSpec = plugin.fleet;
    final listSpec = fleetSpec?.list;
    final statusAction = fleetSpec?.statusAction;
    final latestStatus = statusAction != null
        ? appsState.latestBatch(widget.pluginId, statusAction)
        : null;

    final tabs = <({String id, String label, IconData icon})>[
      if (listSpec != null)
        (id: 'items', label: listSpec.title, icon: Icons.view_list_outlined),
      (
        id: 'members',
        label: 'Equipamentos (${fleet.members.length})',
        icon: Icons.devices_outlined,
      ),
      if ((fleetSpec?.actions ?? const <Map<String, dynamic>>[]).isNotEmpty)
        (id: 'tools', label: 'Avançado', icon: Icons.build_outlined),
      if (plugin.settings?.fleet != null)
        (id: 'settings', label: 'Configuração', icon: Icons.tune),
      (
        id: 'history',
        label: 'Histórico (${fleet.batches.length})',
        icon: Icons.history,
      ),
      (id: 'about', label: 'Como usar', icon: Icons.help_outline),
    ];

    final isCompact = MediaQuery.sizeOf(context).width < 680;
    final hasOverflowActions = plugin.wifi || isAdmin;

    return DefaultTabController(
      key: ValueKey<String>('plugin-app-${widget.pluginId}-${tabs.length}'),
      length: tabs.length,
      child: NestedScrollView(
        headerSliverBuilder: (context, innerBoxIsScrolled) => <Widget>[
          SliverToBoxAdapter(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                if (isCompact)
                  _buildCompactHeader(
                    context,
                    fleet,
                    plugin,
                    fleetSpec,
                    latestStatus,
                    statusAction,
                    isAdmin,
                    hasOverflowActions,
                  )
                else
                  _buildDesktopHeader(
                    context,
                    fleet,
                    plugin,
                    fleetSpec,
                    latestStatus,
                    statusAction,
                    isAdmin,
                  ),
                if (plugin.status != 'active') ...<Widget>[
                  const SizedBox(height: 10),
                  NestedPanel(
                    tone: SemanticTone.warning,
                    child: Text(
                      'Este plugin está "${pluginStatusPresentation(plugin.status).label}". Ative-o na biblioteca para uso completo.',
                    ),
                  ),
                ],
                const SizedBox(height: 8),
              ],
            ),
          ),
          SliverPersistentHeader(
            pinned: true,
            delegate: PinnedSliverHeaderDelegate(
              height: 48,
              backgroundColor: context.colorScheme.surface,
              child: TabBar(
                isScrollable: true,
                tabAlignment: TabAlignment.start,
                indicatorColor: pluginColor,
                labelColor: pluginColor,
                tabs: tabs
                    .map(
                      (t) => Tab(
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: <Widget>[
                            Icon(t.icon, size: 17),
                            const SizedBox(width: 6),
                            Text(t.label),
                          ],
                        ),
                      ),
                    )
                    .toList(growable: false),
              ),
            ),
          ),
        ],
        body: Padding(
          padding: const EdgeInsets.only(top: 12),
          child: TabBarView(
            children: tabs
                .map((t) {
                  return switch (t.id) {
                    'items' => PluginAppItemsTab(
                      fleet: fleet,
                      listSpec: listSpec!,
                      latestStatus: latestStatus,
                      isAdmin: isAdmin,
                    ),
                    'members' => PluginAppMembersTab(
                      pluginId: widget.pluginId,
                      fleet: fleet,
                      appsState: appsState,
                      isAdmin: isAdmin,
                      runFleetOp: _runFleetOp,
                    ),
                    'tools' => PluginAppToolsTab(
                      fleet: fleet,
                      isAdmin: isAdmin,
                    ),
                    'settings' => _buildSettingsTab(context, fleet, isAdmin),
                    'history' => PluginAppHistoryTab(
                      fleet: fleet,
                      appsState: appsState,
                      isAdmin: isAdmin,
                      runFleetOp: _runFleetOp,
                    ),
                    _ => PluginAppAboutTab(
                      fleet: fleet,
                      detail: _detailForAbout,
                    ),
                  };
                })
                .toList(growable: false),
          ),
        ),
      ),
    );
  }

  Widget _buildCompactHeader(
    BuildContext context,
    FleetView fleet,
    PluginSummary plugin,
    PluginFleetSpec? fleetSpec,
    PluginBatchView? latestStatus,
    String? statusAction,
    bool isAdmin,
    bool hasOverflowActions,
  ) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: <Widget>[
        Container(
          width: 40,
          height: 40,
          decoration: BoxDecoration(
            color: context.semanticColors.pluginContainer,
            borderRadius: BorderRadius.circular(10),
          ),
          child: Icon(
            plugin.wifi ? Icons.wifi_tethering : Icons.apps,
            size: 20,
            color: context.semanticColors.onPluginContainer,
          ),
        ),
        const SizedBox(width: 10),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Text(
                fleetSpec?.title ?? plugin.name,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: context.textTheme.titleMedium?.copyWith(
                  fontWeight: FontWeight.w800,
                ),
              ),
              const SizedBox(height: 3),
              Wrap(
                spacing: 6,
                runSpacing: 4,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[
                  SemanticChip(
                    label: countNoun(
                      fleet.members.length,
                      'equipamento',
                      'equipamentos',
                    ),
                    tone: SemanticTone.plugin,
                    compact: true,
                  ),
                  if (latestStatus?.finishedAt != null)
                    Text(
                      'Lido ${formatRelativeTime(latestStatus!.finishedAt)}',
                      style: context.textTheme.labelSmall?.copyWith(
                        color: context.colorScheme.onSurfaceVariant,
                      ),
                    ),
                ],
              ),
            ],
          ),
        ),
        if (isAdmin && statusAction != null)
          IconButton(
            tooltip: 'Atualizar',
            visualDensity: VisualDensity.compact,
            style: IconButton.styleFrom(
              backgroundColor: context.semanticColors.plugin,
              foregroundColor: context.semanticColors.onPlugin,
            ),
            onPressed: () => _refreshFleetStatus(fleet),
            icon: const Icon(Icons.refresh, size: 19),
          ),
        if (hasOverflowActions)
          PopupMenuButton<String>(
            tooltip: 'Mais ações',
            padding: EdgeInsets.zero,
            constraints: const BoxConstraints(minWidth: 36, minHeight: 36),
            onSelected: (cmd) {
              if (cmd == 'wifi') {
                context.go('/wifi');
              } else if (cmd == 'plugins') {
                context.go('/plugins');
              }
            },
            itemBuilder: (_) => <PopupMenuEntry<String>>[
              if (plugin.wifi)
                const PopupMenuItem<String>(
                  value: 'wifi',
                  child: ListTile(
                    dense: true,
                    leading: Icon(Icons.wifi),
                    title: Text('Sinais e clientes'),
                  ),
                ),
              if (isAdmin)
                const PopupMenuItem<String>(
                  value: 'plugins',
                  child: ListTile(
                    dense: true,
                    leading: Icon(Icons.extension_outlined),
                    title: Text('Biblioteca de plugins'),
                  ),
                ),
            ],
          ),
      ],
    );
  }

  Widget _buildDesktopHeader(
    BuildContext context,
    FleetView fleet,
    PluginSummary plugin,
    PluginFleetSpec? fleetSpec,
    PluginBatchView? latestStatus,
    String? statusAction,
    bool isAdmin,
  ) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: <Widget>[
        Container(
          width: 42,
          height: 42,
          decoration: BoxDecoration(
            color: context.semanticColors.pluginContainer,
            borderRadius: BorderRadius.circular(10),
          ),
          child: Icon(
            plugin.wifi ? Icons.wifi_tethering : Icons.apps,
            color: context.semanticColors.onPluginContainer,
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Wrap(
                spacing: 8,
                runSpacing: 6,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[
                  Text(
                    fleetSpec?.title ?? plugin.name,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: context.textTheme.headlineSmall?.copyWith(
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                  SemanticChip(
                    label: countNoun(
                      fleet.members.length,
                      'equipamento',
                      'equipamentos',
                    ),
                    tone: SemanticTone.plugin,
                    compact: true,
                  ),
                  if (latestStatus?.finishedAt != null)
                    SemanticChip(
                      label:
                          'Última leitura ${formatRelativeTime(latestStatus!.finishedAt)}',
                      tone: SemanticTone.info,
                      compact: true,
                    ),
                ],
              ),
              if (plugin.description != null) ...<Widget>[
                const SizedBox(height: 2),
                Text(
                  plugin.description!,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                  style: context.textTheme.bodySmall?.copyWith(
                    color: context.colorScheme.onSurfaceVariant,
                  ),
                ),
              ],
            ],
          ),
        ),
        const SizedBox(width: 12),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            if (plugin.wifi)
              OutlinedButton.icon(
                onPressed: () => context.go('/wifi'),
                icon: const Icon(Icons.wifi, size: 18),
                label: const Text('Sinais e clientes'),
              ),
            if (isAdmin && statusAction != null)
              FilledButton.icon(
                style: FilledButton.styleFrom(
                  backgroundColor: context.semanticColors.plugin,
                  foregroundColor: context.semanticColors.onPlugin,
                ),
                onPressed: () => _refreshFleetStatus(fleet),
                icon: const Icon(Icons.refresh, size: 18),
                label: const Text('Atualizar'),
              ),
            if (isAdmin)
              OutlinedButton.icon(
                onPressed: () => context.go('/plugins'),
                icon: const Icon(Icons.extension_outlined, size: 18),
                label: const Text('Plugins'),
              ),
          ],
        ),
      ],
    );
  }

  Widget _buildSettingsTab(
    BuildContext context,
    FleetView fleet,
    bool isAdmin,
  ) {
    return SingleChildScrollView(
      child: SectionCard(
        title: 'Configuração desejada da frota',
        subtitle:
            'Salvar guarda os valores desejados no NetMonitor sem alterar os equipamentos imediatamente.',
        icon: Icons.tune,
        iconTone: SemanticTone.plugin,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            PluginSettingsForm(
              schema: fleet.plugin.settings?.fleet,
              values: _fleetSettingsDraft,
              readOnly: !isAdmin,
              onChanged: (next) => setState(() => _fleetSettingsDraft = next),
            ),
            if (isAdmin) ...<Widget>[
              const SizedBox(height: 14),
              Align(
                alignment: Alignment.centerRight,
                child: FilledButton.icon(
                  onPressed: () => _runFleetOp(
                    () => ref
                        .read(pluginAppsProvider.notifier)
                        .saveFleetSettings(
                          widget.pluginId,
                          normalizePluginFormParams(
                            fleet.plugin.settings?.fleet,
                            _fleetSettingsDraft,
                            omitEmpty: false,
                          ),
                        ),
                    successMessage: 'Configuração da frota salva.',
                  ),
                  icon: const Icon(Icons.save_outlined, size: 18),
                  label: const Text('Salvar configuração'),
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
