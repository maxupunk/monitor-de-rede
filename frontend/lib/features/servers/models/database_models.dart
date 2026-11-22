import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

export 'system_database_models.dart';

/// Conexão de banco de dados gerenciada para backup/restauração (ADR 013).
@immutable
class DatabaseConnectionResponse {
  const DatabaseConnectionResponse({
    required this.id,
    required this.name,
    required this.engine,
    required this.host,
    required this.port,
    required this.username,
    required this.passwordSet,
    required this.sslMode,
    required this.databases,
    this.storageDestinationId,
    this.storageDestinationName,
    this.viaProbeId,
    this.viaProbeName,
    required this.backupEnabled,
    required this.backupIntervalHours,
    required this.backupRetention,
    this.lastBackupAt,
    this.lastBackupStatus,
    this.lastBackupError,
    this.nextBackupAt,
    this.createdAt,
    this.updatedAt,
  });

  final int id;
  final String name;
  final String engine; // 'postgres' | 'mysql' | 'mariadb'
  final String host;
  final int port;
  final String username;
  final bool passwordSet;
  final String sslMode; // 'disable' | 'prefer' | 'require'
  final List<String> databases;
  final int? storageDestinationId;
  final String? storageDestinationName;
  final int? viaProbeId;
  final String? viaProbeName;
  final bool backupEnabled;
  final int backupIntervalHours;
  final int backupRetention;
  final String? lastBackupAt;
  final String? lastBackupStatus;
  final String? lastBackupError;
  final String? nextBackupAt;
  final String? createdAt;
  final String? updatedAt;

  String get protocolFamily => engine == 'postgres' ? 'postgres' : 'mysql';

  String get engineLabel => switch (engine) {
    'postgres' => 'PostgreSQL',
    'mysql' => 'MySQL',
    'mariadb' => 'MariaDB',
    _ => engine,
  };

  factory DatabaseConnectionResponse.fromJson(Map<String, dynamic> json) {
    return DatabaseConnectionResponse(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] as String?) ?? '',
      engine: (json['engine'] as String?) ?? 'postgres',
      host: (json['host'] as String?) ?? '',
      port: parseIntOrNull(json['port']) ?? 5432,
      username: (json['username'] as String?) ?? '',
      passwordSet: parseBool(json['passwordSet']),
      sslMode: (json['sslMode'] as String?) ?? 'prefer',
      databases: List<String>.unmodifiable(asStringList(json['databases'])),
      storageDestinationId: parseIntOrNull(json['storageDestinationId']),
      storageDestinationName: json['storageDestinationName'] as String?,
      viaProbeId: parseIntOrNull(json['viaProbeId']),
      viaProbeName: json['viaProbeName'] as String?,
      backupEnabled: parseBool(json['backupEnabled']),
      backupIntervalHours: parseIntOrNull(json['backupIntervalHours']) ?? 24,
      backupRetention: parseIntOrNull(json['backupRetention']) ?? 7,
      lastBackupAt: json['lastBackupAt'] as String?,
      lastBackupStatus: json['lastBackupStatus'] as String?,
      lastBackupError: json['lastBackupError'] as String?,
      nextBackupAt: json['nextBackupAt'] as String?,
      createdAt: json['createdAt'] as String?,
      updatedAt: json['updatedAt'] as String?,
    );
  }
}

@immutable
class DatabaseConnectionInput {
  const DatabaseConnectionInput({
    required this.name,
    required this.engine,
    required this.host,
    required this.port,
    required this.username,
    this.password,
    required this.sslMode,
    required this.databases,
    this.storageDestinationId,
    this.viaProbeId,
    required this.backupEnabled,
    required this.backupIntervalHours,
    required this.backupRetention,
  });

  final String name;
  final String engine;
  final String host;
  final int port;
  final String username;
  final String? password;
  final String sslMode;
  final List<String> databases;
  final int? storageDestinationId;
  final int? viaProbeId;
  final bool backupEnabled;
  final int backupIntervalHours;
  final int backupRetention;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'name': name,
    'engine': engine,
    'host': host,
    'port': port,
    'username': username,
    if (password != null && password!.isNotEmpty) 'password': password,
    'sslMode': sslMode,
    'databases': databases,
    'storageDestinationId': storageDestinationId,
    'viaProbeId': viaProbeId,
    'backupEnabled': backupEnabled,
    'backupIntervalHours': backupIntervalHours,
    'backupRetention': backupRetention,
  };
}

@immutable
class AgentRouteOption {
  const AgentRouteOption({
    required this.id,
    required this.name,
    required this.connected,
    required this.allowed,
  });

  final int id;
  final String name;
  final bool connected;
  final bool allowed;

  factory AgentRouteOption.fromJson(Map<String, dynamic> json) {
    return AgentRouteOption(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] as String?) ?? '',
      connected: parseBool(json['connected']),
      allowed: parseBool(json['allowed']),
    );
  }
}

@immutable
class DatabaseContainer {
  const DatabaseContainer({
    required this.hostKey,
    required this.hostName,
    this.probeId,
    required this.containerId,
    required this.containerName,
    required this.image,
    required this.engine,
    required this.running,
    this.project,
    this.existingConnectionId,
  });

  final String hostKey;
  final String hostName;
  final int? probeId;
  final String containerId;
  final String containerName;
  final String image;
  final String engine;
  final bool running;
  final String? project;
  final int? existingConnectionId;

  factory DatabaseContainer.fromJson(Map<String, dynamic> json) {
    return DatabaseContainer(
      hostKey: (json['hostKey'] as String?) ?? '',
      hostName: (json['hostName'] as String?) ?? '',
      probeId: parseIntOrNull(json['probeId']),
      containerId: (json['containerId'] as String?) ?? '',
      containerName: (json['containerName'] as String?) ?? '',
      image: (json['image'] as String?) ?? '',
      engine: (json['engine'] as String?) ?? 'postgres',
      running: parseBool(json['running']) || json['state'] == 'running',
      project: json['project'] as String? ?? json['projectName'] as String?,
      existingConnectionId: parseIntOrNull(json['existingConnectionId']),
    );
  }
}

@immutable
class DatabaseContainerIssue {
  const DatabaseContainerIssue({required this.hostName, required this.reason});

  final String hostName;
  final String reason;

  factory DatabaseContainerIssue.fromJson(Map<String, dynamic> json) {
    return DatabaseContainerIssue(
      hostName: (json['hostName'] as String?) ?? '',
      reason: (json['reason'] as String?) ?? '',
    );
  }
}

@immutable
class DatabaseContainersResponse {
  const DatabaseContainersResponse({
    this.containers = const <DatabaseContainer>[],
    this.issues = const <DatabaseContainerIssue>[],
  });

  final List<DatabaseContainer> containers;
  final List<DatabaseContainerIssue> issues;

  factory DatabaseContainersResponse.fromJson(Map<String, dynamic> json) {
    return DatabaseContainersResponse(
      containers: List<DatabaseContainer>.unmodifiable(
        asJsonList(json['containers'], DatabaseContainer.fromJson),
      ),
      issues: List<DatabaseContainerIssue>.unmodifiable(
        asJsonList(json['issues'], DatabaseContainerIssue.fromJson),
      ),
    );
  }
}

@immutable
class CandidateAddress {
  const CandidateAddress({
    required this.host,
    required this.port,
    required this.kind,
    this.viaProbeId,
    this.label,
  });

  final String host;
  final int port;
  final String kind;
  final int? viaProbeId;
  final String? label;

  factory CandidateAddress.fromJson(Map<String, dynamic> json) {
    return CandidateAddress(
      host: (json['host'] as String?) ?? '',
      port: parseIntOrNull(json['port']) ?? 5432,
      kind: (json['kind'] as String?) ?? '',
      viaProbeId: parseIntOrNull(json['viaProbeId']),
      label: json['label'] as String?,
    );
  }
}

@immutable
class AddressAttempt {
  const AddressAttempt({
    required this.host,
    required this.port,
    required this.ok,
    this.error,
  });

  final String host;
  final int port;
  final bool ok;
  final String? error;

  factory AddressAttempt.fromJson(Map<String, dynamic> json) {
    final addrMap = json['address'] is Map ? asJsonMap(json['address']) : json;
    return AddressAttempt(
      host: (addrMap['host'] as String?) ?? '',
      port: parseIntOrNull(addrMap['port']) ?? 0,
      ok: parseBool(json['ok']) || parseBool(json['reachable']),
      error: json['error'] as String? ?? json['reason'] as String?,
    );
  }
}

@immutable
class DatabaseContainerResolution {
  const DatabaseContainerResolution({
    required this.container,
    this.username,
    this.database,
    required this.addresses,
    this.chosen,
    required this.attempts,
  });

  final DatabaseContainer container;
  final String? username;
  final String? database;
  final List<CandidateAddress> addresses;
  final CandidateAddress? chosen;
  final List<AddressAttempt> attempts;

  factory DatabaseContainerResolution.fromJson(Map<String, dynamic> json) {
    return DatabaseContainerResolution(
      container: json['container'] is Map
          ? DatabaseContainer.fromJson(asJsonMap(json['container']))
          : const DatabaseContainer(
              hostKey: '',
              hostName: '',
              containerId: '',
              containerName: '',
              image: '',
              engine: 'postgres',
              running: false,
            ),
      username: json['username'] as String?,
      database: json['database'] as String?,
      addresses: List<CandidateAddress>.unmodifiable(
        asJsonList(json['addresses'], CandidateAddress.fromJson),
      ),
      chosen: json['chosen'] is Map
          ? CandidateAddress.fromJson(asJsonMap(json['chosen']))
          : null,
      attempts: List<AddressAttempt>.unmodifiable(
        asJsonList(json['attempts'], AddressAttempt.fromJson),
      ),
    );
  }
}

@immutable
class DatabaseProbeResponse {
  const DatabaseProbeResponse({
    required this.latencyMs,
    required this.version,
    required this.databases,
  });

  final double latencyMs;
  final String version;
  final List<String> databases;

  factory DatabaseProbeResponse.fromJson(Map<String, dynamic> json) {
    return DatabaseProbeResponse(
      latencyMs: parseDoubleOrNull(json['latencyMs']) ?? 0,
      version: (json['version'] as String?) ?? '',
      databases: List<String>.unmodifiable(asStringList(json['databases'])),
    );
  }
}

@immutable
class DatabaseBackupResponse {
  const DatabaseBackupResponse({
    required this.id,
    required this.connectionId,
    required this.databaseName,
    this.storageDestinationId,
    this.objectKey,
    this.sizeBytes = 0,
    this.checksum,
    this.tables = 0,
    this.rows = 0,
    this.durationMs = 0,
    required this.status,
    this.error,
    required this.warnings,
    required this.trigger,
    required this.startedAt,
    this.finishedAt,
  });

  final int id;
  final int connectionId;
  final String databaseName;
  final int? storageDestinationId;
  final String? objectKey;
  final int sizeBytes;
  final String? checksum;
  final int tables;
  final int rows;
  final int durationMs;
  final String status; // 'running' | 'success' | 'failed'
  final String? error;
  final List<String> warnings;
  final String trigger; // 'manual' | 'scheduled' | 'pre_deploy'
  final String startedAt;
  final String? finishedAt;

  factory DatabaseBackupResponse.fromJson(Map<String, dynamic> json) {
    return DatabaseBackupResponse(
      id: parseIntOrNull(json['id']) ?? 0,
      connectionId: parseIntOrNull(json['connectionId']) ?? 0,
      databaseName: (json['databaseName'] as String?) ?? '',
      storageDestinationId: parseIntOrNull(json['storageDestinationId']),
      objectKey: json['objectKey'] as String?,
      sizeBytes: parseIntOrNull(json['sizeBytes']) ?? 0,
      checksum: json['checksum'] as String?,
      tables: parseIntOrNull(json['tables']) ?? 0,
      rows: parseIntOrNull(json['rows']) ?? 0,
      durationMs: parseIntOrNull(json['durationMs']) ?? 0,
      status: (json['status'] as String?) ?? 'running',
      error: json['error'] as String?,
      warnings: List<String>.unmodifiable(asStringList(json['warnings'])),
      trigger: (json['trigger'] as String?) ?? 'manual',
      startedAt: (json['startedAt'] as String?) ?? '',
      finishedAt: json['finishedAt'] as String?,
    );
  }
}

@immutable
class DatabaseJobSnapshot {
  const DatabaseJobSnapshot({
    required this.id,
    required this.kind,
    required this.connectionId,
    this.database,
    required this.stage,
    required this.rows,
    required this.bytes,
    required this.databasesDone,
    required this.databasesTotal,
    required this.status,
    this.message,
    required this.startedAt,
    this.finishedAt,
  });

  final String id;
  final String kind; // 'backup' | 'restore'
  final int connectionId;
  final String? database;
  final String stage;
  final int rows;
  final int bytes;
  final int databasesDone;
  final int databasesTotal;
  final String status; // 'running' | 'success' | 'failed'
  final String? message;
  final String startedAt;
  final String? finishedAt;

  bool get isRunning => status == 'running';

  static DatabaseJobSnapshot? tryFromJson(Map<String, dynamic> json) {
    final id = json['id'];
    final kind = json['kind'];
    final connId = parseIntOrNull(json['connectionId']);
    final status = json['status'];
    if (id is! String ||
        (kind != 'backup' && kind != 'restore') ||
        connId == null ||
        status is! String) {
      return null;
    }
    return DatabaseJobSnapshot(
      id: id,
      kind: kind as String,
      connectionId: connId,
      database: json['database'] as String?,
      stage: (json['stage'] as String?) ?? '',
      rows: parseIntOrNull(json['rows']) ?? 0,
      bytes: parseIntOrNull(json['bytes']) ?? 0,
      databasesDone: parseIntOrNull(json['databasesDone']) ?? 0,
      databasesTotal: parseIntOrNull(json['databasesTotal']) ?? 0,
      status: status,
      message: json['message'] as String?,
      startedAt: (json['startedAt'] as String?) ?? '',
      finishedAt: json['finishedAt'] as String?,
    );
  }
}

@immutable
class DatabaseRestoreInput {
  const DatabaseRestoreInput({
    required this.targetConnectionId,
    required this.database,
    required this.mode,
    this.confirmDatabase,
  });

  final int targetConnectionId;
  final String database;
  final String mode; // 'new_database' | 'replace'
  final String? confirmDatabase;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'targetConnectionId': targetConnectionId,
    'database': database,
    'mode': mode,
    if (confirmDatabase != null && confirmDatabase!.isNotEmpty)
      'confirmDatabase': confirmDatabase,
  };
}
