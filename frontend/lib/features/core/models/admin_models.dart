import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

@immutable
class ManagedUser {
  const ManagedUser({
    required this.id,
    required this.email,
    required this.fullName,
    required this.role,
    required this.areaRoles,
    required this.active,
    this.lastLoginAt,
    required this.createdAt,
    required this.updatedAt,
  });

  final int id;
  final String email;
  final String fullName;
  final UserRole role;
  final AreaRoles areaRoles;
  final bool active;
  final String? lastLoginAt;
  final String createdAt;
  final String updatedAt;

  factory ManagedUser.fromJson(Map<String, dynamic> json) {
    return ManagedUser(
      id: parseIntOrNull(json['id']) ?? 0,
      email: (json['email'] ?? '').toString(),
      fullName: (json['fullName'] ?? json['name'] ?? '').toString(),
      role: UserRole.tryParse(json['role']) ?? UserRole.viewer,
      areaRoles: parseAreaRoles(json['areaRoles']),
      active: json['active'] != false,
      lastLoginAt: json['lastLoginAt'] as String?,
      createdAt: (json['createdAt'] ?? '').toString(),
      updatedAt: (json['updatedAt'] ?? '').toString(),
    );
  }
}

@immutable
class AuditLogEntry {
  const AuditLogEntry({
    required this.id,
    this.userId,
    this.userEmail,
    required this.action,
    required this.resourceType,
    this.resourceId,
    this.resourceLabel,
    this.description,
    this.changes,
    this.ipAddress,
    this.userAgent,
    required this.createdAt,
  });

  final int id;
  final int? userId;
  final String? userEmail;
  final String action;
  final String resourceType;
  final int? resourceId;
  final String? resourceLabel;
  final String? description;
  final Object? changes;
  final String? ipAddress;
  final String? userAgent;
  final String createdAt;

  factory AuditLogEntry.fromJson(Map<String, dynamic> json) {
    return AuditLogEntry(
      id: parseIntOrNull(json['id']) ?? 0,
      userId: parseIntOrNull(json['userId']),
      userEmail: json['userEmail'] as String?,
      action: (json['action'] ?? '').toString(),
      resourceType: (json['resourceType'] ?? '').toString(),
      resourceId: parseIntOrNull(json['resourceId']),
      resourceLabel: json['resourceLabel'] as String?,
      description: json['description'] as String?,
      changes: json['changes'],
      ipAddress: json['ipAddress'] as String?,
      userAgent: json['userAgent'] as String?,
      createdAt: (json['createdAt'] ?? '').toString(),
    );
  }
}

@immutable
class AuditFilters {
  const AuditFilters({
    this.userId,
    this.resourceType,
    this.action,
    this.from,
    this.to,
    this.search = '',
  });

  final int? userId;
  final String? resourceType;
  final String? action;
  final String? from;
  final String? to;
  final String search;

  AuditFilters copyWith({
    int? userId,
    bool clearUser = false,
    String? resourceType,
    bool clearResource = false,
    String? action,
    bool clearAction = false,
    String? from,
    bool clearFrom = false,
    String? to,
    bool clearTo = false,
    String? search,
  }) {
    return AuditFilters(
      userId: clearUser ? null : (userId ?? this.userId),
      resourceType: clearResource ? null : (resourceType ?? this.resourceType),
      action: clearAction ? null : (action ?? this.action),
      from: clearFrom ? null : (from ?? this.from),
      to: clearTo ? null : (to ?? this.to),
      search: search ?? this.search,
    );
  }
}

const List<(String, String)> kAuditActionOptions = <(String, String)>[
  ('create', 'Criação'),
  ('update', 'Alteração'),
  ('delete', 'Exclusão'),
  ('login', 'Login'),
  ('logout', 'Logout'),
  ('execute', 'Execução no equipamento'),
];

const List<(String, String)> kAuditResourceOptions = <(String, String)>[
  ('device', 'Dispositivo'),
  ('monitor', 'Monitor'),
  ('site', 'Site'),
  ('network', 'Rede'),
  ('user', 'Usuário'),
  ('probe', 'Probe'),
  ('vpn_peer', 'Peer VPN'),
  ('alert_rule', 'Regra de alerta'),
  ('maintenance_window', 'Janela de manutenção'),
  ('scheduled_task', 'Tarefa agendada'),
  ('storage', 'Armazenamento'),
  ('database_connection', 'Conexão de banco'),
  ('docker_container', 'Container Docker'),
  ('plugin', 'Plugin de dispositivo'),
  ('deploy', 'Deploy contínuo'),
  ('registry', 'Registry de imagens'),
  ('topology_link', 'Enlace de topologia'),
  ('agent', 'Servidor remoto'),
  ('backup', 'Backup do sistema'),
];

String auditActionLabel(String? action) {
  if (action == null || action.isEmpty) return '—';
  for (final (v, label) in kAuditActionOptions) {
    if (v == action) return label;
  }
  return action;
}

String auditResourceLabel(String? resource) {
  if (resource == null || resource.isEmpty) return '—';
  for (final (v, label) in kAuditResourceOptions) {
    if (v == resource) return label;
  }
  return resource;
}

@immutable
class SystemBackupPlan {
  const SystemBackupPlan({
    this.storageDestinationId,
    this.storageDestinationName,
    required this.backupEnabled,
    required this.backupIntervalHours,
    required this.backupRetention,
    this.lastBackupAt,
    this.lastBackupStatus,
    this.lastBackupError,
    this.nextBackupAt,
    this.running = false,
  });

  final int? storageDestinationId;
  final String? storageDestinationName;
  final bool backupEnabled;
  final int backupIntervalHours;
  final int backupRetention;
  final String? lastBackupAt;
  final String? lastBackupStatus;
  final String? lastBackupError;
  final String? nextBackupAt;
  final bool running;

  factory SystemBackupPlan.fromJson(Map<String, dynamic> json) {
    return SystemBackupPlan(
      storageDestinationId: parseIntOrNull(json['storageDestinationId']),
      storageDestinationName: json['storageDestinationName'] as String?,
      backupEnabled: json['backupEnabled'] == true,
      backupIntervalHours: parseIntOrNull(json['backupIntervalHours']) ?? 24,
      backupRetention: parseIntOrNull(json['backupRetention']) ?? 7,
      lastBackupAt: json['lastBackupAt'] as String?,
      lastBackupStatus: json['lastBackupStatus'] as String?,
      lastBackupError: json['lastBackupError'] as String?,
      nextBackupAt: json['nextBackupAt'] as String?,
      running: json['running'] == true,
    );
  }
}

@immutable
class SystemBackupCopy {
  const SystemBackupCopy({
    required this.key,
    required this.name,
    required this.size,
    this.lastModified,
  });

  final String key;
  final String name;
  final int size;
  final String? lastModified;

  factory SystemBackupCopy.fromJson(Map<String, dynamic> json) {
    return SystemBackupCopy(
      key: (json['key'] ?? '').toString(),
      name: (json['name'] ?? json['key'] ?? '').toString(),
      size: parseIntOrNull(json['size']) ?? 0,
      lastModified: json['lastModified'] as String?,
    );
  }
}

@immutable
class BackupTableCount {
  const BackupTableCount({required this.table, required this.rows});

  final String table;
  final int rows;

  factory BackupTableCount.fromJson(Map<String, dynamic> json) {
    return BackupTableCount(
      table: (json['table'] ?? '').toString(),
      rows: parseIntOrNull(json['rows']) ?? 0,
    );
  }
}

@immutable
class BackupCountsResponse {
  const BackupCountsResponse({
    required this.scope,
    required this.tables,
    required this.totalRows,
  });

  final String scope;
  final List<BackupTableCount> tables;
  final int totalRows;

  bool get isFull => scope == 'full';

  factory BackupCountsResponse.fromJson(Map<String, dynamic> json) {
    return BackupCountsResponse(
      scope: (json['scope'] ?? 'config').toString(),
      tables: asJsonList<BackupTableCount>(
        json['tables'],
        BackupTableCount.fromJson,
      ),
      totalRows: parseIntOrNull(json['totalRows']) ?? 0,
    );
  }
}

@immutable
class BackupRestoreResponse {
  const BackupRestoreResponse({
    required this.counts,
    required this.signInAgain,
  });

  final BackupCountsResponse counts;
  final bool signInAgain;

  factory BackupRestoreResponse.fromJson(Map<String, dynamic> json) {
    return BackupRestoreResponse(
      counts: json['counts'] is Map
          ? BackupCountsResponse.fromJson(asJsonMap(json['counts']))
          : BackupCountsResponse.fromJson(json),
      signInAgain: json['signInAgain'] == true,
    );
  }
}

@immutable
class StorageOptionRef {
  const StorageOptionRef({
    required this.id,
    required this.name,
    required this.kind,
  });

  final int id;
  final String name;
  final String kind;

  factory StorageOptionRef.fromJson(Map<String, dynamic> json) {
    return StorageOptionRef(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] ?? '').toString(),
      kind: (json['kind'] ?? 'local').toString(),
    );
  }
}
