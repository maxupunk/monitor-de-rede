import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/device_presentation.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

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
