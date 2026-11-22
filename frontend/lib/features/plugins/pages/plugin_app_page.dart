import 'package:flutter/material.dart';
import 'package:flutter_markdown_plus/flutter_markdown_plus.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/device_defaults.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/plugin_presentation.dart';
import 'package:netmonitor_app/features/network/widgets/device_access_host.dart'
    show showDeviceAuthFailureDialog;
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugin_apps_provider.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/features/plugins/utils/plugin_settings_logic.dart';
import 'package:netmonitor_app/features/plugins/widgets/device_access_dialog.dart';
import 'package:netmonitor_app/features/plugins/widgets/fleet_action_flow.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_output_view.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_settings_form.dart';
import 'package:netmonitor_app/features/plugins/widgets/wifi_qr_dialog.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

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
      final msg = err is ApiException && err.message.trim().isNotEmpty
          ? err.message.trim()
          : err.toString();
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
    final finishedAt = latest?.finishedAt != null
        ? DateTime.tryParse(latest!.finishedAt!)
        : null;
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
                  Row(
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
                                    style: context.textTheme.labelSmall
                                        ?.copyWith(
                                          color: context
                                              .colorScheme
                                              .onSurfaceVariant,
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
                          constraints: const BoxConstraints(
                            minWidth: 36,
                            minHeight: 36,
                          ),
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
                  )
                else
                  Row(
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
                                  style: context.textTheme.headlineSmall
                                      ?.copyWith(fontWeight: FontWeight.w800),
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
                                foregroundColor:
                                    context.semanticColors.onPlugin,
                              ),
                              onPressed: () => _refreshFleetStatus(fleet),
                              icon: const Icon(Icons.refresh, size: 18),
                              label: const Text('Atualizar'),
                            ),
                          if (isAdmin)
                            OutlinedButton.icon(
                              onPressed: () => context.go('/plugins'),
                              icon: const Icon(
                                Icons.extension_outlined,
                                size: 18,
                              ),
                              label: const Text('Plugins'),
                            ),
                        ],
                      ),
                    ],
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
            delegate: _PluginAppTabBarDelegate(
              height: 48,
              child: Material(
                color: context.colorScheme.surface,
                child: Column(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: <Widget>[
                    Expanded(
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
                  ],
                ),
              ),
            ),
          ),
        ],
        body: Padding(
          padding: const EdgeInsets.only(top: 12),
          child: TabBarView(
            children: tabs
                .map((t) {
                  switch (t.id) {
                    case 'items':
                      return _buildItemsTab(
                        context,
                        fleet,
                        listSpec!,
                        latestStatus,
                        isAdmin,
                      );
                    case 'members':
                      return _buildMembersTab(
                        context,
                        fleet,
                        appsState,
                        isAdmin,
                      );
                    case 'tools':
                      return _buildToolsTab(context, fleet, isAdmin);
                    case 'settings':
                      return _buildSettingsTab(context, fleet, isAdmin);
                    case 'history':
                      return _buildHistoryTab(
                        context,
                        fleet,
                        appsState,
                        isAdmin,
                      );
                    default:
                      return _buildAboutTab(context, fleet);
                  }
                })
                .toList(growable: false),
          ),
        ),
      ),
    );
  }

  Widget _buildItemsTab(
    BuildContext context,
    FleetView fleet,
    PluginItemListSpec listSpec,
    PluginBatchView? latestStatus,
    bool isAdmin,
  ) {
    final rawResult = latestStatus?.result;
    final items = <Map<String, dynamic>>[];
    if (rawResult is Map && rawResult[listSpec.itemsKey] is List) {
      for (final raw in rawResult[listSpec.itemsKey] as List) {
        if (raw is Map) items.add(Map<String, dynamic>.from(raw));
      }
    } else if (rawResult is List) {
      for (final raw in rawResult) {
        if (raw is Map) items.add(Map<String, dynamic>.from(raw));
      }
    }
    if (items.isEmpty && latestStatus != null) {
      final seenKeys = <String>{};
      final keyField = listSpec.itemKey ?? 'ssid';
      for (final dev in latestStatus.devices) {
        final out = dev.output;
        List<dynamic>? devList;
        if (out is Map && out[listSpec.itemsKey] is List) {
          devList = out[listSpec.itemsKey] as List<dynamic>;
        } else if (out is List) {
          devList = out;
        }
        if (devList == null) continue;
        for (final raw in devList) {
          if (raw is! Map) continue;
          final map = Map<String, dynamic>.from(raw);
          final dedupKey = map[keyField]?.toString();
          if (dedupKey != null && dedupKey.isNotEmpty) {
            if (!seenKeys.add(dedupKey)) continue;
          }
          items.add(map);
        }
      }
    }

    final addAction = listSpec.addAction != null
        ? fleet.plugin.actionByName(listSpec.addAction!.action)
        : null;

    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (isAdmin && addAction != null) ...<Widget>[
            LayoutBuilder(
              builder: (context, constraints) {
                final addBtn = FilledButton.icon(
                  style: FilledButton.styleFrom(
                    backgroundColor: context.semanticColors.plugin,
                    foregroundColor: context.semanticColors.onPlugin,
                  ),
                  onPressed: () => showFleetActionFlow(
                    context: context,
                    fleet: fleet,
                    action: addAction,
                  ),
                  icon: const Icon(Icons.add, size: 18),
                  label: Text(
                    listSpec.addAction!.title ?? addAction.title,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                );
                if (constraints.maxWidth < 520) {
                  return SymmetricalActionBar(children: <Widget>[addBtn]);
                }
                return Align(alignment: Alignment.centerRight, child: addBtn);
              },
            ),
            const SizedBox(height: 12),
          ],
          if (items.isEmpty)
            const EmptyStateView(
              title: 'Nenhum item lido da frota ainda',
              description:
                  'Toque em "Atualizar" para consultar o estado atual nos equipamentos.',
              icon: Icons.view_list_outlined,
              tone: SemanticTone.plugin,
            )
          else
            ResponsiveGrid(
              xs: 1,
              sm: 2,
              md: 2,
              lg: 3,
              xl: 3,
              children: items
                  .map((item) {
                    final primaryKey = listSpec.itemKey;
                    final title =
                        ((primaryKey != null ? item[primaryKey] : null) ??
                                item['ssid'] ??
                                item['name'] ??
                                item['title'] ??
                                item['id'] ??
                                'Item')
                            .toString();
                    final wifiQr = listSpec.wifiQr;
                    final editAction = listSpec.editAction != null
                        ? fleet.plugin.actionByName(listSpec.editAction!.action)
                        : null;
                    final cardActions = <Widget>[
                      if (wifiQr != null && isAdmin)
                        OutlinedButton.icon(
                          style: OutlinedButton.styleFrom(
                            padding: const EdgeInsets.symmetric(
                              horizontal: 10,
                              vertical: 10,
                            ),
                          ),
                          onPressed: () {
                            final target = fleet.members
                                .where((m) => m.credentialsReady)
                                .firstOrNull;
                            if (target == null) return;
                            final revealParams = <String, dynamic>{};
                            wifiQr.revealFields.forEach((paramKey, itemKey) {
                              if (item.containsKey(itemKey)) {
                                revealParams[paramKey] = item[itemKey];
                              }
                            });
                            showWifiQrRevealDialog(
                              context: context,
                              deviceId: target.deviceId,
                              deviceName: target.deviceName,
                              pluginId: fleet.plugin.id,
                              ssid: (item[wifiQr.ssid] ?? title).toString(),
                              encryption: (item[wifiQr.encryption] ?? '')
                                  .toString(),
                              hidden:
                                  wifiQr.hidden != null &&
                                  item[wifiQr.hidden] == true,
                              revealAction: wifiQr.revealAction,
                              revealParams: revealParams,
                              secretField: wifiQr.secretField,
                            );
                          },
                          icon: const Icon(Icons.qr_code_2, size: 17),
                          label: const Text(
                            'QR Wi-Fi',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      if (editAction != null && isAdmin)
                        FilledButton.tonalIcon(
                          style: FilledButton.styleFrom(
                            backgroundColor:
                                context.semanticColors.pluginContainer,
                            foregroundColor:
                                context.semanticColors.onPluginContainer,
                            padding: const EdgeInsets.symmetric(
                              horizontal: 10,
                              vertical: 10,
                            ),
                          ),
                          onPressed: () => showFleetActionFlow(
                            context: context,
                            fleet: fleet,
                            action: editAction,
                            initialParams: item,
                          ),
                          icon: const Icon(Icons.edit_outlined, size: 17),
                          label: const Text(
                            'Editar',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                    ];

                    return SectionCard(
                      title: title,
                      icon: fleet.plugin.wifi
                          ? Icons.wifi
                          : Icons.extension_outlined,
                      iconTone: SemanticTone.plugin,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: <Widget>[
                          for (final entry
                              in item.entries
                                  .where(
                                    (e) =>
                                        !e.key.startsWith('_') &&
                                        e.key != (primaryKey ?? 'ssid'),
                                  )
                                  .take(5))
                            KeyValueRow(
                              label: humanizePluginFieldKey(entry.key),
                              value: '${entry.value}',
                            ),
                          if (cardActions.isNotEmpty) ...<Widget>[
                            const SizedBox(height: 10),
                            SymmetricalActionBar(children: cardActions),
                          ],
                        ],
                      ),
                    );
                  })
                  .toList(growable: false),
            ),
        ],
      ),
    );
  }

  Widget _buildMembersTab(
    BuildContext context,
    FleetView fleet,
    PluginAppsState appsState,
    bool isAdmin,
  ) {
    final doubtfulCount =
        fleet.members.where((m) => m.compatibility == 'possible').length +
        fleet.candidates.where((c) => c.compatibility == 'possible').length;
    final hasToolbar =
        isAdmin && (fleet.candidates.isNotEmpty || doubtfulCount > 0);

    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (hasToolbar) ...<Widget>[
            LayoutBuilder(
              builder: (context, constraints) {
                final hasBoth =
                    fleet.candidates.isNotEmpty && doubtfulCount > 0;
                final compactLabels = hasBoth && constraints.maxWidth < 440;
                final addLabel = compactLabels
                    ? 'Adicionar (${fleet.candidates.length})'
                    : 'Adicionar equipamento (${fleet.candidates.length})';
                final verifyLabel = compactLabels
                    ? 'Verificar ($doubtfulCount)'
                    : 'Verificar sistema ($doubtfulCount)';

                return SymmetricalActionBar(
                  children: <Widget>[
                    if (fleet.candidates.isNotEmpty)
                      FilledButton.icon(
                        style: FilledButton.styleFrom(
                          backgroundColor: context.semanticColors.plugin,
                          foregroundColor: context.semanticColors.onPlugin,
                          padding: const EdgeInsets.symmetric(
                            horizontal: 12,
                            vertical: 10,
                          ),
                        ),
                        onPressed: () => _openAddCandidateSheet(fleet),
                        icon: const Icon(Icons.add, size: 18),
                        label: Text(
                          addLabel,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    if (doubtfulCount > 0)
                      OutlinedButton.icon(
                        style: OutlinedButton.styleFrom(
                          foregroundColor: context.semanticColors.plugin,
                          side: BorderSide(
                            color: context.semanticColors.plugin,
                          ),
                          padding: const EdgeInsets.symmetric(
                            horizontal: 12,
                            vertical: 10,
                          ),
                        ),
                        onPressed: appsState.identifying
                            ? null
                            : () => ref
                                  .read(pluginAppsProvider.notifier)
                                  .identify(widget.pluginId),
                        icon: appsState.identifying
                            ? const SizedBox(
                                width: 14,
                                height: 14,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : const Icon(Icons.radar, size: 18),
                        label: Text(
                          verifyLabel,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                  ],
                );
              },
            ),
            const SizedBox(height: 12),
          ],
          if (fleet.members.isEmpty)
            const EmptyStateView(
              title: 'Nenhum equipamento neste aplicativo',
              description:
                  'Adicione equipamentos compatíveis para gerenciar em frota.',
              icon: Icons.devices_outlined,
              tone: SemanticTone.plugin,
            )
          else
            ResponsiveGrid(
              xs: 1,
              sm: 2,
              md: 2,
              lg: 3,
              xl: 3,
              children: fleet.members
                  .map((member) {
                    final compat = compatPresentation(member.compatibility);
                    final subtitleParts = <String>[
                      if (member.ipAddress.trim().isNotEmpty)
                        member.ipAddress.trim(),
                      if (member.firmware != null &&
                          member.firmware!.trim().isNotEmpty)
                        member.firmware!.trim(),
                    ];
                    final cleanedReasons = _cleanReasonMessages(member.reasons);
                    final memberActions = <Widget>[
                      if (!member.credentialsReady && isAdmin)
                        FilledButton.icon(
                          style: FilledButton.styleFrom(
                            backgroundColor: context.semanticColors.plugin,
                            foregroundColor: context.semanticColors.onPlugin,
                            padding: const EdgeInsets.symmetric(
                              horizontal: 10,
                              vertical: 10,
                            ),
                          ),
                          onPressed: () async {
                            final ok = await showDeviceAccessDialog(
                              context: context,
                              ref: ref,
                              deviceId: member.deviceId,
                              lockedTransport: member.missingTransport,
                            );
                            if (ok) {
                              await ref
                                  .read(pluginAppsProvider.notifier)
                                  .loadFleet(widget.pluginId);
                            }
                          },
                          icon: const Icon(Icons.vpn_key_outlined, size: 17),
                          label: const Text(
                            'Configurar acesso',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      FilledButton.tonalIcon(
                        style: FilledButton.styleFrom(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 10,
                            vertical: 10,
                          ),
                        ),
                        onPressed: () => context.go(
                          '/devices/${member.deviceId}?tab=plugin-${widget.pluginId}',
                        ),
                        icon: const Icon(Icons.open_in_new, size: 16),
                        label: const Text(
                          'Abrir dispositivo',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                      if (isAdmin)
                        OutlinedButton.icon(
                          style: OutlinedButton.styleFrom(
                            foregroundColor: context.colorScheme.error,
                            side: BorderSide(color: context.colorScheme.error),
                            padding: const EdgeInsets.symmetric(
                              horizontal: 10,
                              vertical: 10,
                            ),
                          ),
                          onPressed: () => _runFleetOp(
                            () => ref
                                .read(pluginAppsProvider.notifier)
                                .removeMember(widget.pluginId, member.deviceId),
                            successMessage:
                                '"${member.deviceName}" removido da frota.',
                          ),
                          icon: const Icon(Icons.link_off, size: 16),
                          label: const Text(
                            'Tirar da frota',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                    ];

                    return SectionCard(
                      title: member.deviceName,
                      subtitle: subtitleParts.isEmpty
                          ? null
                          : subtitleParts.join(' · '),
                      icon: Icons.router_outlined,
                      iconTone: SemanticTone.plugin,
                      trailing: SemanticChip(
                        label: compat.label,
                        tone: compat.tone,
                        compact: true,
                      ),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: <Widget>[
                          for (final e in member.card.entries)
                            KeyValueRow(
                              label: humanizePluginFieldKey(e.key),
                              value: '${e.value}',
                            ),
                          if (member.compatibility != 'likely' &&
                              member.compatibility != 'validated' &&
                              cleanedReasons.isNotEmpty) ...<Widget>[
                            const SizedBox(height: 6),
                            NestedPanel(
                              tone: SemanticTone.warning,
                              padding: const EdgeInsets.all(10),
                              child: Text(
                                cleanedReasons.join(' · '),
                                style: context.textTheme.bodySmall,
                              ),
                            ),
                          ],
                          if (member.error != null) ...<Widget>[
                            const SizedBox(height: 6),
                            NestedPanel(
                              tone: SemanticTone.error,
                              child: Text(member.error!),
                            ),
                          ],
                          const SizedBox(height: 10),
                          SymmetricalActionBar(
                            primaryFullWidthWhenOdd: true,
                            children: memberActions,
                          ),
                        ],
                      ),
                    );
                  })
                  .toList(growable: false),
            ),
        ],
      ),
    );
  }

  Future<void> _openAddCandidateSheet(FleetView fleet) async {
    await showPluginFleetAddSheet(context: context, pluginId: widget.pluginId);
  }

  Widget _buildToolsTab(BuildContext context, FleetView fleet, bool isAdmin) {
    final actions =
        fleet.plugin.fleet?.actions ?? const <Map<String, dynamic>>[];
    return SingleChildScrollView(
      child: ResponsiveGrid(
        xs: 1,
        sm: 2,
        md: 2,
        lg: 3,
        xl: 3,
        children: actions
            .map((raw) {
              final name = (raw['action'] ?? raw['name'] ?? raw['id'] ?? '')
                  .toString();
              final preview = raw['preview'] as String?;
              final action =
                  fleet.plugin.actionByName(name) ??
                  PluginAction(
                    name: name,
                    title: (raw['title'] as String?) ?? name,
                    description: raw['description'] as String?,
                    effect: (raw['effect'] as String?) ?? 'read',
                  );
              return SectionCard(
                title: action.title,
                subtitle: action.description,
                icon: Icons.play_circle_outline,
                iconTone: action.isWrite
                    ? SemanticTone.warning
                    : SemanticTone.plugin,
                child: SymmetricalActionBar(
                  children: <Widget>[
                    FilledButton.tonalIcon(
                      style: FilledButton.styleFrom(
                        backgroundColor: action.isWrite
                            ? context.semanticColors.warningContainer
                            : context.semanticColors.pluginContainer,
                        foregroundColor: action.isWrite
                            ? context.semanticColors.onWarningContainer
                            : context.semanticColors.onPluginContainer,
                      ),
                      onPressed: isAdmin
                          ? () => showFleetActionFlow(
                              context: context,
                              fleet: fleet,
                              action: action,
                              previewAction: preview,
                            )
                          : null,
                      icon: const Icon(Icons.play_arrow, size: 18),
                      label: const Text(
                        'Executar',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ],
                ),
              );
            })
            .toList(growable: false),
      ),
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

  Widget _buildHistoryTab(
    BuildContext context,
    FleetView fleet,
    PluginAppsState appsState,
    bool isAdmin,
  ) {
    if (fleet.batches.isEmpty) {
      return const EmptyStateView(
        title: 'Nenhum lote executado ainda',
        icon: Icons.history,
        tone: SemanticTone.plugin,
      );
    }
    return ListView.separated(
      itemCount: fleet.batches.length,
      separatorBuilder: (_, _) => const SizedBox(height: 10),
      itemBuilder: (context, index) {
        final batch = fleet.batches[index];
        final pres = batchStatusPresentation(batch.status);
        return SectionCard(
          padding: EdgeInsets.zero,
          child: ExpansionTile(
            title: Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    batch.title,
                    style: const TextStyle(fontWeight: FontWeight.w700),
                  ),
                ),
                SemanticChip(label: pres.label, tone: pres.tone, compact: true),
              ],
            ),
            subtitle: Text(
              '${countNoun(batch.devices.length, 'equipamento', 'equipamentos')} · ${formatRelativeTime(batch.createdAt)}',
            ),
            childrenPadding: const EdgeInsets.all(14),
            children: <Widget>[
              if (batch.isRunning && isAdmin)
                Align(
                  alignment: Alignment.centerRight,
                  child: OutlinedButton(
                    onPressed: () => _runFleetOp(
                      () => ref
                          .read(pluginAppsProvider.notifier)
                          .cancelBatch(batch.id),
                      successMessage: 'Cancelamento solicitado.',
                    ),
                    child: const Text('Cancelar lote'),
                  ),
                ),
              if (batch.hasPatch && isAdmin) ...<Widget>[
                NestedPanel(
                  tone: SemanticTone.plugin,
                  child: Wrap(
                    spacing: 8,
                    runSpacing: 6,
                    alignment: WrapAlignment.spaceBetween,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: <Widget>[
                      const Text(
                        'Este lote propõe ajustes para os equipamentos.',
                      ),
                      FilledButton.tonal(
                        onPressed: () => _runFleetOp(
                          () => ref
                              .read(pluginAppsProvider.notifier)
                              .applyPatch(batch),
                          successMessage: 'Sugestão aplicada com sucesso.',
                        ),
                        child: const Text('Aceitar sugestão'),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 8),
              ],
              if (batch.result != null) ...<Widget>[
                PluginOutputView(output: batch.result),
                const SizedBox(height: 8),
              ],
              for (final d in batch.devices)
                ListTile(
                  dense: true,
                  contentPadding: EdgeInsets.zero,
                  title: Text(
                    d.deviceName,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  subtitle: Text(
                    d.error ??
                        (d.runId != null &&
                                appsState.lastSteps[d.runId!] != null
                            ? appsState.lastSteps[d.runId!]!.request
                            : d.status),
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                  trailing: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      if (d.missingTransport != null && isAdmin)
                        TextButton(
                          style: TextButton.styleFrom(
                            visualDensity: VisualDensity.compact,
                            padding: const EdgeInsets.symmetric(horizontal: 8),
                          ),
                          onPressed: () async {
                            final ok = await showDeviceAccessDialog(
                              context: context,
                              ref: ref,
                              deviceId: d.deviceId,
                              lockedTransport: d.missingTransport,
                            );
                            if (ok &&
                                ref
                                    .read(pluginAppsProvider.notifier)
                                    .canRerun(batch.id, d.deviceId)) {
                              await _runFleetOp(
                                () => ref
                                    .read(pluginAppsProvider.notifier)
                                    .rerunDevice(
                                      batch.id,
                                      d.deviceId,
                                      confirmWrite: true,
                                    ),
                              );
                            }
                          },
                          child: const Text('Acesso'),
                        ),
                      SemanticChip(
                        label: batchStatusPresentation(d.status).label,
                        tone: batchStatusPresentation(d.status).tone,
                        compact: true,
                      ),
                    ],
                  ),
                ),
            ],
          ),
        );
      },
    );
  }

  Widget _buildAboutTab(BuildContext context, FleetView fleet) {
    final usage = _detailForAbout?.package.usage ?? '';
    return SingleChildScrollView(
      child: SectionCard(
        title: 'Sobre ${fleet.plugin.name}',
        subtitle: '${fleet.plugin.slug} · v${fleet.plugin.version}',
        icon: Icons.help_outline,
        iconTone: SemanticTone.plugin,
        child: usage.trim().isEmpty
            ? Text(
                fleet.plugin.description ??
                    'Sem documentação de uso cadastrada para este plugin.',
              )
            : MarkdownBody(data: usage),
      ),
    );
  }
}

class _PluginAppTabBarDelegate extends SliverPersistentHeaderDelegate {
  const _PluginAppTabBarDelegate({required this.height, required this.child});

  final double height;
  final Widget child;

  @override
  double get minExtent => height;

  @override
  double get maxExtent => height;

  @override
  Widget build(
    BuildContext context,
    double shrinkOffset,
    bool overlapsContent,
  ) {
    return SizedBox.expand(child: child);
  }

  @override
  bool shouldRebuild(covariant _PluginAppTabBarDelegate oldDelegate) =>
      oldDelegate.height != height || oldDelegate.child != child;
}

List<String> _cleanReasonMessages(List<String> rawReasons) {
  final out = <String>[];
  for (final raw in rawReasons) {
    var cleaned = raw.replaceFirst(RegExp(r'^[\s·•\-;,]+'), '').trim();
    if (cleaned.isEmpty) continue;
    final lower = cleaned.toLowerCase();
    if (lower.contains('firmware ainda não foi lido')) {
      cleaned = 'Firmware ainda não identificado — toque em Detectar sistema.';
    } else {
      cleaned = '${cleaned[0].toUpperCase()}${cleaned.substring(1)}';
    }
    out.add(cleaned);
  }
  return out;
}

String formatCandidateStatusMessage(FleetCandidate candidate) {
  final cleaned = _cleanReasonMessages(candidate.reasons);
  if (cleaned.isNotEmpty) {
    return cleaned.join(' · ');
  }
  return switch (candidate.compatibility) {
    'validated' ||
    'likely' => 'Sistema compatível pronto para entrar na frota.',
    'incompatible' =>
      'Sistema incompatível com os requisitos deste aplicativo.',
    _ => 'Firmware ainda não identificado — toque em Detectar sistema.',
  };
}

/// Abre o painel adaptativo para adicionar equipamentos candidatos à frota.
Future<void> showPluginFleetAddSheet({
  required BuildContext context,
  required int pluginId,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Adicionar equipamento à frota',
    subtitle:
        'Detecte o sistema operacional ou adicione os equipamentos compatíveis.',
    maxWidth: 540,
    builder: (_) => PluginFleetAddSheet(pluginId: pluginId),
  );
}

/// Conteúdo Material Design 3 de seleção/detecção de candidatos de frota.
class PluginFleetAddSheet extends ConsumerStatefulWidget {
  const PluginFleetAddSheet({super.key, required this.pluginId});

  final int pluginId;

  @override
  ConsumerState<PluginFleetAddSheet> createState() =>
      _PluginFleetAddSheetState();
}

class _PluginFleetAddSheetState extends ConsumerState<PluginFleetAddSheet> {
  final _searchCtrl = TextEditingController();
  final Set<int> _identifyingIds = <int>{};
  int? _addingId;
  String _query = '';
  String? _error;

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  ({String label, SemanticTone tone}) _compatBadge(String compat) {
    return switch (compat) {
      'validated' => (label: 'Validado', tone: SemanticTone.success),
      'likely' => (label: 'Compatível', tone: SemanticTone.success),
      'incompatible' => (label: 'Incompatível', tone: SemanticTone.error),
      _ => (label: 'Talvez compatível', tone: SemanticTone.warning),
    };
  }

  Future<void> _detectCandidate(FleetCandidate candidate) async {
    setState(() {
      _identifyingIds.add(candidate.deviceId);
      _error = null;
    });
    try {
      await ref
          .read(pluginAppsProvider.notifier)
          .identify(widget.pluginId, deviceIds: <int>[candidate.deviceId]);
    } catch (err) {
      if (mounted) {
        setState(() => _error = 'Falha ao detectar sistema: $err');
      }
    } finally {
      if (mounted) {
        setState(() => _identifyingIds.remove(candidate.deviceId));
      }
    }
  }

  Future<void> _addCandidate(FleetCandidate candidate) async {
    setState(() {
      _addingId = candidate.deviceId;
      _error = null;
    });
    try {
      await ref
          .read(pluginAppsProvider.notifier)
          .addMember(widget.pluginId, candidate.deviceId);
      if (mounted) Navigator.of(context).pop();
    } catch (err) {
      if (mounted) {
        setState(() {
          _addingId = null;
          _error = 'Falha ao adicionar equipamento: $err';
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final appsState = ref.watch(pluginAppsProvider);
    final candidates =
        appsState.fleets[widget.pluginId]?.candidates ??
        const <FleetCandidate>[];
    final showSearch = candidates.length > 5;

    final filtered = _query.isEmpty
        ? candidates
        : candidates
              .where((c) {
                final q = _query.toLowerCase();
                return c.deviceName.toLowerCase().contains(q) ||
                    c.ipAddress.toLowerCase().contains(q);
              })
              .toList(growable: false);

    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (_error != null) ...<Widget>[
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
          const SizedBox(height: 10),
        ],
        if (showSearch) ...<Widget>[
          TextField(
            controller: _searchCtrl,
            onChanged: (v) => setState(() => _query = v.trim()),
            decoration: InputDecoration(
              hintText: 'Buscar por nome ou IP...',
              prefixIcon: const Icon(Icons.search, size: 18),
              suffixIcon: _query.isNotEmpty
                  ? IconButton(
                      tooltip: 'Limpar busca',
                      icon: const Icon(Icons.clear, size: 18),
                      onPressed: () {
                        _searchCtrl.clear();
                        setState(() => _query = '');
                      },
                    )
                  : null,
            ),
          ),
          const SizedBox(height: 12),
        ],
        if (filtered.isEmpty)
          const EmptyStateView(
            title: 'Nenhum equipamento candidato encontrado',
            icon: Icons.devices_outlined,
            tone: SemanticTone.plugin,
          )
        else
          for (var i = 0; i < filtered.length; i++) ...<Widget>[
            if (i > 0) const SizedBox(height: 10),
            _buildCandidateCard(context, filtered[i], appsState.identifying),
          ],
      ],
    );
  }

  Widget _buildCandidateCard(
    BuildContext context,
    FleetCandidate candidate,
    bool globalIdentifying,
  ) {
    final badge = _compatBadge(candidate.compatibility);
    final ip = candidate.ipAddress.trim();
    final statusMsg = formatCandidateStatusMessage(candidate);
    final isDetecting =
        _identifyingIds.contains(candidate.deviceId) ||
        (globalIdentifying && _identifyingIds.isEmpty);
    final isAdding = _addingId == candidate.deviceId;
    final busy = isDetecting || _addingId != null;

    final showDetect =
        candidate.compatibility == 'possible' ||
        candidate.compatibility == 'incompatible';
    final showAdd = !candidate.isIncompatible;

    final iconBg = candidate.isIncompatible
        ? context.semanticColors.errorContainer
        : context.semanticColors.pluginContainer;
    final iconFg = candidate.isIncompatible
        ? context.semanticColors.onErrorContainer
        : context.semanticColors.onPluginContainer;

    final actionButtons = <Widget>[
      if (showDetect)
        FilledButton.tonalIcon(
          style: FilledButton.styleFrom(
            visualDensity: VisualDensity.compact,
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 10),
          ),
          onPressed: busy ? null : () => _detectCandidate(candidate),
          icon: isDetecting
              ? const SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.radar, size: 16),
          label: const Text(
            'Detectar sistema',
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(fontSize: 12.5, fontWeight: FontWeight.w600),
          ),
        ),
      if (showAdd)
        FilledButton.icon(
          style: FilledButton.styleFrom(
            backgroundColor: context.semanticColors.plugin,
            foregroundColor: context.semanticColors.onPlugin,
            visualDensity: VisualDensity.compact,
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 10),
          ),
          onPressed: busy ? null : () => _addCandidate(candidate),
          icon: isAdding
              ? SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(
                    strokeWidth: 2,
                    color: context.semanticColors.onPlugin,
                  ),
                )
              : const Icon(Icons.add, size: 16),
          label: const Text(
            'Adicionar à frota',
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(fontSize: 12.5, fontWeight: FontWeight.w600),
          ),
        ),
    ];

    return NestedPanel(
      padding: const EdgeInsets.all(14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              Container(
                width: 38,
                height: 38,
                decoration: BoxDecoration(
                  color: iconBg,
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Icon(Icons.router_outlined, size: 20, color: iconFg),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    Text(
                      candidate.deviceName,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.titleSmall?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    if (ip.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 2),
                      Text(
                        ip,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.labelSmall?.copyWith(
                          fontFamily: 'monospace',
                          color: context.colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              const SizedBox(width: 8),
              SemanticChip(label: badge.label, tone: badge.tone, compact: true),
            ],
          ),
          const SizedBox(height: 10),
          Text(
            statusMsg,
            style: context.textTheme.bodySmall?.copyWith(
              color: context.colorScheme.onSurfaceVariant,
              height: 1.35,
            ),
          ),
          if (actionButtons.isNotEmpty) ...<Widget>[
            const SizedBox(height: 12),
            SymmetricalActionBar(children: actionButtons),
          ],
        ],
      ),
    );
  }
}
