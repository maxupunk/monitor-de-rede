import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/features/network/topology/widgets/topology_helpers.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

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
