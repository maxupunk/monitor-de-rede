import 'package:flutter/foundation.dart';

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
      id:
          (json['id'] as num?)?.toInt() ??
          DateTime.now().millisecondsSinceEpoch,
      monitorId: (json['monitorId'] as num?)?.toInt() ?? 0,
      probeId: (json['probeId'] as num?)?.toInt(),
      status: (json['status'] ?? 'unknown').toString(),
      startedAt: (json['startedAt'] ?? json['finishedAt'] ?? '').toString(),
      finishedAt: (json['finishedAt'] ?? json['startedAt'] ?? '').toString(),
      durationMs: (json['durationMs'] as num?)?.toInt() ?? 0,
      latencyMs: (json['latencyMs'] as num?)?.toDouble(),
      message: json['message'] as String?,
      data: json['data'] is Map
          ? Map<String, dynamic>.from(json['data'] as Map)
          : null,
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
      avgLatency: (json['avgLatency'] as num?)?.toDouble(),
      minLatency: (json['minLatency'] as num?)?.toDouble(),
      maxLatency: (json['maxLatency'] as num?)?.toDouble(),
      lastLatency: (json['lastLatency'] as num?)?.toDouble(),
      uptimePercentage: (json['uptimePercentage'] as num?)?.toDouble() ?? 100.0,
      totalChecks: (json['totalChecks'] as num?)?.toInt() ?? 0,
      upChecks: (json['upChecks'] as num?)?.toInt() ?? 0,
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
          ? double.parse(((up / total) * 100).toStringAsFixed(1))
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
      value: (json['value'] as num?)?.toDouble() ?? 0,
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
      value: (json['value'] as num?)?.toDouble() ?? 0,
      unit: (json['unit'] ?? '').toString(),
      recordedAt: (json['recordedAt'] ?? '').toString(),
      usagePercent: (json['usagePercent'] as num?)?.toDouble(),
      totalBytes: (json['totalBytes'] as num?)?.toDouble(),
    );
  }
}

@immutable
class MonitorModel {
  const MonitorModel({
    required this.id,
    required this.deviceId,
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
  final int deviceId;
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
    final deviceMap = json['device'] is Map
        ? Map<String, dynamic>.from(json['device'] as Map)
        : null;
    final probeMap = json['probe'] is Map
        ? Map<String, dynamic>.from(json['probe'] as Map)
        : null;
    final rawResults = json['recentResults'] is List
        ? json['recentResults'] as List
        : const [];
    final rawGaugeHistory = json['gaugeHistory'] is List
        ? json['gaugeHistory'] as List
        : const [];
    final rawInHistory = json['inHistory'] is List
        ? json['inHistory'] as List
        : const [];
    final rawOutHistory = json['outHistory'] is List
        ? json['outHistory'] as List
        : const [];

    return MonitorModel(
      id: (json['id'] as num?)?.toInt() ?? 0,
      deviceId: (json['deviceId'] as num?)?.toInt() ?? 0,
      probeId: (json['probeId'] as num?)?.toInt(),
      name: (json['name'] ?? '').toString(),
      type: (json['type'] ?? 'ping').toString(),
      target: (json['target'] ?? '').toString(),
      port: (json['port'] as num?)?.toInt(),
      configuration: json['configuration'] is Map
          ? Map<String, dynamic>.from(json['configuration'] as Map)
          : const <String, dynamic>{},
      intervalSeconds: (json['intervalSeconds'] as num?)?.toInt() ?? 60,
      timeoutSeconds: (json['timeoutSeconds'] as num?)?.toInt() ?? 5,
      retryCount: (json['retryCount'] as num?)?.toInt(),
      status: (json['status'] ?? json['currentStatus'] ?? 'unknown').toString(),
      isEnabled: enabled,
      lastCheckedAt: json['lastCheckedAt'] as String?,
      lastLatencyMs: ((json['lastLatencyMs'] ?? json['latencyMs']) as num?)
          ?.toDouble(),
      deviceName: deviceMap?['name'] as String?,
      probeName: probeMap?['name'] as String?,
      recentResults: rawResults
          .whereType<Map>()
          .map((e) => MonitorResult.fromJson(Map<String, dynamic>.from(e)))
          .toList(growable: false),
      stats: json['stats'] is Map
          ? MonitorStats.fromJson(
              Map<String, dynamic>.from(json['stats'] as Map),
            )
          : null,
      gaugeMetric: json['gaugeMetric'] is Map
          ? MonitorGaugeMetric.fromJson(
              Map<String, dynamic>.from(json['gaugeMetric'] as Map),
            )
          : null,
      gaugeHistory: rawGaugeHistory
          .whereType<Map>()
          .map((e) => MonitorGaugeSample.fromJson(Map<String, dynamic>.from(e)))
          .toList(growable: false),
      inHistory: rawInHistory
          .whereType<Map>()
          .map((e) => MonitorGaugeSample.fromJson(Map<String, dynamic>.from(e)))
          .toList(growable: false),
      outHistory: rawOutHistory
          .whereType<Map>()
          .map((e) => MonitorGaugeSample.fromJson(Map<String, dynamic>.from(e)))
          .toList(growable: false),
      inBps: (json['inBps'] as num?)?.toDouble(),
      outBps: (json['outBps'] as num?)?.toDouble(),
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
      monitorId: (json['monitorId'] as num?)?.toInt() ?? 0,
      hours: (json['hours'] as num?)?.toInt() ?? 24,
      uptimePercentage: (json['uptimePercentage'] as num?)?.toDouble() ?? 0,
      totalChecks: (json['totalChecks'] as num?)?.toInt() ?? 0,
      upChecks: (json['upChecks'] as num?)?.toInt() ?? 0,
      downChecks: (json['downChecks'] as num?)?.toInt() ?? 0,
      unknownChecks: (json['unknownChecks'] as num?)?.toInt() ?? 0,
      avgLatencyMs: (json['avgLatencyMs'] as num?)?.toDouble(),
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
      type: (json['type'] ?? 'https').toString(),
      description: json['description'] as String?,
      provisioned: json['provisioned'] == true || json['exists'] == true,
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
      id: (json['id'] as num?)?.toInt() ?? 0,
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
      id: (json['id'] as num?)?.toInt() ?? 0,
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
      id: (json['id'] as num?)?.toInt() ?? 0,
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
      id: (json['id'] as num?)?.toInt() ?? 0,
      name: (json['name'] ?? json['ifName'] ?? '').toString(),
      ifIndex: (json['ifIndex'] as num?)?.toInt(),
      operStatus: json['operStatus'] as String?,
    );
  }
}
