import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

/// Séries temporais agregadas em memória (SSE) e histórico persistido por host/container Docker.
@immutable
class DockerAggregateSample {
  const DockerAggregateSample({
    required this.recordedAt,
    required this.cpuPercent,
    required this.memoryPercent,
    required this.memoryUsageBytes,
    required this.networkReceivedBytes,
    required this.networkTransmittedBytes,
    required this.cpuByContainer,
    required this.memoryByContainer,
    required this.containerNames,
  });

  final String recordedAt;
  final double cpuPercent;
  final double memoryPercent;
  final int memoryUsageBytes;
  final int networkReceivedBytes;
  final int networkTransmittedBytes;
  final Map<String, double> cpuByContainer;
  final Map<String, int> memoryByContainer;
  final Map<String, String> containerNames;
}

@immutable
class HostDiskPoint {
  const HostDiskPoint({
    required this.mount,
    required this.usedBytes,
    required this.totalBytes,
  });

  final String mount;
  final int usedBytes;
  final int totalBytes;

  double get usedPercent => totalBytes > 0 ? (usedBytes / totalBytes) * 100 : 0;

  factory HostDiskPoint.fromJson(Map<String, dynamic> json) {
    return HostDiskPoint(
      mount: (json['mount'] as String?) ?? '/',
      usedBytes: parseIntOrNull(json['usedBytes']) ?? 0,
      totalBytes: parseIntOrNull(json['totalBytes']) ?? 0,
    );
  }
}

@immutable
class HostHistoryPoint {
  const HostHistoryPoint({
    required this.at,
    required this.cpuAvg,
    required this.cpuMax,
    required this.memoryUsed,
    required this.memoryTotal,
    required this.load1,
    required this.netRxBps,
    required this.netTxBps,
    required this.disks,
  });

  final String at;
  final double cpuAvg;
  final double cpuMax;
  final int memoryUsed;
  final int memoryTotal;
  final double load1;
  final double netRxBps;
  final double netTxBps;
  final List<HostDiskPoint> disks;

  factory HostHistoryPoint.fromJson(Map<String, dynamic> json) {
    return HostHistoryPoint(
      at: (json['at'] as String?) ?? '',
      cpuAvg: parseDoubleOrNull(json['cpuAvg']) ?? 0,
      cpuMax: parseDoubleOrNull(json['cpuMax']) ?? 0,
      memoryUsed: parseIntOrNull(json['memoryUsed']) ?? 0,
      memoryTotal: parseIntOrNull(json['memoryTotal']) ?? 0,
      load1: parseDoubleOrNull(json['load1']) ?? 0,
      netRxBps: parseDoubleOrNull(json['netRxBps']) ?? 0,
      netTxBps: parseDoubleOrNull(json['netTxBps']) ?? 0,
      disks: List<HostDiskPoint>.unmodifiable(
        asJsonList(json['disks'], HostDiskPoint.fromJson),
      ),
    );
  }
}

@immutable
class ContainerUsageSummary {
  const ContainerUsageSummary({
    required this.name,
    this.project,
    required this.cpuAvg,
    required this.cpuMax,
    required this.memoryMax,
    required this.netRxBytes,
    required this.netTxBytes,
  });

  final String name;
  final String? project;
  final double cpuAvg;
  final double cpuMax;
  final int memoryMax;
  final int netRxBytes;
  final int netTxBytes;

  factory ContainerUsageSummary.fromJson(Map<String, dynamic> json) {
    return ContainerUsageSummary(
      name: (json['name'] as String?) ?? '',
      project: json['project'] as String?,
      cpuAvg: parseDoubleOrNull(json['cpuAvg']) ?? 0,
      cpuMax: parseDoubleOrNull(json['cpuMax']) ?? 0,
      memoryMax: parseIntOrNull(json['memoryMax']) ?? 0,
      netRxBytes: parseIntOrNull(json['netRxBytes']) ?? 0,
      netTxBytes: parseIntOrNull(json['netTxBytes']) ?? 0,
    );
  }
}

@immutable
class HostHistoryResponse {
  const HostHistoryResponse({
    required this.hostKey,
    required this.range,
    required this.stepMinutes,
    required this.host,
    required this.containers,
  });

  final String hostKey;
  final String range;
  final int stepMinutes;
  final List<HostHistoryPoint> host;
  final List<ContainerUsageSummary> containers;

  factory HostHistoryResponse.fromJson(Map<String, dynamic> json) {
    return HostHistoryResponse(
      hostKey: (json['hostKey'] as String?) ?? '',
      range: (json['range'] as String?) ?? '24h',
      stepMinutes: parseIntOrNull(json['stepMinutes']) ?? 5,
      host: List<HostHistoryPoint>.unmodifiable(
        asJsonList(json['host'], HostHistoryPoint.fromJson),
      ),
      containers: List<ContainerUsageSummary>.unmodifiable(
        asJsonList(json['containers'], ContainerUsageSummary.fromJson),
      ),
    );
  }
}

@immutable
class ContainerHistoryPoint {
  const ContainerHistoryPoint({
    required this.at,
    required this.cpuAvg,
    required this.cpuMax,
    required this.memoryAvg,
    required this.memoryMax,
    required this.netRxBytes,
    required this.netTxBytes,
    required this.blockReadBytes,
    required this.blockWriteBytes,
  });

  final String at;
  final double cpuAvg;
  final double cpuMax;
  final int memoryAvg;
  final int memoryMax;
  final int netRxBytes;
  final int netTxBytes;
  final int blockReadBytes;
  final int blockWriteBytes;

  factory ContainerHistoryPoint.fromJson(Map<String, dynamic> json) {
    return ContainerHistoryPoint(
      at: (json['at'] as String?) ?? '',
      cpuAvg: parseDoubleOrNull(json['cpuAvg']) ?? 0,
      cpuMax: parseDoubleOrNull(json['cpuMax']) ?? 0,
      memoryAvg: parseIntOrNull(json['memoryAvg']) ?? 0,
      memoryMax: parseIntOrNull(json['memoryMax']) ?? 0,
      netRxBytes: parseIntOrNull(json['netRxBytes']) ?? 0,
      netTxBytes: parseIntOrNull(json['netTxBytes']) ?? 0,
      blockReadBytes: parseIntOrNull(json['blockReadBytes']) ?? 0,
      blockWriteBytes: parseIntOrNull(json['blockWriteBytes']) ?? 0,
    );
  }
}

@immutable
class ContainerHistoryResponse {
  const ContainerHistoryResponse({
    required this.hostKey,
    required this.containerName,
    required this.range,
    required this.stepMinutes,
    required this.points,
  });

  final String hostKey;
  final String containerName;
  final String range;
  final int stepMinutes;
  final List<ContainerHistoryPoint> points;

  factory ContainerHistoryResponse.fromJson(Map<String, dynamic> json) {
    return ContainerHistoryResponse(
      hostKey: (json['hostKey'] as String?) ?? '',
      containerName: (json['containerName'] as String?) ?? '',
      range: (json['range'] as String?) ?? '24h',
      stepMinutes: parseIntOrNull(json['stepMinutes']) ?? 5,
      points: List<ContainerHistoryPoint>.unmodifiable(
        asJsonList(json['points'], ContainerHistoryPoint.fromJson),
      ),
    );
  }
}

@immutable
class DockerPruneResponse {
  const DockerPruneResponse({
    required this.imagesDeleted,
    required this.spaceReclaimed,
  });

  final int imagesDeleted;
  final int spaceReclaimed;

  factory DockerPruneResponse.fromJson(Map<String, dynamic> json) {
    return DockerPruneResponse(
      imagesDeleted: parseIntOrNull(json['imagesDeleted']) ?? 0,
      spaceReclaimed: parseIntOrNull(json['spaceReclaimed']) ?? 0,
    );
  }
}
