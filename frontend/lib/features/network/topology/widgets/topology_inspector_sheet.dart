import 'package:flutter/material.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/topology/widgets/inspector/topology_edge_inspector.dart';
import 'package:netmonitor_app/features/network/topology/widgets/inspector/topology_node_inspector.dart';

export 'package:netmonitor_app/features/network/providers/devices_provider.dart'
    show topologyNodeInterfacesProvider;
export 'package:netmonitor_app/features/network/topology/widgets/inspector/topology_edge_inspector.dart';
export 'package:netmonitor_app/features/network/topology/widgets/inspector/topology_node_inspector.dart';
export 'package:netmonitor_app/features/network/topology/widgets/inspector/topology_node_interfaces_section.dart';

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
    this.onConnectOnMapFromNode,
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
  final ValueChanged<TopologyNodeModel>? onConnectOnMapFromNode;
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
        onConnectOnMapFromNode: onConnectOnMapFromNode != null
            ? () => onConnectOnMapFromNode!(node)
            : null,
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
