import 'package:flutter/material.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

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
