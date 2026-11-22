import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/core/utils/mdi_icons.dart';
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
      supportsQrCode: parseBool(json['supportsQrCode']),
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
      id: parseIntOrNull(json['id']) ?? 0,
      networkId: parseIntOrNull(json['networkId']) ?? 0,
      interfaceName: (json['interfaceName'] as String?) ?? 'wg0',
      listenPort: parseIntOrNull(json['listenPort']) ?? 51820,
      publicEndpoint: json['publicEndpoint'] as String?,
      publicKey: (json['publicKey'] as String?) ?? '',
      allowPeerToPeer: parseBool(json['allowPeerToPeer']),
      mtu: parseIntOrNull(json['mtu']) ?? 1420,
      dnsServers: json['dnsServers'] as String?,
      active: parseBool(json['active']),
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
    return VpnServerStateModel(
      configured: parseBool(json['configured']),
      server: rawServer is Map
          ? VpnServerConfig.fromJson(asJsonMap(rawServer))
          : null,
      cidr: json['cidr'] as String?,
      serverAddress: json['serverAddress'] as String?,
      peersTotal: parseIntOrNull(json['peersTotal']) ?? 0,
      peersConnected: parseIntOrNull(json['peersConnected']) ?? 0,
      bytesRx: parseIntOrNull(json['bytesRx']) ?? 0,
      bytesTx: parseIntOrNull(json['bytesTx']) ?? 0,
      persistentKeepalive: parseIntOrNull(json['persistentKeepalive']) ?? 25,
      profiles: asJsonList<VpnProfileCard>(
        json['profiles'],
        VpnProfileCard.fromJson,
      ),
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
    return switch (level) {
      'success' => SemanticTone.success,
      'warning' => SemanticTone.warning,
      'error' => SemanticTone.error,
      _ => SemanticTone.info,
    };
  }

  factory VpnPreflightResult.fromJson(Map<String, dynamic> json) {
    return VpnPreflightResult(
      status: (json['status'] as String?) ?? '',
      level: (json['level'] as String?) ?? 'info',
      message: (json['message'] as String?) ?? '',
      recommendation: json['recommendation'] as String?,
      publicIp: json['publicIp'] as String?,
      resolvedIp: json['resolvedIp'] as String?,
      port: parseIntOrNull(json['port']) ?? 51820,
      isCgnat: parseBool(json['isCgnat']),
      behindNat: parseBool(json['behindNat']),
      verified: parseBool(json['verified']),
    );
  }
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
      overridden: parseBool(json['overridden']),
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
      id: parseIntOrNull(json['id']) ?? 0,
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

IconData _vpnSpecificMdiFallback(String raw) {
  return switch (raw.trim().toLowerCase()) {
    'mdi-linux' => Icons.dns_outlined,
    'mdi-microsoft-windows' => Icons.desktop_windows_outlined,
    'mdi-apple' => Icons.laptop_mac_outlined,
    'mdi-android' => Icons.smartphone_outlined,
    'mdi-router-network' => Icons.router_outlined,
    'mdi-access-point-network' => Icons.wifi,
    'mdi-shield-lock-outline' => Icons.vpn_lock_outlined,
    'mdi-apps' => Icons.apps,
    'mdi-database-search-outline' => Icons.storage_outlined,
    'mdi-monitor-eye' => Icons.monitor_heart_outlined,
    'mdi-network-outline' => Icons.hub_outlined,
    'mdi-speedometer' => Icons.speed,
    'mdi-firewall' => Icons.security_outlined,
    _ => Icons.devices_other_outlined,
  };
}

IconData vpnProfileIconData(String profile, List<VpnProfileCard> profiles) {
  var raw = 'mdi-devices';
  for (final option in profiles) {
    if (option.profile == profile) {
      raw = option.icon;
      break;
    }
  }
  return iconFromMdiName(raw, fallback: _vpnSpecificMdiFallback(raw));
}
