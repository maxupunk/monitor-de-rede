import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/features/network/topology/widgets/topology_hud_pills.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

export 'package:netmonitor_app/features/network/topology/widgets/topology_helpers.dart'
    show resolvePathNodeNames;
export 'package:netmonitor_app/features/network/topology/widgets/topology_hud_pills.dart';

/// Ações disponíveis no menu de contexto do HUD da topologia.
enum TopologyHudAction {
  saveLayout,
  resetLayout,
  recalculate,
  addLink,
  connectOnMap,
  unmanagedSwitch,
  pathFinder,
  legend,
  refresh,
}

/// HUD flutuante (barra superior + pílulas de status/caminho/layout + controles
/// ergonômicos no canto inferior direito) da tela de Topologia.
class TopologyHud extends StatelessWidget {
  const TopologyHud({
    super.key,
    required this.filterSpec,
    required this.healthSummary,
    required this.sites,
    required this.canWrite,
    required this.editLayoutEnabled,
    required this.hasUnsavedLayoutChanges,
    this.canUndoLayout = false,
    this.draggedNodeCount = 0,
    required this.savingLayout,
    required this.recalculating,
    required this.hasRemoteLayoutUpdate,
    required this.rightInset,
    this.activePath,
    this.pathNodeNames = const <String>[],
    this.pathPickingSourceName,
    this.linkPickingActive = false,
    this.linkPickingSourceName,
    required this.onOpenSearch,
    required this.onClearSearch,
    required this.onOpenFilters,
    required this.onSiteChanged,
    required this.onLayoutChanged,
    required this.onOverlayChanged,
    required this.onToggleEditLayout,
    this.onToggleSnapToGrid,
    required this.onSaveLayout,
    this.onUndoLayout,
    required this.onDiscardLayout,
    this.zoomListenable,
    this.readZoomScale,
    required this.onZoomIn,
    required this.onZoomOut,
    required this.onFitToScreen,
    required this.onTapProblems,
    required this.onTapOffline,
    required this.onClearPath,
    required this.onCancelPathPicking,
    this.onCancelLinkPicking,
    required this.onReloadRemoteLayout,
    required this.onAction,
  });

  final TopologyFilterSpec filterSpec;
  final TopologyHealthSummary healthSummary;
  final List<SiteModel> sites;
  final bool canWrite;
  final bool editLayoutEnabled;
  final bool hasUnsavedLayoutChanges;
  final bool canUndoLayout;
  final int draggedNodeCount;
  final bool savingLayout;
  final bool recalculating;
  final bool hasRemoteLayoutUpdate;
  final double rightInset;
  final TopologyPathResult? activePath;
  final List<String> pathNodeNames;
  final String? pathPickingSourceName;
  final bool linkPickingActive;
  final String? linkPickingSourceName;

  final VoidCallback onOpenSearch;
  final VoidCallback onClearSearch;
  final VoidCallback onOpenFilters;
  final ValueChanged<int?> onSiteChanged;
  final ValueChanged<TopologyLayoutKind> onLayoutChanged;
  final ValueChanged<TopologyOverlayMode> onOverlayChanged;
  final VoidCallback onToggleEditLayout;
  final VoidCallback? onToggleSnapToGrid;
  final VoidCallback onSaveLayout;
  final VoidCallback? onUndoLayout;
  final VoidCallback onDiscardLayout;
  final Listenable? zoomListenable;
  final double Function()? readZoomScale;
  final VoidCallback onZoomIn;
  final VoidCallback onZoomOut;
  final VoidCallback onFitToScreen;
  final VoidCallback onTapProblems;
  final VoidCallback onTapOffline;
  final VoidCallback onClearPath;
  final VoidCallback onCancelPathPicking;
  final VoidCallback? onCancelLinkPicking;
  final VoidCallback onReloadRemoteLayout;
  final ValueChanged<TopologyHudAction> onAction;

  @override
  Widget build(BuildContext context) {
    final width = MediaQuery.sizeOf(context).width;
    final isCompact = width < 720;

    return Stack(
      children: <Widget>[
        Positioned(
          top: 10,
          left: 10,
          right: 10 + rightInset,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              if (isCompact)
                _CompactTopBar(
                  filterSpec: filterSpec,
                  canWrite: canWrite,
                  savingLayout: savingLayout,
                  recalculating: recalculating,
                  pathPickingActive: pathPickingSourceName != null,
                  linkPickingActive: linkPickingActive,
                  onOpenSearch: onOpenSearch,
                  onClearSearch: onClearSearch,
                  onOpenFilters: onOpenFilters,
                  onAction: onAction,
                )
              else
                _DesktopTopBar(
                  filterSpec: filterSpec,
                  sites: sites,
                  canWrite: canWrite,
                  editLayoutEnabled: editLayoutEnabled,
                  savingLayout: savingLayout,
                  recalculating: recalculating,
                  pathPickingActive: pathPickingSourceName != null,
                  linkPickingActive: linkPickingActive,
                  onSiteChanged: onSiteChanged,
                  onOpenSearch: onOpenSearch,
                  onClearSearch: onClearSearch,
                  onLayoutChanged: onLayoutChanged,
                  onOverlayChanged: onOverlayChanged,
                  onOpenFilters: onOpenFilters,
                  onToggleEditLayout: onToggleEditLayout,
                  onAction: onAction,
                ),
              const SizedBox(height: 6),
              Wrap(
                spacing: 6,
                runSpacing: 6,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[
                  TopologyLiveStatusPill(
                    summary: healthSummary,
                    onlyProblemsActive: filterSpec.onlyProblems,
                    offlineFilterActive: filterSpec.statuses.contains(
                      TopologyNodeStatus.offline,
                    ),
                    onTapProblems: onTapProblems,
                    onTapOffline: onTapOffline,
                  ),
                  if (filterSpec.overlayMode != TopologyOverlayMode.status)
                    TopologyActiveOverlayChip(
                      mode: filterSpec.overlayMode,
                      onClear: () =>
                          onOverlayChanged(TopologyOverlayMode.status),
                    ),
                  if (hasRemoteLayoutUpdate)
                    TopologyRemoteLayoutNoticePill(
                      onReload: onReloadRemoteLayout,
                    ),
                ],
              ),
              if (linkPickingActive) ...<Widget>[
                const SizedBox(height: 6),
                TopologyFloatingBanner(
                  key: const ValueKey<String>('topology-link-picking-banner'),
                  icon: Icons.add_link,
                  tone: SemanticTone.primary,
                  label: linkPickingSourceName != null
                      ? 'Conectando $linkPickingSourceName → toque no equipamento destino no mapa'
                      : 'Conectar no mapa → toque no equipamento de origem',
                  actionLabel: onCancelLinkPicking != null ? 'Cancelar' : null,
                  onAction: onCancelLinkPicking,
                ),
              ] else if (pathPickingSourceName != null) ...<Widget>[
                const SizedBox(height: 6),
                TopologyFloatingBanner(
                  icon: Icons.route,
                  tone: SemanticTone.info,
                  label:
                      'Origem: $pathPickingSourceName — toque no nó de destino',
                  actionLabel: 'Cancelar',
                  onAction: onCancelPathPicking,
                ),
              ] else if (activePath != null &&
                  activePath!.nodeIds.length >= 2) ...<Widget>[
                const SizedBox(height: 6),
                TopologyFloatingBanner(
                  icon: Icons.alt_route,
                  tone: SemanticTone.warning,
                  label:
                      'Caminho: ${pathNodeNames.isNotEmpty ? pathNodeNames.join(' → ') : activePath!.nodeIds.map((id) => '#$id').join(' → ')} (${countNoun(activePath!.hopCount, 'salto', 'saltos')})',
                  actionLabel: 'Fechar',
                  onAction: onClearPath,
                ),
              ],
              if (hasUnsavedLayoutChanges) ...<Widget>[
                const SizedBox(height: 6),
                TopologyUnsavedLayoutPill(
                  saving: savingLayout,
                  canUndo: canUndoLayout,
                  movedCount: draggedNodeCount,
                  snapToGrid: filterSpec.snapToGrid,
                  onToggleSnapToGrid: onToggleSnapToGrid,
                  onSave: onSaveLayout,
                  onUndo: onUndoLayout,
                  onDiscard: onDiscardLayout,
                ),
              ] else if (canWrite && editLayoutEnabled) ...<Widget>[
                const SizedBox(height: 6),
                TopologyEditModeHelperPill(
                  snapToGrid: filterSpec.snapToGrid,
                  onToggleSnapToGrid: onToggleSnapToGrid,
                  onDone: onToggleEditLayout,
                ),
              ],
            ],
          ),
        ),
        Positioned(
          right: 12 + rightInset,
          bottom: 12,
          child: TopologyThumbZoneControls(
            editLayoutEnabled: editLayoutEnabled,
            canWrite: canWrite,
            overlayMode: filterSpec.overlayMode,
            zoomListenable: zoomListenable,
            readZoomScale: readZoomScale,
            onZoomIn: onZoomIn,
            onZoomOut: onZoomOut,
            onFitToScreen: onFitToScreen,
            onToggleEditLayout: onToggleEditLayout,
            onOverlayChanged: onOverlayChanged,
          ),
        ),
      ],
    );
  }
}

class _CompactTopBar extends StatelessWidget {
  const _CompactTopBar({
    required this.filterSpec,
    required this.canWrite,
    required this.savingLayout,
    required this.recalculating,
    required this.pathPickingActive,
    required this.linkPickingActive,
    required this.onOpenSearch,
    required this.onClearSearch,
    required this.onOpenFilters,
    required this.onAction,
  });

  final TopologyFilterSpec filterSpec;
  final bool canWrite;
  final bool savingLayout;
  final bool recalculating;
  final bool pathPickingActive;
  final bool linkPickingActive;
  final VoidCallback onOpenSearch;
  final VoidCallback onClearSearch;
  final VoidCallback onOpenFilters;
  final ValueChanged<TopologyHudAction> onAction;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final hasSearch = filterSpec.search.trim().isNotEmpty;
    final filterCount = filterSpec.activeFilterCount;

    return Material(
      elevation: 3,
      color: scheme.surface.withValues(alpha: 0.96),
      shadowColor: scheme.shadow.withValues(alpha: 0.22),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(14),
        side: BorderSide(color: scheme.outlineVariant),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
        child: Row(
          children: <Widget>[
            Expanded(
              child: InkWell(
                onTap: onOpenSearch,
                borderRadius: BorderRadius.circular(10),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 8,
                  ),
                  child: Row(
                    children: <Widget>[
                      Icon(
                        Icons.search,
                        size: 18,
                        color: hasSearch
                            ? scheme.primary
                            : scheme.onSurfaceVariant,
                      ),
                      const SizedBox(width: 8),
                      Expanded(
                        child: Text(
                          hasSearch ? filterSpec.search : 'Buscar nó...',
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: context.textTheme.bodyMedium?.copyWith(
                            color: hasSearch
                                ? scheme.onSurface
                                : scheme.onSurfaceVariant,
                            fontWeight: hasSearch
                                ? FontWeight.w600
                                : FontWeight.w500,
                          ),
                        ),
                      ),
                      if (hasSearch)
                        InkResponse(
                          radius: 16,
                          onTap: onClearSearch,
                          child: Icon(
                            Icons.close,
                            size: 16,
                            color: scheme.onSurfaceVariant,
                          ),
                        ),
                    ],
                  ),
                ),
              ),
            ),
            const SizedBox(width: 4),
            Badge.count(
              isLabelVisible: filterCount > 0,
              count: filterCount,
              child: IconButton.filledTonal(
                visualDensity: VisualDensity.compact,
                tooltip: 'Filtros e visualização',
                icon: const Icon(Icons.tune, size: 19),
                onPressed: onOpenFilters,
              ),
            ),
            const SizedBox(width: 2),
            _HudActionsMenu(
              canWrite: canWrite,
              savingLayout: savingLayout,
              recalculating: recalculating,
              pathPickingActive: pathPickingActive,
              linkPickingActive: linkPickingActive,
              onAction: onAction,
            ),
          ],
        ),
      ),
    );
  }
}

class _DesktopTopBar extends StatelessWidget {
  const _DesktopTopBar({
    required this.filterSpec,
    required this.sites,
    required this.canWrite,
    required this.editLayoutEnabled,
    required this.savingLayout,
    required this.recalculating,
    required this.pathPickingActive,
    required this.linkPickingActive,
    required this.onSiteChanged,
    required this.onOpenSearch,
    required this.onClearSearch,
    required this.onLayoutChanged,
    required this.onOverlayChanged,
    required this.onOpenFilters,
    required this.onToggleEditLayout,
    required this.onAction,
  });

  final TopologyFilterSpec filterSpec;
  final List<SiteModel> sites;
  final bool canWrite;
  final bool editLayoutEnabled;
  final bool savingLayout;
  final bool recalculating;
  final bool pathPickingActive;
  final bool linkPickingActive;
  final ValueChanged<int?> onSiteChanged;
  final VoidCallback onOpenSearch;
  final VoidCallback onClearSearch;
  final ValueChanged<TopologyLayoutKind> onLayoutChanged;
  final ValueChanged<TopologyOverlayMode> onOverlayChanged;
  final VoidCallback onOpenFilters;
  final VoidCallback onToggleEditLayout;
  final ValueChanged<TopologyHudAction> onAction;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final hasSearch = filterSpec.search.trim().isNotEmpty;
    final filterCount = filterSpec.activeFilterCount;
    final validSiteId = sites.any((s) => s.id == filterSpec.siteId)
        ? filterSpec.siteId
        : null;

    return Material(
      elevation: 3,
      color: scheme.surface.withValues(alpha: 0.96),
      shadowColor: scheme.shadow.withValues(alpha: 0.2),
      shape: RoundedRectangleBorder(
        borderRadius: AppTheme.borderRadius,
        side: BorderSide(color: scheme.outlineVariant),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
        child: Wrap(
          spacing: 8,
          runSpacing: 6,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            if (sites.isNotEmpty)
              SizedBox(
                width: 176,
                child: AppDropdownFormField<int?>(
                  key: ValueKey<String>(
                    'topo-site-${validSiteId ?? 'all'}-${sites.length}',
                  ),
                  initialValue: validSiteId,
                  isDense: true,
                  decoration: const InputDecoration(
                    contentPadding: EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 8,
                    ),
                    prefixIcon: Icon(Icons.location_on_outlined, size: 17),
                  ),
                  items: <DropdownMenuItem<int?>>[
                    const DropdownMenuItem<int?>(
                      value: null,
                      child: Text(
                        'Todos os sites',
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    for (final site in sites)
                      DropdownMenuItem<int?>(
                        value: site.id,
                        child: Text(site.name, overflow: TextOverflow.ellipsis),
                      ),
                  ],
                  onChanged: onSiteChanged,
                ),
              ),
            ConstrainedBox(
              constraints: const BoxConstraints(minWidth: 165, maxWidth: 230),
              child: OutlinedButton.icon(
                style: OutlinedButton.styleFrom(
                  minimumSize: const Size(0, 38),
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                  side: BorderSide(
                    color: hasSearch ? scheme.primary : scheme.outlineVariant,
                  ),
                  foregroundColor: hasSearch
                      ? scheme.primary
                      : scheme.onSurface,
                ),
                onPressed: onOpenSearch,
                icon: const Icon(Icons.search, size: 17),
                label: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    Flexible(
                      child: Text(
                        hasSearch ? filterSpec.search : 'Buscar nó...',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    if (hasSearch) ...<Widget>[
                      const SizedBox(width: 4),
                      InkResponse(
                        radius: 14,
                        onTap: onClearSearch,
                        child: const Icon(Icons.close, size: 15),
                      ),
                    ],
                  ],
                ),
              ),
            ),
            PopupMenuButton<TopologyLayoutKind>(
              tooltip: 'Alterar layout da topologia',
              onSelected: onLayoutChanged,
              itemBuilder: (_) => <PopupMenuEntry<TopologyLayoutKind>>[
                for (final kind in TopologyLayoutKind.values)
                  PopupMenuItem<TopologyLayoutKind>(
                    value: kind,
                    child: ListTile(
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      leading: Icon(
                        kind.icon,
                        color: filterSpec.layoutMode == kind
                            ? scheme.primary
                            : null,
                      ),
                      title: Text(
                        kind.label,
                        style: TextStyle(
                          fontWeight: filterSpec.layoutMode == kind
                              ? FontWeight.w700
                              : FontWeight.w500,
                        ),
                      ),
                      subtitle: Text(
                        kind.subtitle,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ),
              ],
              child: Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: 10,
                  vertical: 7,
                ),
                decoration: BoxDecoration(
                  color: scheme.surfaceContainerHigh,
                  borderRadius: BorderRadius.circular(10),
                  border: Border.all(color: scheme.outlineVariant),
                ),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    Icon(
                      filterSpec.layoutMode.icon,
                      size: 16,
                      color: scheme.primary,
                    ),
                    const SizedBox(width: 6),
                    Text(
                      filterSpec.layoutMode.label,
                      style: context.textTheme.labelMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    const SizedBox(width: 2),
                    const Icon(Icons.arrow_drop_down, size: 18),
                  ],
                ),
              ),
            ),
            PopupMenuButton<TopologyOverlayMode>(
              tooltip: 'Camada de visualização',
              onSelected: onOverlayChanged,
              itemBuilder: (_) => <PopupMenuEntry<TopologyOverlayMode>>[
                for (final mode in TopologyOverlayMode.values)
                  PopupMenuItem<TopologyOverlayMode>(
                    value: mode,
                    child: ListTile(
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      leading: Icon(
                        mode.icon,
                        color: filterSpec.overlayMode == mode
                            ? scheme.primary
                            : null,
                      ),
                      title: Text(
                        mode.label,
                        style: TextStyle(
                          fontWeight: filterSpec.overlayMode == mode
                              ? FontWeight.w700
                              : FontWeight.w500,
                        ),
                      ),
                      subtitle: Text(
                        mode.subtitle,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  ),
              ],
              child: Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: 10,
                  vertical: 7,
                ),
                decoration: BoxDecoration(
                  color: filterSpec.overlayMode == TopologyOverlayMode.status
                      ? scheme.surfaceContainerHigh
                      : scheme.primaryContainer,
                  borderRadius: BorderRadius.circular(10),
                  border: Border.all(
                    color: filterSpec.overlayMode == TopologyOverlayMode.status
                        ? scheme.outlineVariant
                        : scheme.primary,
                  ),
                ),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    Icon(
                      filterSpec.overlayMode.icon,
                      size: 16,
                      color:
                          filterSpec.overlayMode == TopologyOverlayMode.status
                          ? scheme.onSurface
                          : scheme.onPrimaryContainer,
                    ),
                    const SizedBox(width: 6),
                    Text(
                      filterSpec.overlayMode.shortLabel,
                      style: context.textTheme.labelMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                        color:
                            filterSpec.overlayMode == TopologyOverlayMode.status
                            ? scheme.onSurface
                            : scheme.onPrimaryContainer,
                      ),
                    ),
                    const SizedBox(width: 2),
                    const Icon(Icons.arrow_drop_down, size: 18),
                  ],
                ),
              ),
            ),
            Badge.count(
              isLabelVisible: filterCount > 0,
              count: filterCount,
              child: FilledButton.tonalIcon(
                style: FilledButton.styleFrom(
                  minimumSize: const Size(0, 38),
                  padding: const EdgeInsets.symmetric(horizontal: 12),
                ),
                onPressed: onOpenFilters,
                icon: const Icon(Icons.tune, size: 17),
                label: const Text('Filtros'),
              ),
            ),
            if (canWrite)
              FilterChip(
                avatar: Icon(
                  editLayoutEnabled
                      ? Icons.lock_open_outlined
                      : Icons.lock_outline,
                  size: 16,
                ),
                label: Text(
                  editLayoutEnabled ? 'Editando posições' : 'Posições travadas',
                ),
                selected: editLayoutEnabled,
                onSelected: (_) => onToggleEditLayout(),
              ),
            _HudActionsMenu(
              canWrite: canWrite,
              savingLayout: savingLayout,
              recalculating: recalculating,
              pathPickingActive: pathPickingActive,
              linkPickingActive: linkPickingActive,
              onAction: onAction,
            ),
          ],
        ),
      ),
    );
  }
}

class _HudActionsMenu extends StatelessWidget {
  const _HudActionsMenu({
    required this.canWrite,
    required this.savingLayout,
    required this.recalculating,
    required this.pathPickingActive,
    required this.linkPickingActive,
    required this.onAction,
  });

  final bool canWrite;
  final bool savingLayout;
  final bool recalculating;
  final bool pathPickingActive;
  final bool linkPickingActive;
  final ValueChanged<TopologyHudAction> onAction;

  @override
  Widget build(BuildContext context) {
    return PopupMenuButton<TopologyHudAction>(
      tooltip: 'Mais ações da topologia',
      icon: const Icon(Icons.more_vert),
      onSelected: onAction,
      itemBuilder: (_) => <PopupMenuEntry<TopologyHudAction>>[
        if (canWrite) ...<PopupMenuEntry<TopologyHudAction>>[
          PopupMenuItem<TopologyHudAction>(
            value: TopologyHudAction.saveLayout,
            enabled: !savingLayout,
            child: const ListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              leading: Icon(Icons.save_outlined),
              title: Text('Salvar layout'),
            ),
          ),
          const PopupMenuItem<TopologyHudAction>(
            value: TopologyHudAction.resetLayout,
            child: ListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              leading: Icon(Icons.restart_alt),
              title: Text('Resetar layout'),
            ),
          ),
          const PopupMenuDivider(),
          const PopupMenuItem<TopologyHudAction>(
            value: TopologyHudAction.addLink,
            child: ListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              leading: Icon(Icons.add_link),
              title: Text('Novo link'),
            ),
          ),
          PopupMenuItem<TopologyHudAction>(
            value: TopologyHudAction.connectOnMap,
            child: ListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              leading: const Icon(Icons.touch_app_outlined),
              title: Text(
                linkPickingActive
                    ? 'Cancelar conexão no mapa'
                    : 'Conectar no mapa',
              ),
            ),
          ),
          const PopupMenuItem<TopologyHudAction>(
            value: TopologyHudAction.unmanagedSwitch,
            child: ListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              leading: Icon(Icons.hub_outlined),
              title: Text('Switch não gerenciado'),
            ),
          ),
          PopupMenuItem<TopologyHudAction>(
            value: TopologyHudAction.recalculate,
            enabled: !recalculating,
            child: const ListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              leading: Icon(Icons.sync),
              title: Text('Recalcular LLDP/CDP/ARP'),
            ),
          ),
          const PopupMenuDivider(),
        ],
        PopupMenuItem<TopologyHudAction>(
          value: TopologyHudAction.pathFinder,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: const Icon(Icons.route),
            title: Text(
              pathPickingActive ? 'Cancelar caminho' : 'Traçar caminho A → B',
            ),
          ),
        ),
        const PopupMenuItem<TopologyHudAction>(
          value: TopologyHudAction.legend,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(Icons.info_outline),
            title: Text('Legenda'),
          ),
        ),
        const PopupMenuItem<TopologyHudAction>(
          value: TopologyHudAction.refresh,
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(Icons.refresh),
            title: Text('Atualizar'),
          ),
        ),
      ],
    );
  }
}
