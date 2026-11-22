import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/providers/topology_provider.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Painel lateral fixo (desktop >= 900px) para o Inspetor de Enlace.
class TopologyEdgeInspectorCard extends StatelessWidget {
  const TopologyEdgeInspectorCard({
    super.key,
    required this.edge,
    required this.allNodes,
    required this.canWrite,
    required this.onClose,
    required this.onFocusNode,
    required this.onStartLinkTest,
    required this.onConfirmLink,
    required this.onEditLink,
    required this.onDeleteLink,
  });

  final TopologyEdgeModel edge;
  final List<TopologyNodeModel> allNodes;
  final bool canWrite;
  final VoidCallback onClose;
  final ValueChanged<TopologyNodeModel> onFocusNode;
  final VoidCallback onStartLinkTest;
  final VoidCallback onConfirmLink;
  final VoidCallback onEditLink;
  final VoidCallback onDeleteLink;

  @override
  Widget build(BuildContext context) {
    final edgeStatus = classifyEdgeStatus(edge);
    return Card(
      elevation: 4,
      clipBehavior: Clip.antiAlias,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Padding(
            padding: const EdgeInsets.fromLTRB(14, 12, 8, 12),
            child: Row(
              children: <Widget>[
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Row(
                        children: <Widget>[
                          Text(
                            isVirtualTopologyEdge(edge)
                                ? 'Enlace de Hierarquia'
                                : 'Enlace #${edge.id}',
                            style: context.textTheme.titleMedium?.copyWith(
                              fontWeight: FontWeight.w800,
                            ),
                          ),
                          const SizedBox(width: 8),
                          SemanticChip(
                            label: edgeStatus.label,
                            tone: edgeStatus.tone,
                            compact: true,
                          ),
                        ],
                      ),
                      const SizedBox(height: 2),
                      Text(
                        formatDiscoveryMethodLabel(edge.discoveryMethod),
                        style: context.textTheme.bodySmall?.copyWith(
                          color: context.colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ),
                ),
                IconButton(
                  tooltip: 'Fechar inspetor',
                  visualDensity: VisualDensity.compact,
                  icon: const Icon(Icons.close),
                  onPressed: onClose,
                ),
              ],
            ),
          ),
          const Divider(height: 1),
          Expanded(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(14),
              child: TopologyEdgeInspectorBody(
                edge: edge,
                allNodes: allNodes,
                canWrite: canWrite,
                onFocusNode: onFocusNode,
                onStartLinkTest: onStartLinkTest,
                onConfirmLink: onConfirmLink,
                onEditLink: onEditLink,
                onDeleteLink: onDeleteLink,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// Abre o Inspetor de Enlace em bottom sheet adaptativo no mobile (< 900px).
Future<void> showTopologyEdgeInspectorSheet({
  required BuildContext context,
  required TopologyEdgeModel edge,
  required List<TopologyNodeModel> allNodes,
  required bool canWrite,
  required ValueChanged<TopologyNodeModel> onFocusNode,
  required VoidCallback onStartLinkTest,
  required VoidCallback onConfirmLink,
  required VoidCallback onEditLink,
  required VoidCallback onDeleteLink,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: isVirtualTopologyEdge(edge)
        ? 'Enlace de Hierarquia'
        : 'Enlace #${edge.id}',
    subtitle: formatDiscoveryMethodLabel(edge.discoveryMethod),
    builder: (ctx) => TopologyEdgeInspectorBody(
      edge: edge,
      allNodes: allNodes,
      canWrite: canWrite,
      onFocusNode: (node) {
        Navigator.of(ctx).pop();
        onFocusNode(node);
      },
      onStartLinkTest: () {
        Navigator.of(ctx).pop();
        onStartLinkTest();
      },
      onConfirmLink: () {
        Navigator.of(ctx).pop();
        onConfirmLink();
      },
      onEditLink: () {
        Navigator.of(ctx).pop();
        onEditLink();
      },
      onDeleteLink: () {
        Navigator.of(ctx).pop();
        onDeleteLink();
      },
    ),
  );
}

/// Corpo compartilhado do Inspetor de Enlace.
class TopologyEdgeInspectorBody extends ConsumerWidget {
  const TopologyEdgeInspectorBody({
    super.key,
    required this.edge,
    required this.allNodes,
    required this.canWrite,
    required this.onFocusNode,
    required this.onStartLinkTest,
    required this.onConfirmLink,
    required this.onEditLink,
    required this.onDeleteLink,
  });

  final TopologyEdgeModel edge;
  final List<TopologyNodeModel> allNodes;
  final bool canWrite;
  final ValueChanged<TopologyNodeModel> onFocusNode;
  final VoidCallback onStartLinkTest;
  final VoidCallback onConfirmLink;
  final VoidCallback onEditLink;
  final VoidCallback onDeleteLink;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scheme = context.colorScheme;
    final topoState = ref.watch(topologyProvider);
    final liveNodes = topoState.loaded && topoState.nodes.isNotEmpty
        ? topoState.nodes
        : allNodes;
    final liveEdge = topoState.loaded
        ? (topoState.edges.where((e) => e.id == edge.id).firstOrNull ?? edge)
        : edge;
    final byId = <int, TopologyNodeModel>{for (final n in liveNodes) n.id: n};
    final sourceNode = byId[liveEdge.sourceDeviceId];
    final targetNode = byId[liveEdge.targetDeviceId];
    final linkKind = classifyLinkKind(liveEdge.linkType);
    final util = resolveEdgeUtilization(liveEdge);
    final utilTone = utilizationTone(util);
    final utilColors = context.semanticColors.resolve(utilTone, scheme);
    final speedLabel = formatEdgeSpeedLabel(liveEdge);
    final trafficLabel = formatEdgeTrafficLabel(liveEdge);
    final confidencePct = edgeConfidencePercent(liveEdge.confidence);
    final isVirtual = isVirtualTopologyEdge(liveEdge);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        NestedPanel(
          padding: const EdgeInsets.all(10),
          child: Row(
            children: <Widget>[
              Expanded(
                child: _EndpointChipButton(
                  label:
                      sourceNode?.name ??
                      liveEdge.sourceDeviceName ??
                      '#${liveEdge.sourceDeviceId}',
                  subtitle: liveEdge.sourceInterfaceName,
                  icon: sourceNode != null
                      ? topologyNodeIcon(sourceNode)
                      : Icons.devices_other_outlined,
                  onTap: sourceNode != null
                      ? () => onFocusNode(sourceNode)
                      : null,
                ),
              ),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 6),
                child: Icon(
                  Icons.swap_horiz,
                  size: 20,
                  color: scheme.onSurfaceVariant,
                ),
              ),
              Expanded(
                child: _EndpointChipButton(
                  label:
                      targetNode?.name ??
                      liveEdge.targetDeviceName ??
                      '#${liveEdge.targetDeviceId}',
                  subtitle: liveEdge.targetInterfaceName,
                  icon: targetNode != null
                      ? topologyNodeIcon(targetNode)
                      : Icons.devices_other_outlined,
                  onTap: targetNode != null
                      ? () => onFocusNode(targetNode)
                      : null,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 12),
        if (util != null) ...<Widget>[
          NestedPanel(
            padding: const EdgeInsets.all(12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Text(
                      'Utilização do Enlace',
                      style: context.textTheme.labelMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    const Spacer(),
                    SemanticChip(
                      label: formatPercent(util * 100),
                      tone: utilTone,
                      compact: true,
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                ClipRRect(
                  borderRadius: BorderRadius.circular(6),
                  child: LinearProgressIndicator(
                    value: util.clamp(0.0, 1.0),
                    minHeight: 8,
                    backgroundColor: scheme.surfaceContainerHighest,
                    color: utilColors.foreground,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
        ],
        NestedPanel(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: withDividers(<Widget>[
              KeyValueRow(
                label: 'Meio / Tipo',
                value: linkKind.label,
                trailing: SemanticChip(
                  label: liveEdge.confirmed ? 'Confirmado' : 'Automático',
                  tone: liveEdge.confirmed
                      ? SemanticTone.success
                      : SemanticTone.info,
                  compact: true,
                ),
              ),
              KeyValueRow(label: 'Velocidade', value: speedLabel),
              if (liveEdge.inBps != null ||
                  liveEdge.outBps != null) ...<Widget>[
                KeyValueRow(
                  label: 'Download (RX)',
                  value: formatBps(liveEdge.inBps ?? 0),
                ),
                KeyValueRow(
                  label: 'Upload (TX)',
                  value: formatBps(liveEdge.outBps ?? 0),
                ),
              ] else if (trafficLabel != null)
                KeyValueRow(label: 'Tráfego ao vivo', value: trafficLabel),
              if (liveEdge.sourceInterfaceName != null)
                KeyValueRow(
                  label: 'Porta origem',
                  value: liveEdge.sourceInterfaceName!,
                  monospace: true,
                ),
              if (liveEdge.targetInterfaceName != null)
                KeyValueRow(
                  label: 'Porta destino',
                  value: liveEdge.targetInterfaceName!,
                  monospace: true,
                ),
              if (confidencePct != null && !isVirtual)
                KeyValueRow(
                  label: 'Confiança',
                  value: formatPercent(confidencePct, digits: 0),
                ),
              if (liveEdge.lastSeenAt != null &&
                  liveEdge.lastSeenAt!.isNotEmpty)
                KeyValueRow(
                  label: 'Última leitura',
                  value: formatRelativeTime(liveEdge.lastSeenAt),
                ),
            ]),
          ),
        ),
        const SizedBox(height: 14),
        SymmetricalActionBar(
          children: <Widget>[
            if (!isVirtual)
              FilledButton.tonalIcon(
                onPressed: onStartLinkTest,
                icon: const Icon(Icons.speed, size: 17),
                label: const Text(
                  'Testar enlace',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            if (canWrite && canConfirmTopologyEdge(liveEdge))
              OutlinedButton.icon(
                onPressed: onConfirmLink,
                icon: const Icon(Icons.verified_outlined, size: 17),
                label: const Text(
                  'Confirmar enlace',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            if (canWrite && !isVirtual)
              OutlinedButton.icon(
                onPressed: onEditLink,
                icon: const Icon(Icons.edit_outlined, size: 17),
                label: const Text(
                  'Editar',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            if (canWrite && !isVirtual)
              OutlinedButton.icon(
                style: OutlinedButton.styleFrom(
                  foregroundColor: scheme.error,
                  side: BorderSide(color: scheme.error),
                ),
                onPressed: onDeleteLink,
                icon: Icon(Icons.delete_outline, color: scheme.error, size: 18),
                label: Text(
                  'Remover link',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(color: scheme.error),
                ),
              ),
          ],
        ),
      ],
    );
  }
}

class _EndpointChipButton extends StatelessWidget {
  const _EndpointChipButton({
    required this.label,
    required this.icon,
    this.subtitle,
    this.onTap,
  });

  final String label;
  final String? subtitle;
  final IconData icon;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    return Material(
      color: scheme.surface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(10),
        side: BorderSide(color: scheme.outlineVariant),
      ),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 8),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Row(
                children: <Widget>[
                  Icon(icon, size: 16, color: scheme.primary),
                  const SizedBox(width: 5),
                  Expanded(
                    child: Text(
                      label,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.labelMedium?.copyWith(
                        fontWeight: FontWeight.w800,
                      ),
                    ),
                  ),
                ],
              ),
              if (subtitle != null && subtitle!.isNotEmpty) ...<Widget>[
                const SizedBox(height: 2),
                Text(
                  subtitle!,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: context.textTheme.labelSmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                    fontFamily: 'monospace',
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}
