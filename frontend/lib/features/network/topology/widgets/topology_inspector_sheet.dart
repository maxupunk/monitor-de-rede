import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_proactive_widgets.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Painel lateral fixo (desktop >= 900px) que alterna entre Inspetor de Nó
/// e Inspetor de Enlace conforme a seleção ativa.
class TopologyDockedInspector extends StatelessWidget {
  const TopologyDockedInspector({
    super.key,
    this.selectedNode,
    this.selectedEdge,
    required this.allNodes,
    required this.allEdges,
    required this.pluginActions,
    required this.canWrite,
    required this.pingLoading,
    this.pingReport,
    this.pingError,
    required this.onClose,
    required this.onQuickPing,
    required this.onDismissPing,
    required this.onOpenNodeDetails,
    required this.onTracePathFromNode,
    required this.onTraceUpstream,
    required this.onTraceroute,
    required this.onStartNodeLinkTest,
    required this.onNewLinkFromNode,
    required this.onEditDevice,
    required this.onOpenPluginsTab,
    required this.onFocusNode,
    required this.onInspectEdge,
    required this.onStartEdgeLinkTest,
    required this.onConfirmEdge,
    required this.onEditEdge,
    required this.onDeleteEdge,
  });

  final TopologyNodeModel? selectedNode;
  final TopologyEdgeModel? selectedEdge;
  final List<TopologyNodeModel> allNodes;
  final List<TopologyEdgeModel> allEdges;
  final List<TopologyPluginActionModel> pluginActions;
  final bool canWrite;
  final bool pingLoading;
  final QuickPingReport? pingReport;
  final String? pingError;

  final VoidCallback onClose;
  final ValueChanged<TopologyNodeModel> onQuickPing;
  final VoidCallback onDismissPing;
  final ValueChanged<TopologyNodeModel> onOpenNodeDetails;
  final ValueChanged<TopologyNodeModel> onTracePathFromNode;
  final ValueChanged<TopologyNodeModel> onTraceUpstream;
  final ValueChanged<TopologyNodeModel> onTraceroute;
  final ValueChanged<TopologyNodeModel> onStartNodeLinkTest;
  final ValueChanged<TopologyNodeModel> onNewLinkFromNode;
  final ValueChanged<TopologyNodeModel> onEditDevice;
  final ValueChanged<TopologyNodeModel> onOpenPluginsTab;
  final ValueChanged<TopologyNodeModel> onFocusNode;
  final ValueChanged<TopologyEdgeModel> onInspectEdge;
  final ValueChanged<TopologyEdgeModel> onStartEdgeLinkTest;
  final ValueChanged<TopologyEdgeModel> onConfirmEdge;
  final ValueChanged<TopologyEdgeModel> onEditEdge;
  final ValueChanged<TopologyEdgeModel> onDeleteEdge;

  @override
  Widget build(BuildContext context) {
    if (selectedNode != null) {
      final node = selectedNode!;
      return TopologyNodeInspectorCard(
        node: node,
        allNodes: allNodes,
        allEdges: allEdges,
        pluginActions: pluginActions,
        canWrite: canWrite,
        pingLoading: pingLoading,
        pingReport: pingReport,
        pingError: pingError,
        onClose: onClose,
        onQuickPing: () => onQuickPing(node),
        onDismissPing: onDismissPing,
        onOpenDetails: () => onOpenNodeDetails(node),
        onTracePathFromNode: () => onTracePathFromNode(node),
        onTraceUpstream: () => onTraceUpstream(node),
        onTraceroute: () => onTraceroute(node),
        onStartLinkTest: () => onStartNodeLinkTest(node),
        onNewLinkFromNode: () => onNewLinkFromNode(node),
        onEditDevice: () => onEditDevice(node),
        onOpenPluginsTab: () => onOpenPluginsTab(node),
        onFocusNeighbor: onFocusNode,
        onInspectEdge: onInspectEdge,
      );
    }
    if (selectedEdge != null) {
      final edge = selectedEdge!;
      return TopologyEdgeInspectorCard(
        edge: edge,
        allNodes: allNodes,
        canWrite: canWrite,
        onClose: onClose,
        onFocusNode: onFocusNode,
        onStartLinkTest: () => onStartEdgeLinkTest(edge),
        onConfirmLink: () => onConfirmEdge(edge),
        onEditLink: () => onEditEdge(edge),
        onDeleteLink: () => onDeleteEdge(edge),
      );
    }
    return const SizedBox.shrink();
  }
}

/// Painel lateral fixo (desktop >= 900px) para o Inspetor de Nó.
class TopologyNodeInspectorCard extends StatelessWidget {
  const TopologyNodeInspectorCard({
    super.key,
    required this.node,
    required this.allNodes,
    required this.allEdges,
    required this.pluginActions,
    required this.canWrite,
    required this.pingLoading,
    this.pingReport,
    this.pingError,
    required this.onClose,
    required this.onQuickPing,
    required this.onDismissPing,
    required this.onOpenDetails,
    required this.onTracePathFromNode,
    required this.onTraceUpstream,
    required this.onTraceroute,
    required this.onStartLinkTest,
    required this.onNewLinkFromNode,
    required this.onEditDevice,
    required this.onOpenPluginsTab,
    required this.onFocusNeighbor,
    required this.onInspectEdge,
  });

  final TopologyNodeModel node;
  final List<TopologyNodeModel> allNodes;
  final List<TopologyEdgeModel> allEdges;
  final List<TopologyPluginActionModel> pluginActions;
  final bool canWrite;
  final bool pingLoading;
  final QuickPingReport? pingReport;
  final String? pingError;

  final VoidCallback onClose;
  final VoidCallback onQuickPing;
  final VoidCallback onDismissPing;
  final VoidCallback onOpenDetails;
  final VoidCallback onTracePathFromNode;
  final VoidCallback onTraceUpstream;
  final VoidCallback onTraceroute;
  final VoidCallback onStartLinkTest;
  final VoidCallback onNewLinkFromNode;
  final VoidCallback onEditDevice;
  final VoidCallback onOpenPluginsTab;
  final ValueChanged<TopologyNodeModel> onFocusNeighbor;
  final ValueChanged<TopologyEdgeModel> onInspectEdge;

  @override
  Widget build(BuildContext context) {
    return Card(
      elevation: 4,
      clipBehavior: Clip.antiAlias,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          _NodeInspectorHeader(node: node, onClose: onClose),
          const Divider(height: 1),
          Expanded(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(14),
              child: TopologyNodeInspectorBody(
                node: node,
                allNodes: allNodes,
                allEdges: allEdges,
                pluginActions: pluginActions,
                canWrite: canWrite,
                pingLoading: pingLoading,
                pingReport: pingReport,
                pingError: pingError,
                onQuickPing: onQuickPing,
                onDismissPing: onDismissPing,
                onOpenDetails: onOpenDetails,
                onTracePathFromNode: onTracePathFromNode,
                onTraceUpstream: onTraceUpstream,
                onTraceroute: onTraceroute,
                onStartLinkTest: onStartLinkTest,
                onNewLinkFromNode: onNewLinkFromNode,
                onEditDevice: onEditDevice,
                onOpenPluginsTab: onOpenPluginsTab,
                onFocusNeighbor: onFocusNeighbor,
                onInspectEdge: onInspectEdge,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

/// Abre o Inspetor de Nó como Draggable Bottom Sheet no mobile (< 900px)
/// permitindo espiar o grafo acima.
Future<void> showTopologyNodeInspectorSheet({
  required BuildContext context,
  required TopologyNodeModel node,
  required List<TopologyNodeModel> allNodes,
  required List<TopologyEdgeModel> allEdges,
  required List<TopologyPluginActionModel> pluginActions,
  required bool canWrite,
  required Future<QuickPingReport> Function(TopologyNodeModel node)
  onRunQuickPing,
  required VoidCallback onOpenDetails,
  required VoidCallback onTracePathFromNode,
  required VoidCallback onTraceUpstream,
  required VoidCallback onTraceroute,
  required VoidCallback onStartLinkTest,
  required VoidCallback onNewLinkFromNode,
  required VoidCallback onEditDevice,
  required VoidCallback onOpenPluginsTab,
  required ValueChanged<TopologyNodeModel> onFocusNeighbor,
  required ValueChanged<TopologyEdgeModel> onInspectEdge,
}) {
  return showModalBottomSheet<void>(
    context: context,
    isScrollControlled: true,
    useSafeArea: true,
    showDragHandle: true,
    barrierColor: Colors.black.withValues(alpha: 0.25),
    builder: (sheetCtx) {
      return DraggableScrollableSheet(
        expand: false,
        initialChildSize: 0.56,
        minChildSize: 0.34,
        maxChildSize: 0.9,
        builder: (context, scrollController) {
          return _MobileNodeInspectorSheetContent(
            scrollController: scrollController,
            node: node,
            allNodes: allNodes,
            allEdges: allEdges,
            pluginActions: pluginActions,
            canWrite: canWrite,
            onRunQuickPing: onRunQuickPing,
            onClose: () => Navigator.of(sheetCtx).pop(),
            onOpenDetails: () {
              Navigator.of(sheetCtx).pop();
              onOpenDetails();
            },
            onTracePathFromNode: () {
              Navigator.of(sheetCtx).pop();
              onTracePathFromNode();
            },
            onTraceUpstream: () {
              Navigator.of(sheetCtx).pop();
              onTraceUpstream();
            },
            onTraceroute: () {
              Navigator.of(sheetCtx).pop();
              onTraceroute();
            },
            onStartLinkTest: () {
              Navigator.of(sheetCtx).pop();
              onStartLinkTest();
            },
            onNewLinkFromNode: () {
              Navigator.of(sheetCtx).pop();
              onNewLinkFromNode();
            },
            onEditDevice: () {
              Navigator.of(sheetCtx).pop();
              onEditDevice();
            },
            onOpenPluginsTab: () {
              Navigator.of(sheetCtx).pop();
              onOpenPluginsTab();
            },
            onFocusNeighbor: (neighbor) {
              Navigator.of(sheetCtx).pop();
              onFocusNeighbor(neighbor);
            },
            onInspectEdge: (edge) {
              Navigator.of(sheetCtx).pop();
              onInspectEdge(edge);
            },
          );
        },
      );
    },
  );
}

class _MobileNodeInspectorSheetContent extends StatefulWidget {
  const _MobileNodeInspectorSheetContent({
    required this.scrollController,
    required this.node,
    required this.allNodes,
    required this.allEdges,
    required this.pluginActions,
    required this.canWrite,
    required this.onRunQuickPing,
    required this.onClose,
    required this.onOpenDetails,
    required this.onTracePathFromNode,
    required this.onTraceUpstream,
    required this.onTraceroute,
    required this.onStartLinkTest,
    required this.onNewLinkFromNode,
    required this.onEditDevice,
    required this.onOpenPluginsTab,
    required this.onFocusNeighbor,
    required this.onInspectEdge,
  });

  final ScrollController scrollController;
  final TopologyNodeModel node;
  final List<TopologyNodeModel> allNodes;
  final List<TopologyEdgeModel> allEdges;
  final List<TopologyPluginActionModel> pluginActions;
  final bool canWrite;
  final Future<QuickPingReport> Function(TopologyNodeModel node) onRunQuickPing;
  final VoidCallback onClose;
  final VoidCallback onOpenDetails;
  final VoidCallback onTracePathFromNode;
  final VoidCallback onTraceUpstream;
  final VoidCallback onTraceroute;
  final VoidCallback onStartLinkTest;
  final VoidCallback onNewLinkFromNode;
  final VoidCallback onEditDevice;
  final VoidCallback onOpenPluginsTab;
  final ValueChanged<TopologyNodeModel> onFocusNeighbor;
  final ValueChanged<TopologyEdgeModel> onInspectEdge;

  @override
  State<_MobileNodeInspectorSheetContent> createState() =>
      _MobileNodeInspectorSheetContentState();
}

class _MobileNodeInspectorSheetContentState
    extends State<_MobileNodeInspectorSheetContent> {
  bool _pingLoading = false;
  QuickPingReport? _pingReport;
  String? _pingError;

  Future<void> _runPing() async {
    setState(() {
      _pingLoading = true;
      _pingError = null;
    });
    try {
      final report = await widget.onRunQuickPing(widget.node);
      if (mounted) {
        setState(() {
          _pingReport = report;
          _pingLoading = false;
        });
      }
    } catch (err) {
      if (mounted) {
        setState(() {
          _pingError = err.toString();
          _pingLoading = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        _NodeInspectorHeader(node: widget.node, onClose: widget.onClose),
        const Divider(height: 1),
        Expanded(
          child: SingleChildScrollView(
            controller: widget.scrollController,
            padding: const EdgeInsets.fromLTRB(16, 12, 16, 20),
            child: TopologyNodeInspectorBody(
              node: widget.node,
              allNodes: widget.allNodes,
              allEdges: widget.allEdges,
              pluginActions: widget.pluginActions,
              canWrite: widget.canWrite,
              pingLoading: _pingLoading,
              pingReport: _pingReport,
              pingError: _pingError,
              onQuickPing: _runPing,
              onDismissPing: () => setState(() {
                _pingReport = null;
                _pingError = null;
              }),
              onOpenDetails: widget.onOpenDetails,
              onTracePathFromNode: widget.onTracePathFromNode,
              onTraceUpstream: widget.onTraceUpstream,
              onTraceroute: widget.onTraceroute,
              onStartLinkTest: widget.onStartLinkTest,
              onNewLinkFromNode: widget.onNewLinkFromNode,
              onEditDevice: widget.onEditDevice,
              onOpenPluginsTab: widget.onOpenPluginsTab,
              onFocusNeighbor: widget.onFocusNeighbor,
              onInspectEdge: widget.onInspectEdge,
            ),
          ),
        ),
      ],
    );
  }
}

class _NodeInspectorHeader extends StatelessWidget {
  const _NodeInspectorHeader({required this.node, required this.onClose});

  final TopologyNodeModel node;
  final VoidCallback onClose;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final status = classifyNodeStatus(node.status);
    final statusColors = context.semanticColors.resolve(status.tone, scheme);
    final subtitleParts = <String>[
      topologyNodeTypeLabel(node),
      if (node.siteName != null && node.siteName!.trim().isNotEmpty)
        node.siteName!.trim(),
    ];

    return Padding(
      padding: const EdgeInsets.fromLTRB(14, 12, 8, 12),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Container(
            width: 42,
            height: 42,
            decoration: BoxDecoration(
              color: statusColors.container,
              borderRadius: BorderRadius.circular(12),
              border: Border.all(color: statusColors.border),
            ),
            child: Icon(
              topologyNodeIcon(node),
              size: 22,
              color: statusColors.onContainer,
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Expanded(
                      child: Text(
                        node.name,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.titleMedium?.copyWith(
                          fontWeight: FontWeight.w800,
                        ),
                      ),
                    ),
                    const SizedBox(width: 6),
                    SemanticChip(
                      label: status.label,
                      tone: status.tone,
                      compact: true,
                    ),
                  ],
                ),
                const SizedBox(height: 3),
                Text(
                  subtitleParts.join(' · '),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: context.textTheme.bodySmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
                if (node.ipAddress != null &&
                    node.ipAddress!.trim().isNotEmpty) ...<Widget>[
                  const SizedBox(height: 4),
                  InkWell(
                    onTap: () {
                      Clipboard.setData(ClipboardData(text: node.ipAddress!));
                      ScaffoldMessenger.of(context).showSnackBar(
                        SnackBar(
                          content: Text('IP ${node.ipAddress!} copiado.'),
                          duration: const Duration(seconds: 2),
                        ),
                      );
                    },
                    borderRadius: BorderRadius.circular(6),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 2,
                        vertical: 2,
                      ),
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: <Widget>[
                          Text(
                            node.ipAddress!,
                            style: context.textTheme.labelMedium?.copyWith(
                              fontFamily: 'monospace',
                              fontWeight: FontWeight.w700,
                              color: scheme.primary,
                            ),
                          ),
                          const SizedBox(width: 4),
                          Icon(
                            Icons.copy_outlined,
                            size: 13,
                            color: scheme.primary,
                          ),
                        ],
                      ),
                    ),
                  ),
                ],
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
    );
  }
}

/// Corpo compartilhado do Inspetor de Nó (usado no mobile bottom sheet e no
/// painel lateral fixo do desktop).
class TopologyNodeInspectorBody extends ConsumerWidget {
  const TopologyNodeInspectorBody({
    super.key,
    required this.node,
    required this.allNodes,
    required this.allEdges,
    required this.pluginActions,
    required this.canWrite,
    required this.pingLoading,
    this.pingReport,
    this.pingError,
    required this.onQuickPing,
    required this.onDismissPing,
    required this.onOpenDetails,
    required this.onTracePathFromNode,
    required this.onTraceUpstream,
    required this.onTraceroute,
    required this.onStartLinkTest,
    required this.onNewLinkFromNode,
    required this.onEditDevice,
    required this.onOpenPluginsTab,
    required this.onFocusNeighbor,
    required this.onInspectEdge,
  });

  final TopologyNodeModel node;
  final List<TopologyNodeModel> allNodes;
  final List<TopologyEdgeModel> allEdges;
  final List<TopologyPluginActionModel> pluginActions;
  final bool canWrite;
  final bool pingLoading;
  final QuickPingReport? pingReport;
  final String? pingError;

  final VoidCallback onQuickPing;
  final VoidCallback onDismissPing;
  final VoidCallback onOpenDetails;
  final VoidCallback onTracePathFromNode;
  final VoidCallback onTraceUpstream;
  final VoidCallback onTraceroute;
  final VoidCallback onStartLinkTest;
  final VoidCallback onNewLinkFromNode;
  final VoidCallback onEditDevice;
  final VoidCallback onOpenPluginsTab;
  final ValueChanged<TopologyNodeModel> onFocusNeighbor;
  final ValueChanged<TopologyEdgeModel> onInspectEdge;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final scheme = context.colorScheme;
    final byId = <int, TopologyNodeModel>{for (final n in allNodes) n.id: n};
    final neighbors = connectedNeighbors(allEdges, node.id);
    final hasIp = node.ipAddress != null && node.ipAddress!.trim().isNotEmpty;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        // ── Faixa de telemetria ao vivo ──
        _NodeTelemetryStrip(node: node, neighborsCount: neighbors.length),
        const SizedBox(height: 12),

        // ── Resultado inline do Ping Rápido ──
        if (pingLoading) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.info,
            padding: const EdgeInsets.all(10),
            child: Row(
              children: <Widget>[
                const SizedBox(
                  width: 16,
                  height: 16,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    'Executando ping rápido em ${node.ipAddress ?? node.name}...',
                    style: context.textTheme.bodySmall?.copyWith(
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
        ] else if (pingError != null) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.error,
            padding: const EdgeInsets.all(10),
            child: Row(
              children: <Widget>[
                Expanded(
                  child: Text(pingError!, style: context.textTheme.bodySmall),
                ),
                IconButton(
                  visualDensity: VisualDensity.compact,
                  icon: const Icon(Icons.close, size: 16),
                  onPressed: onDismissPing,
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
        ] else if (pingReport != null) ...<Widget>[
          _QuickPingInlineBanner(report: pingReport!, onDismiss: onDismissPing),
          const SizedBox(height: 12),
        ],

        // ── Linha horizontal de ações rápidas (M3 tonal buttons) ──
        SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          child: Row(
            children: <Widget>[
              if (!node.isSystem && hasIp) ...<Widget>[
                FilledButton.tonalIcon(
                  onPressed: pingLoading ? null : onQuickPing,
                  icon: const Icon(Icons.network_ping, size: 17),
                  label: const Text('Ping rápido'),
                ),
                const SizedBox(width: 8),
              ],
              FilledButton.icon(
                onPressed: onOpenDetails,
                icon: const Icon(Icons.open_in_new, size: 17),
                label: const Text('Detalhes'),
              ),
              const SizedBox(width: 8),
              FilledButton.tonalIcon(
                onPressed: onTracePathFromNode,
                icon: const Icon(Icons.route, size: 17),
                label: const Text('Traçar caminho'),
              ),
              const SizedBox(width: 8),
              FilledButton.tonalIcon(
                onPressed: onTraceUpstream,
                icon: const Icon(Icons.north_east, size: 17),
                label: const Text('Até a raiz'),
              ),
              if (hasIp) ...<Widget>[
                const SizedBox(width: 8),
                OutlinedButton.icon(
                  onPressed: onTraceroute,
                  icon: const Icon(Icons.alt_route, size: 17),
                  label: const Text('Traceroute'),
                ),
              ],
              const SizedBox(width: 8),
              OutlinedButton.icon(
                onPressed: onStartLinkTest,
                icon: const Icon(Icons.speed, size: 17),
                label: const Text('Testar enlace'),
              ),
              if (canWrite) ...<Widget>[
                const SizedBox(width: 8),
                OutlinedButton.icon(
                  onPressed: onNewLinkFromNode,
                  icon: const Icon(Icons.add_link, size: 17),
                  label: const Text('Novo link'),
                ),
              ],
              if (canWrite && !node.isSystem) ...<Widget>[
                const SizedBox(width: 8),
                OutlinedButton.icon(
                  onPressed: onEditDevice,
                  icon: const Icon(Icons.edit_outlined, size: 17),
                  label: const Text('Editar'),
                ),
              ],
              const SizedBox(width: 8),
              AiAskButton(
                compact: true,
                prompt:
                    'Analise a situação atual e as conexões na topologia do equipamento ${node.name}${hasIp ? ' (${node.ipAddress})' : ''}.',
                contextData: AiChatContext(
                  deviceId: node.id,
                  deviceName: node.name,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 14),

        // ── Metadados do equipamento ──
        NestedPanel(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          child: Column(
            children: <Widget>[
              if (node.vendor != null && node.vendor!.trim().isNotEmpty)
                KeyValueRow(
                  label: 'Fabricante / Modelo',
                  value: <String>[
                    node.vendor!.trim(),
                    if (node.model != null && node.model!.trim().isNotEmpty)
                      node.model!.trim(),
                  ].join(' · '),
                ),
              KeyValueRow(label: 'Site', value: node.siteName ?? 'Sem site'),
              KeyValueRow(
                label: 'Interfaces',
                value: formatInteger(node.interfaceCount),
              ),
              KeyValueRow(
                label: 'SNMP',
                value: node.snmpEnabled ? 'Ativo' : 'Desativado',
              ),
            ],
          ),
        ),

        // ── Ações de Plugins (token teal obrigatório §12) ──
        if (pluginActions.isNotEmpty) ...<Widget>[
          const SizedBox(height: 14),
          Row(
            children: <Widget>[
              Icon(
                Icons.extension_outlined,
                size: 16,
                color: context.semanticColors.plugin,
              ),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  'Ações de Plugins',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: context.textTheme.labelLarge?.copyWith(
                    color: context.semanticColors.plugin,
                    fontWeight: FontWeight.w800,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 6),
          NestedPanel(
            tone: SemanticTone.plugin,
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
            child: Material(
              type: MaterialType.transparency,
              child: Column(
                children: <Widget>[
                  for (final act in pluginActions)
                    ListTile(
                      dense: true,
                      contentPadding: EdgeInsets.zero,
                      leading: Icon(
                        Icons.extension_outlined,
                        color: context.semanticColors.plugin,
                        size: 18,
                      ),
                      title: Text(
                        act.label,
                        style: const TextStyle(fontWeight: FontWeight.w600),
                      ),
                      subtitle: Text(act.pluginName),
                      trailing: const Icon(Icons.chevron_right, size: 18),
                      onTap: onOpenPluginsTab,
                    ),
                ],
              ),
            ),
          ),
        ],

        // ── Vizinhos / Enlaces conectados ──
        const SizedBox(height: 14),
        Row(
          children: <Widget>[
            Icon(Icons.hub_outlined, size: 16, color: scheme.primary),
            const SizedBox(width: 6),
            Expanded(
              child: Text(
                'Conexões e Vizinhos (${neighbors.length})',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: context.textTheme.labelLarge?.copyWith(
                  fontWeight: FontWeight.w800,
                ),
              ),
            ),
          ],
        ),
        const SizedBox(height: 6),
        if (neighbors.isEmpty)
          NestedPanel(
            padding: const EdgeInsets.all(12),
            child: Text(
              'Nenhum enlace conectado a este equipamento.',
              style: context.textTheme.bodySmall?.copyWith(
                color: scheme.onSurfaceVariant,
              ),
            ),
          )
        else
          for (final link in neighbors) ...<Widget>[
            _NeighborLinkTile(
              link: link,
              neighbor: byId[link.neighborId],
              onTapNeighbor: () {
                final target = byId[link.neighborId];
                if (target != null) onFocusNeighbor(target);
              },
              onTapEdge: () => onInspectEdge(link.edge),
            ),
            const SizedBox(height: 6),
          ],
      ],
    );
  }
}

class _NodeTelemetryStrip extends StatelessWidget {
  const _NodeTelemetryStrip({required this.node, required this.neighborsCount});

  final TopologyNodeModel node;
  final int neighborsCount;

  @override
  Widget build(BuildContext context) {
    final tiles = <Widget>[
      if (node.cpuPercent != null)
        _TelemetryMiniChip(
          label: 'CPU',
          value: formatPercent(node.cpuPercent, digits: 0),
          tone: classifyHealthPercent(node.cpuPercent!).tone,
          icon: Icons.memory,
        ),
      if (node.memoryPercent != null)
        _TelemetryMiniChip(
          label: 'RAM',
          value: formatPercent(node.memoryPercent, digits: 0),
          tone: classifyHealthPercent(node.memoryPercent!).tone,
          icon: Icons.sd_storage_outlined,
        ),
      _TelemetryMiniChip(
        label: 'Alertas',
        value: '${node.monitorsDown}',
        tone: node.monitorsDown > 0 ? SemanticTone.error : SemanticTone.success,
        icon: node.monitorsDown > 0
            ? Icons.warning_amber_rounded
            : Icons.verified_outlined,
      ),
      _TelemetryMiniChip(
        label: 'Enlaces',
        value: '$neighborsCount',
        tone: SemanticTone.info,
        icon: Icons.cable_outlined,
      ),
      if (isUnmanagedSwitchNode(node))
        const _TelemetryMiniChip(
          label: 'Tipo',
          value: 'Não gerenc.',
          tone: SemanticTone.secondary,
          icon: Icons.hub_outlined,
        ),
    ];

    return Wrap(spacing: 8, runSpacing: 8, children: tiles);
  }
}

class _TelemetryMiniChip extends StatelessWidget {
  const _TelemetryMiniChip({
    required this.label,
    required this.value,
    required this.tone,
    required this.icon,
  });

  final String label;
  final String value;
  final SemanticTone tone;
  final IconData icon;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(tone, scheme);

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: colors.container.withValues(alpha: 0.72),
        borderRadius: BorderRadius.circular(10),
        border: Border.all(color: colors.border),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Icon(icon, size: 14, color: colors.onContainer),
          const SizedBox(width: 6),
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Text(
                label,
                style: context.textTheme.labelSmall?.copyWith(
                  fontSize: 10,
                  color: colors.onContainer.withValues(alpha: 0.85),
                ),
              ),
              Text(
                value,
                style: context.textTheme.labelMedium?.copyWith(
                  fontWeight: FontWeight.w800,
                  color: colors.onContainer,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _QuickPingInlineBanner extends StatelessWidget {
  const _QuickPingInlineBanner({required this.report, required this.onDismiss});

  final QuickPingReport report;
  final VoidCallback onDismiss;

  @override
  Widget build(BuildContext context) {
    final tone = report.lossPct >= 50
        ? SemanticTone.error
        : report.lossPct > 0
        ? SemanticTone.warning
        : SemanticTone.success;

    return NestedPanel(
      tone: tone,
      padding: const EdgeInsets.all(10),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              const Icon(Icons.network_ping, size: 16),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  'Ping rápido · ${report.target}',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: context.textTheme.labelLarge?.copyWith(
                    fontWeight: FontWeight.w800,
                  ),
                ),
              ),
              IconButton(
                tooltip: 'Fechar resultado',
                visualDensity: VisualDensity.compact,
                icon: const Icon(Icons.close, size: 16),
                onPressed: onDismiss,
              ),
            ],
          ),
          if (report.verdict.isNotEmpty) ...<Widget>[
            Text(report.verdict, style: context.textTheme.bodySmall),
            const SizedBox(height: 4),
          ],
          Wrap(
            spacing: 10,
            runSpacing: 4,
            children: <Widget>[
              Text(
                'RTT: ${formatLatency(report.small?.avgMs)}',
                style: context.textTheme.labelSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              Text(
                'Perda: ${formatPercent(report.lossPct)}',
                style: context.textTheme.labelSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              if (report.jitterMs != null)
                Text(
                  'Jitter: ${formatLatency(report.jitterMs)}',
                  style: context.textTheme.labelSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

class _NeighborLinkTile extends StatelessWidget {
  const _NeighborLinkTile({
    required this.link,
    required this.neighbor,
    required this.onTapNeighbor,
    required this.onTapEdge,
  });

  final TopologyNeighborLink link;
  final TopologyNodeModel? neighbor;
  final VoidCallback onTapNeighbor;
  final VoidCallback onTapEdge;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final edge = link.edge;
    final neighborName =
        neighbor?.name ??
        (link.isOutgoing ? edge.targetDeviceName : edge.sourceDeviceName) ??
        '#${link.neighborId}';
    final status = classifyNodeStatus(neighbor?.status);
    final statusColors = context.semanticColors.resolve(status.tone, scheme);
    final badgeLabel = formatEdgeBadgeLabel(edge);
    final edgeTone = edgeSemanticTone(edge);

    final localPort = link.isOutgoing
        ? edge.sourceInterfaceName
        : edge.targetInterfaceName;
    final remotePort = link.isOutgoing
        ? edge.targetInterfaceName
        : edge.sourceInterfaceName;
    final portsText = <String>[
      if (localPort != null && localPort.isNotEmpty) localPort,
      if (remotePort != null && remotePort.isNotEmpty) remotePort,
    ].join(' ↔ ');

    return Material(
      color: scheme.surfaceContainerLow,
      shape: RoundedRectangleBorder(
        borderRadius: AppTheme.borderRadius,
        side: BorderSide(color: scheme.outlineVariant),
      ),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onTapNeighbor,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
          child: Row(
            children: <Widget>[
              Stack(
                clipBehavior: Clip.none,
                children: <Widget>[
                  Icon(
                    neighbor != null
                        ? topologyNodeIcon(neighbor!)
                        : Icons.devices_other_outlined,
                    size: 20,
                    color: scheme.primary,
                  ),
                  Positioned(
                    right: -2,
                    bottom: -2,
                    child: Container(
                      width: 8,
                      height: 8,
                      decoration: BoxDecoration(
                        shape: BoxShape.circle,
                        color: statusColors.foreground,
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
                      neighborName,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.bodyMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    if (portsText.isNotEmpty ||
                        neighbor?.ipAddress != null) ...<Widget>[
                      const SizedBox(height: 1),
                      Text(
                        portsText.isNotEmpty
                            ? portsText
                            : (neighbor?.ipAddress ?? ''),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.labelSmall?.copyWith(
                          color: scheme.onSurfaceVariant,
                          fontFamily: portsText.isEmpty ? 'monospace' : null,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              const SizedBox(width: 6),
              SemanticChip(
                label: badgeLabel,
                tone: edgeTone,
                compact: true,
                onTap: onTapEdge,
              ),
              const SizedBox(width: 4),
              Icon(
                Icons.chevron_right,
                size: 18,
                color: scheme.onSurfaceVariant,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

// ── Edge / Link Inspector ───────────────────────────────────────────────────

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
class TopologyEdgeInspectorBody extends StatelessWidget {
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
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final byId = <int, TopologyNodeModel>{for (final n in allNodes) n.id: n};
    final sourceNode = byId[edge.sourceDeviceId];
    final targetNode = byId[edge.targetDeviceId];
    final linkKind = classifyLinkKind(edge.linkType);
    final util = resolveEdgeUtilization(edge);
    final utilTone = utilizationTone(util);
    final utilColors = context.semanticColors.resolve(utilTone, scheme);
    final speedLabel = formatEdgeSpeedLabel(edge);
    final trafficLabel = formatEdgeTrafficLabel(edge);
    final confidencePct = edgeConfidencePercent(edge.confidence);
    final isVirtual = isVirtualTopologyEdge(edge);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        // ── Cabeçalho clicável Origem ↔ Destino ──
        NestedPanel(
          padding: const EdgeInsets.all(10),
          child: Row(
            children: <Widget>[
              Expanded(
                child: _EndpointChipButton(
                  label:
                      sourceNode?.name ??
                      edge.sourceDeviceName ??
                      '#${edge.sourceDeviceId}',
                  subtitle: edge.sourceInterfaceName,
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
                      edge.targetDeviceName ??
                      '#${edge.targetDeviceId}',
                  subtitle: edge.targetInterfaceName,
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

        // ── Barra de utilização (se houver telemetria) ──
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

        // ── Detalhes técnicos ──
        NestedPanel(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
          child: Column(
            children: <Widget>[
              KeyValueRow(
                label: 'Meio / Tipo',
                value: linkKind.label,
                trailing: SemanticChip(
                  label: edge.confirmed ? 'Confirmado' : 'Automático',
                  tone: edge.confirmed
                      ? SemanticTone.success
                      : SemanticTone.info,
                  compact: true,
                ),
              ),
              KeyValueRow(label: 'Velocidade', value: speedLabel),
              if (edge.inBps != null || edge.outBps != null) ...<Widget>[
                KeyValueRow(
                  label: 'Download (RX)',
                  value: formatBps(edge.inBps ?? 0),
                ),
                KeyValueRow(
                  label: 'Upload (TX)',
                  value: formatBps(edge.outBps ?? 0),
                ),
              ] else if (trafficLabel != null)
                KeyValueRow(label: 'Tráfego ao vivo', value: trafficLabel),
              if (edge.sourceInterfaceName != null)
                KeyValueRow(
                  label: 'Porta origem',
                  value: edge.sourceInterfaceName!,
                ),
              if (edge.targetInterfaceName != null)
                KeyValueRow(
                  label: 'Porta destino',
                  value: edge.targetInterfaceName!,
                ),
              if (confidencePct != null && !isVirtual)
                KeyValueRow(label: 'Confiança', value: '$confidencePct%'),
              if (edge.lastSeenAt != null && edge.lastSeenAt!.isNotEmpty)
                KeyValueRow(
                  label: 'Última leitura',
                  value: formatRelativeTime(edge.lastSeenAt),
                ),
            ],
          ),
        ),
        const SizedBox(height: 14),

        // ── Ações do enlace ──
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            if (!isVirtual)
              FilledButton.tonalIcon(
                onPressed: onStartLinkTest,
                icon: const Icon(Icons.speed, size: 17),
                label: const Text('Testar enlace'),
              ),
            if (canWrite && canConfirmTopologyEdge(edge))
              OutlinedButton.icon(
                onPressed: onConfirmLink,
                icon: const Icon(Icons.verified_outlined, size: 17),
                label: const Text('Confirmar enlace'),
              ),
            if (canWrite && !isVirtual)
              OutlinedButton.icon(
                onPressed: onEditLink,
                icon: const Icon(Icons.edit_outlined, size: 17),
                label: const Text('Editar'),
              ),
            if (canWrite && !isVirtual)
              TextButton.icon(
                onPressed: onDeleteLink,
                icon: Icon(Icons.delete_outline, color: scheme.error, size: 18),
                label: Text(
                  'Remover link',
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
