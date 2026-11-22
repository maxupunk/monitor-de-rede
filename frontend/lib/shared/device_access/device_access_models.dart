import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

/// Porta TCP padrão por protocolo de transporte de acesso a equipamento.
int defaultTransportPort(String kind, {bool https = false}) {
  return switch (kind.trim().toLowerCase()) {
    'telnet' => 23,
    'http' => https ? 443 : 80,
    _ => 22,
  };
}

/// Metadados de chave SSH dedicada provisionada no equipamento (ADR 016).
@immutable
class DedicatedAccessInfo {
  const DedicatedAccessInfo({
    required this.username,
    this.fingerprint,
    this.hostKey,
    this.platform,
    this.rotatedAt,
    this.publicKey,
    this.keyComment,
  });

  final String username;
  final String? fingerprint;
  final String? hostKey;
  final String? platform;
  final String? rotatedAt;
  final String? publicKey;
  final String? keyComment;

  factory DedicatedAccessInfo.fromJson(Map<String, dynamic> json) {
    return DedicatedAccessInfo(
      username: (json['username'] as String?) ?? '',
      fingerprint: (json['keyFingerprint'] ?? json['fingerprint'])?.toString(),
      hostKey: json['hostKey']?.toString(),
      platform: json['platform']?.toString(),
      rotatedAt: (json['provisionedAt'] ?? json['rotatedAt'])?.toString(),
      publicKey: json['publicKey']?.toString(),
      keyComment: json['keyComment']?.toString(),
    );
  }
}

typedef DedicatedAccess = DedicatedAccessInfo;

/// Visão unificada de credencial de transporte (`ssh` | `telnet` | `http`) em `/devices/:id/access`.
@immutable
class DeviceCredentialView {
  const DeviceCredentialView({
    required this.kind,
    this.username = '',
    this.port,
    this.https = false,
    this.storage = 'ask',
    this.viaProbeId,
    this.hasSecret = false,
    this.sessionActive = false,
    this.sessionExpiresAt,
    this.dedicatedInfo,
    this.dedicatedFlag = false,
    this.fingerprint,
    this.updatedAt,
  });

  final String kind;
  final String username;
  final int? port;
  final bool https;
  final String storage;
  final int? viaProbeId;
  final bool hasSecret;
  final bool sessionActive;
  final String? sessionExpiresAt;
  final DedicatedAccessInfo? dedicatedInfo;
  final bool dedicatedFlag;
  final String? fingerprint;
  final String? updatedAt;

  bool get useHttps => https;
  bool get dedicated =>
      dedicatedInfo != null || storage == 'key' || dedicatedFlag;
  String? get effectiveFingerprint => fingerprint ?? dedicatedInfo?.fingerprint;

  factory DeviceCredentialView.fromJson(Map<String, dynamic> json) {
    final dedicatedRaw = json['dedicated'];
    final parsedDedicated = dedicatedRaw is Map
        ? DedicatedAccessInfo.fromJson(asJsonMap(dedicatedRaw))
        : null;
    final isDedicated =
        parsedDedicated != null ||
        dedicatedRaw == true ||
        json['authMode'] == 'dedicated_key' ||
        json['storage'] == 'key';
    final isSessionActive = json['sessionActive'] == true;

    return DeviceCredentialView(
      kind: (json['kind'] ?? json['transport'] ?? 'ssh').toString(),
      username: (json['username'] ?? parsedDedicated?.username ?? '')
          .toString(),
      port: parseIntOrNull(json['port']),
      https: json['https'] == true || json['useHttps'] == true,
      storage: (json['storage'] as String?) ?? (isDedicated ? 'key' : 'ask'),
      viaProbeId: parseIntOrNull(json['viaProbeId']),
      hasSecret:
          isSessionActive ||
          isDedicated ||
          (json['hasSecret'] ?? json['hasPassword'] ?? json['configured']) ==
              true,
      sessionActive: isSessionActive,
      sessionExpiresAt: json['sessionExpiresAt']?.toString(),
      dedicatedInfo: parsedDedicated,
      dedicatedFlag: isDedicated,
      fingerprint: (json['fingerprint'] ?? parsedDedicated?.fingerprint)
          ?.toString(),
      updatedAt: (json['updatedAt'] ?? parsedDedicated?.rotatedAt)?.toString(),
    );
  }
}

typedef CredentialView = DeviceCredentialView;

/// Agente remoto elegível para rotear `Command::DeviceIo` até o equipamento.
@immutable
class AgentRouteOption {
  const AgentRouteOption({
    int? id,
    int? probeId,
    required this.name,
    this.online = true,
    this.hasDeviceIo = true,
  }) : id = id ?? probeId ?? 0;

  final int id;
  final String name;
  final bool online;
  final bool hasDeviceIo;

  int get probeId => id;

  factory AgentRouteOption.fromJson(Map<String, dynamic> json) {
    final hasExplicitOnline =
        json.containsKey('online') ||
        json.containsKey('status') ||
        json.containsKey('connected');
    final onlineVal = hasExplicitOnline
        ? (json['online'] == true ||
              json['status'] == 'online' ||
              json['connected'] == true)
        : true;
    return AgentRouteOption(
      id: parseIntOrNull(json['probeId'] ?? json['id']) ?? 0,
      name: (json['name'] ?? 'Agente').toString(),
      online: onlineVal,
      hasDeviceIo:
          json['hasDeviceIo'] != false &&
          json['deviceIo'] != false &&
          json['allowed'] != false,
    );
  }
}

/// Modelo completo retornado por `GET /api/devices/:id/access`.
@immutable
class DeviceAccessViewModel {
  const DeviceAccessViewModel({
    required this.deviceId,
    this.deviceName = '',
    this.platformHint,
    this.credentials = const <DeviceCredentialView>[],
    this.agents = const <AgentRouteOption>[],
    this.dedicatedPlatform,
  });

  final int deviceId;
  final String deviceName;
  final String? platformHint;
  final List<DeviceCredentialView> credentials;
  final List<AgentRouteOption> agents;
  final String? dedicatedPlatform;

  bool get supportsDedicatedKey =>
      dedicatedPlatform != null && dedicatedPlatform!.trim().isNotEmpty;

  DeviceCredentialView? get dedicatedCredential {
    for (final c in credentials) {
      if (c.dedicated) return c;
    }
    return null;
  }

  DeviceCredentialView? credentialFor(
    String kind, {
    bool includeDedicated = false,
  }) {
    final normalized = kind.trim().toLowerCase();
    for (final c in credentials) {
      if (c.kind.toLowerCase() != normalized) continue;
      if (!includeDedicated && c.dedicated) continue;
      return c;
    }
    return null;
  }

  List<AgentRouteOption> get eligibleAgents =>
      agents.where((a) => a.hasDeviceIo).toList(growable: false);

  DeviceAccessViewModel copyWith({String? deviceName, String? platformHint}) {
    return DeviceAccessViewModel(
      deviceId: deviceId,
      deviceName: deviceName ?? this.deviceName,
      platformHint: platformHint ?? this.platformHint,
      credentials: credentials,
      agents: agents,
      dedicatedPlatform: dedicatedPlatform,
    );
  }

  factory DeviceAccessViewModel.fromJson(
    Map<String, dynamic> json, {
    int? fallbackDeviceId,
  }) {
    return DeviceAccessViewModel(
      deviceId: parseIntOrNull(json['deviceId']) ?? fallbackDeviceId ?? 0,
      deviceName: (json['deviceName'] ?? '').toString(),
      platformHint:
          (json['dedicatedPlatform'] ?? json['platform'] ?? json['osName'])
              ?.toString(),
      credentials: asJsonList<DeviceCredentialView>(
        json['credentials'],
        DeviceCredentialView.fromJson,
      ),
      agents: asJsonList<AgentRouteOption>(
        json['agents'],
        AgentRouteOption.fromJson,
      ),
      dedicatedPlatform: json['dedicatedPlatform']?.toString(),
    );
  }
}

typedef DeviceAccessView = DeviceAccessViewModel;
