import 'package:flutter/foundation.dart';

/// Capacidade anunciada por um agente (Docker ou Compose).
@immutable
class AgentCapabilityView {
  const AgentCapabilityView({
    required this.available,
    this.version,
    this.reason,
  });

  final bool available;
  final String? version;
  final String? reason;

  factory AgentCapabilityView.fromJson(Map<String, dynamic> json) {
    return AgentCapabilityView(
      available: json['available'] == true,
      version: json['version'] as String?,
      reason: json['reason'] as String?,
    );
  }
}

/// Informações do host enviadas pelo agente ao conectar.
@immutable
class AgentHostInfo {
  const AgentHostInfo({
    this.hostname,
    this.os,
    this.arch,
    this.inContainer = false,
    this.policy = const <String>[],
    this.docker = const AgentCapabilityView(available: false),
    this.compose = const AgentCapabilityView(available: false),
  });

  final String? hostname;
  final String? os;
  final String? arch;
  final bool inContainer;
  final List<String> policy;
  final AgentCapabilityView docker;
  final AgentCapabilityView compose;

  factory AgentHostInfo.fromJson(Map<String, dynamic> json) {
    final rawPolicy = json['policy'];
    final rawDocker = json['docker'];
    final rawCompose = json['compose'];
    return AgentHostInfo(
      hostname: json['hostname'] as String?,
      os: json['os'] as String?,
      arch: json['arch'] as String?,
      inContainer: json['inContainer'] == true,
      policy: rawPolicy is List
          ? List<String>.unmodifiable(rawPolicy.whereType<String>())
          : const <String>[],
      docker: rawDocker is Map
          ? AgentCapabilityView.fromJson(Map<String, dynamic>.from(rawDocker))
          : const AgentCapabilityView(available: false),
      compose: rawCompose is Map
          ? AgentCapabilityView.fromJson(Map<String, dynamic>.from(rawCompose))
          : const AgentCapabilityView(available: false),
    );
  }

  static AgentHostInfo? tryFromDynamic(Object? raw) {
    if (raw is! Map) return null;
    final map = Map<String, dynamic>.from(raw);
    if (map['policy'] is! List || map['docker'] is! Map) return null;
    return AgentHostInfo.fromJson(map);
  }
}

/// Servidor remoto / agente conectado à central (ADR 011 / ADR 014).
@immutable
class AgentView {
  const AgentView({
    required this.id,
    required this.name,
    required this.hostKey,
    this.siteId,
    this.deviceId,
    this.deviceName,
    this.deviceIp,
    required this.status,
    required this.connected,
    this.version,
    this.lastSeenAt,
    this.registeredAt,
    this.enforceTunnelIp = false,
    this.local = false,
    this.host = const AgentHostInfo(),
    this.createdAt,
  });

  final int id;
  final String name;
  final String hostKey;
  final int? siteId;
  final int? deviceId;
  final String? deviceName;
  final String? deviceIp;
  final String status;
  final bool connected;
  final String? version;
  final String? lastSeenAt;
  final String? registeredAt;
  final bool enforceTunnelIp;
  final bool local;
  final AgentHostInfo host;
  final String? createdAt;

  AgentView copyWith({
    String? name,
    int? siteId,
    int? deviceId,
    String? deviceName,
    String? deviceIp,
    String? status,
    bool? connected,
    String? version,
    String? lastSeenAt,
    String? registeredAt,
    bool? enforceTunnelIp,
    bool? local,
    AgentHostInfo? host,
  }) {
    return AgentView(
      id: id,
      name: name ?? this.name,
      hostKey: hostKey,
      siteId: siteId ?? this.siteId,
      deviceId: deviceId ?? this.deviceId,
      deviceName: deviceName ?? this.deviceName,
      deviceIp: deviceIp ?? this.deviceIp,
      status: status ?? this.status,
      connected: connected ?? this.connected,
      version: version ?? this.version,
      lastSeenAt: lastSeenAt ?? this.lastSeenAt,
      registeredAt: registeredAt ?? this.registeredAt,
      enforceTunnelIp: enforceTunnelIp ?? this.enforceTunnelIp,
      local: local ?? this.local,
      host: host ?? this.host,
      createdAt: createdAt,
    );
  }

  factory AgentView.fromJson(Map<String, dynamic> json) {
    final rawHost = json['host'];
    return AgentView(
      id: (json['id'] as num?)?.toInt() ?? 0,
      name: (json['name'] as String?) ?? '',
      hostKey: (json['hostKey'] as String?) ?? 'agent-${json['id']}',
      siteId: (json['siteId'] as num?)?.toInt(),
      deviceId: (json['deviceId'] as num?)?.toInt(),
      deviceName: json['deviceName'] as String?,
      deviceIp: json['deviceIp'] as String?,
      status: (json['status'] as String?) ?? 'offline',
      connected: json['connected'] == true,
      version: json['version'] as String?,
      lastSeenAt: json['lastSeenAt'] as String?,
      registeredAt: json['registeredAt'] as String?,
      enforceTunnelIp: json['enforceTunnelIp'] == true,
      local: json['local'] == true,
      host: rawHost is Map
          ? AgentHostInfo.fromJson(Map<String, dynamic>.from(rawHost))
          : const AgentHostInfo(),
      createdAt: json['createdAt'] as String?,
    );
  }
}

/// Política de permissões sugerida/configurada para instalação do agente (`AGENT_ALLOW`).
@immutable
class InstallPolicyView {
  const InstallPolicyView({required this.allow, required this.databaseTargets});

  final List<String> allow;
  final String databaseTargets;

  InstallPolicyView copyWith({List<String>? allow, String? databaseTargets}) {
    return InstallPolicyView(
      allow: allow ?? this.allow,
      databaseTargets: databaseTargets ?? this.databaseTargets,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
    'allow': allow,
    'databaseTargets': databaseTargets,
  };

  factory InstallPolicyView.fromJson(Map<String, dynamic> json) {
    final rawAllow = json['allow'];
    return InstallPolicyView(
      allow: rawAllow is List
          ? List<String>.unmodifiable(rawAllow.whereType<String>())
          : const <String>['read'],
      databaseTargets: (json['databaseTargets'] as String?) ?? '',
    );
  }
}

/// Comandos de instalação gerados pelo servidor (systemd e Docker).
@immutable
class AgentInstallCommands {
  const AgentInstallCommands({
    this.addressId,
    required this.serverUrl,
    required this.dockerCommand,
    required this.systemdCommand,
    required this.policy,
  });

  final String? addressId;
  final String serverUrl;
  final String dockerCommand;
  final String systemdCommand;
  final InstallPolicyView policy;

  factory AgentInstallCommands.fromJson(Map<String, dynamic> json) {
    final rawPolicy = json['policy'];
    return AgentInstallCommands(
      addressId: json['addressId'] as String?,
      serverUrl: (json['serverUrl'] as String?) ?? '',
      dockerCommand: (json['dockerCommand'] as String?) ?? '',
      systemdCommand: (json['systemdCommand'] as String?) ?? '',
      policy: rawPolicy is Map
          ? InstallPolicyView.fromJson(Map<String, dynamic>.from(rawPolicy))
          : const InstallPolicyView(
              allow: <String>['read'],
              databaseTargets: '',
            ),
    );
  }
}

/// Código de enrollment único retornado ao criar ou reemitir credencial do agente.
@immutable
class AgentEnrollmentView {
  const AgentEnrollmentView({
    required this.code,
    required this.expiresInSeconds,
    required this.commands,
  });

  final String code;
  final int expiresInSeconds;
  final AgentInstallCommands commands;

  factory AgentEnrollmentView.fromJson(Map<String, dynamic> json) {
    final rawCmd = json['commands'];
    return AgentEnrollmentView(
      code: (json['code'] as String?) ?? '',
      expiresInSeconds: (json['expiresInSeconds'] as num?)?.toInt() ?? 1800,
      commands: rawCmd is Map
          ? AgentInstallCommands.fromJson(Map<String, dynamic>.from(rawCmd))
          : const AgentInstallCommands(
              serverUrl: '',
              dockerCommand: '',
              systemdCommand: '',
              policy: InstallPolicyView(
                allow: <String>['read'],
                databaseTargets: '',
              ),
            ),
    );
  }
}

@immutable
class AgentWithEnrollment {
  const AgentWithEnrollment({required this.agent, required this.enrollment});

  final AgentView agent;
  final AgentEnrollmentView enrollment;

  factory AgentWithEnrollment.fromJson(Map<String, dynamic> json) {
    return AgentWithEnrollment(
      agent: AgentView.fromJson(
        Map<String, dynamic>.from(json['agent'] as Map),
      ),
      enrollment: AgentEnrollmentView.fromJson(
        Map<String, dynamic>.from(json['enrollment'] as Map),
      ),
    );
  }
}

@immutable
class AgentCreateInput {
  const AgentCreateInput({
    required this.name,
    this.siteId,
    this.deviceId,
    this.enforceTunnelIp,
  });

  final String name;
  final int? siteId;
  final int? deviceId;
  final bool? enforceTunnelIp;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'name': name,
    'siteId': siteId,
    'deviceId': deviceId,
    'enforceTunnelIp': enforceTunnelIp,
  };
}

@immutable
class AgentUpdateInput {
  const AgentUpdateInput({this.name, this.deviceId, this.enforceTunnelIp});

  final String? name;
  final int? deviceId;
  final bool? enforceTunnelIp;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'name': name,
    'deviceId': deviceId,
    'enforceTunnelIp': enforceTunnelIp,
  };
}

/// Endereço de acesso da central lido de `GET /api/server-addresses` para comandos do agente.
@immutable
class ServerAddressOption {
  const ServerAddressOption({
    required this.id,
    required this.label,
    required this.kind,
    required this.host,
    this.url,
    this.isDefault = false,
  });

  final String id;
  final String label;
  final String kind;
  final String host;
  final String? url;
  final bool isDefault;

  factory ServerAddressOption.fromJson(Map<String, dynamic> json) {
    return ServerAddressOption(
      id: (json['id'] ?? '').toString(),
      label: (json['label'] as String?) ?? (json['host'] as String?) ?? '',
      kind: (json['kind'] as String?) ?? 'lan',
      host: (json['host'] as String?) ?? '',
      url: json['url'] as String?,
      isDefault: json['isDefault'] == true || json['default'] == true,
    );
  }
}
