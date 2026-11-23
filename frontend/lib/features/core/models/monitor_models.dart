import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

@immutable
class MonitorResult {
  const MonitorResult({
    required this.id,
    required this.monitorId,
    this.probeId,
    required this.status,
    required this.startedAt,
    required this.finishedAt,
    required this.durationMs,
    this.latencyMs,
    this.message,
    this.data,
  });

  final int id;
  final int monitorId;
  final int? probeId;
  final String status;
  final String startedAt;
  final String finishedAt;
  final int durationMs;
  final double? latencyMs;
  final String? message;
  final Map<String, dynamic>? data;

  factory MonitorResult.fromJson(Map<String, dynamic> json) {
    return MonitorResult(
      id: parseIntOrNull(json['id']) ?? DateTime.now().millisecondsSinceEpoch,
      monitorId: parseIntOrNull(json['monitorId']) ?? 0,
      probeId: parseIntOrNull(json['probeId']),
      status: (json['status'] ?? 'unknown').toString(),
      startedAt: (json['startedAt'] ?? json['finishedAt'] ?? '').toString(),
      finishedAt: (json['finishedAt'] ?? json['startedAt'] ?? '').toString(),
      durationMs: parseIntOrNull(json['durationMs']) ?? 0,
      latencyMs: parseDoubleOrNull(json['latencyMs']),
      message: json['message'] as String?,
      data: json['data'] is Map ? asJsonMap(json['data']) : null,
    );
  }
}

@immutable
class MonitorStats {
  const MonitorStats({
    this.avgLatency,
    this.minLatency,
    this.maxLatency,
    this.lastLatency,
    required this.uptimePercentage,
    required this.totalChecks,
    required this.upChecks,
  });

  final double? avgLatency;
  final double? minLatency;
  final double? maxLatency;
  final double? lastLatency;
  final double uptimePercentage;
  final int totalChecks;
  final int upChecks;

  factory MonitorStats.fromJson(Map<String, dynamic> json) {
    return MonitorStats(
      avgLatency: parseDoubleOrNull(json['avgLatency']),
      minLatency: parseDoubleOrNull(json['minLatency']),
      maxLatency: parseDoubleOrNull(json['maxLatency']),
      lastLatency: parseDoubleOrNull(json['lastLatency']),
      uptimePercentage: parseDoubleOrNull(json['uptimePercentage']) ?? 100.0,
      totalChecks: parseIntOrNull(json['totalChecks']) ?? 0,
      upChecks: parseIntOrNull(json['upChecks']) ?? 0,
    );
  }

  static MonitorStats compute(List<MonitorResult> results) {
    final latencies = results
        .map((r) => r.latencyMs)
        .whereType<double>()
        .where((v) => v.isFinite)
        .toList(growable: false);
    final total = results.length;
    final up = results.where((r) => r.status == 'up').length;
    double? avg;
    double? min;
    double? max;
    if (latencies.isNotEmpty) {
      var sum = 0.0;
      min = latencies.first;
      max = latencies.first;
      for (final l in latencies) {
        sum += l;
        if (l < min!) min = l;
        if (l > max!) max = l;
      }
      avg = (sum / latencies.length).roundToDouble();
    }
    return MonitorStats(
      avgLatency: avg,
      minLatency: min,
      maxLatency: max,
      lastLatency: latencies.isNotEmpty ? latencies.last : null,
      uptimePercentage: total > 0
          ? (((up / total) * 1000).roundToDouble() / 10.0)
          : 100.0,
      totalChecks: total,
      upChecks: up,
    );
  }
}

@immutable
class MonitorGaugeSample {
  const MonitorGaugeSample({required this.value, required this.recordedAt});

  final double value;
  final String recordedAt;

  factory MonitorGaugeSample.fromJson(Map<String, dynamic> json) {
    return MonitorGaugeSample(
      value: parseDoubleOrNull(json['value']) ?? 0,
      recordedAt: (json['recordedAt'] ?? '').toString(),
    );
  }
}

@immutable
class MonitorGaugeMetric {
  const MonitorGaugeMetric({
    required this.name,
    required this.value,
    required this.unit,
    required this.recordedAt,
    this.usagePercent,
    this.totalBytes,
  });

  final String name;
  final double value;
  final String unit;
  final String recordedAt;
  final double? usagePercent;
  final double? totalBytes;

  factory MonitorGaugeMetric.fromJson(Map<String, dynamic> json) {
    return MonitorGaugeMetric(
      name: (json['name'] ?? '').toString(),
      value: parseDoubleOrNull(json['value']) ?? 0,
      unit: (json['unit'] ?? '').toString(),
      recordedAt: (json['recordedAt'] ?? '').toString(),
      usagePercent: parseDoubleOrNull(json['usagePercent']),
      totalBytes: parseDoubleOrNull(json['totalBytes']),
    );
  }
}

@immutable
class MonitorModel {
  const MonitorModel({
    required this.id,
    this.deviceId,
    this.probeId,
    required this.name,
    required this.type,
    required this.target,
    this.port,
    this.configuration = const <String, dynamic>{},
    required this.intervalSeconds,
    required this.timeoutSeconds,
    this.retryCount,
    required this.status,
    required this.isEnabled,
    this.lastCheckedAt,
    this.lastLatencyMs,
    this.deviceName,
    this.probeName,
    this.recentResults = const <MonitorResult>[],
    this.stats,
    this.gaugeMetric,
    this.gaugeHistory = const <MonitorGaugeSample>[],
    this.inHistory = const <MonitorGaugeSample>[],
    this.outHistory = const <MonitorGaugeSample>[],
    this.inBps,
    this.outBps,
    this.inMaintenance = false,
  });

  final int id;
  final int? deviceId;
  final int? probeId;
  final String name;
  final String type;
  final String target;
  final int? port;
  final Map<String, dynamic> configuration;
  final int intervalSeconds;
  final int timeoutSeconds;
  final int? retryCount;
  final String status;
  final bool isEnabled;
  final String? lastCheckedAt;
  final double? lastLatencyMs;
  final String? deviceName;
  final String? probeName;
  final List<MonitorResult> recentResults;
  final MonitorStats? stats;
  final MonitorGaugeMetric? gaugeMetric;
  final List<MonitorGaugeSample> gaugeHistory;
  final List<MonitorGaugeSample> inHistory;
  final List<MonitorGaugeSample> outHistory;
  final double? inBps;
  final double? outBps;
  final bool inMaintenance;

  String get snmpMode =>
      (configuration['mode'] ?? configuration['metric'] ?? '').toString();

  bool get isSnmpGauge {
    if (type != 'snmp') return false;
    const modes = <String>{
      'cpu',
      'cpu_usage',
      'memory',
      'memory_usage',
      'interface_traffic',
      'traffic',
      'temperature',
      'disk',
    };
    return modes.contains(snmpMode);
  }

  bool get isTrafficMonitor =>
      type == 'snmp' &&
      (snmpMode == 'interface_traffic' || snmpMode == 'traffic');

  String get normalizedStatus {
    if (!isEnabled) return 'disabled';
    return switch (status.toLowerCase()) {
      'online' || 'up' => 'up',
      'offline' || 'down' => 'down',
      'warning' || 'degraded' => 'warning',
      'disabled' => 'disabled',
      _ => 'unknown',
    };
  }

  MonitorModel copyWith({
    int? id,
    int? deviceId,
    int? probeId,
    String? name,
    String? type,
    String? target,
    int? port,
    Map<String, dynamic>? configuration,
    int? intervalSeconds,
    int? timeoutSeconds,
    int? retryCount,
    String? status,
    bool? isEnabled,
    String? lastCheckedAt,
    double? lastLatencyMs,
    String? deviceName,
    String? probeName,
    List<MonitorResult>? recentResults,
    MonitorStats? stats,
    MonitorGaugeMetric? gaugeMetric,
    List<MonitorGaugeSample>? gaugeHistory,
    List<MonitorGaugeSample>? inHistory,
    List<MonitorGaugeSample>? outHistory,
    double? inBps,
    double? outBps,
    bool? inMaintenance,
  }) {
    return MonitorModel(
      id: id ?? this.id,
      deviceId: deviceId ?? this.deviceId,
      probeId: probeId ?? this.probeId,
      name: name ?? this.name,
      type: type ?? this.type,
      target: target ?? this.target,
      port: port ?? this.port,
      configuration: configuration ?? this.configuration,
      intervalSeconds: intervalSeconds ?? this.intervalSeconds,
      timeoutSeconds: timeoutSeconds ?? this.timeoutSeconds,
      retryCount: retryCount ?? this.retryCount,
      status: status ?? this.status,
      isEnabled: isEnabled ?? this.isEnabled,
      lastCheckedAt: lastCheckedAt ?? this.lastCheckedAt,
      lastLatencyMs: lastLatencyMs ?? this.lastLatencyMs,
      deviceName: deviceName ?? this.deviceName,
      probeName: probeName ?? this.probeName,
      recentResults: recentResults ?? this.recentResults,
      stats: stats ?? this.stats,
      gaugeMetric: gaugeMetric ?? this.gaugeMetric,
      gaugeHistory: gaugeHistory ?? this.gaugeHistory,
      inHistory: inHistory ?? this.inHistory,
      outHistory: outHistory ?? this.outHistory,
      inBps: inBps ?? this.inBps,
      outBps: outBps ?? this.outBps,
      inMaintenance: inMaintenance ?? this.inMaintenance,
    );
  }

  factory MonitorModel.fromJson(Map<String, dynamic> json) {
    final enabled =
        (json['isEnabled'] as bool?) ?? (json['enabled'] as bool?) ?? true;
    final deviceMap = json['device'] is Map ? asJsonMap(json['device']) : null;
    final probeMap = json['probe'] is Map ? asJsonMap(json['probe']) : null;

    return MonitorModel(
      id: parseIntOrNull(json['id']) ?? 0,
      deviceId: parseIntOrNull(json['deviceId']),
      probeId: parseIntOrNull(json['probeId']),
      name: (json['name'] ?? '').toString(),
      type: (json['type'] ?? 'ping').toString(),
      target: (json['target'] ?? '').toString(),
      port: parseIntOrNull(json['port']),
      configuration: asJsonMap(json['configuration']),
      intervalSeconds: parseIntOrNull(json['intervalSeconds']) ?? 60,
      timeoutSeconds: parseIntOrNull(json['timeoutSeconds']) ?? 5,
      retryCount: parseIntOrNull(json['retryCount']),
      status: (json['status'] ?? json['currentStatus'] ?? 'unknown').toString(),
      isEnabled: enabled,
      lastCheckedAt: (json['lastCheckedAt'] ?? json['lastRunAt']) as String?,
      lastLatencyMs: parseDoubleOrNull(
        json['lastLatencyMs'] ?? json['latencyMs'],
      ),
      deviceName: (json['deviceName'] ?? deviceMap?['name']) as String?,
      probeName: (json['probeName'] ?? probeMap?['name']) as String?,
      recentResults: asJsonList<MonitorResult>(
        json['recentResults'],
        MonitorResult.fromJson,
      ),
      stats: json['stats'] is Map
          ? MonitorStats.fromJson(asJsonMap(json['stats']))
          : null,
      gaugeMetric: json['gaugeMetric'] is Map
          ? MonitorGaugeMetric.fromJson(asJsonMap(json['gaugeMetric']))
          : null,
      gaugeHistory: asJsonList<MonitorGaugeSample>(
        json['gaugeHistory'],
        MonitorGaugeSample.fromJson,
      ),
      inHistory: asJsonList<MonitorGaugeSample>(
        json['inHistory'],
        MonitorGaugeSample.fromJson,
      ),
      outHistory: asJsonList<MonitorGaugeSample>(
        json['outHistory'],
        MonitorGaugeSample.fromJson,
      ),
      inBps: parseDoubleOrNull(json['inBps']),
      outBps: parseDoubleOrNull(json['outBps']),
      inMaintenance: json['inMaintenance'] == true,
    );
  }
}

@immutable
class MonitorUptimeResponse {
  const MonitorUptimeResponse({
    required this.monitorId,
    required this.hours,
    required this.uptimePercentage,
    required this.totalChecks,
    required this.upChecks,
    required this.downChecks,
    required this.unknownChecks,
    this.avgLatencyMs,
  });

  final int monitorId;
  final int hours;
  final double uptimePercentage;
  final int totalChecks;
  final int upChecks;
  final int downChecks;
  final int unknownChecks;
  final double? avgLatencyMs;

  factory MonitorUptimeResponse.fromJson(Map<String, dynamic> json) {
    return MonitorUptimeResponse(
      monitorId: parseIntOrNull(json['monitorId']) ?? 0,
      hours: parseIntOrNull(json['hours']) ?? 24,
      uptimePercentage: parseDoubleOrNull(json['uptimePercentage']) ?? 0,
      totalChecks: parseIntOrNull(json['totalChecks']) ?? 0,
      upChecks: parseIntOrNull(json['upChecks']) ?? 0,
      downChecks: parseIntOrNull(json['downChecks']) ?? 0,
      unknownChecks: parseIntOrNull(json['unknownChecks']) ?? 0,
      avgLatencyMs: parseDoubleOrNull(json['avgLatencyMs']),
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
    required this.type,
    this.description,
    this.provisioned = false,
  });

  final String id;
  final String name;
  final String category;
  final String target;
  final String type;
  final String? description;
  final bool provisioned;

  factory SaasPresetItem.fromJson(Map<String, dynamic> json) {
    return SaasPresetItem(
      id: (json['id'] ?? '').toString(),
      name: (json['name'] ?? '').toString(),
      category: (json['category'] ?? 'Outros').toString(),
      target: (json['target'] ?? json['url'] ?? '').toString(),
      type: (json['checkType'] ?? json['type'] ?? 'https').toString(),
      description: json['description'] as String?,
      provisioned:
          json['isProvisioned'] == true ||
          json['provisioned'] == true ||
          json['exists'] == true,
    );
  }
}

@immutable
class DeviceRefOption {
  const DeviceRefOption({
    required this.id,
    required this.name,
    this.ipAddress,
    this.status,
  });

  final int id;
  final String name;
  final String? ipAddress;
  final String? status;

  factory DeviceRefOption.fromJson(Map<String, dynamic> json) {
    return DeviceRefOption(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] ?? '').toString(),
      ipAddress: json['ipAddress'] as String?,
      status: json['status'] as String?,
    );
  }
}

@immutable
class ProbeRefOption {
  const ProbeRefOption({required this.id, required this.name, this.status});

  final int id;
  final String name;
  final String? status;

  factory ProbeRefOption.fromJson(Map<String, dynamic> json) {
    return ProbeRefOption(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] ?? '').toString(),
      status: json['status'] as String?,
    );
  }
}

@immutable
class SiteRefOption {
  const SiteRefOption({required this.id, required this.name});

  final int id;
  final String name;

  factory SiteRefOption.fromJson(Map<String, dynamic> json) {
    return SiteRefOption(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] ?? '').toString(),
    );
  }
}

@immutable
class DeviceInterfaceRef {
  const DeviceInterfaceRef({
    required this.id,
    required this.name,
    this.ifIndex,
    this.operStatus,
  });

  final int id;
  final String name;
  final int? ifIndex;
  final String? operStatus;

  factory DeviceInterfaceRef.fromJson(Map<String, dynamic> json) {
    return DeviceInterfaceRef(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] ?? json['ifName'] ?? '').toString(),
      ifIndex: parseIntOrNull(json['ifIndex']),
      operStatus: json['operStatus'] as String?,
    );
  }
}
