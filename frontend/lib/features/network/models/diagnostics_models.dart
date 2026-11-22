import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

@immutable
class TracerouteHop {
  const TracerouteHop({
    required this.hop,
    required this.status,
    required this.rttMs,
    this.ip,
    this.hostname,
    this.avgRttMs,
  });

  final int hop;
  final String? ip;
  final String? hostname;
  final List<double?> rttMs;
  final double? avgRttMs;
  final String status;

  factory TracerouteHop.fromJson(Map<String, dynamic> json) {
    final rawRtts = json['rttMs'];
    final rtts = rawRtts is List
        ? rawRtts.map(parseDoubleOrNull).toList(growable: false)
        : const <double?>[];
    return TracerouteHop(
      hop: parseIntOrNull(json['hop']) ?? 0,
      ip: json['ip'] as String?,
      hostname: json['hostname'] as String?,
      rttMs: rtts,
      avgRttMs: parseDoubleOrNull(json['avgRttMs']),
      status: (json['status'] as String?) ?? 'intermediate',
    );
  }
}

@immutable
class SpeedTestProgress {
  const SpeedTestProgress({
    required this.phase,
    required this.progressPct,
    this.currentMbps,
    this.pingMs,
    this.jitterMs,
    this.downloadMbps,
    this.uploadMbps,
    this.serverName,
    this.serverLocation,
  });

  final String phase;
  final double progressPct;
  final double? currentMbps;
  final double? pingMs;
  final double? jitterMs;
  final double? downloadMbps;
  final double? uploadMbps;
  final String? serverName;
  final String? serverLocation;

  factory SpeedTestProgress.fromJson(Map<String, dynamic> json) {
    return SpeedTestProgress(
      phase: (json['phase'] as String?) ?? 'ping',
      progressPct: parseDoubleOrNull(json['progressPct']) ?? 0,
      currentMbps: parseDoubleOrNull(json['currentMbps']),
      pingMs: parseDoubleOrNull(json['pingMs']),
      jitterMs: parseDoubleOrNull(json['jitterMs']),
      downloadMbps: parseDoubleOrNull(json['downloadMbps']),
      uploadMbps: parseDoubleOrNull(json['uploadMbps']),
      serverName: json['serverName'] as String?,
      serverLocation: json['serverLocation'] as String?,
    );
  }
}

@immutable
class SpeedTestResult {
  const SpeedTestResult({
    required this.pingMs,
    required this.jitterMs,
    required this.downloadMbps,
    required this.uploadMbps,
    required this.timestamp,
    this.serverName,
    this.serverLocation,
  });

  final double pingMs;
  final double jitterMs;
  final double downloadMbps;
  final double uploadMbps;
  final String? serverName;
  final String? serverLocation;
  final String timestamp;

  factory SpeedTestResult.fromJson(Map<String, dynamic> json) {
    return SpeedTestResult(
      pingMs: parseDoubleOrNull(json['pingMs']) ?? 0,
      jitterMs: parseDoubleOrNull(json['jitterMs']) ?? 0,
      downloadMbps: parseDoubleOrNull(json['downloadMbps']) ?? 0,
      uploadMbps: parseDoubleOrNull(json['uploadMbps']) ?? 0,
      serverName: json['serverName'] as String?,
      serverLocation: json['serverLocation'] as String?,
      timestamp:
          (json['timestamp'] as String?) ?? DateTime.now().toIso8601String(),
    );
  }
}

@immutable
class PlaybookStepResult {
  const PlaybookStepResult({
    required this.stepIndex,
    required this.stepName,
    required this.description,
    required this.status,
    this.message,
    this.data,
  });

  final int stepIndex;
  final String stepName;
  final String description;
  final String status;
  final String? message;
  final Map<String, dynamic>? data;

  factory PlaybookStepResult.fromJson(Map<String, dynamic> json) {
    return PlaybookStepResult(
      stepIndex: parseIntOrNull(json['stepIndex']) ?? 0,
      stepName: (json['stepName'] as String?) ?? '',
      description: (json['description'] as String?) ?? '',
      status: (json['status'] as String?) ?? 'running',
      message: json['message'] as String?,
      data: json['data'] is Map
          ? Map<String, dynamic>.from(json['data'] as Map)
          : null,
    );
  }
}

@immutable
class PlaybookSummary {
  const PlaybookSummary({
    required this.playbookType,
    required this.status,
    required this.diagnosis,
    required this.recommendations,
    required this.steps,
    this.target,
  });

  final String playbookType;
  final String? target;
  final String status;
  final String diagnosis;
  final List<String> recommendations;
  final List<PlaybookStepResult> steps;

  factory PlaybookSummary.fromJson(Map<String, dynamic> json) {
    return PlaybookSummary(
      playbookType: (json['playbookType'] as String?) ?? '',
      target: json['target'] as String?,
      status: (json['status'] as String?) ?? 'success',
      diagnosis: (json['diagnosis'] as String?) ?? '',
      recommendations: asStringList(json['recommendations']),
      steps: asJsonMapList(
        json['steps'],
      ).map(PlaybookStepResult.fromJson).toList(growable: false),
    );
  }
}

@immutable
class QuickPingRtt {
  const QuickPingRtt({
    required this.minMs,
    required this.avgMs,
    required this.maxMs,
  });

  final double minMs;
  final double avgMs;
  final double maxMs;

  static QuickPingRtt? tryFromJson(Object? raw) {
    if (raw is! Map) return null;
    final json = Map<String, dynamic>.from(raw);
    return QuickPingRtt(
      minMs: parseDoubleOrNull(json['minMs']) ?? 0,
      avgMs: parseDoubleOrNull(json['avgMs']) ?? 0,
      maxMs: parseDoubleOrNull(json['maxMs']) ?? 0,
    );
  }
}

@immutable
class QuickPingReport {
  const QuickPingReport({
    required this.target,
    required this.sent,
    required this.samples,
    required this.lossPct,
    required this.capacityNote,
    required this.verdict,
    this.small,
    this.large,
    this.jitterMs,
    this.queueingLevel,
    this.deltaMinMs,
    this.capacityLabel,
  });

  final String target;
  final int sent;
  final int samples;
  final double lossPct;
  final QuickPingRtt? small;
  final QuickPingRtt? large;
  final double? jitterMs;
  final String? queueingLevel;
  final double? deltaMinMs;
  final String? capacityLabel;
  final String capacityNote;
  final String verdict;

  factory QuickPingReport.fromJson(Map<String, dynamic> json) {
    final cap = json['capacity'];
    String? capLabel;
    if (cap is Map) {
      final minM = parseDoubleOrNull(cap['minMbps']);
      final maxM = parseDoubleOrNull(cap['maxMbps']);
      if (minM != null && maxM != null) {
        capLabel =
            '${formatDecimal(minM, maxDigits: 0)}–${formatDecimal(maxM, maxDigits: 0)} Mbps';
      } else if (cap['label'] != null) {
        capLabel = cap['label'].toString();
      }
    } else if (cap is String) {
      capLabel = cap;
    }

    return QuickPingReport(
      target: (json['target'] as String?) ?? '',
      sent: parseIntOrNull(json['sent']) ?? 0,
      samples: parseIntOrNull(json['samples']) ?? 0,
      lossPct: parseDoubleOrNull(json['lossPct']) ?? 0,
      small: QuickPingRtt.tryFromJson(json['small']),
      large: QuickPingRtt.tryFromJson(json['large']),
      jitterMs: parseDoubleOrNull(json['jitterMs']),
      queueingLevel: json['queueing']?.toString(),
      deltaMinMs: parseDoubleOrNull(json['deltaMinMs']),
      capacityLabel: capLabel,
      capacityNote: (json['capacityNote'] as String?) ?? '',
      verdict: (json['verdict'] as String?) ?? '',
    );
  }
}

@immutable
class PortScanItem {
  const PortScanItem({
    required this.port,
    required this.protocol,
    required this.status,
    required this.latencyMs,
    required this.attempts,
    this.service,
    this.error,
  });

  final int port;
  final String protocol;
  final String status;
  final String? service;
  final double latencyMs;
  final int attempts;
  final String? error;

  factory PortScanItem.fromJson(Map<String, dynamic> json) {
    return PortScanItem(
      port: parseIntOrNull(json['port']) ?? 0,
      protocol: (json['protocol'] as String?) ?? 'tcp',
      status: (json['status'] as String?) ?? 'closed',
      service: json['service'] as String?,
      latencyMs: parseDoubleOrNull(json['latencyMs']) ?? 0,
      attempts: parseIntOrNull(json['attempts']) ?? 1,
      error: json['error'] as String?,
    );
  }
}

@immutable
class SnmpTestResult {
  const SnmpTestResult({
    required this.responded,
    this.version,
    this.community,
    this.sysName,
    this.sysDescr,
    this.sysUpTime,
    this.message,
  });

  final bool responded;
  final String? version;
  final String? community;
  final String? sysName;
  final String? sysDescr;
  final int? sysUpTime;
  final String? message;

  /// Normaliza resposta de teste direto (`success`) e detecção automática (`detected`).
  factory SnmpTestResult.fromJson(Map<String, dynamic> json) {
    final test = json['result'] is Map ? asJsonMap(json['result']) : json;
    final system = asJsonMap(test['system']);
    final responded = (json['detected'] ?? test['success']) == true;
    return SnmpTestResult(
      responded: responded,
      version: json['version'] as String?,
      community: json['community'] as String?,
      sysName: system['sysName'] as String?,
      sysDescr: system['sysDescr'] as String?,
      sysUpTime: parseIntOrNull(system['sysUpTime']),
      message: test['message'] as String?,
    );
  }
}

@immutable
class DnsServerModel {
  const DnsServerModel({
    required this.id,
    required this.name,
    required this.address,
    required this.protocol,
    this.isDefault = true,
    this.description,
    this.createdAt,
  });

  final int id;
  final String name;
  final String address;
  final String protocol;
  final bool isDefault;
  final String? description;
  final String? createdAt;

  factory DnsServerModel.fromJson(Map<String, dynamic> json) {
    return DnsServerModel(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] as String?) ?? '',
      address: (json['address'] as String?) ?? '',
      protocol: (json['protocol'] as String?) ?? 'udp',
      isDefault: (json['isDefault'] as bool?) ?? true,
      description: json['description'] as String?,
      createdAt: json['createdAt'] as String?,
    );
  }
}

@immutable
class DnsRankingEntry {
  const DnsRankingEntry({
    required this.server,
    required this.label,
    required this.protocol,
    required this.successRate,
    this.avgLookupTimeMs,
    this.minLookupTimeMs,
    this.maxLookupTimeMs,
    this.totalChecks,
    this.error,
  });

  final String server;
  final String label;
  final String protocol;
  final double? avgLookupTimeMs;
  final double? minLookupTimeMs;
  final double? maxLookupTimeMs;
  final double successRate;
  final int? totalChecks;
  final String? error;

  /// Normaliza taxa de sucesso para 0..100 (benchmark envia 0..1; histórico envia 0..100).
  double get normalizedSuccessPct =>
      successRate <= 1.0 ? successRate * 100.0 : successRate;

  factory DnsRankingEntry.fromJson(Map<String, dynamic> json) {
    return DnsRankingEntry(
      server: (json['server'] as String?) ?? '',
      label: (json['label'] ?? json['server'] ?? '').toString(),
      protocol: (json['protocol'] as String?) ?? 'udp',
      avgLookupTimeMs: parseDoubleOrNull(json['avgLookupTimeMs']),
      minLookupTimeMs: parseDoubleOrNull(json['minLookupTimeMs']),
      maxLookupTimeMs: parseDoubleOrNull(json['maxLookupTimeMs']),
      successRate: parseDoubleOrNull(json['successRate']) ?? 0,
      totalChecks: parseIntOrNull(json['totalChecks'] ?? json['totalQueries']),
      error: json['error'] as String?,
    );
  }
}

@immutable
class DnsHistoryPoint {
  const DnsHistoryPoint({
    required this.timestamp,
    required this.status,
    this.latencyMs,
  });

  final String timestamp;
  final double? latencyMs;
  final String status;

  factory DnsHistoryPoint.fromJson(Map<String, dynamic> json) {
    return DnsHistoryPoint(
      timestamp: (json['timestamp'] as String?) ?? '',
      latencyMs: parseDoubleOrNull(json['latencyMs']),
      status: (json['status'] as String?) ?? 'up',
    );
  }
}

@immutable
class DnsSeriesItem {
  const DnsSeriesItem({
    required this.server,
    required this.label,
    required this.protocol,
    required this.points,
  });

  final String server;
  final String label;
  final String protocol;
  final List<DnsHistoryPoint> points;

  factory DnsSeriesItem.fromJson(Map<String, dynamic> json) {
    return DnsSeriesItem(
      server: (json['server'] as String?) ?? '',
      label: (json['label'] ?? json['server'] ?? '').toString(),
      protocol: (json['protocol'] as String?) ?? 'udp',
      points: asJsonMapList(
        json['points'],
      ).map(DnsHistoryPoint.fromJson).toList(growable: false),
    );
  }
}

@immutable
class DnsBatchProvisionResponse {
  const DnsBatchProvisionResponse({
    required this.createdCount,
    required this.alreadyMonitoredCount,
    required this.totalRequested,
  });

  final int createdCount;
  final int alreadyMonitoredCount;
  final int totalRequested;

  factory DnsBatchProvisionResponse.fromJson(Map<String, dynamic> json) {
    return DnsBatchProvisionResponse(
      createdCount: parseIntOrNull(json['createdCount']) ?? 0,
      alreadyMonitoredCount: parseIntOrNull(json['alreadyMonitoredCount']) ?? 0,
      totalRequested: parseIntOrNull(json['totalRequested']) ?? 0,
    );
  }
}

@immutable
class VendorRegistryStatus {
  const VendorRegistryStatus({
    required this.entries,
    required this.builtinEntries,
    required this.stale,
    required this.autoUpdate,
    required this.updating,
    required this.sources,
    this.updatedAt,
  });

  final int entries;
  final String? updatedAt;
  final int builtinEntries;
  final bool stale;
  final bool autoUpdate;
  final bool updating;
  final List<String> sources;

  factory VendorRegistryStatus.fromJson(Map<String, dynamic> json) {
    return VendorRegistryStatus(
      entries: parseIntOrNull(json['entries']) ?? 0,
      updatedAt: json['updatedAt'] as String?,
      builtinEntries: parseIntOrNull(json['builtinEntries']) ?? 0,
      stale: json['stale'] == true,
      autoUpdate: json['autoUpdate'] == true,
      updating: json['updating'] == true,
      sources: asStringList(json['sources']),
    );
  }
}

@immutable
class MacVendorLookup {
  const MacVendorLookup({
    required this.source,
    required this.locallyAdministered,
    this.vendor,
    this.organization,
    this.hintDeviceType,
    this.hintConfidence,
  });

  final String? vendor;
  final String? organization;
  final String source;
  final bool locallyAdministered;
  final String? hintDeviceType;
  final double? hintConfidence;

  factory MacVendorLookup.fromJson(Map<String, dynamic> json) {
    final hint = asJsonMap(json['hint']);
    return MacVendorLookup(
      vendor: json['vendor'] as String?,
      organization: json['organization'] as String?,
      source: (json['source'] as String?) ?? 'none',
      locallyAdministered: json['locallyAdministered'] == true,
      hintDeviceType: hint['deviceType'] as String?,
      hintConfidence: parseDoubleOrNull(hint['confidence']),
    );
  }
}

@immutable
class SaasPresetItem {
  const SaasPresetItem({
    required this.id,
    required this.name,
    required this.category,
    required this.target,
    this.type = 'http',
    this.recommended = false,
    this.alreadyProvisioned = false,
    this.description,
  });

  final String id;
  final String name;
  final String category;
  final String target;
  final String type;
  final bool recommended;
  final bool alreadyProvisioned;
  final String? description;

  factory SaasPresetItem.fromJson(Map<String, dynamic> json) {
    return SaasPresetItem(
      id: (json['id'] ?? '').toString(),
      name: (json['name'] ?? '').toString(),
      category: (json['category'] ?? 'cloud').toString(),
      target: (json['target'] ?? json['url'] ?? '').toString(),
      type: (json['type'] ?? 'http').toString(),
      recommended: json['recommended'] == true,
      alreadyProvisioned:
          json['alreadyProvisioned'] == true || json['provisioned'] == true,
      description: json['description'] as String?,
    );
  }
}

@immutable
class HeatmapCell {
  const HeatmapCell({
    required this.day,
    required this.hour,
    this.avgLatencyMs,
    this.p95LatencyMs,
    this.checks = 0,
    this.downChecks = 0,
  });

  final String day;
  final int hour;
  final double? avgLatencyMs;
  final double? p95LatencyMs;
  final int checks;
  final int downChecks;

  factory HeatmapCell.fromJson(Map<String, dynamic> json) {
    return HeatmapCell(
      day: (json['day'] ?? json['date'] ?? '').toString(),
      hour: parseIntOrNull(json['hour']) ?? 0,
      avgLatencyMs: parseDoubleOrNull(json['avgLatencyMs'] ?? json['avgMs']),
      p95LatencyMs: parseDoubleOrNull(json['p95LatencyMs'] ?? json['p95Ms']),
      checks: parseIntOrNull(json['checks'] ?? json['totalChecks']) ?? 0,
      downChecks: parseIntOrNull(json['downChecks']) ?? 0,
    );
  }
}
