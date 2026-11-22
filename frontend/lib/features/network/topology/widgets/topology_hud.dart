import 'package:flutter/material.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Ações disponíveis no menu de contexto do HUD da topologia.
enum TopologyHudAction {
  saveLayout,
  resetLayout,
  recalculate,
  addLink,
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
    required this.onZoomIn,
    required this.onZoomOut,
    required this.onFitToScreen,
    required this.onTapProblems,
    required this.onTapOffline,
    required this.onClearPath,
    required this.onCancelPathPicking,
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
  final VoidCallback onZoomIn;
  final VoidCallback onZoomOut;
  final VoidCallback onFitToScreen;
  final VoidCallback onTapProblems;
  final VoidCallback onTapOffline;
  final VoidCallback onClearPath;
  final VoidCallback onCancelPathPicking;
  final VoidCallback onReloadRemoteLayout;
  final ValueChanged<TopologyHudAction> onAction;

  @override
  Widget build(BuildContext context) {
    final width = MediaQuery.sizeOf(context).width;
    final isCompact = width < 720;

    return Stack(
      children: <Widget>[
        // ── Barra superior + pílulas informativas (deixa ~88%+ do mobile livre) ──
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
                  _LiveStatusPill(
                    summary: healthSummary,
                    onlyProblemsActive: filterSpec.onlyProblems,
                    offlineFilterActive: filterSpec.statuses.contains(
                      TopologyNodeStatus.offline,
                    ),
                    onTapProblems: onTapProblems,
                    onTapOffline: onTapOffline,
                  ),
                  if (filterSpec.overlayMode != TopologyOverlayMode.status)
                    _ActiveOverlayChip(
                      mode: filterSpec.overlayMode,
                      onClear: () =>
                          onOverlayChanged(TopologyOverlayMode.status),
                    ),
                  if (hasRemoteLayoutUpdate)
                    _RemoteLayoutNoticePill(onReload: onReloadRemoteLayout),
                ],
              ),
              if (pathPickingSourceName != null) ...<Widget>[
                const SizedBox(height: 6),
                _PathPickingBanner(
                  sourceName: pathPickingSourceName!,
                  onCancel: onCancelPathPicking,
                ),
              ] else if (activePath != null &&
                  activePath!.nodeIds.length >= 2) ...<Widget>[
                const SizedBox(height: 6),
                _PathTraceBanner(
                  path: activePath!,
                  nodeNames: pathNodeNames,
                  onClose: onClearPath,
                ),
              ],
              if (hasUnsavedLayoutChanges) ...<Widget>[
                const SizedBox(height: 6),
                _UnsavedLayoutPill(
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
                _EditModeHelperPill(
                  snapToGrid: filterSpec.snapToGrid,
                  onToggleSnapToGrid: onToggleSnapToGrid,
                  onDone: onToggleEditLayout,
                ),
              ],
            ],
          ),
        ),

        // ── Controles ergonômicos na zona do polegar (canto inferior direito) ──
        Positioned(
          right: 12 + rightInset,
          bottom: 12,
          child: _ThumbZoneControls(
            editLayoutEnabled: editLayoutEnabled,
            canWrite: canWrite,
            overlayMode: filterSpec.overlayMode,
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
    required this.onAction,
  });

  final bool canWrite;
  final bool savingLayout;
  final bool recalculating;
  final bool pathPickingActive;
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

class _LiveStatusPill extends StatelessWidget {
  const _LiveStatusPill({
    required this.summary,
    required this.onlyProblemsActive,
    required this.offlineFilterActive,
    required this.onTapProblems,
    required this.onTapOffline,
  });

  final TopologyHealthSummary summary;
  final bool onlyProblemsActive;
  final bool offlineFilterActive;
  final VoidCallback onTapProblems;
  final VoidCallback onTapOffline;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final sem = context.semanticColors;

    return Material(
      elevation: 2,
      color: scheme.surface.withValues(alpha: 0.94),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(20),
        side: BorderSide(color: scheme.outlineVariant),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
        child: Wrap(
          spacing: 8,
          runSpacing: 4,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            _StatusDotSegment(
              color: sem.success,
              label: '${summary.online} online',
            ),
            InkWell(
              onTap: onTapProblems,
              borderRadius: BorderRadius.circular(12),
              child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
                decoration: onlyProblemsActive
                    ? BoxDecoration(
                        color: sem.warningContainer,
                        borderRadius: BorderRadius.circular(12),
                      )
                    : null,
                child: _StatusDotSegment(
                  color: sem.warning,
                  label:
                      '${summary.warning} ${summary.warning == 1 ? 'alerta' : 'alertas'}',
                  highlighted: onlyProblemsActive || summary.warning > 0,
                  textColor: onlyProblemsActive ? sem.onWarningContainer : null,
                ),
              ),
            ),
            InkWell(
              onTap: onTapOffline,
              borderRadius: BorderRadius.circular(12),
              child: Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
                decoration: offlineFilterActive
                    ? BoxDecoration(
                        color: sem.errorContainer,
                        borderRadius: BorderRadius.circular(12),
                      )
                    : null,
                child: _StatusDotSegment(
                  color: sem.error,
                  label: '${summary.offline} off',
                  highlighted: offlineFilterActive || summary.offline > 0,
                  textColor: offlineFilterActive ? sem.onErrorContainer : null,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _StatusDotSegment extends StatelessWidget {
  const _StatusDotSegment({
    required this.color,
    required this.label,
    this.highlighted = false,
    this.textColor,
  });

  final Color color;
  final String label;
  final bool highlighted;
  final Color? textColor;

  @override
  Widget build(BuildContext context) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Container(
          width: 7,
          height: 7,
          decoration: BoxDecoration(shape: BoxShape.circle, color: color),
        ),
        const SizedBox(width: 5),
        Text(
          label,
          style: context.textTheme.labelSmall?.copyWith(
            fontWeight: highlighted ? FontWeight.w800 : FontWeight.w600,
            color: textColor ?? context.colorScheme.onSurface,
          ),
        ),
      ],
    );
  }
}

class _ActiveOverlayChip extends StatelessWidget {
  const _ActiveOverlayChip({required this.mode, required this.onClear});

  final TopologyOverlayMode mode;
  final VoidCallback onClear;

  @override
  Widget build(BuildContext context) {
    return SemanticChip(
      label: 'Camada: ${mode.shortLabel}',
      tone: SemanticTone.primary,
      icon: mode.icon,
      compact: true,
      onTap: onClear,
    );
  }
}

class _RemoteLayoutNoticePill extends StatelessWidget {
  const _RemoteLayoutNoticePill({required this.onReload});

  final VoidCallback onReload;

  @override
  Widget build(BuildContext context) {
    return SemanticChip(
      label: 'Layout atualizado · Recarregar',
      tone: SemanticTone.info,
      icon: Icons.sync,
      compact: true,
      onTap: onReload,
    );
  }
}

class _PathPickingBanner extends StatelessWidget {
  const _PathPickingBanner({required this.sourceName, required this.onCancel});

  final String sourceName;
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(SemanticTone.info, scheme);

    return Material(
      elevation: 3,
      color: colors.container,
      shape: RoundedRectangleBorder(
        borderRadius: AppTheme.borderRadius,
        side: BorderSide(color: colors.border),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Icon(Icons.route, size: 17, color: colors.onContainer),
            const SizedBox(width: 8),
            Flexible(
              child: Text(
                'Origem: $sourceName — toque no nó de destino',
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: context.textTheme.labelMedium?.copyWith(
                  color: colors.onContainer,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
            const SizedBox(width: 8),
            TextButton(
              style: TextButton.styleFrom(
                foregroundColor: colors.onContainer,
                visualDensity: VisualDensity.compact,
                padding: const EdgeInsets.symmetric(horizontal: 8),
              ),
              onPressed: onCancel,
              child: const Text('Cancelar'),
            ),
          ],
        ),
      ),
    );
  }
}

class _PathTraceBanner extends StatelessWidget {
  const _PathTraceBanner({
    required this.path,
    required this.nodeNames,
    required this.onClose,
  });

  final TopologyPathResult path;
  final List<String> nodeNames;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(SemanticTone.warning, scheme);
    final hops = path.hopCount;
    final hopsLabel = '$hops ${hops == 1 ? 'salto' : 'saltos'}';
    final chain = nodeNames.isNotEmpty
        ? nodeNames.join(' → ')
        : path.nodeIds.map((id) => '#$id').join(' → ');

    return Material(
      elevation: 3,
      color: colors.container,
      shape: RoundedRectangleBorder(
        borderRadius: AppTheme.borderRadius,
        side: BorderSide(color: colors.border),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Icon(Icons.alt_route, size: 17, color: colors.onContainer),
            const SizedBox(width: 8),
            Flexible(
              child: Text(
                'Caminho: $chain ($hopsLabel)',
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                style: context.textTheme.labelMedium?.copyWith(
                  color: colors.onContainer,
                  fontWeight: FontWeight.w700,
                ),
              ),
            ),
            const SizedBox(width: 8),
            TextButton(
              style: TextButton.styleFrom(
                foregroundColor: colors.onContainer,
                visualDensity: VisualDensity.compact,
                padding: const EdgeInsets.symmetric(horizontal: 8),
              ),
              onPressed: onClose,
              child: const Text('Fechar'),
            ),
          ],
        ),
      ),
    );
  }
}

class _EditModeHelperPill extends StatelessWidget {
  const _EditModeHelperPill({
    required this.snapToGrid,
    this.onToggleSnapToGrid,
    required this.onDone,
  });

  final bool snapToGrid;
  final VoidCallback? onToggleSnapToGrid;
  final VoidCallback onDone;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(SemanticTone.primary, scheme);

    return Material(
      elevation: 3,
      color: scheme.surface.withValues(alpha: 0.96),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(24),
        side: BorderSide(color: colors.foreground, width: 1.4),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 5),
        child: Wrap(
          spacing: 8,
          runSpacing: 4,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            Icon(Icons.open_with, size: 16, color: colors.foreground),
            Text(
              'Arraste os nós para mover',
              style: context.textTheme.labelMedium?.copyWith(
                fontWeight: FontWeight.w700,
                color: scheme.onSurface,
              ),
            ),
            if (onToggleSnapToGrid != null)
              FilterChip(
                visualDensity: VisualDensity.compact,
                materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                avatar: const Icon(Icons.grid_4x4, size: 14),
                label: const Text('Grade'),
                selected: snapToGrid,
                onSelected: (_) => onToggleSnapToGrid!(),
              ),
            TextButton(
              style: TextButton.styleFrom(
                minimumSize: const Size(0, 30),
                padding: const EdgeInsets.symmetric(horizontal: 10),
                visualDensity: VisualDensity.compact,
              ),
              onPressed: onDone,
              child: const Text('Concluir'),
            ),
          ],
        ),
      ),
    );
  }
}

class _UnsavedLayoutPill extends StatelessWidget {
  const _UnsavedLayoutPill({
    required this.saving,
    required this.canUndo,
    required this.movedCount,
    required this.snapToGrid,
    this.onToggleSnapToGrid,
    required this.onSave,
    this.onUndo,
    required this.onDiscard,
  });

  final bool saving;
  final bool canUndo;
  final int movedCount;
  final bool snapToGrid;
  final VoidCallback? onToggleSnapToGrid;
  final VoidCallback onSave;
  final VoidCallback? onUndo;
  final VoidCallback onDiscard;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(SemanticTone.primary, scheme);

    return Material(
      elevation: 3,
      color: scheme.surface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(24),
        side: BorderSide(color: colors.foreground, width: 1.5),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 5),
        child: Wrap(
          spacing: 8,
          runSpacing: 4,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            Icon(Icons.open_with, size: 16, color: colors.foreground),
            Text(
              'Layout editado',
              style: context.textTheme.labelMedium?.copyWith(
                fontWeight: FontWeight.w700,
              ),
            ),
            if (movedCount > 0)
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                decoration: BoxDecoration(
                  color: colors.container,
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Text(
                  '$movedCount',
                  style: context.textTheme.labelSmall?.copyWith(
                    fontWeight: FontWeight.w800,
                    color: colors.onContainer,
                  ),
                ),
              ),
            if (onToggleSnapToGrid != null)
              FilterChip(
                visualDensity: VisualDensity.compact,
                materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                avatar: const Icon(Icons.grid_4x4, size: 14),
                label: const Text('Grade'),
                selected: snapToGrid,
                onSelected: saving ? null : (_) => onToggleSnapToGrid!(),
              ),
            FilledButton(
              style: FilledButton.styleFrom(
                minimumSize: const Size(0, 32),
                padding: const EdgeInsets.symmetric(horizontal: 12),
                visualDensity: VisualDensity.compact,
              ),
              onPressed: saving ? null : onSave,
              child: saving
                  ? const SizedBox(
                      width: 14,
                      height: 14,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Text('Salvar'),
            ),
            if (canUndo && onUndo != null)
              OutlinedButton.icon(
                style: OutlinedButton.styleFrom(
                  minimumSize: const Size(0, 32),
                  padding: const EdgeInsets.symmetric(horizontal: 10),
                  visualDensity: VisualDensity.compact,
                ),
                onPressed: saving ? null : onUndo,
                icon: const Icon(Icons.undo, size: 15),
                label: const Text('Desfazer'),
              ),
            TextButton(
              style: TextButton.styleFrom(
                minimumSize: const Size(0, 32),
                padding: const EdgeInsets.symmetric(horizontal: 10),
                visualDensity: VisualDensity.compact,
              ),
              onPressed: saving ? null : onDiscard,
              child: const Text('Descartar'),
            ),
          ],
        ),
      ),
    );
  }
}

class _ThumbZoneControls extends StatelessWidget {
  const _ThumbZoneControls({
    required this.editLayoutEnabled,
    required this.canWrite,
    required this.overlayMode,
    required this.onZoomIn,
    required this.onZoomOut,
    required this.onFitToScreen,
    required this.onToggleEditLayout,
    required this.onOverlayChanged,
  });

  final bool editLayoutEnabled;
  final bool canWrite;
  final TopologyOverlayMode overlayMode;
  final VoidCallback onZoomIn;
  final VoidCallback onZoomOut;
  final VoidCallback onFitToScreen;
  final VoidCallback onToggleEditLayout;
  final ValueChanged<TopologyOverlayMode> onOverlayChanged;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final hasCustomOverlay = overlayMode != TopologyOverlayMode.status;

    return Material(
      elevation: 3,
      color: scheme.surface.withValues(alpha: 0.96),
      shadowColor: scheme.shadow.withValues(alpha: 0.24),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: BorderSide(color: scheme.outlineVariant),
      ),
      clipBehavior: Clip.antiAlias,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          IconButton(
            tooltip: 'Aumentar zoom',
            visualDensity: VisualDensity.compact,
            icon: const Icon(Icons.add, size: 20),
            onPressed: onZoomIn,
          ),
          IconButton(
            tooltip: 'Diminuir zoom',
            visualDensity: VisualDensity.compact,
            icon: const Icon(Icons.remove, size: 20),
            onPressed: onZoomOut,
          ),
          IconButton(
            tooltip: 'Ajustar à tela',
            visualDensity: VisualDensity.compact,
            icon: const Icon(Icons.fit_screen_outlined, size: 20),
            onPressed: onFitToScreen,
          ),
          const SizedBox(width: 34, child: Divider(height: 1)),
          if (canWrite)
            Tooltip(
              message: editLayoutEnabled
                  ? 'Bloquear posições'
                  : 'Editar posições',
              child: InkWell(
                onTap: onToggleEditLayout,
                child: Container(
                  width: 40,
                  height: 40,
                  color: editLayoutEnabled
                      ? scheme.primaryContainer
                      : Colors.transparent,
                  child: Icon(
                    editLayoutEnabled
                        ? Icons.lock_open_outlined
                        : Icons.lock_outline,
                    size: 20,
                    color: editLayoutEnabled
                        ? scheme.onPrimaryContainer
                        : scheme.onSurface,
                  ),
                ),
              ),
            ),
          PopupMenuButton<TopologyOverlayMode>(
            tooltip: 'Camadas da topologia',
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
                      color: overlayMode == mode ? scheme.primary : null,
                    ),
                    title: Text(
                      mode.label,
                      style: TextStyle(
                        fontWeight: overlayMode == mode
                            ? FontWeight.w700
                            : FontWeight.w500,
                      ),
                    ),
                  ),
                ),
            ],
            child: Container(
              width: 40,
              height: 40,
              color: hasCustomOverlay
                  ? scheme.secondaryContainer
                  : Colors.transparent,
              child: Icon(
                Icons.layers_outlined,
                size: 20,
                color: hasCustomOverlay
                    ? scheme.onSecondaryContainer
                    : scheme.onSurface,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// Helper que resolve os nomes dos nós de um caminho para exibir no banner.
List<String> resolvePathNodeNames(
  TopologyPathResult? path,
  List<TopologyNodeModel> nodes,
) {
  if (path == null || path.nodeIds.isEmpty) return const <String>[];
  final byId = <int, TopologyNodeModel>{for (final n in nodes) n.id: n};
  return path.nodeIds
      .map((id) => byId[id]?.name ?? '#$id')
      .toList(growable: false);
}
