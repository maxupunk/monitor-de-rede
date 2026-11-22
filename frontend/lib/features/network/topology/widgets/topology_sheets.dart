import 'package:flutter/material.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
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

/// Abre a folha para criar ou editar um enlace manual da topologia.
Future<void> showTopologyLinkFormSheet({
  required BuildContext context,
  required List<TopologyNodeModel> nodes,
  int? initialSourceId,
  TopologyEdgeModel? existingEdge,
  required Future<bool> Function(Map<String, dynamic> payload) onSubmit,
}) {
  final isEditing = existingEdge != null;
  int? srcId =
      existingEdge?.sourceDeviceId ?? initialSourceId ?? nodes.firstOrNull?.id;
  int? tgtId =
      existingEdge?.targetDeviceId ??
      nodes.where((n) => n.id != srcId).firstOrNull?.id;
  String linkType = existingEdge?.linkType ?? 'ethernet';
  const validTypes = <String>{'ethernet', 'fiber', 'wireless', 'vpn'};
  if (!validTypes.contains(linkType)) linkType = 'ethernet';
  var saving = false;

  return showAdaptiveFormSheet<void>(
    context: context,
    title: isEditing ? 'Editar Enlace' : 'Adicionar Enlace Manual',
    subtitle: isEditing
        ? '${existingEdge.sourceDeviceName ?? '#${existingEdge.sourceDeviceId}'} ↔ ${existingEdge.targetDeviceName ?? '#${existingEdge.targetDeviceId}'}'
        : 'Conecte dois equipamentos manualmente no mapa',
    builder: (ctx) => StatefulBuilder(
      builder: (context, setModal) {
        final canSave =
            !saving &&
            (isEditing || (srcId != null && tgtId != null && srcId != tgtId));
        return Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            if (!isEditing) ...<Widget>[
              ResponsiveFieldRow(
                breakpoint: 480,
                children: <Widget>[
                  AppDropdownFormField<int?>(
                    initialValue: nodes.any((n) => n.id == srcId)
                        ? srcId
                        : null,
                    decoration: const InputDecoration(
                      labelText: 'Equipamento A',
                    ),
                    items: nodes
                        .map(
                          (n) => DropdownMenuItem<int?>(
                            value: n.id,
                            child: Text(
                              n.name,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                        )
                        .toList(growable: false),
                    onChanged: (v) => setModal(() => srcId = v),
                  ),
                  AppDropdownFormField<int?>(
                    initialValue: nodes.any((n) => n.id == tgtId)
                        ? tgtId
                        : null,
                    decoration: const InputDecoration(
                      labelText: 'Equipamento B',
                    ),
                    items: nodes
                        .map(
                          (n) => DropdownMenuItem<int?>(
                            value: n.id,
                            child: Text(
                              n.name,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                        )
                        .toList(growable: false),
                    onChanged: (v) => setModal(() => tgtId = v),
                  ),
                ],
              ),
              const SizedBox(height: 12),
            ],
            AppDropdownFormField<String>(
              initialValue: linkType,
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
                  child: Text(
                    'Wireless / Rádio',
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                DropdownMenuItem(
                  value: 'vpn',
                  child: Text('Túnel VPN', overflow: TextOverflow.ellipsis),
                ),
              ],
              onChanged: (v) => setModal(() => linkType = v ?? 'ethernet'),
            ),
            const SizedBox(height: 16),
            Align(
              alignment: Alignment.centerRight,
              child: FilledButton.icon(
                onPressed: !canSave
                    ? null
                    : () async {
                        setModal(() => saving = true);
                        final payload = isEditing
                            ? <String, dynamic>{
                                'sourceInterfaceId':
                                    existingEdge.sourceInterfaceId,
                                'targetInterfaceId':
                                    existingEdge.targetInterfaceId,
                                'linkType': linkType,
                              }
                            : <String, dynamic>{
                                'sourceDeviceId': srcId,
                                'targetDeviceId': tgtId,
                                'linkType': linkType,
                              };
                        final ok = await onSubmit(payload);
                        if (ctx.mounted) {
                          if (ok) {
                            Navigator.of(ctx).pop();
                          } else {
                            setModal(() => saving = false);
                          }
                        }
                      },
                icon: saving
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : Icon(isEditing ? Icons.save_outlined : Icons.add_link),
                label: Text(isEditing ? 'Salvar enlace' : 'Criar enlace'),
              ),
            ),
          ],
        );
      },
    ),
  );
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
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton.icon(
            onPressed: _saving ? null : _handleSubmit,
            icon: _saving
                ? const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.hub_outlined),
            label: const Text('Adicionar switch'),
          ),
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
                    'Teclado (desktop): + / - (zoom), 0 (ajustar à tela), Esc (limpar seleção).',
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
