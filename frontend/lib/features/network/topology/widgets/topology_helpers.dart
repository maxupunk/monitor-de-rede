import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/link_test_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/models/wifi_models.dart';
import 'package:netmonitor_app/features/network/topology/canvas/topology_canvas.dart';
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Mapa `deviceId -> macAddress` para enriquecer a busca da topologia.
Map<int, String> buildTopologyMacIndex(List<DeviceModel> devices) {
  final map = <int, String>{};
  for (final d in devices) {
    final mac = d.macAddress?.trim();
    if (mac != null && mac.isNotEmpty) {
      map[d.id] = mac;
    }
  }
  return map;
}

/// Constrói metadados extras por nó (clientes Wi-Fi e ações de plugin).
Map<int, TopologyNodeExtras> buildTopologyNodeExtras({
  required List<TopologyNodeModel> nodes,
  required Map<int, List<TopologyPluginActionModel>> pluginActions,
  required Map<int, WifiDeviceSnapshotModel> wifiSnapshots,
}) {
  final extras = <int, TopologyNodeExtras>{};
  for (final node in nodes) {
    final wifiCount = wifiSnapshots[node.id]?.stations.length ?? 0;
    final hasPlugins = (pluginActions[node.id]?.isNotEmpty ?? false);
    if (wifiCount > 0 || hasPlugins || node.monitorsDown > 0) {
      extras[node.id] = TopologyNodeExtras(
        alertCount: node.monitorsDown,
        wifiClientsCount: wifiCount,
        hasPluginActions: hasPlugins,
      );
    }
  }
  return extras;
}

/// Constrói a sobreposição de testes de enlace (`linkTestsProvider`) por `edge.id`.
Map<int, TopologyLinkTestEdgeOverlay> buildTopologyLinkTestOverlays({
  required List<TopologyEdgeModel> edges,
  required List<LinkTestModel> tests,
}) {
  if (edges.isEmpty || tests.isEmpty) {
    return const <int, TopologyLinkTestEdgeOverlay>{};
  }
  final edgeIds = edges.map((e) => e.id).toSet();
  final overlays = <int, TopologyLinkTestEdgeOverlay>{};

  for (final test in tests) {
    final linkId = test.linkId;
    if (linkId == null || !edgeIds.contains(linkId)) continue;
    if (overlays.containsKey(linkId) && !test.isActive) continue;

    final bps =
        test.summary?.minForwardBps ??
        test.summary?.minReverseBps ??
        (test.legs.isNotEmpty
            ? (test.legs.first.forwardBps ?? test.legs.first.reverseBps)
            : null);
    final label = test.isActive
        ? (bps != null ? 'Testando · ${formatBps(bps)}' : 'Testando...')
        : (bps != null ? formatBps(bps) : test.status);
    final tone = switch (test.status) {
      'completed' => SemanticTone.success,
      'failed' => SemanticTone.error,
      'cancelled' => SemanticTone.warning,
      _ => SemanticTone.primary,
    };

    overlays[linkId] = TopologyLinkTestEdgeOverlay(
      label: label,
      tone: tone,
      active: test.isActive,
    );
  }
  return overlays;
}

/// Resolve posições e caixas de cluster combinando posições salvas/arrastadas
/// com o layout calculado pelo motor puro.
TopologyLayoutOutput resolveTopologyLayoutState({
  required List<TopologyNodeModel> visibleNodes,
  required List<TopologyEdgeModel> visibleEdges,
  required TopologyLayoutKind layoutMode,
  required Map<int, TopologyPosition> providerPositions,
  required Map<int, Offset> layoutOverridePositions,
  required bool useEngineLayout,
}) {
  if (visibleNodes.isEmpty) {
    return const TopologyLayoutOutput(
      positions: <int, Offset>{},
      bounds: Rect.zero,
    );
  }

  final savedOffsets = layoutOverridePositions.isNotEmpty
      ? layoutOverridePositions
      : offsetsFromTopologyPositions(providerPositions);

  return computeTopologyLayoutOutput(
    nodes: visibleNodes,
    edges: visibleEdges,
    mode: layoutMode,
    savedPositions: savedOffsets,
    respectSavedPositions: !useEngineLayout && savedOffsets.isNotEmpty,
  );
}
