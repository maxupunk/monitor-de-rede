import 'package:flutter/material.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Estado da conexão de um peer WireGuard (`VpnPeerConnectionStatus`).
enum VpnConnectionStatus {
  connected(
    'connected',
    'Conectado',
    SemanticTone.success,
    Icons.check_circle_outline,
  ),
  unstable(
    'unstable',
    'Instável',
    SemanticTone.warning,
    Icons.warning_amber_outlined,
  ),
  disconnected(
    'disconnected',
    'Desconectado',
    SemanticTone.error,
    Icons.cancel_outlined,
  ),
  awaiting(
    'awaiting',
    'Aguardando',
    SemanticTone.secondary,
    Icons.hourglass_empty,
  );

  const VpnConnectionStatus(this.id, this.label, this.tone, this.icon);

  final String id;
  final String label;
  final SemanticTone tone;
  final IconData icon;

  static VpnConnectionStatus fromRaw(Object? raw) {
    final text = raw?.toString() ?? '';
    for (final value in VpnConnectionStatus.values) {
      if (value.id == text) return value;
    }
    return VpnConnectionStatus.awaiting;
  }
}

/// Cartão de perfil de dispositivo VPN retornado pelo backend (`ProfileCard`).
@immutable
class VpnProfileCard {
  const VpnProfileCard({
    required this.profile,
    required this.label,
    required this.icon,
    required this.supportsQrCode,
  });

  final String profile;
  final String label;
  final String icon;
  final bool supportsQrCode;

  factory VpnProfileCard.fromJson(Map<String, dynamic> json) {
    return VpnProfileCard(
      profile: (json['profile'] as String?) ?? '',
      label: (json['label'] as String?) ?? (json['profile'] as String?) ?? '',
      icon: (json['icon'] as String?) ?? 'mdi-devices',
      supportsQrCode: json['supportsQrCode'] == true,
    );
  }
}

/// Configuração do servidor WireGuard (`VpnServerResponse`).
@immutable
class VpnServerConfig {
  const VpnServerConfig({
    required this.id,
    required this.networkId,
    required this.interfaceName,
    required this.listenPort,
    this.publicEndpoint,
    required this.publicKey,
    required this.allowPeerToPeer,
    required this.mtu,
    this.dnsServers,
    required this.active,
    this.lastSyncedAt,
    this.createdAt,
    this.updatedAt,
  });

  final int id;
  final int networkId;
  final String interfaceName;
  final int listenPort;
  final String? publicEndpoint;
  final String publicKey;
  final bool allowPeerToPeer;
  final int mtu;
  final String? dnsServers;
  final bool active;
  final String? lastSyncedAt;
  final String? createdAt;
  final String? updatedAt;

  factory VpnServerConfig.fromJson(Map<String, dynamic> json) {
    return VpnServerConfig(
      id: (json['id'] as num?)?.toInt() ?? 0,
      networkId: (json['networkId'] as num?)?.toInt() ?? 0,
      interfaceName: (json['interfaceName'] as String?) ?? 'wg0',
      listenPort: (json['listenPort'] as num?)?.toInt() ?? 51820,
      publicEndpoint: json['publicEndpoint'] as String?,
      publicKey: (json['publicKey'] as String?) ?? '',
      allowPeerToPeer: json['allowPeerToPeer'] == true,
      mtu: (json['mtu'] as num?)?.toInt() ?? 1420,
      dnsServers: json['dnsServers'] as String?,
      active: json['active'] == true,
      lastSyncedAt: json['lastSyncedAt'] as String?,
      createdAt: json['createdAt'] as String?,
      updatedAt: json['updatedAt'] as String?,
    );
  }
}

/// Estado agregado do servidor WireGuard (`VpnServerStateResponse`).
@immutable
class VpnServerStateModel {
  const VpnServerStateModel({
    required this.configured,
    this.server,
    this.cidr,
    this.serverAddress,
    required this.peersTotal,
    required this.peersConnected,
    required this.bytesRx,
    required this.bytesTx,
    required this.persistentKeepalive,
    this.profiles = const <VpnProfileCard>[],
  });

  final bool configured;
  final VpnServerConfig? server;
  final String? cidr;
  final String? serverAddress;
  final int peersTotal;
  final int peersConnected;
  final int bytesRx;
  final int bytesTx;
  final int persistentKeepalive;
  final List<VpnProfileCard> profiles;

  factory VpnServerStateModel.fromJson(Map<String, dynamic> json) {
    final rawServer = json['server'];
    final rawProfiles = json['profiles'];
    return VpnServerStateModel(
      configured: json['configured'] == true,
      server: rawServer is Map
          ? VpnServerConfig.fromJson(Map<String, dynamic>.from(rawServer))
          : null,
      cidr: json['cidr'] as String?,
      serverAddress: json['serverAddress'] as String?,
      peersTotal: (json['peersTotal'] as num?)?.toInt() ?? 0,
      peersConnected: (json['peersConnected'] as num?)?.toInt() ?? 0,
      bytesRx: (json['bytesRx'] as num?)?.toInt() ?? 0,
      bytesTx: (json['bytesTx'] as num?)?.toInt() ?? 0,
      persistentKeepalive: (json['persistentKeepalive'] as num?)?.toInt() ?? 25,
      profiles: rawProfiles is List
          ? rawProfiles
                .whereType<Map>()
                .map(
                  (item) =>
                      VpnProfileCard.fromJson(Map<String, dynamic>.from(item)),
                )
                .toList(growable: false)
          : const <VpnProfileCard>[],
    );
  }
}

/// Resultado do teste de acessibilidade externa (`PreflightResult`).
@immutable
class VpnPreflightResult {
  const VpnPreflightResult({
    required this.status,
    required this.level,
    required this.message,
    this.recommendation,
    this.publicIp,
    this.resolvedIp,
    required this.port,
    required this.isCgnat,
    required this.behindNat,
    required this.verified,
  });

  final String status;
  final String level;
  final String message;
  final String? recommendation;
  final String? publicIp;
  final String? resolvedIp;
  final int port;
  final bool isCgnat;
  final bool behindNat;
  final bool verified;

  SemanticTone get tone {
    switch (level) {
      case 'success':
        return SemanticTone.success;
      case 'warning':
        return SemanticTone.warning;
      case 'error':
        return SemanticTone.error;
      default:
        return SemanticTone.info;
    }
  }

  factory VpnPreflightResult.fromJson(Map<String, dynamic> json) {
    return VpnPreflightResult(
      status: (json['status'] as String?) ?? '',
      level: (json['level'] as String?) ?? 'info',
      message: (json['message'] as String?) ?? '',
      recommendation: json['recommendation'] as String?,
      publicIp: json['publicIp'] as String?,
      resolvedIp: json['resolvedIp'] as String?,
      port: (json['port'] as num?)?.toInt() ?? 51820,
      isCgnat: json['isCgnat'] == true,
      behindNat: json['behindNat'] == true,
      verified: json['verified'] == true,
    );
  }
}

/// Dispositivo vinculado a um peer VPN (`VpnPeerDeviceView`).
@immutable
class VpnPeerDevice {
  const VpnPeerDevice({
    required this.id,
    required this.name,
    required this.ipAddress,
    required this.snmpEnabled,
    required this.status,
  });

  final int id;
  final String name;
  final String ipAddress;
  final bool snmpEnabled;
  final String status;

  factory VpnPeerDevice.fromJson(Map<String, dynamic> json) {
    return VpnPeerDevice(
      id: (json['id'] as num?)?.toInt() ?? 0,
      name: (json['name'] as String?) ?? '',
      ipAddress: (json['ipAddress'] as String?) ?? '',
      snmpEnabled: json['snmpEnabled'] == true,
      status: (json['status'] as String?) ?? 'unknown',
    );
  }
}

/// Peer WireGuard listado (`VpnPeerListItem`).
@immutable
class VpnPeer {
  const VpnPeer({
    this.device,
    required this.id,
    required this.vpnServerId,
    required this.deviceId,
    required this.publicKey,
    required this.deviceProfile,
    required this.persistentKeepalive,
    this.lastHandshakeAt,
    this.lastSeenAt,
    required this.bytesRx,
    required this.bytesTx,
    required this.enabled,
    this.createdAt,
    this.updatedAt,
    required this.connectionStatus,
    this.needsFirewallHint = false,
    this.pingOutsideTunnel = false,
    this.pingMonitorId,
  });

  final VpnPeerDevice? device;
  final int id;
  final int vpnServerId;
  final int deviceId;
  final String publicKey;
  final String deviceProfile;
  final int persistentKeepalive;
  final String? lastHandshakeAt;
  final String? lastSeenAt;
  final int bytesRx;
  final int bytesTx;
  final bool enabled;
  final String? createdAt;
  final String? updatedAt;
  final VpnConnectionStatus connectionStatus;
  final bool needsFirewallHint;
  final bool pingOutsideTunnel;
  final int? pingMonitorId;

  String get displayName =>
      device?.name.isNotEmpty == true ? device!.name : 'Peer #$id';
  String get ipAddress => device?.ipAddress ?? '—';

  VpnPeer copyWith({
    VpnPeerDevice? device,
    VpnConnectionStatus? connectionStatus,
    String? lastHandshakeAt,
    bool clearLastHandshakeAt = false,
    String? lastSeenAt,
    bool clearLastSeenAt = false,
    int? bytesRx,
    int? bytesTx,
    bool? needsFirewallHint,
    bool? pingOutsideTunnel,
    int? pingMonitorId,
    bool clearPingMonitorId = false,
  }) {
    return VpnPeer(
      device: device ?? this.device,
      id: id,
      vpnServerId: vpnServerId,
      deviceId: deviceId,
      publicKey: publicKey,
      deviceProfile: deviceProfile,
      persistentKeepalive: persistentKeepalive,
      lastHandshakeAt: clearLastHandshakeAt
          ? null
          : (lastHandshakeAt ?? this.lastHandshakeAt),
      lastSeenAt: clearLastSeenAt ? null : (lastSeenAt ?? this.lastSeenAt),
      bytesRx: bytesRx ?? this.bytesRx,
      bytesTx: bytesTx ?? this.bytesTx,
      enabled: enabled,
      createdAt: createdAt,
      updatedAt: updatedAt,
      connectionStatus: connectionStatus ?? this.connectionStatus,
      needsFirewallHint: needsFirewallHint ?? this.needsFirewallHint,
      pingOutsideTunnel: pingOutsideTunnel ?? this.pingOutsideTunnel,
      pingMonitorId: clearPingMonitorId
          ? null
          : (pingMonitorId ?? this.pingMonitorId),
    );
  }

  factory VpnPeer.fromJson(Map<String, dynamic> json) {
    final rawDevice = json['device'];
    return VpnPeer(
      device: rawDevice is Map
          ? VpnPeerDevice.fromJson(Map<String, dynamic>.from(rawDevice))
          : null,
      id: (json['id'] as num?)?.toInt() ?? 0,
      vpnServerId: (json['vpnServerId'] as num?)?.toInt() ?? 0,
      deviceId: (json['deviceId'] as num?)?.toInt() ?? 0,
      publicKey: (json['publicKey'] as String?) ?? '',
      deviceProfile: (json['deviceProfile'] as String?) ?? 'linux',
      persistentKeepalive: (json['persistentKeepalive'] as num?)?.toInt() ?? 25,
      lastHandshakeAt: json['lastHandshakeAt'] as String?,
      lastSeenAt: json['lastSeenAt'] as String?,
      bytesRx: (json['bytesRx'] as num?)?.toInt() ?? 0,
      bytesTx: (json['bytesTx'] as num?)?.toInt() ?? 0,
      enabled: json['enabled'] != false,
      createdAt: json['createdAt'] as String?,
      updatedAt: json['updatedAt'] as String?,
      connectionStatus: VpnConnectionStatus.fromRaw(json['connectionStatus']),
      needsFirewallHint: json['needsFirewallHint'] == true,
      pingOutsideTunnel: json['pingOutsideTunnel'] == true,
      pingMonitorId: (json['pingMonitorId'] as num?)?.toInt(),
    );
  }
}

/// Política do agente remoto embarcada no instalador VPN (`InstallPolicyView`).
@immutable
class VpnInstallPolicy {
  const VpnInstallPolicy({
    this.allow = const <String>[],
    this.databaseTargets = '',
  });

  final List<String> allow;
  final String databaseTargets;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'allow': allow,
    'databaseTargets': databaseTargets,
  };

  VpnInstallPolicy copyWith({List<String>? allow, String? databaseTargets}) {
    return VpnInstallPolicy(
      allow: allow ?? this.allow,
      databaseTargets: databaseTargets ?? this.databaseTargets,
    );
  }

  factory VpnInstallPolicy.fromJson(Map<String, dynamic> json) {
    final rawAllow = json['allow'];
    return VpnInstallPolicy(
      allow: rawAllow is List
          ? rawAllow.map((item) => item.toString()).toList(growable: false)
          : const <String>[],
      databaseTargets: (json['databaseTargets'] as String?) ?? '',
    );
  }
}

/// Permissões conhecidas do agente remoto (`AGENT_ALLOW`, ADR 011/013/021/022).
const List<({String id, String label, String description})>
vpnAgentPermissions = <({String id, String label, String description})>[
  (
    id: 'metrics',
    label: 'Métricas do host',
    description: 'CPU, memória, disco e rede do servidor.',
  ),
  (
    id: 'docker_read',
    label: 'Docker (leitura)',
    description: 'Listar containers, logs, imagens e estatísticas.',
  ),
  (
    id: 'docker_control',
    label: 'Docker (controle)',
    description: 'Iniciar, parar e reiniciar containers não protegidos.',
  ),
  (
    id: 'deploy',
    label: 'Deploy contínuo',
    description: 'Aplicar revisões Compose em AGENT_STACKS_DIR.',
  ),
  (
    id: 'build',
    label: 'Compilar imagens',
    description: 'Construir imagens Docker no builder dedicado.',
  ),
  (
    id: 'database',
    label: 'Ponte de banco',
    description: 'Túnel de bytes para backups de bancos na rede do agente.',
  ),
  (
    id: 'device_io',
    label: 'Acesso a equipamentos',
    description:
        'Encaminhar SSH/Telnet/HTTP de plugins para dispositivos remotos.',
  ),
];

@immutable
class VpnArtifactSummaryItem {
  const VpnArtifactSummaryItem({required this.label, required this.value});

  final String label;
  final String value;

  factory VpnArtifactSummaryItem.fromJson(Map<String, dynamic> json) {
    return VpnArtifactSummaryItem(
      label: (json['label'] as String?) ?? '',
      value: (json['value'] as String?) ?? '',
    );
  }
}

@immutable
class VpnArtifactVariant {
  const VpnArtifactVariant({
    required this.id,
    required this.label,
    this.hint,
    this.icon,
    required this.fileName,
    required this.language,
    required this.content,
    this.instructions = const <String>[],
  });

  final String id;
  final String label;
  final String? hint;
  final String? icon;
  final String fileName;
  final String language;
  final String content;
  final List<String> instructions;

  factory VpnArtifactVariant.fromJson(Map<String, dynamic> json) {
    final rawInstructions = json['instructions'];
    return VpnArtifactVariant(
      id: (json['id'] as String?) ?? '',
      label: (json['label'] as String?) ?? '',
      hint: json['hint'] as String?,
      icon: json['icon'] as String?,
      fileName: (json['fileName'] as String?) ?? 'wg0.conf',
      language: (json['language'] as String?) ?? 'ini',
      content: (json['content'] as String?) ?? '',
      instructions: rawInstructions is List
          ? rawInstructions.map((e) => e.toString()).toList(growable: false)
          : const <String>[],
    );
  }
}

@immutable
class VpnArtifactAgentInstall {
  const VpnArtifactAgentInstall({
    required this.enrollCode,
    required this.policy,
  });

  final String enrollCode;
  final VpnInstallPolicy policy;

  factory VpnArtifactAgentInstall.fromJson(Map<String, dynamic> json) {
    final rawPolicy = json['policy'];
    return VpnArtifactAgentInstall(
      enrollCode: (json['enrollCode'] as String?) ?? '',
      policy: rawPolicy is Map
          ? VpnInstallPolicy.fromJson(Map<String, dynamic>.from(rawPolicy))
          : const VpnInstallPolicy(),
    );
  }
}

/// Artefato de provisionamento WireGuard (`SerializedVpnArtifact`).
///
/// Regra de segurança (cofre de uso único):
/// A chave privada aparece apenas na criação ou rotação. Leituras posteriores
/// vêm com `CHAVE-PRIVADA-INDISPONIVEL` no conteúdo e `qrSvg == null`.
@immutable
class VpnArtifact {
  const VpnArtifact({
    this.peerId,
    this.qrSvg,
    required this.profile,
    required this.label,
    required this.delivery,
    required this.fileName,
    required this.language,
    required this.content,
    this.instructions = const <String>[],
    required this.supportsQrCode,
    this.summary = const <VpnArtifactSummaryItem>[],
    this.variants = const <VpnArtifactVariant>[],
    this.agentInstall,
  });

  static const String unavailableMarker = 'CHAVE-PRIVADA-INDISPONIVEL';

  final int? peerId;
  final String? qrSvg;
  final String profile;
  final String label;
  final String delivery;
  final String fileName;
  final String language;
  final String content;
  final List<String> instructions;
  final bool supportsQrCode;
  final List<VpnArtifactSummaryItem> summary;
  final List<VpnArtifactVariant> variants;
  final VpnArtifactAgentInstall? agentInstall;

  /// Verdadeiro apenas quando o cofre ainda continha a chave privada nesta leitura.
  bool get hasPrivateKey =>
      content.trim().isNotEmpty && !content.contains(unavailableMarker);

  VpnArtifact copyWith({
    VpnArtifactAgentInstall? agentInstall,
    List<VpnArtifactVariant>? variants,
  }) {
    return VpnArtifact(
      peerId: peerId,
      qrSvg: qrSvg,
      profile: profile,
      label: label,
      delivery: delivery,
      fileName: fileName,
      language: language,
      content: content,
      instructions: instructions,
      supportsQrCode: supportsQrCode,
      summary: summary,
      variants: variants ?? this.variants,
      agentInstall: agentInstall ?? this.agentInstall,
    );
  }

  factory VpnArtifact.fromJson(Map<String, dynamic> json) {
    final rawSummary = json['summary'];
    final rawVariants = json['variants'];
    final rawInstructions = json['instructions'];
    final rawAgent = json['agentInstall'];
    return VpnArtifact(
      peerId: (json['peerId'] as num?)?.toInt(),
      qrSvg: json['qrSvg'] as String?,
      profile: (json['profile'] as String?) ?? '',
      label: (json['label'] as String?) ?? '',
      delivery: (json['delivery'] as String?) ?? 'file',
      fileName: (json['fileName'] as String?) ?? 'wg0.conf',
      language: (json['language'] as String?) ?? 'ini',
      content: (json['content'] as String?) ?? '',
      instructions: rawInstructions is List
          ? rawInstructions.map((e) => e.toString()).toList(growable: false)
          : const <String>[],
      supportsQrCode: json['supportsQrCode'] == true,
      summary: rawSummary is List
          ? rawSummary
                .whereType<Map>()
                .map(
                  (e) => VpnArtifactSummaryItem.fromJson(
                    Map<String, dynamic>.from(e),
                  ),
                )
                .toList(growable: false)
          : const <VpnArtifactSummaryItem>[],
      variants: rawVariants is List
          ? rawVariants
                .whereType<Map>()
                .map(
                  (e) =>
                      VpnArtifactVariant.fromJson(Map<String, dynamic>.from(e)),
                )
                .toList(growable: false)
          : const <VpnArtifactVariant>[],
      agentInstall: rawAgent is Map
          ? VpnArtifactAgentInstall.fromJson(
              Map<String, dynamic>.from(rawAgent),
            )
          : null,
    );
  }
}

/// Payload de criação de peer (`POST /api/vpn/peers`).
@immutable
class CreateVpnPeerPayload {
  const CreateVpnPeerPayload({
    required this.name,
    required this.profile,
    this.ipAddress,
    this.siteId,
    this.snmpEnabled = false,
    this.snmpCommunity,
    this.description,
    this.installAgent = false,
    this.agentId,
    this.agentPolicy,
  });

  final String name;
  final String profile;
  final String? ipAddress;
  final int? siteId;
  final bool snmpEnabled;
  final String? snmpCommunity;
  final String? description;
  final bool installAgent;
  final int? agentId;
  final VpnInstallPolicy? agentPolicy;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'name': name.trim(),
    'profile': profile,
    if (ipAddress != null && ipAddress!.trim().isNotEmpty)
      'ipAddress': ipAddress!.trim(),
    if (siteId != null) 'siteId': siteId,
    'snmpEnabled': snmpEnabled,
    if (snmpEnabled &&
        snmpCommunity != null &&
        snmpCommunity!.trim().isNotEmpty)
      'snmpCommunity': snmpCommunity!.trim(),
    if (description != null && description!.trim().isNotEmpty)
      'description': description!.trim(),
    if (installAgent) 'installAgent': true,
    if (agentId != null) 'agentId': agentId,
    if (installAgent && agentPolicy != null)
      'agentPolicy': agentPolicy!.toJson(),
  };
}

/// Endereço utilizável da central (`GET /api/server-addresses`).
@immutable
class VpnServerAddressHint {
  const VpnServerAddressHint({
    required this.id,
    required this.kind,
    required this.label,
    required this.address,
    this.overridden = false,
  });

  final String id;
  final String kind;
  final String label;
  final String address;
  final bool overridden;

  factory VpnServerAddressHint.fromJson(Map<String, dynamic> json) {
    return VpnServerAddressHint(
      id: (json['id'] as String?) ?? '',
      kind: (json['kind'] as String?) ?? '',
      label: (json['label'] as String?) ?? '',
      address: (json['address'] as String?) ?? '',
      overridden: json['overridden'] == true,
    );
  }
}

/// Site simplificado para seleção no wizard VPN.
@immutable
class VpnSiteOption {
  const VpnSiteOption({required this.id, required this.name});

  final int id;
  final String name;

  factory VpnSiteOption.fromJson(Map<String, dynamic> json) {
    return VpnSiteOption(
      id: (json['id'] as num?)?.toInt() ?? 0,
      name: (json['name'] as String?) ?? '',
    );
  }
}

/// Ponto de métrica para o gráfico de tráfego VPN em `DeviceVpnTab`.
@immutable
class VpnMetricSample {
  const VpnMetricSample({
    required this.metricName,
    required this.value,
    required this.recordedAt,
  });

  final String metricName;
  final double value;
  final DateTime recordedAt;
}

/// Apresentação derivada do registro do backend (`state.profiles`), sem catálogo paralelo.
String vpnProfileLabel(String profile, List<VpnProfileCard> profiles) {
  for (final option in profiles) {
    if (option.profile == profile) return option.label;
  }
  return profile;
}

bool vpnProfileSupportsQrCode(String profile, List<VpnProfileCard> profiles) {
  for (final option in profiles) {
    if (option.profile == profile) return option.supportsQrCode;
  }
  return false;
}

IconData vpnProfileIconData(String profile, List<VpnProfileCard> profiles) {
  var raw = 'mdi-devices';
  for (final option in profiles) {
    if (option.profile == profile) {
      raw = option.icon;
      break;
    }
  }
  return iconFromMdiName(raw);
}

/// Mapeia ícones MDI comuns do backend para ícones nativos Material Design 3.
IconData iconFromMdiName(String? name) {
  switch (name) {
    case 'mdi-linux':
    case 'mdi-server':
    case 'mdi-server-network':
      return Icons.dns_outlined;
    case 'mdi-microsoft-windows':
      return Icons.desktop_windows_outlined;
    case 'mdi-apple':
      return Icons.laptop_mac_outlined;
    case 'mdi-android':
    case 'mdi-cellphone':
      return Icons.smartphone_outlined;
    case 'mdi-router-wireless':
    case 'mdi-router-network':
    case 'mdi-router':
      return Icons.router_outlined;
    case 'mdi-wifi':
    case 'mdi-access-point':
    case 'mdi-access-point-network':
      return Icons.wifi;
    case 'mdi-shield-lock-outline':
    case 'mdi-vpn':
      return Icons.vpn_lock_outlined;
    case 'mdi-puzzle-outline':
    case 'mdi-puzzle':
      return Icons.extension_outlined;
    case 'mdi-apps':
      return Icons.apps;
    case 'mdi-lan':
      return Icons.lan_outlined;
    case 'mdi-database':
    case 'mdi-database-search-outline':
      return Icons.storage_outlined;
    case 'mdi-docker':
      return Icons.widgets_outlined;
    case 'mdi-monitor-eye':
      return Icons.monitor_heart_outlined;
    case 'mdi-network-outline':
      return Icons.hub_outlined;
    case 'mdi-speedometer':
      return Icons.speed;
    case 'mdi-firewall':
      return Icons.security_outlined;
    default:
      return Icons.devices_other_outlined;
  }
}

/// Intervalo numérico de um CIDR IPv4 (para validação e detecção de sobreposição).
@immutable
class ParsedIpv4Cidr {
  const ParsedIpv4Cidr({
    required this.network,
    required this.broadcast,
    required this.prefix,
  });

  final int network;
  final int broadcast;
  final int prefix;
}

ParsedIpv4Cidr? parseIpv4Cidr(String raw) {
  final trimmed = raw.trim();
  final parts = trimmed.split('/');
  if (parts.length != 2) return null;
  final octets = parts[0].split('.');
  if (octets.length != 4) return null;
  final prefix = int.tryParse(parts[1]);
  if (prefix == null || prefix < 8 || prefix > 30) return null;

  var ip = 0;
  for (final octet in octets) {
    final value = int.tryParse(octet);
    if (value == null || value < 0 || value > 255) return null;
    ip = (ip << 8) | value;
  }
  final mask = prefix == 0 ? 0 : ((0xFFFFFFFF << (32 - prefix)) & 0xFFFFFFFF);
  final network = ip & mask;
  final broadcast = network | (~mask & 0xFFFFFFFF);
  return ParsedIpv4Cidr(network: network, broadcast: broadcast, prefix: prefix);
}

bool cidrOverlaps(String a, String b) {
  final pa = parseIpv4Cidr(a);
  final pb = parseIpv4Cidr(b);
  if (pa == null || pb == null) return false;
  return pa.network <= pb.broadcast && pb.network <= pa.broadcast;
}

const List<String> _candidateVpnCidrs = <String>[
  '10.8.0.0/24',
  '10.9.0.0/24',
  '10.10.10.0/24',
  '10.66.66.0/24',
  '10.100.0.0/24',
  '172.24.0.0/24',
  '172.28.0.0/24',
  '192.168.250.0/24',
];

/// Sugere faixas CIDR privadas que não colidem com redes já cadastradas.
List<String> freeVpnCidrs(Iterable<String> existingCidrs) {
  final existing = existingCidrs.where((c) => c.trim().isNotEmpty).toList();
  return _candidateVpnCidrs
      .where(
        (candidate) => !existing.any((other) => cidrOverlaps(candidate, other)),
      )
      .toList(growable: false);
}
