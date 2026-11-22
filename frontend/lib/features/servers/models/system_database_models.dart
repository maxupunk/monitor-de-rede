import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

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
    return DatabaseInfo(
      dbType: (json['dbType'] as String?) ?? 'sqlite',
      sizeBytes: parseIntOrNull(json['sizeBytes']) ?? 0,
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
      totalBytes: parseIntOrNull(json['totalBytes']) ?? 0,
      dataBytes: parseIntOrNull(json['dataBytes']) ?? 0,
      indexBytes: parseIntOrNull(json['indexBytes']) ?? 0,
      rows: parseIntOrNull(json['rows']) ?? 0,
      rowsEstimated: parseBool(json['rowsEstimated']),
      retentionDays: parseIntOrNull(json['retentionDays']),
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
    return DatabaseUsageSection(
      kind: (json['kind'] as String?) ?? 'main',
      title: (json['title'] as String?) ?? '',
      name: (json['name'] as String?) ?? '',
      sizeBytes: parseIntOrNull(json['sizeBytes']) ?? 0,
      otherBytes: parseIntOrNull(json['otherBytes']) ?? 0,
      detailAvailable: parseBool(json['detailAvailable'], fallback: true),
      tables: List<TableUsage>.unmodifiable(
        asJsonList(json['tables'], TableUsage.fromJson),
      ),
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
    return DatabaseUsage(
      dbType: (json['dbType'] as String?) ?? 'sqlite',
      totalBytes: parseIntOrNull(json['totalBytes']) ?? 0,
      databases: List<DatabaseUsageSection>.unmodifiable(
        asJsonList(json['databases'], DatabaseUsageSection.fromJson),
      ),
    );
  }
}
