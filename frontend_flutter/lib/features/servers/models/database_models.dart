import 'package:flutter/foundation.dart';

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
    final rawDbs = json['databases'];
    return DatabaseConnectionResponse(
      id: (json['id'] as num?)?.toInt() ?? 0,
      name: (json['name'] as String?) ?? '',
      engine: (json['engine'] as String?) ?? 'postgres',
      host: (json['host'] as String?) ?? '',
      port: (json['port'] as num?)?.toInt() ?? 5432,
      username: (json['username'] as String?) ?? '',
      passwordSet: json['passwordSet'] == true,
      sslMode: (json['sslMode'] as String?) ?? 'prefer',
      databases: rawDbs is List
          ? List<String>.unmodifiable(rawDbs.whereType<String>())
          : const <String>[],
      storageDestinationId: (json['storageDestinationId'] as num?)?.toInt(),
      storageDestinationName: json['storageDestinationName'] as String?,
      viaProbeId: (json['viaProbeId'] as num?)?.toInt(),
      viaProbeName: json['viaProbeName'] as String?,
      backupEnabled: json['backupEnabled'] == true,
      backupIntervalHours: (json['backupIntervalHours'] as num?)?.toInt() ?? 24,
      backupRetention: (json['backupRetention'] as num?)?.toInt() ?? 7,
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
      id: (json['id'] as num?)?.toInt() ?? 0,
      name: (json['name'] as String?) ?? '',
      connected: json['connected'] == true,
      allowed: json['allowed'] == true,
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
      probeId: (json['probeId'] as num?)?.toInt(),
      containerId: (json['containerId'] as String?) ?? '',
      containerName: (json['containerName'] as String?) ?? '',
      image: (json['image'] as String?) ?? '',
      engine: (json['engine'] as String?) ?? 'postgres',
      running: json['running'] == true || json['state'] == 'running',
      project: json['project'] as String? ?? json['projectName'] as String?,
      existingConnectionId: (json['existingConnectionId'] as num?)?.toInt(),
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
    final rawContainers = json['containers'];
    final rawIssues = json['issues'];
    return DatabaseContainersResponse(
      containers: rawContainers is List
          ? List<DatabaseContainer>.unmodifiable(
              rawContainers.whereType<Map>().map(
                (e) => DatabaseContainer.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <DatabaseContainer>[],
      issues: rawIssues is List
          ? List<DatabaseContainerIssue>.unmodifiable(
              rawIssues.whereType<Map>().map(
                (e) => DatabaseContainerIssue.fromJson(
                  Map<String, dynamic>.from(e),
                ),
              ),
            )
          : const <DatabaseContainerIssue>[],
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
      port: (json['port'] as num?)?.toInt() ?? 5432,
      kind: (json['kind'] as String?) ?? '',
      viaProbeId: (json['viaProbeId'] as num?)?.toInt(),
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
    final rawAddr = json['address'];
    final addrMap = rawAddr is Map ? Map<String, dynamic>.from(rawAddr) : json;
    return AddressAttempt(
      host: (addrMap['host'] as String?) ?? '',
      port: (addrMap['port'] as num?)?.toInt() ?? 0,
      ok: json['ok'] == true || json['reachable'] == true,
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
    final rawAddrs = json['addresses'];
    final rawAttempts = json['attempts'];
    return DatabaseContainerResolution(
      container: json['container'] is Map
          ? DatabaseContainer.fromJson(
              Map<String, dynamic>.from(json['container'] as Map),
            )
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
      addresses: rawAddrs is List
          ? List<CandidateAddress>.unmodifiable(
              rawAddrs.whereType<Map>().map(
                (e) => CandidateAddress.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <CandidateAddress>[],
      chosen: json['chosen'] is Map
          ? CandidateAddress.fromJson(
              Map<String, dynamic>.from(json['chosen'] as Map),
            )
          : null,
      attempts: rawAttempts is List
          ? List<AddressAttempt>.unmodifiable(
              rawAttempts.whereType<Map>().map(
                (e) => AddressAttempt.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <AddressAttempt>[],
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
    final rawDbs = json['databases'];
    return DatabaseProbeResponse(
      latencyMs: (json['latencyMs'] as num?)?.toDouble() ?? 0,
      version: (json['version'] as String?) ?? '',
      databases: rawDbs is List
          ? List<String>.unmodifiable(rawDbs.whereType<String>())
          : const <String>[],
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
    final rawWarn = json['warnings'];
    return DatabaseBackupResponse(
      id: (json['id'] as num?)?.toInt() ?? 0,
      connectionId: (json['connectionId'] as num?)?.toInt() ?? 0,
      databaseName: (json['databaseName'] as String?) ?? '',
      storageDestinationId: (json['storageDestinationId'] as num?)?.toInt(),
      objectKey: json['objectKey'] as String?,
      sizeBytes: (json['sizeBytes'] as num?)?.toInt() ?? 0,
      checksum: json['checksum'] as String?,
      tables: (json['tables'] as num?)?.toInt() ?? 0,
      rows: (json['rows'] as num?)?.toInt() ?? 0,
      durationMs: (json['durationMs'] as num?)?.toInt() ?? 0,
      status: (json['status'] as String?) ?? 'running',
      error: json['error'] as String?,
      warnings: rawWarn is List
          ? List<String>.unmodifiable(rawWarn.whereType<String>())
          : const <String>[],
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
    final connId = json['connectionId'];
    final status = json['status'];
    if (id is! String ||
        (kind != 'backup' && kind != 'restore') ||
        connId is! num ||
        status is! String) {
      return null;
    }
    return DatabaseJobSnapshot(
      id: id,
      kind: kind as String,
      connectionId: connId.toInt(),
      database: json['database'] as String?,
      stage: (json['stage'] as String?) ?? '',
      rows: (json['rows'] as num?)?.toInt() ?? 0,
      bytes: (json['bytes'] as num?)?.toInt() ?? 0,
      databasesDone: (json['databasesDone'] as num?)?.toInt() ?? 0,
      databasesTotal: (json['databasesTotal'] as num?)?.toInt() ?? 0,
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

/// Informações do banco de dados interno do NetMonitor (`GET /api/settings/database-size`).
@immutable
class DatabaseInfo {
  const DatabaseInfo({
    required this.dbType,
    required this.sizeBytes,
    this.earliestRecord,
    this.latestRecord,
  });

  final String dbType; // 'sqlite' | 'postgres'
  final int sizeBytes;
  final String? earliestRecord;
  final String? latestRecord;

  String get displayType =>
      dbType.toLowerCase() == 'postgres' ? 'PostgreSQL' : 'SQLite';

  factory DatabaseInfo.fromJson(Map<String, dynamic> json) {
    final rawSize = json['sizeBytes'];
    final size = rawSize is num
        ? rawSize.toInt()
        : int.tryParse(rawSize?.toString() ?? '0') ?? 0;
    return DatabaseInfo(
      dbType: (json['dbType'] as String?) ?? 'sqlite',
      sizeBytes: size,
      earliestRecord: json['earliestRecord'] as String?,
      latestRecord: json['latestRecord'] as String?,
    );
  }
}

@immutable
class TableUsage {
  const TableUsage({
    required this.table,
    required this.label,
    required this.area,
    required this.areaTitle,
    required this.totalBytes,
    required this.dataBytes,
    required this.indexBytes,
    required this.rows,
    required this.rowsEstimated,
    this.retentionDays,
  });

  final String table;
  final String label;
  final String area;
  final String areaTitle;
  final int totalBytes;
  final int dataBytes;
  final int indexBytes;
  final int rows;
  final bool rowsEstimated;
  final int? retentionDays;

  factory TableUsage.fromJson(Map<String, dynamic> json) {
    return TableUsage(
      table: (json['table'] as String?) ?? '',
      label: (json['label'] as String?) ?? (json['table'] as String?) ?? '',
      area: (json['area'] as String?) ?? 'core',
      areaTitle: (json['areaTitle'] as String?) ?? 'Núcleo',
      totalBytes: (json['totalBytes'] as num?)?.toInt() ?? 0,
      dataBytes: (json['dataBytes'] as num?)?.toInt() ?? 0,
      indexBytes: (json['indexBytes'] as num?)?.toInt() ?? 0,
      rows: (json['rows'] as num?)?.toInt() ?? 0,
      rowsEstimated: json['rowsEstimated'] == true,
      retentionDays: (json['retentionDays'] as num?)?.toInt(),
    );
  }
}

@immutable
class DatabaseUsageSection {
  const DatabaseUsageSection({
    required this.kind,
    required this.title,
    required this.name,
    required this.sizeBytes,
    required this.otherBytes,
    required this.detailAvailable,
    required this.tables,
  });

  final String kind; // 'main' | 'logs'
  final String title;
  final String name;
  final int sizeBytes;
  final int otherBytes;
  final bool detailAvailable;
  final List<TableUsage> tables;

  factory DatabaseUsageSection.fromJson(Map<String, dynamic> json) {
    final rawTables = json['tables'];
    return DatabaseUsageSection(
      kind: (json['kind'] as String?) ?? 'main',
      title: (json['title'] as String?) ?? '',
      name: (json['name'] as String?) ?? '',
      sizeBytes: (json['sizeBytes'] as num?)?.toInt() ?? 0,
      otherBytes: (json['otherBytes'] as num?)?.toInt() ?? 0,
      detailAvailable: json['detailAvailable'] != false,
      tables: rawTables is List
          ? List<TableUsage>.unmodifiable(
              rawTables.whereType<Map>().map(
                (e) => TableUsage.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <TableUsage>[],
    );
  }
}

@immutable
class DatabaseUsage {
  const DatabaseUsage({
    required this.dbType,
    required this.totalBytes,
    required this.databases,
  });

  final String dbType;
  final int totalBytes;
  final List<DatabaseUsageSection> databases;

  factory DatabaseUsage.fromJson(Map<String, dynamic> json) {
    final rawDbs = json['databases'];
    return DatabaseUsage(
      dbType: (json['dbType'] as String?) ?? 'sqlite',
      totalBytes: (json['totalBytes'] as num?)?.toInt() ?? 0,
      databases: rawDbs is List
          ? List<DatabaseUsageSection>.unmodifiable(
              rawDbs.whereType<Map>().map(
                (e) =>
                    DatabaseUsageSection.fromJson(Map<String, dynamic>.from(e)),
              ),
            )
          : const <DatabaseUsageSection>[],
    );
  }
}
