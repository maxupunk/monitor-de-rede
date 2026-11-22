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

/// Sugere automaticamente o meio físico/lógico (`fiber`, `wireless`, `vpn`,
/// `ethernet`) a partir do nome, alias ou tipo SNMP de uma interface.
String? inferTopologyLinkTypeFromInterface(
  String? interfaceName, {
  String? alias,
  String? ifType,
}) {
  final primary = (interfaceName ?? '').trim().toLowerCase();
  final secondary = '${alias ?? ''} ${ifType ?? ''}'.trim().toLowerCase();
  if (primary.isEmpty && secondary.isEmpty) return null;

  bool matchesAny(String text, List<String> tokens) {
    if (text.isEmpty) return false;
    for (final token in tokens) {
      if (text.contains(token)) return true;
    }
    return false;
  }

  const fiberTokens = <String>[
    'sfp',
    'qsfp',
    'xfp',
    'pon',
    'gpon',
    'epon',
    'fiber',
    'fibra',
    'optical',
  ];
  const wirelessTokens = <String>[
    'wlan',
    'ath',
    'wifi',
    'wi-fi',
    'radio',
    'mesh',
    'wds',
    'ra0',
    'rai0',
    'phy',
  ];
  const vpnTokens = <String>[
    'wg',
    'wireguard',
    'tun',
    'tap',
    'vpn',
    'ipsec',
    'gre',
    'l2tp',
    'ovpn',
    'ppp',
    'vti',
    'tailscale',
    'zerotier',
  ];
  const ethernetTokens = <String>[
    'eth',
    'ether',
    'eno',
    'ens',
    'enp',
    'ge-',
    'xe-',
    'lan',
    'wan',
    'br-',
    'port',
  ];

  // Prioriza o nome técnico da porta; usa alias/ifType como desempate secundário.
  for (final target in <String>[primary, secondary]) {
    if (matchesAny(target, fiberTokens)) return 'fiber';
    if (matchesAny(target, vpnTokens)) return 'vpn';
    if (matchesAny(target, wirelessTokens)) return 'wireless';
    if (matchesAny(target, ethernetTokens)) return 'ethernet';
  }
  return null;
}

/// Combina as sugestões de mídia das portas de origem e destino priorizando
/// meios específicos (`fiber` > `vpn` > `wireless` > `ethernet`).
String? resolveSuggestedTopologyLinkType({
  DeviceInterfaceModel? sourceInterface,
  DeviceInterfaceModel? targetInterface,
}) {
  final srcKind = sourceInterface != null
      ? inferTopologyLinkTypeFromInterface(
          sourceInterface.displayName,
          alias: sourceInterface.subtitleText,
          ifType: sourceInterface.ifType,
        )
      : null;
  final tgtKind = targetInterface != null
      ? inferTopologyLinkTypeFromInterface(
          targetInterface.displayName,
          alias: targetInterface.subtitleText,
          ifType: targetInterface.ifType,
        )
      : null;

  const priority = <String>['fiber', 'vpn', 'wireless', 'ethernet'];
  for (final kind in priority) {
    if (srcKind == kind || tgtKind == kind) return kind;
  }
  return srcKind ?? tgtKind;
}

/// Filtra e ordena equipamentos candidatos para seleção de Origem/Destino de enlace:
/// - Oculta `excludedNodeId` (ex.: Origem nunca aparece na lista de Destino).
/// - Prioriza equipamentos do mesmo site (`preferSiteId`) no topo.
/// - Suporta busca ao vivo por Nome, IP, Site, Tipo (incluindo rótulo PT-BR), Fabricante e MAC.
List<TopologyNodeModel> rankTopologyEndpointCandidates({
  required List<TopologyNodeModel> nodes,
  required String query,
  int? excludedNodeId,
  int? preferSiteId,
  Map<int, String>? macByNodeId,
}) {
  final candidates = excludedNodeId == null
      ? List<TopologyNodeModel>.from(nodes)
      : nodes.where((n) => n.id != excludedNodeId).toList();
  if (candidates.isEmpty) return const <TopologyNodeModel>[];

  int statusRank(TopologyNodeModel n) {
    return switch (classifyNodeStatus(n.status)) {
      TopologyNodeStatus.online => 0,
      TopologyNodeStatus.warning => 1,
      TopologyNodeStatus.unknown => 2,
      TopologyNodeStatus.offline => 3,
    };
  }

  bool isPreferredSite(TopologyNodeModel n) =>
      preferSiteId != null && n.siteId == preferSiteId;

  final cleanQuery = query.trim();
  if (cleanQuery.isEmpty) {
    candidates.sort((a, b) {
      final siteA = isPreferredSite(a);
      final siteB = isPreferredSite(b);
      if (siteA != siteB) return siteA ? -1 : 1;
      final stCmp = statusRank(a).compareTo(statusRank(b));
      if (stCmp != 0) return stCmp;
      return compareTopologyNodes(a, b);
    });
    return List<TopologyNodeModel>.unmodifiable(candidates);
  }

  final engineHits = searchTopologyNodes(
    candidates,
    cleanQuery,
    limit: candidates.length,
    macByNodeId: macByNodeId,
  );
  final scoreById = <int, double>{
    for (final hit in engineHits) hit.node.id: hit.score,
  };

  final lowerTerms = cleanQuery
      .toLowerCase()
      .split(RegExp(r'\s+'))
      .where((t) => t.isNotEmpty)
      .toList(growable: false);

  for (final node in candidates) {
    if (scoreById.containsKey(node.id)) continue;
    final localizedHaystack =
        '${node.name} ${node.ipAddress ?? ''} ${node.siteName ?? ''} '
                '${topologyNodeTypeLabel(node)} ${node.type} '
                '${node.vendor ?? ''} ${node.model ?? ''} '
                '${macByNodeId?[node.id] ?? ''}'
            .toLowerCase();
    if (lowerTerms.every(localizedHaystack.contains)) {
      scoreById[node.id] = 0.45;
    }
  }

  final matched = candidates
      .where((n) => scoreById.containsKey(n.id))
      .toList(growable: false);
  final sorted = List<TopologyNodeModel>.from(matched)
    ..sort((a, b) {
      final effA = scoreById[a.id]! + (isPreferredSite(a) ? 0.12 : 0.0);
      final effB = scoreById[b.id]! + (isPreferredSite(b) ? 0.12 : 0.0);
      final scoreCmp = effB.compareTo(effA);
      if (scoreCmp != 0) return scoreCmp;
      final siteA = isPreferredSite(a);
      final siteB = isPreferredSite(b);
      if (siteA != siteB) return siteA ? -1 : 1;
      return compareTopologyNodes(a, b);
    });
  return List<TopologyNodeModel>.unmodifiable(sorted);
}

/// Resolve os nomes legíveis dos nós de um caminho para exibir no banner do HUD.
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
