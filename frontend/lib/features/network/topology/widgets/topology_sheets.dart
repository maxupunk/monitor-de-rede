import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/features/network/topology/widgets/topology_helpers.dart';
import 'package:netmonitor_app/features/network/topology/widgets/topology_inspector_sheet.dart'
    show topologyNodeInterfacesProvider;
import 'package:netmonitor_app/features/network/widgets/device_form_sheet.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

// ── 1. Search & Jump-to-Node Sheet ──────────────────────────────────────────

/// Abre a folha instantânea de busca ranqueada para pular até qualquer nó.
Future<void> showTopologySearchSheet({
  required BuildContext context,
  required List<TopologyNodeModel> nodes,
  required String initialQuery,
  Map<int, String>? macByNodeId,
  required ValueChanged<TopologyNodeModel> onSelectNode,
  required ValueChanged<String> onApplySearchFilter,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Localizar Equipamento',
    subtitle: 'Busque por nome, IP, MAC, fabricante, tipo ou site',
    maxWidth: 540,
    builder: (ctx) => _TopologySearchSheetBody(
      nodes: nodes,
      initialQuery: initialQuery,
      macByNodeId: macByNodeId,
      onSelectNode: (node) {
        Navigator.of(ctx).pop();
        onSelectNode(node);
      },
      onApplySearchFilter: (q) {
        Navigator.of(ctx).pop();
        onApplySearchFilter(q);
      },
    ),
  );
}

class _TopologySearchSheetBody extends StatefulWidget {
  const _TopologySearchSheetBody({
    required this.nodes,
    required this.initialQuery,
    this.macByNodeId,
    required this.onSelectNode,
    required this.onApplySearchFilter,
  });

  final List<TopologyNodeModel> nodes;
  final String initialQuery;
  final Map<int, String>? macByNodeId;
  final ValueChanged<TopologyNodeModel> onSelectNode;
  final ValueChanged<String> onApplySearchFilter;

  @override
  State<_TopologySearchSheetBody> createState() =>
      _TopologySearchSheetBodyState();
}

class _TopologySearchSheetBodyState extends State<_TopologySearchSheetBody> {
  late final TextEditingController _ctrl;
  TopologyNodeStatus? _quickStatus;

  @override
  void initState() {
    super.initState();
    _ctrl = TextEditingController(text: widget.initialQuery);
  }

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final query = _ctrl.text.trim();
    final scoped = _quickStatus == null
        ? widget.nodes
        : widget.nodes
              .where((n) => classifyNodeStatus(n.status) == _quickStatus)
              .toList(growable: false);

    final hits = query.isEmpty
        ? scoped
              .take(25)
              .map(
                (n) => TopologySearchHit(
                  node: n,
                  score: 1.0,
                  matchedField: TopologySearchField.name,
                ),
              )
              .toList(growable: false)
        : searchTopologyNodes(
            scoped,
            query,
            limit: 25,
            macByNodeId: widget.macByNodeId,
          );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          controller: _ctrl,
          autofocus: true,
          decoration: InputDecoration(
            hintText: 'Ex.: Core-CCR, 10.10.0.1, MikroTik, Matriz...',
            prefixIcon: const Icon(Icons.search),
            suffixIcon: query.isNotEmpty
                ? IconButton(
                    tooltip: 'Limpar busca',
                    icon: const Icon(Icons.close),
                    onPressed: () => setState(() => _ctrl.clear()),
                  )
                : null,
          ),
          onChanged: (_) => setState(() {}),
          onSubmitted: (_) {
            if (hits.isNotEmpty) widget.onSelectNode(hits.first.node);
          },
        ),
        const SizedBox(height: 10),
        Wrap(
          spacing: 8,
          runSpacing: 6,
          children: <Widget>[
            ChoiceChip(
              label: Text('Todos (${widget.nodes.length})'),
              selected: _quickStatus == null,
              onSelected: (_) => setState(() => _quickStatus = null),
            ),
            ChoiceChip(
              label: const Text('Offline'),
              selected: _quickStatus == TopologyNodeStatus.offline,
              onSelected: (_) => setState(
                () => _quickStatus = _quickStatus == TopologyNodeStatus.offline
                    ? null
                    : TopologyNodeStatus.offline,
              ),
            ),
            ChoiceChip(
              label: const Text('Alertas'),
              selected: _quickStatus == TopologyNodeStatus.warning,
              onSelected: (_) => setState(
                () => _quickStatus = _quickStatus == TopologyNodeStatus.warning
                    ? null
                    : TopologyNodeStatus.warning,
              ),
            ),
          ],
        ),
        if (query.isNotEmpty) ...<Widget>[
          const SizedBox(height: 10),
          Align(
            alignment: Alignment.centerRight,
            child: TextButton.icon(
              onPressed: () => widget.onApplySearchFilter(query),
              icon: const Icon(Icons.filter_alt_outlined, size: 17),
              label: const Text('Destacar resultados no mapa'),
            ),
          ),
        ],
        const SizedBox(height: 8),
        if (hits.isEmpty)
          NestedPanel(
            padding: const EdgeInsets.all(16),
            child: Text(
              'Nenhum equipamento encontrado para "$query".',
              style: context.textTheme.bodyMedium?.copyWith(
                color: scheme.onSurfaceVariant,
              ),
            ),
          )
        else
          for (final hit in hits) ...<Widget>[
            _SearchHitTile(
              hit: hit,
              showMatchBadge: query.isNotEmpty,
              onTap: () => widget.onSelectNode(hit.node),
            ),
            const SizedBox(height: 6),
          ],
      ],
    );
  }
}

class _SearchHitTile extends StatelessWidget {
  const _SearchHitTile({
    required this.hit,
    required this.showMatchBadge,
    required this.onTap,
  });

  final TopologySearchHit hit;
  final bool showMatchBadge;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final node = hit.node;
    final status = classifyNodeStatus(node.status);
    final subtitleParts = <String>[
      if (node.ipAddress != null && node.ipAddress!.isNotEmpty) node.ipAddress!,
      topologyNodeTypeLabel(node),
      if (node.siteName != null && node.siteName!.isNotEmpty) node.siteName!,
    ];

    return Material(
      color: scheme.surfaceContainerLow,
      shape: RoundedRectangleBorder(
        borderRadius: AppTheme.borderRadius,
        side: BorderSide(color: scheme.outlineVariant),
      ),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          child: Row(
            children: <Widget>[
              Icon(topologyNodeIcon(node), size: 20, color: scheme.primary),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      node.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.bodyMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    if (subtitleParts.isNotEmpty)
                      Text(
                        subtitleParts.join(' · '),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.labelSmall?.copyWith(
                          color: scheme.onSurfaceVariant,
                        ),
                      ),
                  ],
                ),
              ),
              const SizedBox(width: 8),
              if (showMatchBadge) ...<Widget>[
                SemanticChip(
                  label: hit.matchedField.label,
                  tone: SemanticTone.secondary,
                  compact: true,
                ),
                const SizedBox(width: 6),
              ],
              SemanticChip(
                label: status.label,
                tone: status.tone,
                compact: true,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

// ── 2. Filters, Layout & Visual Options Sheet ───────────────────────────────

/// Abre a folha completa de Filtros, Layout e Opções Visuais da topologia.
Future<void> showTopologyFiltersSheet({
  required BuildContext context,
  required TopologyFilterSpec initialSpec,
  required List<SiteModel> sites,
  required ValueChanged<TopologyFilterSpec> onSpecChanged,
  required ValueChanged<int?> onSiteChanged,
  required ValueChanged<TopologyLayoutKind> onLayoutSelected,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Filtros, Layout e Visualização',
    subtitle: 'Personalize o grafo, camadas ao vivo e critérios de exibição',
    maxWidth: 620,
    builder: (ctx) => _TopologyFiltersSheetBody(
      initialSpec: initialSpec,
      sites: sites,
      onSpecChanged: onSpecChanged,
      onSiteChanged: onSiteChanged,
      onLayoutSelected: onLayoutSelected,
    ),
  );
}

class _TopologyFiltersSheetBody extends StatefulWidget {
  const _TopologyFiltersSheetBody({
    required this.initialSpec,
    required this.sites,
    required this.onSpecChanged,
    required this.onSiteChanged,
    required this.onLayoutSelected,
  });

  final TopologyFilterSpec initialSpec;
  final List<SiteModel> sites;
  final ValueChanged<TopologyFilterSpec> onSpecChanged;
  final ValueChanged<int?> onSiteChanged;
  final ValueChanged<TopologyLayoutKind> onLayoutSelected;

  @override
  State<_TopologyFiltersSheetBody> createState() =>
      _TopologyFiltersSheetBodyState();
}

class _TopologyFiltersSheetBodyState extends State<_TopologyFiltersSheetBody> {
  late TopologyFilterSpec _spec;

  @override
  void initState() {
    super.initState();
    _spec = widget.initialSpec;
  }

  void _update(TopologyFilterSpec next) {
    setState(() => _spec = next);
    widget.onSpecChanged(next);
  }

  @override
  Widget build(BuildContext context) {
    final validSiteId = widget.sites.any((s) => s.id == _spec.siteId)
        ? _spec.siteId
        : null;
    final selectedStatus = _spec.statuses.length == 1
        ? _spec.statuses.first.name
        : 'all';
    final selectedType = _spec.types.length == 1 ? _spec.types.first : 'all';

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        // ── 1. Layout (ChoiceCards) ──
        Text(
          'Layout do Mapa',
          style: context.textTheme.titleSmall?.copyWith(
            fontWeight: FontWeight.w800,
          ),
        ),
        const SizedBox(height: 8),
        for (final kind in TopologyLayoutKind.values) ...<Widget>[
          ChoiceCard(
            title: kind.label,
            subtitle: kind.subtitle,
            icon: kind.icon,
            selected: _spec.layoutMode == kind,
            onTap: () {
              final next = _spec.copyWith(layoutMode: kind);
              setState(() => _spec = next);
              widget.onLayoutSelected(kind);
            },
          ),
          const SizedBox(height: 8),
        ],

        const Divider(height: 24),

        // ── 2. Camadas / Overlay ──
        Text(
          'Camada de Sobreposição',
          style: context.textTheme.titleSmall?.copyWith(
            fontWeight: FontWeight.w800,
          ),
        ),
        const SizedBox(height: 8),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            for (final mode in TopologyOverlayMode.values)
              ChoiceChip(
                avatar: Icon(mode.icon, size: 16),
                label: Text(mode.label),
                selected: _spec.overlayMode == mode,
                onSelected: (_) => _update(_spec.copyWith(overlayMode: mode)),
              ),
          ],
        ),

        const Divider(height: 24),

        // ── 3. Filtros de Dados ──
        Row(
          children: <Widget>[
            Expanded(
              child: Text(
                'Filtros de Equipamentos',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w800,
                ),
              ),
            ),
            if (_spec.hasActiveFilters)
              TextButton.icon(
                onPressed: () {
                  final hadSite = _spec.siteId != null;
                  final cleared = _spec.clearDataFilters();
                  _update(cleared);
                  if (hadSite) widget.onSiteChanged(null);
                },
                icon: const Icon(Icons.filter_alt_off_outlined, size: 16),
                label: const Text('Limpar'),
              ),
          ],
        ),
        const SizedBox(height: 10),
        if (widget.sites.isNotEmpty) ...<Widget>[
          AppDropdownFormField<int?>(
            key: ValueKey<String>('filter-site-${validSiteId ?? 'all'}'),
            initialValue: validSiteId,
            decoration: const InputDecoration(labelText: 'Site'),
            items: <DropdownMenuItem<int?>>[
              const DropdownMenuItem<int?>(
                value: null,
                child: Text('Todos os sites', overflow: TextOverflow.ellipsis),
              ),
              for (final s in widget.sites)
                DropdownMenuItem<int?>(
                  value: s.id,
                  child: Text(s.name, overflow: TextOverflow.ellipsis),
                ),
            ],
            onChanged: (v) {
              setState(
                () => _spec = _spec.copyWith(siteId: v, clearSiteId: v == null),
              );
              widget.onSiteChanged(v);
            },
          ),
          const SizedBox(height: 10),
        ],
        ResponsiveFieldRow(
          breakpoint: 480,
          children: <Widget>[
            AppDropdownFormField<String>(
              key: ValueKey<String>('filter-status-$selectedStatus'),
              initialValue: selectedStatus,
              decoration: const InputDecoration(labelText: 'Status dos nós'),
              items: const <DropdownMenuItem<String>>[
                DropdownMenuItem(
                  value: 'all',
                  child: Text('Todos', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 'online',
                  child: Text('Online', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 'warning',
                  child: Text('Alerta', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 'offline',
                  child: Text('Offline', overflow: TextOverflow.ellipsis),
                ),
                DropdownMenuItem(
                  value: 'unknown',
                  child: Text('Sem dados', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: (v) {
                final parsed = TopologyNodeStatus.tryFromStorageKey(v);
                _update(
                  _spec.copyWith(
                    statuses: parsed != null
                        ? <TopologyNodeStatus>{parsed}
                        : const <TopologyNodeStatus>{},
                  ),
                );
              },
            ),
            AppDropdownFormField<String>(
              key: ValueKey<String>('filter-type-$selectedType'),
              initialValue:
                  selectedType == 'all' ||
                      deviceTypeOptions.any((opt) => opt.$1 == selectedType)
                  ? selectedType
                  : 'all',
              decoration: const InputDecoration(
                labelText: 'Tipo de equipamento',
              ),
              items: <DropdownMenuItem<String>>[
                const DropdownMenuItem(
                  value: 'all',
                  child: Text('Todos', overflow: TextOverflow.ellipsis),
                ),
                for (final opt in deviceTypeOptions)
                  DropdownMenuItem(
                    value: opt.$1,
                    child: Text(opt.$2, overflow: TextOverflow.ellipsis),
                  ),
              ],
              onChanged: (v) {
                _update(
                  _spec.copyWith(
                    types: v != null && v != 'all'
                        ? <String>{v}
                        : const <String>{},
                  ),
                );
              },
            ),
          ],
        ),
        const SizedBox(height: 10),
        Material(
          type: MaterialType.transparency,
          child: SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Apenas nós e enlaces com problema'),
            value: _spec.onlyProblems,
            onChanged: (v) => _update(_spec.copyWith(onlyProblems: v)),
          ),
        ),
        const SizedBox(height: 6),
        Row(
          children: <Widget>[
            Expanded(
              child: Text(
                'Ao filtrar nós',
                style: context.textTheme.bodyMedium?.copyWith(
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            SegmentedButton<TopologyFilterDisplayMode>(
              showSelectedIcon: false,
              segments: const <ButtonSegment<TopologyFilterDisplayMode>>[
                ButtonSegment(
                  value: TopologyFilterDisplayMode.dim,
                  label: Text('Esmaecer'),
                ),
                ButtonSegment(
                  value: TopologyFilterDisplayMode.hide,
                  label: Text('Ocultar'),
                ),
              ],
              selected: <TopologyFilterDisplayMode>{_spec.filterDisplayMode},
              onSelectionChanged: (sel) =>
                  _update(_spec.copyWith(filterDisplayMode: sel.first)),
            ),
          ],
        ),

        const Divider(height: 24),

        // ── 4. Opções Visuais ──
        Text(
          'Opções Visuais',
          style: context.textTheme.titleSmall?.copyWith(
            fontWeight: FontWeight.w800,
          ),
        ),
        const SizedBox(height: 10),
        Row(
          children: <Widget>[
            Expanded(
              child: Text(
                'Estilo das conexões',
                style: context.textTheme.bodyMedium?.copyWith(
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
            SegmentedButton<TopologyEdgeStyle>(
              showSelectedIcon: false,
              segments: const <ButtonSegment<TopologyEdgeStyle>>[
                ButtonSegment(
                  value: TopologyEdgeStyle.curved,
                  label: Text('Curvas'),
                ),
                ButtonSegment(
                  value: TopologyEdgeStyle.orthogonal,
                  label: Text('Ortogonais'),
                ),
              ],
              selected: <TopologyEdgeStyle>{_spec.edgeStyle},
              onSelectionChanged: (sel) =>
                  _update(_spec.copyWith(edgeStyle: sel.first)),
            ),
          ],
        ),
        const SizedBox(height: 6),
        Material(
          type: MaterialType.transparency,
          child: Column(
            children: <Widget>[
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Rótulos nos enlaces'),
                value: _spec.showLabels,
                onChanged: (v) => _update(_spec.copyWith(showLabels: v)),
              ),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Fluxo de tráfego animado'),
                value: _spec.showTrafficFlow,
                onChanged: (v) => _update(_spec.copyWith(showTrafficFlow: v)),
              ),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Minimapa'),
                value: _spec.showMinimap,
                onChanged: (v) => _update(_spec.copyWith(showMinimap: v)),
              ),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Ocultar nós isolados'),
                value: _spec.hideIsolated,
                onChanged: (v) => _update(_spec.copyWith(hideIsolated: v)),
              ),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Endereços IP nos nós'),
                value: _spec.showIpLabels,
                onChanged: (v) => _update(_spec.copyWith(showIpLabels: v)),
              ),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Badges de CPU/RAM nos nós'),
                value: _spec.showHealthBadges,
                onChanged: (v) => _update(_spec.copyWith(showHealthBadges: v)),
              ),
              SwitchListTile(
                contentPadding: EdgeInsets.zero,
                title: const Text('Alinhar à grade ao arrastar'),
                value: _spec.snapToGrid,
                onChanged: (v) => _update(_spec.copyWith(snapToGrid: v)),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

// ── 3. Create / Edit Manual Link Sheet ──────────────────────────────────────

/// Abre a folha pesquisável e sensível a interfaces para criar ou editar
/// um enlace manual da topologia.
Future<void> showTopologyLinkFormSheet({
  required BuildContext context,
  required List<TopologyNodeModel> nodes,
  int? initialSourceId,
  int? initialTargetId,
  TopologyEdgeModel? existingEdge,
  Map<int, String>? macByNodeId,
  ValueChanged<int?>? onPickOnMap,
  required Future<bool> Function(Map<String, dynamic> payload) onSubmit,
}) {
  final isEditing = existingEdge != null;
  return showAdaptiveFormSheet<void>(
    context: context,
    title: isEditing ? 'Editar Enlace' : 'Adicionar Enlace Manual',
    subtitle: isEditing
        ? '${existingEdge.sourceDeviceName ?? '#${existingEdge.sourceDeviceId}'} ↔ ${existingEdge.targetDeviceName ?? '#${existingEdge.targetDeviceId}'}'
        : 'Pesquise equipamentos e portas ou selecione direto no mapa',
    maxWidth: 600,
    builder: (ctx) => _TopologyLinkFormSheetBody(
      nodes: nodes,
      initialSourceId: initialSourceId,
      initialTargetId: initialTargetId,
      existingEdge: existingEdge,
      macByNodeId: macByNodeId,
      onPickOnMap: onPickOnMap != null
          ? (srcId) {
              Navigator.of(ctx).pop();
              onPickOnMap(srcId);
            }
          : null,
      onCancel: () => Navigator.of(ctx).pop(),
      onSubmit: (payload) async {
        final ok = await onSubmit(payload);
        if (ctx.mounted && ok) {
          Navigator.of(ctx).pop();
        }
        return ok;
      },
    ),
  );
}

class _TopologyLinkFormSheetBody extends ConsumerStatefulWidget {
  const _TopologyLinkFormSheetBody({
    required this.nodes,
    this.initialSourceId,
    this.initialTargetId,
    this.existingEdge,
    this.macByNodeId,
    this.onPickOnMap,
    required this.onCancel,
    required this.onSubmit,
  });

  final List<TopologyNodeModel> nodes;
  final int? initialSourceId;
  final int? initialTargetId;
  final TopologyEdgeModel? existingEdge;
  final Map<int, String>? macByNodeId;
  final ValueChanged<int?>? onPickOnMap;
  final VoidCallback onCancel;
  final Future<bool> Function(Map<String, dynamic> payload) onSubmit;

  @override
  ConsumerState<_TopologyLinkFormSheetBody> createState() =>
      _TopologyLinkFormSheetBodyState();
}

class _TopologyLinkFormSheetBodyState
    extends ConsumerState<_TopologyLinkFormSheetBody> {
  static const Set<String> _validLinkTypes = <String>{
    'ethernet',
    'fiber',
    'wireless',
    'vpn',
  };

  int? _srcId;
  int? _tgtId;
  int? _srcInterfaceId;
  int? _tgtInterfaceId;
  DeviceInterfaceModel? _srcInterfaceObj;
  DeviceInterfaceModel? _tgtInterfaceObj;
  String _linkType = 'ethernet';
  bool _userPickedLinkType = false;
  bool _autoSuggestedLinkType = false;
  bool _saving = false;

  bool get _isEditing => widget.existingEdge != null;

  @override
  void initState() {
    super.initState();
    final edge = widget.existingEdge;
    if (edge != null) {
      _srcId = edge.sourceDeviceId;
      _tgtId = edge.targetDeviceId;
      _srcInterfaceId = edge.sourceInterfaceId;
      _tgtInterfaceId = edge.targetInterfaceId;
      _linkType = _validLinkTypes.contains(edge.linkType)
          ? edge.linkType
          : 'ethernet';
      _userPickedLinkType = true;
      return;
    }

    final validInitialSrc =
        widget.initialSourceId != null &&
            widget.nodes.any((n) => n.id == widget.initialSourceId)
        ? widget.initialSourceId
        : widget.nodes.firstOrNull?.id;
    _srcId = validInitialSrc;

    final srcNode = _nodeById(_srcId);
    if (widget.initialTargetId != null &&
        widget.initialTargetId != _srcId &&
        widget.nodes.any((n) => n.id == widget.initialTargetId)) {
      _tgtId = widget.initialTargetId;
    } else {
      final rankedTargets = rankTopologyEndpointCandidates(
        nodes: widget.nodes,
        query: '',
        excludedNodeId: _srcId,
        preferSiteId: srcNode?.siteId,
        macByNodeId: widget.macByNodeId,
      );
      _tgtId = rankedTargets.firstOrNull?.id;
    }
  }

  TopologyNodeModel? _nodeById(int? id) {
    if (id == null) return null;
    for (final n in widget.nodes) {
      if (n.id == id) return n;
    }
    return null;
  }

  void _applyInterfaceAutoSuggestion() {
    if (_userPickedLinkType) return;
    final suggested = resolveSuggestedTopologyLinkType(
      sourceInterface: _srcInterfaceObj,
      targetInterface: _tgtInterfaceObj,
    );
    if (suggested != null && _validLinkTypes.contains(suggested)) {
      _linkType = suggested;
      _autoSuggestedLinkType = true;
    }
  }

  void _swapEndpoints() {
    if (_isEditing || _saving) return;
    setState(() {
      final prevSrc = _srcId;
      _srcId = _tgtId;
      _tgtId = prevSrc;

      final prevSrcIfId = _srcInterfaceId;
      _srcInterfaceId = _tgtInterfaceId;
      _tgtInterfaceId = prevSrcIfId;

      final prevSrcIfObj = _srcInterfaceObj;
      _srcInterfaceObj = _tgtInterfaceObj;
      _tgtInterfaceObj = prevSrcIfObj;
    });
  }

  Future<void> _pickSourceNode() async {
    if (_isEditing || _saving) return;
    final tgtNode = _nodeById(_tgtId);
    final chosen = await _showTopologyEndpointPickerSheet(
      context: context,
      title: 'Selecionar Origem',
      subtitle: 'Busque por nome, IP, site, fabricante ou tipo',
      nodes: widget.nodes,
      selectedNodeId: _srcId,
      excludedNodeId: null,
      preferSiteId: tgtNode?.siteId,
      macByNodeId: widget.macByNodeId,
    );
    if (chosen == null || !mounted || chosen.id == _srcId) return;
    setState(() {
      if (chosen.id == _tgtId) {
        // Se escolheu o mesmo nó que estava em Destino, inverte os dois lados.
        final oldSrc = _srcId;
        _srcId = chosen.id;
        _tgtId = oldSrc;
        final oldSrcIfId = _srcInterfaceId;
        _srcInterfaceId = _tgtInterfaceId;
        _tgtInterfaceId = oldSrcIfId;
        final oldSrcIfObj = _srcInterfaceObj;
        _srcInterfaceObj = _tgtInterfaceObj;
        _tgtInterfaceObj = oldSrcIfObj;
      } else {
        _srcId = chosen.id;
        _srcInterfaceId = null;
        _srcInterfaceObj = null;
        _applyInterfaceAutoSuggestion();
      }
    });
  }

  Future<void> _pickTargetNode() async {
    if (_isEditing || _saving) return;
    final srcNode = _nodeById(_srcId);
    final chosen = await _showTopologyEndpointPickerSheet(
      context: context,
      title: 'Selecionar Destino',
      subtitle: srcNode != null
          ? 'Origem: ${srcNode.name} — equipamentos do mesmo site vêm primeiro'
          : 'Busque por nome, IP, site, fabricante ou tipo',
      nodes: widget.nodes,
      selectedNodeId: _tgtId,
      excludedNodeId: _srcId,
      preferSiteId: srcNode?.siteId,
      macByNodeId: widget.macByNodeId,
    );
    if (chosen == null || !mounted || chosen.id == _tgtId) return;
    setState(() {
      _tgtId = chosen.id;
      _tgtInterfaceId = null;
      _tgtInterfaceObj = null;
      _applyInterfaceAutoSuggestion();
    });
  }

  Future<void> _handleSubmit() async {
    final canSave =
        !_saving &&
        (_isEditing || (_srcId != null && _tgtId != null && _srcId != _tgtId));
    if (!canSave) return;
    setState(() => _saving = true);
    final payload = _isEditing
        ? <String, dynamic>{
            'sourceInterfaceId': _srcInterfaceId,
            'targetInterfaceId': _tgtInterfaceId,
            'linkType': _linkType,
          }
        : <String, dynamic>{
            'sourceDeviceId': _srcId,
            'targetDeviceId': _tgtId,
            if (_srcInterfaceId != null) 'sourceInterfaceId': _srcInterfaceId,
            if (_tgtInterfaceId != null) 'targetInterfaceId': _tgtInterfaceId,
            'linkType': _linkType,
          };
    final ok = await widget.onSubmit(payload);
    if (mounted && !ok) {
      setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final srcNode = _nodeById(_srcId);
    final tgtNode = _nodeById(_tgtId);
    final sameSite =
        srcNode?.siteId != null && srcNode?.siteId == tgtNode?.siteId;
    final canSave =
        !_saving &&
        (_isEditing || (_srcId != null && _tgtId != null && _srcId != _tgtId));

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (!_isEditing && widget.onPickOnMap != null) ...<Widget>[
          FilledButton.tonalIcon(
            key: const ValueKey<String>('topology-link-pick-on-map'),
            onPressed: _saving ? null : () => widget.onPickOnMap!(_srcId),
            icon: const Icon(Icons.touch_app_outlined, size: 18),
            label: Text(
              srcNode != null
                  ? 'Selecionar destino no mapa (origem: ${srcNode.name})'
                  : 'Selecionar no mapa',
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
            ),
          ),
          const SizedBox(height: 12),
        ],

        // ── Origem + Porta de Origem ──
        _TopologyEndpointPickerCard(
          key: const ValueKey<String>('topology-link-source-card'),
          roleLabel: 'Origem',
          node: srcNode,
          fallbackName: widget.existingEdge?.sourceDeviceName,
          enabled: !_isEditing && !_saving,
          onTap: _pickSourceNode,
        ),
        if (srcNode != null && !srcNode.isSystem) ...<Widget>[
          const SizedBox(height: 8),
          _TopologyInterfacePickerField(
            fieldKey: const ValueKey<String>('topology-link-source-iface'),
            label: 'Porta de origem',
            deviceId: srcNode.id,
            selectedInterfaceId: _srcInterfaceId,
            fallbackInterfaceName:
                widget.existingEdge?.sourceDeviceId == srcNode.id
                ? widget.existingEdge?.sourceInterfaceName
                : null,
            enabled: !_saving,
            onChanged: (iface) {
              setState(() {
                _srcInterfaceId = iface?.id;
                _srcInterfaceObj = iface;
                _applyInterfaceAutoSuggestion();
              });
            },
          ),
        ],

        // ── Botão 1-toque Inverter Origem / Destino (⇅) ──
        if (!_isEditing)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 6),
            child: Row(
              children: <Widget>[
                const Expanded(child: Divider()),
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 8),
                  child: IconButton.filledTonal(
                    key: const ValueKey<String>('topology-link-swap-endpoints'),
                    tooltip: 'Inverter origem/destino',
                    visualDensity: VisualDensity.compact,
                    onPressed: (_srcId != null && _tgtId != null && !_saving)
                        ? _swapEndpoints
                        : null,
                    icon: const Icon(Icons.swap_vert, size: 20),
                  ),
                ),
                const Expanded(child: Divider()),
              ],
            ),
          )
        else
          const SizedBox(height: 12),

        // ── Destino + Porta de Destino ──
        _TopologyEndpointPickerCard(
          key: const ValueKey<String>('topology-link-target-card'),
          roleLabel: 'Destino',
          node: tgtNode,
          fallbackName: widget.existingEdge?.targetDeviceName,
          sameSiteHighlight: sameSite,
          enabled: !_isEditing && !_saving,
          onTap: _pickTargetNode,
        ),
        if (tgtNode != null && !tgtNode.isSystem) ...<Widget>[
          const SizedBox(height: 8),
          _TopologyInterfacePickerField(
            fieldKey: const ValueKey<String>('topology-link-target-iface'),
            label: 'Porta de destino',
            deviceId: tgtNode.id,
            selectedInterfaceId: _tgtInterfaceId,
            fallbackInterfaceName:
                widget.existingEdge?.targetDeviceId == tgtNode.id
                ? widget.existingEdge?.targetInterfaceName
                : null,
            enabled: !_saving,
            onChanged: (iface) {
              setState(() {
                _tgtInterfaceId = iface?.id;
                _tgtInterfaceObj = iface;
                _applyInterfaceAutoSuggestion();
              });
            },
          ),
        ],

        const SizedBox(height: 14),
        if (_autoSuggestedLinkType) ...<Widget>[
          Wrap(
            spacing: 8,
            runSpacing: 4,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              Text(
                'Meio físico / lógico',
                style: context.textTheme.labelMedium?.copyWith(
                  color: scheme.onSurfaceVariant,
                  fontWeight: FontWeight.w700,
                ),
              ),
              const SemanticChip(
                label: 'Sugerido pela porta',
                tone: SemanticTone.info,
                icon: Icons.auto_awesome,
                compact: true,
              ),
            ],
          ),
          const SizedBox(height: 6),
        ],
        AppDropdownFormField<String>(
          key: ValueKey<String>('topology-link-type-$_linkType'),
          initialValue: _linkType,
          decoration: const InputDecoration(labelText: 'Tipo de enlace'),
          items: const <DropdownMenuItem<String>>[
            DropdownMenuItem(
              value: 'ethernet',
              child: Text('Ethernet', overflow: TextOverflow.ellipsis),
            ),
            DropdownMenuItem(
              value: 'fiber',
              child: Text('Fibra óptica', overflow: TextOverflow.ellipsis),
            ),
            DropdownMenuItem(
              value: 'wireless',
              child: Text('Wireless / Rádio', overflow: TextOverflow.ellipsis),
            ),
            DropdownMenuItem(
              value: 'vpn',
              child: Text('Túnel VPN', overflow: TextOverflow.ellipsis),
            ),
          ],
          onChanged: _saving
              ? null
              : (v) => setState(() {
                  _linkType = v ?? 'ethernet';
                  _userPickedLinkType = true;
                  _autoSuggestedLinkType = false;
                }),
        ),
        const SizedBox(height: 16),
        SymmetricalActionBar(
          children: <Widget>[
            OutlinedButton(
              onPressed: _saving ? null : widget.onCancel,
              child: const Text('Cancelar'),
            ),
            FilledButton.icon(
              onPressed: canSave ? _handleSubmit : null,
              icon: _saving
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : Icon(_isEditing ? Icons.save_outlined : Icons.add_link),
              label: Text(
                _isEditing ? 'Salvar enlace' : 'Criar enlace',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _TopologyEndpointPickerCard extends StatelessWidget {
  const _TopologyEndpointPickerCard({
    super.key,
    required this.roleLabel,
    required this.node,
    this.fallbackName,
    this.sameSiteHighlight = false,
    required this.enabled,
    required this.onTap,
  });

  final String roleLabel;
  final TopologyNodeModel? node;
  final String? fallbackName;
  final bool sameSiteHighlight;
  final bool enabled;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final status = classifyNodeStatus(node?.status);
    final statusColors = context.semanticColors.resolve(status.tone, scheme);
    final hasNode = node != null;
    final displayTitle =
        node?.name ?? fallbackName ?? 'Selecionar equipamento...';
    final siteName = node?.siteName?.trim();
    final ip = node?.ipAddress?.trim();

    return Material(
      color: scheme.surfaceContainerLow,
      shape: RoundedRectangleBorder(
        borderRadius: AppTheme.borderRadius,
        side: BorderSide(color: scheme.outlineVariant),
      ),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: enabled ? onTap : null,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          child: Row(
            children: <Widget>[
              Container(
                width: 38,
                height: 38,
                decoration: BoxDecoration(
                  color: hasNode
                      ? statusColors.container
                      : scheme.surfaceContainerHighest,
                  borderRadius: BorderRadius.circular(10),
                  border: Border.all(
                    color: hasNode
                        ? statusColors.border
                        : scheme.outlineVariant,
                  ),
                ),
                child: Icon(
                  hasNode
                      ? topologyNodeIcon(node!)
                      : Icons.devices_other_outlined,
                  size: 20,
                  color: hasNode
                      ? statusColors.onContainer
                      : scheme.onSurfaceVariant,
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Row(
                      children: <Widget>[
                        Text(
                          roleLabel.toUpperCase(),
                          style: context.textTheme.labelSmall?.copyWith(
                            fontSize: 10,
                            letterSpacing: 0.5,
                            fontWeight: FontWeight.w800,
                            color: scheme.primary,
                          ),
                        ),
                        if (hasNode) ...<Widget>[
                          const SizedBox(width: 6),
                          Container(
                            width: 7,
                            height: 7,
                            decoration: BoxDecoration(
                              color: statusColors.foreground,
                              shape: BoxShape.circle,
                            ),
                          ),
                          const SizedBox(width: 4),
                          Flexible(
                            child: Text(
                              topologyNodeTypeLabel(node!),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: context.textTheme.labelSmall?.copyWith(
                                color: scheme.onSurfaceVariant,
                              ),
                            ),
                          ),
                        ],
                      ],
                    ),
                    const SizedBox(height: 2),
                    Text(
                      displayTitle,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.bodyMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                        color: hasNode
                            ? scheme.onSurface
                            : scheme.onSurfaceVariant,
                      ),
                    ),
                    if (hasNode &&
                        ((ip != null && ip.isNotEmpty) ||
                            (siteName != null &&
                                siteName.isNotEmpty))) ...<Widget>[
                      const SizedBox(height: 3),
                      Wrap(
                        spacing: 6,
                        runSpacing: 4,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        children: <Widget>[
                          if (ip != null && ip.isNotEmpty)
                            Text(
                              ip,
                              style: context.textTheme.labelSmall?.copyWith(
                                fontFamily: 'monospace',
                                fontWeight: FontWeight.w600,
                                color: scheme.onSurfaceVariant,
                              ),
                            ),
                          if (siteName != null && siteName.isNotEmpty)
                            SemanticChip(
                              label: siteName,
                              tone: sameSiteHighlight
                                  ? SemanticTone.primary
                                  : SemanticTone.secondary,
                              compact: true,
                            ),
                        ],
                      ),
                    ],
                  ],
                ),
              ),
              if (enabled) ...<Widget>[
                const SizedBox(width: 8),
                Icon(Icons.manage_search, size: 20, color: scheme.primary),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

Future<TopologyNodeModel?> _showTopologyEndpointPickerSheet({
  required BuildContext context,
  required String title,
  required String subtitle,
  required List<TopologyNodeModel> nodes,
  int? selectedNodeId,
  int? excludedNodeId,
  int? preferSiteId,
  Map<int, String>? macByNodeId,
}) {
  return showAdaptiveFormSheet<TopologyNodeModel>(
    context: context,
    title: title,
    subtitle: subtitle,
    maxWidth: 560,
    builder: (ctx) => _TopologyEndpointPickerBody(
      nodes: nodes,
      selectedNodeId: selectedNodeId,
      excludedNodeId: excludedNodeId,
      preferSiteId: preferSiteId,
      macByNodeId: macByNodeId,
      onSelected: (node) => Navigator.of(ctx).pop(node),
    ),
  );
}

class _TopologyEndpointPickerBody extends StatefulWidget {
  const _TopologyEndpointPickerBody({
    required this.nodes,
    this.selectedNodeId,
    this.excludedNodeId,
    this.preferSiteId,
    this.macByNodeId,
    required this.onSelected,
  });

  final List<TopologyNodeModel> nodes;
  final int? selectedNodeId;
  final int? excludedNodeId;
  final int? preferSiteId;
  final Map<int, String>? macByNodeId;
  final ValueChanged<TopologyNodeModel> onSelected;

  @override
  State<_TopologyEndpointPickerBody> createState() =>
      _TopologyEndpointPickerBodyState();
}

class _TopologyEndpointPickerBodyState
    extends State<_TopologyEndpointPickerBody> {
  final TextEditingController _searchCtrl = TextEditingController();

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final query = _searchCtrl.text.trim();
    final candidates = rankTopologyEndpointCandidates(
      nodes: widget.nodes,
      query: query,
      excludedNodeId: widget.excludedNodeId,
      preferSiteId: widget.preferSiteId,
      macByNodeId: widget.macByNodeId,
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          key: const ValueKey<String>('topology-endpoint-search-input'),
          controller: _searchCtrl,
          autofocus: true,
          decoration: InputDecoration(
            hintText: 'Filtrar por nome, IP, site ou tipo...',
            prefixIcon: const Icon(Icons.search),
            suffixIcon: query.isNotEmpty
                ? IconButton(
                    tooltip: 'Limpar busca',
                    icon: const Icon(Icons.close),
                    onPressed: () => setState(_searchCtrl.clear),
                  )
                : null,
          ),
          onChanged: (_) => setState(() {}),
          onSubmitted: (_) {
            if (candidates.isNotEmpty) widget.onSelected(candidates.first);
          },
        ),
        const SizedBox(height: 10),
        if (candidates.isEmpty)
          NestedPanel(
            padding: const EdgeInsets.all(14),
            child: Text(
              query.isEmpty
                  ? 'Nenhum outro equipamento disponível para conectar.'
                  : 'Nenhum equipamento encontrado para "$query".',
              style: context.textTheme.bodyMedium?.copyWith(
                color: scheme.onSurfaceVariant,
              ),
            ),
          )
        else
          for (final node in candidates.take(40)) ...<Widget>[
            _EndpointCandidateTile(
              node: node,
              selected: node.id == widget.selectedNodeId,
              isSameSite:
                  widget.preferSiteId != null &&
                  node.siteId == widget.preferSiteId,
              onTap: () => widget.onSelected(node),
            ),
            const SizedBox(height: 6),
          ],
      ],
    );
  }
}

class _EndpointCandidateTile extends StatelessWidget {
  const _EndpointCandidateTile({
    required this.node,
    required this.selected,
    required this.isSameSite,
    required this.onTap,
  });

  final TopologyNodeModel node;
  final bool selected;
  final bool isSameSite;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final status = classifyNodeStatus(node.status);
    final statusColors = context.semanticColors.resolve(status.tone, scheme);
    final ip = node.ipAddress?.trim();
    final site = node.siteName?.trim();

    return Material(
      color: selected
          ? scheme.primaryContainer.withValues(alpha: 0.32)
          : scheme.surfaceContainerLow,
      shape: RoundedRectangleBorder(
        borderRadius: AppTheme.borderRadius,
        side: BorderSide(
          color: selected ? scheme.primary : scheme.outlineVariant,
          width: selected ? 1.5 : 1.0,
        ),
      ),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
          child: Row(
            children: <Widget>[
              Stack(
                clipBehavior: Clip.none,
                children: <Widget>[
                  Icon(topologyNodeIcon(node), size: 20, color: scheme.primary),
                  Positioned(
                    right: -2,
                    bottom: -2,
                    child: Container(
                      width: 8,
                      height: 8,
                      decoration: BoxDecoration(
                        color: statusColors.foreground,
                        shape: BoxShape.circle,
                        border: Border.all(color: scheme.surface, width: 1.2),
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      node.name,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.bodyMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    const SizedBox(height: 2),
                    Text(
                      <String>[
                        if (ip != null && ip.isNotEmpty) ip,
                        topologyNodeTypeLabel(node),
                      ].join(' · '),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.labelSmall?.copyWith(
                        color: scheme.onSurfaceVariant,
                      ),
                    ),
                    if (site != null && site.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 4),
                      SemanticChip(
                        label: site,
                        tone: isSameSite
                            ? SemanticTone.primary
                            : SemanticTone.secondary,
                        compact: true,
                      ),
                    ],
                  ],
                ),
              ),
              const SizedBox(width: 8),
              SemanticChip(
                label: status.label,
                tone: status.tone,
                compact: true,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _TopologyInterfacePickerField extends ConsumerWidget {
  const _TopologyInterfacePickerField({
    required this.fieldKey,
    required this.label,
    required this.deviceId,
    required this.selectedInterfaceId,
    this.fallbackInterfaceName,
    required this.enabled,
    required this.onChanged,
  });

  final Key fieldKey;
  final String label;
  final int deviceId;
  final int? selectedInterfaceId;
  final String? fallbackInterfaceName;
  final bool enabled;
  final ValueChanged<DeviceInterfaceModel?> onChanged;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scheme = context.colorScheme;
    final asyncIfaces = ref.watch(topologyNodeInterfacesProvider(deviceId));
    final interfaces =
        asyncIfaces.valueOrNull ?? const <DeviceInterfaceModel>[];
    final selected = selectedInterfaceId != null
        ? interfaces.where((i) => i.id == selectedInterfaceId).firstOrNull
        : null;
    final upCount = interfaces.where((i) => i.isUp).length;

    return asyncIfaces.when(
      loading: () => Container(
        key: fieldKey,
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 9),
        decoration: BoxDecoration(
          color: scheme.surfaceContainerLowest,
          borderRadius: BorderRadius.circular(10),
          border: Border.all(color: scheme.outlineVariant),
        ),
        child: Row(
          children: <Widget>[
            const SizedBox(
              width: 14,
              height: 14,
              child: CircularProgressIndicator(strokeWidth: 2),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                '$label: carregando portas...',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: context.textTheme.labelMedium?.copyWith(
                  color: scheme.onSurfaceVariant,
                ),
              ),
            ),
          ],
        ),
      ),
      error: (_, _) => const SizedBox.shrink(),
      data: (list) {
        if (list.isEmpty && fallbackInterfaceName == null) {
          return const SizedBox.shrink();
        }
        final statusColors = selected != null
            ? context.semanticColors.resolve(
                selected.isUp ? SemanticTone.success : SemanticTone.error,
                scheme,
              )
            : null;
        final speedLabel = selected != null && selected.negotiatedSpeedBps > 0
            ? formatLinkSpeed(selected.negotiatedSpeedBps)
            : null;

        return Material(
          color: scheme.surfaceContainerLowest,
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(10),
            side: BorderSide(color: scheme.outlineVariant),
          ),
          clipBehavior: Clip.antiAlias,
          child: InkWell(
            key: fieldKey,
            onTap: (!enabled || list.isEmpty)
                ? null
                : () async {
                    final picked = await _showTopologyInterfacePickerSheet(
                      context: context,
                      title: label,
                      interfaces: list,
                      selectedInterfaceId: selectedInterfaceId,
                    );
                    if (picked != null) {
                      onChanged(picked.interfaceModel);
                    }
                  },
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
              child: Row(
                children: <Widget>[
                  Icon(
                    Icons.settings_ethernet,
                    size: 17,
                    color: selected != null
                        ? scheme.primary
                        : scheme.onSurfaceVariant,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: selected != null
                        ? Row(
                            children: <Widget>[
                              Container(
                                width: 7,
                                height: 7,
                                decoration: BoxDecoration(
                                  color: statusColors!.foreground,
                                  shape: BoxShape.circle,
                                ),
                              ),
                              const SizedBox(width: 6),
                              Flexible(
                                child: Text(
                                  selected.displayName,
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: context.textTheme.bodySmall?.copyWith(
                                    fontFamily: 'monospace',
                                    fontWeight: FontWeight.w700,
                                  ),
                                ),
                              ),
                              if (selected.subtitleText != null) ...<Widget>[
                                const SizedBox(width: 6),
                                Flexible(
                                  child: Text(
                                    '· ${selected.subtitleText!}',
                                    maxLines: 1,
                                    overflow: TextOverflow.ellipsis,
                                    style: context.textTheme.labelSmall
                                        ?.copyWith(
                                          color: scheme.onSurfaceVariant,
                                        ),
                                  ),
                                ),
                              ],
                            ],
                          )
                        : Text(
                            fallbackInterfaceName != null
                                ? '$label: $fallbackInterfaceName'
                                : '$label: Automática / Sem porta específica',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: context.textTheme.bodySmall?.copyWith(
                              color: scheme.onSurfaceVariant,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                  ),
                  const SizedBox(width: 6),
                  if (speedLabel != null)
                    SemanticChip(
                      label: speedLabel,
                      tone: selected!.isUp
                          ? SemanticTone.info
                          : SemanticTone.secondary,
                      compact: true,
                    )
                  else if (list.isNotEmpty)
                    SemanticChip(
                      label: '$upCount/${list.length} UP',
                      tone: upCount > 0
                          ? SemanticTone.success
                          : SemanticTone.secondary,
                      compact: true,
                    ),
                  if (selected != null && enabled) ...<Widget>[
                    const SizedBox(width: 4),
                    InkResponse(
                      radius: 14,
                      onTap: () => onChanged(null),
                      child: Icon(
                        Icons.close,
                        size: 16,
                        color: scheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ],
              ),
            ),
          ),
        );
      },
    );
  }
}

@immutable
class _InterfacePickResult {
  const _InterfacePickResult(this.interfaceModel);
  final DeviceInterfaceModel? interfaceModel;
}

Future<_InterfacePickResult?> _showTopologyInterfacePickerSheet({
  required BuildContext context,
  required String title,
  required List<DeviceInterfaceModel> interfaces,
  int? selectedInterfaceId,
}) {
  return showAdaptiveFormSheet<_InterfacePickResult>(
    context: context,
    title: title,
    subtitle: 'Selecione a interface física/lógica ou mantenha automática',
    maxWidth: 540,
    builder: (ctx) => _TopologyInterfacePickerBody(
      interfaces: interfaces,
      selectedInterfaceId: selectedInterfaceId,
      onPicked: (res) => Navigator.of(ctx).pop(res),
    ),
  );
}

class _TopologyInterfacePickerBody extends StatefulWidget {
  const _TopologyInterfacePickerBody({
    required this.interfaces,
    this.selectedInterfaceId,
    required this.onPicked,
  });

  final List<DeviceInterfaceModel> interfaces;
  final int? selectedInterfaceId;
  final ValueChanged<_InterfacePickResult> onPicked;

  @override
  State<_TopologyInterfacePickerBody> createState() =>
      _TopologyInterfacePickerBodyState();
}

class _TopologyInterfacePickerBodyState
    extends State<_TopologyInterfacePickerBody> {
  final TextEditingController _ctrl = TextEditingController();

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final query = _ctrl.text.trim().toLowerCase();
    final filtered = query.isEmpty
        ? widget.interfaces
        : widget.interfaces
              .where((iface) {
                final speed = iface.negotiatedSpeedBps > 0
                    ? formatLinkSpeed(iface.negotiatedSpeedBps)
                    : '';
                final hay =
                    '${iface.displayName} ${iface.subtitleText ?? ''} '
                            '$speed ${iface.isUp ? 'up' : 'down'}'
                        .toLowerCase();
                return hay.contains(query);
              })
              .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          key: const ValueKey<String>('topology-iface-search-input'),
          controller: _ctrl,
          autofocus: true,
          decoration: InputDecoration(
            hintText: 'Filtrar porta (ex.: sfp, ether1, wan0, 10 Gbps)...',
            prefixIcon: const Icon(Icons.search),
            suffixIcon: query.isNotEmpty
                ? IconButton(
                    tooltip: 'Limpar filtro',
                    icon: const Icon(Icons.close),
                    onPressed: () => setState(_ctrl.clear),
                  )
                : null,
          ),
          onChanged: (_) => setState(() {}),
        ),
        const SizedBox(height: 10),
        Material(
          color: widget.selectedInterfaceId == null
              ? scheme.primaryContainer.withValues(alpha: 0.3)
              : scheme.surfaceContainerLow,
          shape: RoundedRectangleBorder(
            borderRadius: AppTheme.borderRadius,
            side: BorderSide(
              color: widget.selectedInterfaceId == null
                  ? scheme.primary
                  : scheme.outlineVariant,
            ),
          ),
          clipBehavior: Clip.antiAlias,
          child: InkWell(
            onTap: () => widget.onPicked(const _InterfacePickResult(null)),
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
              child: Row(
                children: <Widget>[
                  Icon(
                    Icons.hdr_auto_outlined,
                    size: 19,
                    color: scheme.primary,
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      'Automática / Sem porta específica',
                      style: context.textTheme.bodyMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
        const SizedBox(height: 8),
        for (final iface in filtered) ...<Widget>[
          _InterfaceOptionCard(
            iface: iface,
            selected: iface.id == widget.selectedInterfaceId,
            onTap: () => widget.onPicked(_InterfacePickResult(iface)),
          ),
          const SizedBox(height: 6),
        ],
      ],
    );
  }
}

class _InterfaceOptionCard extends StatelessWidget {
  const _InterfaceOptionCard({
    required this.iface,
    required this.selected,
    required this.onTap,
  });

  final DeviceInterfaceModel iface;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final statusColors = context.semanticColors.resolve(
      iface.isUp ? SemanticTone.success : SemanticTone.error,
      scheme,
    );
    final speedLabel = iface.negotiatedSpeedBps > 0
        ? formatLinkSpeed(iface.negotiatedSpeedBps)
        : (iface.isUp ? '—' : 'Sem link');
    final subtitle = iface.subtitleText;

    return Material(
      color: selected
          ? scheme.primaryContainer.withValues(alpha: 0.3)
          : scheme.surfaceContainerLow,
      shape: RoundedRectangleBorder(
        borderRadius: AppTheme.borderRadius,
        side: BorderSide(
          color: selected ? scheme.primary : scheme.outlineVariant,
        ),
      ),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 9),
          child: Row(
            children: <Widget>[
              Container(
                width: 8,
                height: 8,
                decoration: BoxDecoration(
                  color: statusColors.foreground,
                  shape: BoxShape.circle,
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      iface.displayName,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.bodyMedium?.copyWith(
                        fontFamily: 'monospace',
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    if (subtitle != null) ...<Widget>[
                      const SizedBox(height: 1),
                      Text(
                        subtitle,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.labelSmall?.copyWith(
                          color: scheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              const SizedBox(width: 8),
              SemanticChip(
                label: iface.isUp ? 'UP' : 'DOWN',
                tone: iface.isUp ? SemanticTone.success : SemanticTone.error,
                compact: true,
              ),
              const SizedBox(width: 6),
              SemanticChip(
                label: speedLabel,
                tone: iface.isUp && iface.negotiatedSpeedBps > 0
                    ? SemanticTone.info
                    : SemanticTone.secondary,
                compact: true,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

// ── 4. Unmanaged Switch Sheet ───────────────────────────────────────────────

/// Abre a folha para cadastrar um Switch Não Gerenciável na topologia.
Future<void> showUnmanagedSwitchSheet({
  required BuildContext context,
  required List<SiteModel> sites,
  int? initialSiteId,
  required Future<bool> Function(Map<String, dynamic> payload) onSubmit,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Novo Switch Não Gerenciável',
    subtitle: 'Adicione um switch/hub sem SNMP para mapear derivações físicas',
    builder: (ctx) => _UnmanagedSwitchSheetBody(
      sites: sites,
      initialSiteId: initialSiteId,
      onSubmit: (payload) async {
        final ok = await onSubmit(payload);
        if (ctx.mounted && ok) {
          Navigator.of(ctx).pop();
        }
        return ok;
      },
    ),
  );
}

class _UnmanagedSwitchSheetBody extends StatefulWidget {
  const _UnmanagedSwitchSheetBody({
    required this.sites,
    this.initialSiteId,
    required this.onSubmit,
  });

  final List<SiteModel> sites;
  final int? initialSiteId;
  final Future<bool> Function(Map<String, dynamic> payload) onSubmit;

  @override
  State<_UnmanagedSwitchSheetBody> createState() =>
      _UnmanagedSwitchSheetBodyState();
}

class _UnmanagedSwitchSheetBodyState extends State<_UnmanagedSwitchSheetBody> {
  final TextEditingController _nameCtrl = TextEditingController();
  final TextEditingController _vendorCtrl = TextEditingController();
  final TextEditingController _portsCtrl = TextEditingController(text: '8');
  int? _selectedSiteId;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    _selectedSiteId = widget.sites.any((s) => s.id == widget.initialSiteId)
        ? widget.initialSiteId
        : null;
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _vendorCtrl.dispose();
    _portsCtrl.dispose();
    super.dispose();
  }

  Future<void> _handleSubmit() async {
    if (_nameCtrl.text.trim().isEmpty || _saving) return;
    setState(() => _saving = true);
    final ok = await widget.onSubmit(<String, dynamic>{
      'name': _nameCtrl.text.trim(),
      if (_vendorCtrl.text.trim().isNotEmpty) 'vendor': _vendorCtrl.text.trim(),
      'portCount': int.tryParse(_portsCtrl.text.trim()) ?? 8,
      'siteId': _selectedSiteId,
    });
    if (mounted && !ok) {
      setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          controller: _nameCtrl,
          decoration: const InputDecoration(labelText: 'Nome do switch *'),
        ),
        const SizedBox(height: 12),
        ResponsiveFieldRow(
          breakpoint: 480,
          children: <Widget>[
            TextField(
              controller: _vendorCtrl,
              decoration: const InputDecoration(labelText: 'Fabricante'),
            ),
            TextField(
              controller: _portsCtrl,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Portas'),
            ),
          ],
        ),
        if (widget.sites.isNotEmpty) ...<Widget>[
          const SizedBox(height: 12),
          AppDropdownFormField<int?>(
            initialValue: _selectedSiteId,
            decoration: const InputDecoration(labelText: 'Site'),
            items: <DropdownMenuItem<int?>>[
              const DropdownMenuItem<int?>(
                value: null,
                child: Text('Nenhum', overflow: TextOverflow.ellipsis),
              ),
              for (final s in widget.sites)
                DropdownMenuItem<int?>(
                  value: s.id,
                  child: Text(s.name, overflow: TextOverflow.ellipsis),
                ),
            ],
            onChanged: (v) => setState(() => _selectedSiteId = v),
          ),
        ],
        const SizedBox(height: 16),
        SymmetricalActionBar(
          children: <Widget>[
            OutlinedButton(
              onPressed: _saving ? null : () => Navigator.of(context).pop(),
              child: const Text('Cancelar'),
            ),
            FilledButton.icon(
              onPressed: _saving ? null : _handleSubmit,
              icon: _saving
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.hub_outlined),
              label: const Text(
                'Adicionar switch',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
          ],
        ),
      ],
    );
  }
}

// ── 5. Path Target Picker Sheet ─────────────────────────────────────────────

/// Folha rápida para escolher o equipamento de destino ao traçar caminho.
Future<void> showTopologyPathTargetSheet({
  required BuildContext context,
  required TopologyNodeModel sourceNode,
  required List<TopologyNodeModel> nodes,
  required ValueChanged<TopologyNodeModel> onSelectTarget,
  required VoidCallback onPickOnCanvas,
}) {
  final candidates = nodes
      .where((n) => n.id != sourceNode.id)
      .toList(growable: false);

  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Traçar Caminho',
    subtitle: 'Origem: ${sourceNode.name} — selecione o destino',
    maxWidth: 520,
    builder: (ctx) => _PathTargetPickerBody(
      candidates: candidates,
      onSelectTarget: (target) {
        Navigator.of(ctx).pop();
        onSelectTarget(target);
      },
      onPickOnCanvas: () {
        Navigator.of(ctx).pop();
        onPickOnCanvas();
      },
    ),
  );
}

class _PathTargetPickerBody extends StatefulWidget {
  const _PathTargetPickerBody({
    required this.candidates,
    required this.onSelectTarget,
    required this.onPickOnCanvas,
  });

  final List<TopologyNodeModel> candidates;
  final ValueChanged<TopologyNodeModel> onSelectTarget;
  final VoidCallback onPickOnCanvas;

  @override
  State<_PathTargetPickerBody> createState() => _PathTargetPickerBodyState();
}

class _PathTargetPickerBodyState extends State<_PathTargetPickerBody> {
  final TextEditingController _ctrl = TextEditingController();

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final query = _ctrl.text.trim();
    final hits = query.isEmpty
        ? widget.candidates
              .take(20)
              .map(
                (n) => TopologySearchHit(
                  node: n,
                  score: 1,
                  matchedField: TopologySearchField.name,
                ),
              )
              .toList(growable: false)
        : searchTopologyNodes(widget.candidates, query, limit: 20);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        OutlinedButton.icon(
          onPressed: widget.onPickOnCanvas,
          icon: const Icon(Icons.touch_app_outlined, size: 18),
          label: const Text('Tocar no destino diretamente no mapa'),
        ),
        const SizedBox(height: 10),
        TextField(
          controller: _ctrl,
          decoration: const InputDecoration(
            hintText: 'Filtrar equipamento de destino...',
            prefixIcon: Icon(Icons.search),
          ),
          onChanged: (_) => setState(() {}),
        ),
        const SizedBox(height: 10),
        for (final hit in hits) ...<Widget>[
          _SearchHitTile(
            hit: hit,
            showMatchBadge: false,
            onTap: () => widget.onSelectTarget(hit.node),
          ),
          const SizedBox(height: 6),
        ],
      ],
    );
  }
}

// ── 6. Interactive Legend Sheet ─────────────────────────────────────────────

/// Abre a legenda interativa da topologia (cores, enlaces, ícones e gestos).
Future<void> showTopologyLegendSheet(BuildContext context) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Legenda da Topologia',
    subtitle: 'Status, utilização de enlaces, tipos de mídia e gestos',
    maxWidth: 600,
    builder: (ctx) => const _TopologyLegendBody(),
  );
}

class _TopologyLegendBody extends StatelessWidget {
  const _TopologyLegendBody();

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        _LegendSectionTitle('Status dos Equipamentos'),
        const SizedBox(height: 8),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            for (final entry in kTopologyNodeStatusLegend)
              SemanticChip(
                label: entry.label,
                tone: entry.tone,
                icon: entry.icon,
              ),
          ],
        ),
        const Divider(height: 24),
        _LegendSectionTitle('Utilização e Saúde dos Enlaces'),
        const SizedBox(height: 8),
        for (final entry in kTopologyUtilizationLegend) ...<Widget>[
          _LegendLineRow(entry: entry),
          const SizedBox(height: 6),
        ],
        const Divider(height: 24),
        _LegendSectionTitle('Espessura por Velocidade Nominal'),
        const SizedBox(height: 8),
        for (final entry in kTopologySpeedTierLegend) ...<Widget>[
          _LegendLineRow(entry: entry),
          const SizedBox(height: 6),
        ],
        const Divider(height: 24),
        _LegendSectionTitle('Tipos de Conexão'),
        const SizedBox(height: 8),
        for (final entry in kTopologyLinkKindLegend) ...<Widget>[
          _LegendLineRow(entry: entry),
          const SizedBox(height: 6),
        ],
        const Divider(height: 24),
        _LegendSectionTitle('Gestos e Atalhos'),
        const SizedBox(height: 8),
        NestedPanel(
          padding: const EdgeInsets.all(12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              _TipRow(
                icon: Icons.pinch_outlined,
                text:
                    'Pinça ou roda do mouse para dar zoom; arraste o fundo para mover o mapa.',
                color: scheme.primary,
              ),
              const SizedBox(height: 6),
              _TipRow(
                icon: Icons.touch_app_outlined,
                text:
                    'Toque em um nó ou linha de enlace para abrir o inspetor detalhado.',
                color: scheme.primary,
              ),
              const SizedBox(height: 6),
              _TipRow(
                icon: Icons.lock_outline,
                text:
                    'Destrave o cadeado (canto inferior direito) para reposicionar nós sem acidentes.',
                color: scheme.primary,
              ),
              const SizedBox(height: 6),
              _TipRow(
                icon: Icons.keyboard_outlined,
                text:
                    'Teclado (desktop): + / - (zoom), 0 (ajustar à tela), Delete (remover enlace selecionado), Ctrl+Z (desfazer), Esc (limpar seleção).',
                color: scheme.primary,
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _LegendSectionTitle extends StatelessWidget {
  const _LegendSectionTitle(this.text);

  final String text;

  @override
  Widget build(BuildContext context) {
    return Text(
      text,
      style: context.textTheme.titleSmall?.copyWith(
        fontWeight: FontWeight.w800,
      ),
    );
  }
}

class _LegendLineRow extends StatelessWidget {
  const _LegendLineRow({required this.entry});

  final TopologyLegendEntry entry;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(entry.tone, scheme);
    final stroke = entry.strokeWidth ?? 2.6;

    return Row(
      children: <Widget>[
        SizedBox(
          width: 44,
          height: 16,
          child: CustomPaint(
            painter: _LegendLineSamplePainter(
              color: colors.foreground,
              lineStyle: entry.lineStyle ?? TopologyLineStyle.solid,
              strokeWidth: stroke,
            ),
          ),
        ),
        const SizedBox(width: 10),
        if (entry.icon != null) ...<Widget>[
          Icon(entry.icon, size: 16, color: colors.foreground),
          const SizedBox(width: 6),
        ],
        Expanded(
          child: Text(
            entry.description != null
                ? '${entry.label} · ${entry.description}'
                : entry.label,
            style: context.textTheme.bodySmall?.copyWith(
              fontWeight: FontWeight.w600,
            ),
          ),
        ),
      ],
    );
  }
}

class _LegendLineSamplePainter extends CustomPainter {
  const _LegendLineSamplePainter({
    required this.color,
    required this.lineStyle,
    required this.strokeWidth,
  });

  final Color color;
  final TopologyLineStyle lineStyle;
  final double strokeWidth;

  @override
  void paint(Canvas canvas, Size size) {
    final y = size.height / 2;
    final paint = Paint()
      ..color = color
      ..strokeWidth = strokeWidth
      ..strokeCap = StrokeCap.round
      ..style = PaintingStyle.stroke;

    switch (lineStyle) {
      case TopologyLineStyle.solid:
        canvas.drawLine(Offset(0, y), Offset(size.width, y), paint);
      case TopologyLineStyle.dashed:
        const dash = 7.0;
        const gap = 5.0;
        var x = 0.0;
        while (x < size.width) {
          final end = (x + dash).clamp(0.0, size.width);
          canvas.drawLine(Offset(x, y), Offset(end, y), paint);
          x += dash + gap;
        }
      case TopologyLineStyle.dotted:
        const step = 6.0;
        var x = 2.0;
        while (x < size.width) {
          canvas.drawCircle(
            Offset(x, y),
            strokeWidth * 0.55,
            Paint()..color = color,
          );
          x += step;
        }
    }
  }

  @override
  bool shouldRepaint(covariant _LegendLineSamplePainter oldDelegate) =>
      oldDelegate.color != color ||
      oldDelegate.lineStyle != lineStyle ||
      oldDelegate.strokeWidth != strokeWidth;
}

class _TipRow extends StatelessWidget {
  const _TipRow({required this.icon, required this.text, required this.color});

  final IconData icon;
  final String text;
  final Color color;

  @override
  Widget build(BuildContext context) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Icon(icon, size: 16, color: color),
        const SizedBox(width: 8),
        Expanded(child: Text(text, style: context.textTheme.bodySmall)),
      ],
    );
  }
}
