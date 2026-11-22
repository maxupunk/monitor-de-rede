import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/vpn/models/vpn_server_models.dart';

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
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] as String?) ?? '',
      ipAddress: (json['ipAddress'] as String?) ?? '',
      snmpEnabled: parseBool(json['snmpEnabled']),
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
          ? VpnPeerDevice.fromJson(asJsonMap(rawDevice))
          : null,
      id: parseIntOrNull(json['id']) ?? 0,
      vpnServerId: parseIntOrNull(json['vpnServerId']) ?? 0,
      deviceId: parseIntOrNull(json['deviceId']) ?? 0,
      publicKey: (json['publicKey'] as String?) ?? '',
      deviceProfile: (json['deviceProfile'] as String?) ?? 'linux',
      persistentKeepalive: parseIntOrNull(json['persistentKeepalive']) ?? 25,
      lastHandshakeAt: json['lastHandshakeAt'] as String?,
      lastSeenAt: json['lastSeenAt'] as String?,
      bytesRx: parseIntOrNull(json['bytesRx']) ?? 0,
      bytesTx: parseIntOrNull(json['bytesTx']) ?? 0,
      enabled: parseBool(json['enabled'], fallback: true),
      createdAt: json['createdAt'] as String?,
      updatedAt: json['updatedAt'] as String?,
      connectionStatus: VpnConnectionStatus.fromRaw(json['connectionStatus']),
      needsFirewallHint: parseBool(json['needsFirewallHint']),
      pingOutsideTunnel: parseBool(json['pingOutsideTunnel']),
      pingMonitorId: parseIntOrNull(json['pingMonitorId']),
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
    return VpnInstallPolicy(
      allow: asStringList(json['allow']),
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
    return VpnArtifactVariant(
      id: (json['id'] as String?) ?? '',
      label: (json['label'] as String?) ?? '',
      hint: json['hint'] as String?,
      icon: json['icon'] as String?,
      fileName: (json['fileName'] as String?) ?? 'wg0.conf',
      language: (json['language'] as String?) ?? 'ini',
      content: (json['content'] as String?) ?? '',
      instructions: asStringList(json['instructions']),
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
          ? VpnInstallPolicy.fromJson(asJsonMap(rawPolicy))
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
    final rawAgent = json['agentInstall'];
    return VpnArtifact(
      peerId: parseIntOrNull(json['peerId']),
      qrSvg: json['qrSvg'] as String?,
      profile: (json['profile'] as String?) ?? '',
      label: (json['label'] as String?) ?? '',
      delivery: (json['delivery'] as String?) ?? 'file',
      fileName: (json['fileName'] as String?) ?? 'wg0.conf',
      language: (json['language'] as String?) ?? 'ini',
      content: (json['content'] as String?) ?? '',
      instructions: asStringList(json['instructions']),
      supportsQrCode: parseBool(json['supportsQrCode']),
      summary: asJsonList<VpnArtifactSummaryItem>(
        json['summary'],
        VpnArtifactSummaryItem.fromJson,
      ),
      variants: asJsonList<VpnArtifactVariant>(
        json['variants'],
        VpnArtifactVariant.fromJson,
      ),
      agentInstall: rawAgent is Map
          ? VpnArtifactAgentInstall.fromJson(asJsonMap(rawAgent))
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
