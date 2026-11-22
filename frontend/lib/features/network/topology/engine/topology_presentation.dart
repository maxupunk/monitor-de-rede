import 'package:flutter/material.dart' show IconData, Icons, immutable;
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Modos de sobreposição visual do mapa de topologia.
enum TopologyOverlayMode {
  status(
    label: 'Status',
    shortLabel: 'Status',
    subtitle: 'Estado operacional dos nós e enlaces',
    icon: Icons.check_circle_outline,
  ),
  traffic(
    label: 'Tráfego ao vivo',
    shortLabel: 'Tráfego',
    subtitle: 'Cor e fluxo animado pela utilização do enlace',
    icon: Icons.swap_vert_outlined,
  ),
  latency(
    label: 'Latência',
    shortLabel: 'Latência',
    subtitle: 'Tempo de resposta (RTT) dos enlaces e nós',
    icon: Icons.speed_outlined,
  ),
  health(
    label: 'Saúde',
    shortLabel: 'Saúde',
    subtitle: 'Destaque de CPU, memória e alertas ativos',
    icon: Icons.monitor_heart_outlined,
  ),
  linkTest(
    label: 'Teste de Enlace',
    shortLabel: 'Teste',
    subtitle: 'Sobreposição de testes ativos de banda e perda',
    icon: Icons.bolt_outlined,
  );

  const TopologyOverlayMode({
    required this.label,
    required this.shortLabel,
    required this.subtitle,
    required this.icon,
  });

  final String label;
  final String shortLabel;
  final String subtitle;
  final IconData icon;

  static TopologyOverlayMode fromStorageKey(String? raw) {
    for (final mode in values) {
      if (mode.name == raw) return mode;
    }
    return TopologyOverlayMode.status;
  }
}

/// Estado normalizado de um nó no mapa.
enum TopologyNodeStatus {
  online(
    label: 'Online',
    tone: SemanticTone.success,
    icon: Icons.check_circle_outlined,
  ),
  warning(
    label: 'Alerta',
    tone: SemanticTone.warning,
    icon: Icons.warning_amber_outlined,
  ),
  offline(
    label: 'Offline',
    tone: SemanticTone.error,
    icon: Icons.cancel_outlined,
  ),
  unknown(
    label: 'Sem dados',
    tone: SemanticTone.info,
    icon: Icons.help_outline,
  );

  const TopologyNodeStatus({
    required this.label,
    required this.tone,
    required this.icon,
  });

  final String label;
  final SemanticTone tone;
  final IconData icon;

  static TopologyNodeStatus? tryFromStorageKey(String? raw) {
    for (final status in values) {
      if (status.name == raw) return status;
    }
    return null;
  }
}

/// Estado normalizado de um enlace.
enum TopologyEdgeStatus {
  up(
    label: 'Ativo',
    tone: SemanticTone.success,
    icon: Icons.arrow_circle_up_outlined,
  ),
  down(
    label: 'Fora do ar',
    tone: SemanticTone.error,
    icon: Icons.arrow_circle_down_outlined,
  ),
  unknown(
    label: 'Sem dados',
    tone: SemanticTone.info,
    icon: Icons.help_outline,
  );

  const TopologyEdgeStatus({
    required this.label,
    required this.tone,
    required this.icon,
  });

  final String label;
  final SemanticTone tone;
  final IconData icon;
}

/// Estilo de traço de um enlace no canvas e na legenda.
enum TopologyLineStyle { solid, dashed, dotted }

/// Meio físico ou lógico de um enlace (paridade com `LINK_KINDS`).
enum TopologyLinkKind {
  ethernet(
    label: 'Cabo Ethernet',
    tone: SemanticTone.primary,
    icon: Icons.settings_ethernet,
    lineStyle: TopologyLineStyle.solid,
  ),
  fiber(
    label: 'Fibra óptica',
    tone: SemanticTone.secondary,
    icon: Icons.cable_outlined,
    lineStyle: TopologyLineStyle.solid,
  ),
  wireless(
    label: 'Sem fio / rádio',
    tone: SemanticTone.info,
    icon: Icons.wifi,
    lineStyle: TopologyLineStyle.dashed,
  ),
  vpn(
    label: 'Túnel VPN',
    tone: SemanticTone.warning,
    icon: Icons.vpn_lock_outlined,
    lineStyle: TopologyLineStyle.dashed,
  ),
  inferred(
    label: 'Inferido (sub-rede)',
    tone: SemanticTone.info,
    icon: Icons.lan_outlined,
    lineStyle: TopologyLineStyle.dashed,
  ),
  parent(
    label: 'Hierarquia (pai)',
    tone: SemanticTone.secondary,
    icon: Icons.account_tree_outlined,
    lineStyle: TopologyLineStyle.dotted,
  );

  const TopologyLinkKind({
    required this.label,
    required this.tone,
    required this.icon,
    required this.lineStyle,
  });

  final String label;
  final SemanticTone tone;
  final IconData icon;
  final TopologyLineStyle lineStyle;

  static TopologyLinkKind? tryFromStorageKey(String? raw) {
    for (final kind in values) {
      if (kind.name == raw) return kind;
    }
    return null;
  }
}

/// Faixa de utilização de enlace (`< 70%`, `>= 70%`, `>= 90%`).
enum TopologyUtilizationLevel {
  normal(
    label: 'Uso normal (< 70%)',
    tone: SemanticTone.success,
    threshold: 0.0,
  ),
  high(label: 'Uso alto (≥ 70%)', tone: SemanticTone.warning, threshold: 0.7),
  critical(label: 'Saturado (≥ 90%)', tone: SemanticTone.error, threshold: 0.9);

  const TopologyUtilizationLevel({
    required this.label,
    required this.tone,
    required this.threshold,
  });

  final String label;
  final SemanticTone tone;
  final double threshold;
}

/// Classe de velocidade nominal para espessura visual do enlace.
enum TopologySpeedTier { unknown, slow, gigabit, fast }

/// Faixa de saúde de CPU / memória do equipamento.
enum TopologyHealthLevel {
  normal(label: 'Folgado (< 70%)', tone: SemanticTone.success),
  high(label: 'Alto (≥ 70%)', tone: SemanticTone.caution),
  critical(label: 'No limite (≥ 90%)', tone: SemanticTone.error);

  const TopologyHealthLevel({required this.label, required this.tone});

  final String label;
  final SemanticTone tone;
}

/// Limiar de alerta de CPU/memória (%).
const double kTopologyHealthWarningPercent = 70.0;

/// Limiar crítico de CPU/memória (%).
const double kTopologyHealthCriticalPercent = 90.0;

const Set<String> _unmanagedSwitchTypes = <String>{'unmanaged_switch', 'hub'};

/// Classifica a string bruta de status de um nó.
TopologyNodeStatus classifyNodeStatus(String? status) {
  switch ((status ?? '').trim().toLowerCase()) {
    case 'online':
    case 'up':
      return TopologyNodeStatus.online;
    case 'offline':
    case 'down':
      return TopologyNodeStatus.offline;
    case 'warning':
    case 'degraded':
      return TopologyNodeStatus.warning;
    default:
      return TopologyNodeStatus.unknown;
  }
}

/// Classifica o estado operacional efetivo de um enlace considerando portas.
TopologyEdgeStatus classifyEdgeStatus(TopologyEdgeModel edge) {
  final raw = edge.status.trim().toLowerCase();
  if (raw == 'down' || raw == 'offline') return TopologyEdgeStatus.down;
  if (edge.sourceInterfaceStatus?.trim().toLowerCase() == 'down' ||
      edge.targetInterfaceStatus?.trim().toLowerCase() == 'down') {
    return TopologyEdgeStatus.down;
  }
  if (raw == 'up' || raw == 'online') return TopologyEdgeStatus.up;
  return TopologyEdgeStatus.unknown;
}

/// Classifica o meio físico/lógico do enlace.
TopologyLinkKind classifyLinkKind(String? linkType) {
  switch ((linkType ?? '').trim().toLowerCase()) {
    case 'fiber':
      return TopologyLinkKind.fiber;
    case 'wireless':
    case 'radio':
    case 'wifi':
      return TopologyLinkKind.wireless;
    case 'vpn':
    case 'wireguard':
      return TopologyLinkKind.vpn;
    case 'inferred':
    case 'subnet':
    case 'subnet_inference':
      return TopologyLinkKind.inferred;
    case 'parent':
    case 'parent_hierarchy':
    case 'hierarchy':
      return TopologyLinkKind.parent;
    default:
      return TopologyLinkKind.ethernet;
  }
}

/// Indica se o nó representa um switch não gerenciável ou hub.
bool isUnmanagedSwitchNode(TopologyNodeModel node) {
  return _unmanagedSwitchTypes.contains(node.type.trim().toLowerCase());
}

/// Indica se o enlace é virtual (derivado da hierarquia pai → filho).
bool isVirtualTopologyEdge(TopologyEdgeModel edge) {
  return edge.id < 0 ||
      classifyLinkKind(edge.linkType) == TopologyLinkKind.parent;
}

/// Indica se um enlace descoberto pode ser confirmado manualmente pelo operador.
bool canConfirmTopologyEdge(TopologyEdgeModel edge) {
  return !isVirtualTopologyEdge(edge) &&
      !edge.confirmed &&
      edge.discoveryMethod.trim().toLowerCase() != 'user_defined' &&
      edge.discoveryMethod.trim().toLowerCase() != 'manual';
}

/// Rótulo legível em PT-BR do método de descoberta do enlace.
String formatDiscoveryMethodLabel(String? method) {
  switch ((method ?? '').trim().toLowerCase()) {
    case 'lldp':
      return 'Descoberto por LLDP';
    case 'cdp':
      return 'Descoberto por CDP';
    case 'arp':
      return 'Descoberto por ARP';
    case 'snmp':
      return 'Descoberto por SNMP';
    case 'subnet_inference':
    case 'subnet':
      return 'Inferido pela sub-rede';
    case 'user_defined':
    case 'manual':
      return 'Definido manualmente';
    case 'parent_hierarchy':
    case 'parent':
      return 'Hierarquia (dispositivo pai)';
    case '':
      return 'Origem desconhecida';
    default:
      return 'Descoberto automaticamente';
  }
}

/// Rótulo legível em PT-BR do tipo do equipamento na topologia.
String topologyNodeTypeLabel(TopologyNodeModel node) {
  if (node.isSystem) return 'Servidor NetMonitor';
  final raw = node.type.trim().toLowerCase();
  if (_unmanagedSwitchTypes.contains(raw)) return 'Switch não gerenciável';
  return switch (raw) {
    'router' || 'gateway' => 'Roteador',
    'firewall' => 'Firewall',
    'switch' => 'Switch',
    'access_point' || 'ap' => 'Access Point',
    'server' => 'Servidor',
    'nas' => 'NAS / Storage',
    'workstation' || 'desktop' || 'pc' => 'Estação de trabalho',
    'mobile' || 'phone' => 'Dispositivo móvel',
    'printer' => 'Impressora',
    'camera' || 'cctv' => 'Câmera',
    'voip' => 'Telefone VoIP',
    'iot' => 'IoT',
    'media' || 'tv' => 'Mídia',
    'ups' => 'Nobreak (UPS)',
    'vm' => 'Máquina Virtual',
    'web_device' => 'Dispositivo web',
    'other' => 'Outro',
    'unknown' || '' => 'Desconhecido',
    _ => node.type,
  };
}

/// Ícone Material correspondente ao nó na topologia.
IconData topologyNodeIcon(TopologyNodeModel node) {
  if (node.isSystem) return Icons.monitor_heart_outlined;
  final raw = node.type.trim().toLowerCase();
  if (_unmanagedSwitchTypes.contains(raw)) return Icons.hub_outlined;
  return switch (raw) {
    'router' || 'gateway' => Icons.router_outlined,
    'firewall' => Icons.security_outlined,
    'switch' => Icons.lan_outlined,
    'access_point' || 'ap' => Icons.wifi_tethering,
    'server' => Icons.dns_outlined,
    'nas' => Icons.storage_outlined,
    'workstation' || 'desktop' || 'pc' => Icons.computer_outlined,
    'mobile' || 'phone' => Icons.smartphone_outlined,
    'printer' => Icons.print_outlined,
    'camera' || 'cctv' => Icons.videocam_outlined,
    'voip' => Icons.phone_in_talk_outlined,
    'iot' => Icons.sensors_outlined,
    'media' || 'tv' => Icons.tv_outlined,
    'ups' => Icons.battery_charging_full_outlined,
    'vm' => Icons.cloud_queue_outlined,
    'web_device' => Icons.language_outlined,
    _ => Icons.devices_other_outlined,
  };
}

/// Velocidade nominal efetiva em bps (a do enlace ou a menor das duas portas).
double? resolveEdgeSpeedBps(TopologyEdgeModel edge) {
  if (edge.speedBps != null && edge.speedBps!.isFinite && edge.speedBps! > 0) {
    return edge.speedBps;
  }
  final speeds = <double>[
    if (edge.sourceInterfaceSpeed != null &&
        edge.sourceInterfaceSpeed!.isFinite &&
        edge.sourceInterfaceSpeed! > 0)
      edge.sourceInterfaceSpeed!,
    if (edge.targetInterfaceSpeed != null &&
        edge.targetInterfaceSpeed!.isFinite &&
        edge.targetInterfaceSpeed! > 0)
      edge.targetInterfaceSpeed!,
  ];
  if (speeds.isEmpty) return null;
  return speeds.reduce((a, b) => a < b ? a : b);
}

/// Classifica a velocidade nominal em degraus para espessura de traço.
TopologySpeedTier classifySpeedTier(double? speedBps) {
  if (speedBps == null || !speedBps.isFinite || speedBps <= 0) {
    return TopologySpeedTier.unknown;
  }
  if (speedBps <= 100000000) return TopologySpeedTier.slow;
  if (speedBps < 10000000000) return TopologySpeedTier.gigabit;
  return TopologySpeedTier.fast;
}

/// Espessura base recomendada do traço para um enlace.
double edgeBaseStrokeWidth(TopologyEdgeModel edge) {
  return switch (classifySpeedTier(resolveEdgeSpeedBps(edge))) {
    TopologySpeedTier.slow => 1.8,
    TopologySpeedTier.gigabit => 2.5,
    TopologySpeedTier.fast => 3.5,
    TopologySpeedTier.unknown => 2.0,
  };
}

/// Utilização normalizada `0.0..1.0` do enlace (ou `null` sem telemetria).
double? resolveEdgeUtilization(TopologyEdgeModel edge) {
  final raw = edge.utilization;
  if (raw != null && raw.isFinite) {
    final normalized = raw > 1.0 && raw <= 100.0 ? raw / 100.0 : raw;
    return normalized.clamp(0.0, 1.0);
  }
  final speed = resolveEdgeSpeedBps(edge);
  if (speed == null || speed <= 0) return null;
  final inVal = edge.inBps != null && edge.inBps!.isFinite ? edge.inBps! : -1.0;
  final outVal = edge.outBps != null && edge.outBps!.isFinite
      ? edge.outBps!
      : -1.0;
  final peak = inVal > outVal ? inVal : outVal;
  if (peak < 0) return null;
  return (peak / speed).clamp(0.0, 1.0);
}

/// Classifica a razão de utilização (`0.0..1.0`) em nível semântico.
TopologyUtilizationLevel? classifyUtilization(double? utilization) {
  if (utilization == null || !utilization.isFinite) return null;
  if (utilization >= TopologyUtilizationLevel.critical.threshold) {
    return TopologyUtilizationLevel.critical;
  }
  if (utilization >= TopologyUtilizationLevel.high.threshold) {
    return TopologyUtilizationLevel.high;
  }
  return TopologyUtilizationLevel.normal;
}

/// Converte razão de utilização (`0.0..1.0`) diretamente para `SemanticTone`.
SemanticTone utilizationTone(double? utilization) {
  return classifyUtilization(utilization)?.tone ?? SemanticTone.info;
}

/// Converte latência em ms para `SemanticTone`.
SemanticTone latencyTone(double? latencyMs) {
  if (latencyMs == null || !latencyMs.isFinite || latencyMs < 0) {
    return SemanticTone.info;
  }
  if (latencyMs <= 30.0) return SemanticTone.success;
  if (latencyMs <= 100.0) return SemanticTone.warning;
  return SemanticTone.error;
}

/// Resolve o `SemanticTone` principal de um enlace segundo o overlay ativo.
SemanticTone edgeSemanticTone(
  TopologyEdgeModel edge, {
  TopologyOverlayMode overlay = TopologyOverlayMode.status,
  double? latencyMs,
}) {
  final status = classifyEdgeStatus(edge);
  if (status == TopologyEdgeStatus.down) return SemanticTone.error;

  switch (overlay) {
    case TopologyOverlayMode.traffic:
      final util = resolveEdgeUtilization(edge);
      if (util != null) return utilizationTone(util);
      return status == TopologyEdgeStatus.up
          ? SemanticTone.success
          : SemanticTone.info;
    case TopologyOverlayMode.latency:
      if (latencyMs != null) return latencyTone(latencyMs);
      return status == TopologyEdgeStatus.up
          ? SemanticTone.info
          : SemanticTone.secondary;
    case TopologyOverlayMode.health:
      final util = resolveEdgeUtilization(edge);
      if (util != null) return utilizationTone(util);
      return status == TopologyEdgeStatus.up
          ? SemanticTone.success
          : SemanticTone.info;
    case TopologyOverlayMode.linkTest:
      return SemanticTone.primary;
    case TopologyOverlayMode.status:
      final util = resolveEdgeUtilization(edge);
      if (util != null && util >= TopologyUtilizationLevel.high.threshold) {
        return utilizationTone(util);
      }
      if (status == TopologyEdgeStatus.up) {
        return classifyLinkKind(edge.linkType).tone;
      }
      return SemanticTone.info;
  }
}

/// Formata a velocidade nominal do enlace usando `formatLinkSpeed` canônico.
String formatEdgeSpeedLabel(TopologyEdgeModel edge) {
  return formatLinkSpeed(resolveEdgeSpeedBps(edge));
}

/// Formata o tráfego ao vivo (`"↓ 15 Mbps ↑ 45 Mbps"`) usando `formatBps`.
/// Retorna `null` quando não há medição de tráfego.
String? formatEdgeTrafficLabel(TopologyEdgeModel edge) {
  final hasDir =
      (edge.inBps != null && edge.inBps!.isFinite) ||
      (edge.outBps != null && edge.outBps!.isFinite);
  if (hasDir) {
    final rx = formatBps(edge.inBps ?? 0);
    final tx = formatBps(edge.outBps ?? 0);
    return '↓ $rx ↑ $tx';
  }
  if (edge.trafficBps != null && edge.trafficBps!.isFinite) {
    return formatBps(edge.trafficBps);
  }
  if (edge.trafficLabel != null && edge.trafficLabel!.trim().isNotEmpty) {
    return edge.trafficLabel!.trim();
  }
  return null;
}

/// Rótulo compacto exibido na pílula do enlace no canvas (usa formatadores canônicos).
String formatEdgeBadgeLabel(
  TopologyEdgeModel edge, {
  TopologyOverlayMode overlay = TopologyOverlayMode.status,
  double? latencyMs,
}) {
  if (overlay == TopologyOverlayMode.latency && latencyMs != null) {
    return formatLatency(latencyMs);
  }
  if (overlay == TopologyOverlayMode.traffic) {
    final total =
        edge.trafficBps ??
        ((edge.inBps != null || edge.outBps != null)
            ? (edge.inBps ?? 0) + (edge.outBps ?? 0)
            : null);
    if (total != null && total.isFinite) {
      final util = resolveEdgeUtilization(edge);
      if (util != null) {
        return '${formatBps(total)} (${formatPercent(util * 100, digits: 0)})';
      }
      return formatBps(total);
    }
  }
  final total = edge.trafficBps;
  if (total != null && total.isFinite && total > 0) {
    return formatBps(total);
  }
  final speed = resolveEdgeSpeedBps(edge);
  if (speed != null) {
    return formatLinkSpeed(speed);
  }
  return classifyLinkKind(edge.linkType).label;
}

/// Converte confiança `0..1` ou `0..100` em inteiro `0..100`.
int? edgeConfidencePercent(double? confidence) {
  if (confidence == null || !confidence.isFinite) return null;
  final pct = confidence <= 1.0 ? confidence * 100.0 : confidence;
  return pct.clamp(0.0, 100.0).round();
}

/// Classifica percentual de CPU/memória em nível semântico.
TopologyHealthLevel classifyHealthPercent(double percent) {
  if (percent >= kTopologyHealthCriticalPercent) {
    return TopologyHealthLevel.critical;
  }
  if (percent >= kTopologyHealthWarningPercent) {
    return TopologyHealthLevel.high;
  }
  return TopologyHealthLevel.normal;
}

/// Texto resumido de saúde de um nó (`"CPU 37% · RAM 62%"` ou `null`).
String? formatNodeHealthSummary(TopologyNodeModel node) {
  final parts = <String>[];
  var critical = false;
  if (node.cpuPercent != null && node.cpuPercent!.isFinite) {
    final level = classifyHealthPercent(node.cpuPercent!);
    if (level == TopologyHealthLevel.critical) critical = true;
    parts.add('CPU ${node.cpuPercent!.round()}%');
  }
  if (node.memoryPercent != null && node.memoryPercent!.isFinite) {
    final level = classifyHealthPercent(node.memoryPercent!);
    if (level == TopologyHealthLevel.critical) critical = true;
    parts.add('RAM ${node.memoryPercent!.round()}%');
  }
  if (parts.isEmpty) return null;
  final prefix = critical ? '⚠ ' : '';
  return '$prefix${parts.join(' · ')}';
}

/// Resumo agregado de saúde dos nós e enlaces do grafo.
@immutable
class TopologyHealthSummary {
  const TopologyHealthSummary({
    required this.total,
    required this.online,
    required this.warning,
    required this.offline,
    required this.unknown,
    this.problemEdges = 0,
  });

  final int total;
  final int online;
  final int warning;
  final int offline;
  final int unknown;
  final int problemEdges;

  int get problemNodes => warning + offline;
  bool get hasProblems => problemNodes > 0 || problemEdges > 0;

  /// Texto compacto para pílula de status no HUD (`"18 online · 1 alerta · 0 off"`).
  String get compactLabel =>
      '$online online · $warning ${warning == 1 ? 'alerta' : 'alertas'} · $offline off';

  @override
  bool operator ==(Object other) {
    if (identical(this, other)) return true;
    return other is TopologyHealthSummary &&
        other.total == total &&
        other.online == online &&
        other.warning == warning &&
        other.offline == offline &&
        other.unknown == unknown &&
        other.problemEdges == problemEdges;
  }

  @override
  int get hashCode =>
      Object.hash(total, online, warning, offline, unknown, problemEdges);
}

/// Calcula a contagem de saúde (`online`, `warning`, `offline`, `unknown`) do grafo.
TopologyHealthSummary computeTopologyHealthSummary(
  List<TopologyNodeModel> nodes, [
  List<TopologyEdgeModel> edges = const <TopologyEdgeModel>[],
]) {
  var online = 0;
  var warning = 0;
  var offline = 0;
  var unknown = 0;

  for (final node in nodes) {
    switch (classifyNodeStatus(node.status)) {
      case TopologyNodeStatus.online:
        online += 1;
      case TopologyNodeStatus.warning:
        warning += 1;
      case TopologyNodeStatus.offline:
        offline += 1;
      case TopologyNodeStatus.unknown:
        unknown += 1;
    }
  }

  var problemEdges = 0;
  for (final edge in edges) {
    final isDown = classifyEdgeStatus(edge) == TopologyEdgeStatus.down;
    final utilLevel = classifyUtilization(resolveEdgeUtilization(edge));
    if (isDown ||
        utilLevel == TopologyUtilizationLevel.high ||
        utilLevel == TopologyUtilizationLevel.critical) {
      problemEdges += 1;
    }
  }

  return TopologyHealthSummary(
    total: nodes.length,
    online: online,
    warning: warning,
    offline: offline,
    unknown: unknown,
    problemEdges: problemEdges,
  );
}

/// Item declarativo de legenda para renderização em qualquer sheet/dialog.
@immutable
class TopologyLegendEntry {
  const TopologyLegendEntry({
    required this.label,
    required this.tone,
    this.description,
    this.icon,
    this.lineStyle,
    this.strokeWidth,
  });

  final String label;
  final String? description;
  final SemanticTone tone;
  final IconData? icon;
  final TopologyLineStyle? lineStyle;
  final double? strokeWidth;
}

/// Definições de legenda para status de equipamentos.
final List<TopologyLegendEntry> kTopologyNodeStatusLegend =
    List<TopologyLegendEntry>.unmodifiable(<TopologyLegendEntry>[
      for (final status in TopologyNodeStatus.values)
        TopologyLegendEntry(
          label: status.label,
          tone: status.tone,
          icon: status.icon,
        ),
    ]);

/// Definições de legenda para utilização de enlaces.
final List<TopologyLegendEntry> kTopologyUtilizationLegend =
    List<TopologyLegendEntry>.unmodifiable(<TopologyLegendEntry>[
      for (final level in TopologyUtilizationLevel.values)
        TopologyLegendEntry(
          label: level.label,
          tone: level.tone,
          lineStyle: TopologyLineStyle.solid,
        ),
      const TopologyLegendEntry(
        label: 'Enlace fora do ar',
        tone: SemanticTone.error,
        lineStyle: TopologyLineStyle.dashed,
      ),
    ]);

/// Definições de legenda para tipos físicos/lógicos de enlace.
final List<TopologyLegendEntry> kTopologyLinkKindLegend =
    List<TopologyLegendEntry>.unmodifiable(<TopologyLegendEntry>[
      for (final kind in TopologyLinkKind.values)
        TopologyLegendEntry(
          label: kind.label,
          tone: kind.tone,
          icon: kind.icon,
          lineStyle: kind.lineStyle,
        ),
    ]);

/// Definições de legenda para espessura por velocidade nominal.
const List<TopologyLegendEntry> kTopologySpeedTierLegend =
    <TopologyLegendEntry>[
      TopologyLegendEntry(
        label: '≤ 100 Mbps',
        description: 'Traço fino',
        tone: SemanticTone.primary,
        lineStyle: TopologyLineStyle.solid,
        strokeWidth: 1.8,
      ),
      TopologyLegendEntry(
        label: '1 Gbps',
        description: 'Traço médio',
        tone: SemanticTone.primary,
        lineStyle: TopologyLineStyle.solid,
        strokeWidth: 2.5,
      ),
      TopologyLegendEntry(
        label: '≥ 10 Gbps',
        description: 'Traço espesso',
        tone: SemanticTone.primary,
        lineStyle: TopologyLineStyle.solid,
        strokeWidth: 3.5,
      ),
    ];
