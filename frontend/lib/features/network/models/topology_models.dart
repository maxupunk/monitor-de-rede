import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';

enum TopologyLayoutMode { hierarchical, force, radial, grid }

@immutable
class TopologyPosition {
  const TopologyPosition(this.x, this.y);

  final double x;
  final double y;

  TopologyPosition translate(double dx, double dy) =>
      TopologyPosition(x + dx, y + dy);
}

@immutable
class TopologyNodeModel {
  const TopologyNodeModel({
    required this.id,
    required this.name,
    required this.type,
    required this.status,
    this.siteId,
    this.siteName,
    this.interfaceCount = 0,
    this.ipAddress,
    this.vendor,
    this.model,
    this.snmpEnabled = false,
    this.parentId,
    this.isSystem = false,
    this.monitorsDown = 0,
    this.cpuPercent,
    this.memoryPercent,
  });

  final int id;
  final String name;
  final String type;
  final String status;
  final int? siteId;
  final String? siteName;
  final int interfaceCount;
  final String? ipAddress;
  final String? vendor;
  final String? model;
  final bool snmpEnabled;
  final int? parentId;
  final bool isSystem;
  final int monitorsDown;
  final double? cpuPercent;
  final double? memoryPercent;

  factory TopologyNodeModel.fromJson(Map<String, dynamic> json) {
    return TopologyNodeModel(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] as String?) ?? 'Dispositivo',
      type: (json['type'] as String?) ?? 'other',
      status: (json['status'] as String?) ?? 'unknown',
      siteId: parseIntOrNull(json['siteId']),
      siteName: json['siteName'] as String?,
      interfaceCount: parseIntOrNull(json['interfaceCount']) ?? 0,
      ipAddress: json['ipAddress'] as String?,
      vendor: json['vendor'] as String?,
      model: json['model'] as String?,
      snmpEnabled: json['snmpEnabled'] == true,
      parentId: parseIntOrNull(json['parentId']),
      isSystem: json['isSystem'] == true,
      monitorsDown: parseIntOrNull(json['monitorsDown']) ?? 0,
      cpuPercent: parseDoubleOrNull(json['cpuPercent']),
      memoryPercent: parseDoubleOrNull(json['memoryPercent']),
    );
  }

  TopologyNodeModel copyWith({
    String? name,
    String? status,
    String? ipAddress,
    int? interfaceCount,
    double? cpuPercent,
    double? memoryPercent,
  }) {
    return TopologyNodeModel(
      id: id,
      name: name ?? this.name,
      type: type,
      status: status ?? this.status,
      siteId: siteId,
      siteName: siteName,
      interfaceCount: interfaceCount ?? this.interfaceCount,
      ipAddress: ipAddress ?? this.ipAddress,
      vendor: vendor,
      model: model,
      snmpEnabled: snmpEnabled,
      parentId: parentId,
      isSystem: isSystem,
      monitorsDown: monitorsDown,
      cpuPercent: cpuPercent ?? this.cpuPercent,
      memoryPercent: memoryPercent ?? this.memoryPercent,
    );
  }
}

@immutable
class TopologyEdgeModel {
  const TopologyEdgeModel({
    required this.id,
    required this.sourceDeviceId,
    required this.targetDeviceId,
    required this.linkType,
    required this.discoveryMethod,
    required this.status,
    this.sourceDeviceName,
    this.targetDeviceName,
    this.sourceInterfaceId,
    this.targetInterfaceId,
    this.sourceInterfaceName,
    this.targetInterfaceName,
    this.sourceInterfaceSpeed,
    this.targetInterfaceSpeed,
    this.sourceInterfaceStatus,
    this.targetInterfaceStatus,
    this.inBps,
    this.outBps,
    this.trafficBps,
    this.trafficLabel,
    this.speedBps,
    this.utilization,
    this.confidence = 1,
    this.confirmed = false,
    this.stale = false,
    this.lastSeenAt,
  });

  final int id;
  final int sourceDeviceId;
  final int targetDeviceId;
  final String? sourceDeviceName;
  final String? targetDeviceName;
  final int? sourceInterfaceId;
  final int? targetInterfaceId;
  final String? sourceInterfaceName;
  final String? targetInterfaceName;
  final double? sourceInterfaceSpeed;
  final double? targetInterfaceSpeed;
  final String? sourceInterfaceStatus;
  final String? targetInterfaceStatus;
  final double? inBps;
  final double? outBps;
  final double? trafficBps;
  final String? trafficLabel;
  final double? speedBps;
  final double? utilization;
  final String linkType;
  final String discoveryMethod;
  final double confidence;
  final bool confirmed;
  final String status;
  final bool stale;
  final String? lastSeenAt;

  bool get isVirtualHierarchy => id < 0;

  String get formattedTraffic {
    if (trafficLabel != null && trafficLabel!.isNotEmpty) return trafficLabel!;
    if (trafficBps != null) return formatBps(trafficBps);
    return '';
  }

  factory TopologyEdgeModel.fromJson(Map<String, dynamic> json) {
    final inB = parseDoubleOrNull(json['inBps']);
    final outB = parseDoubleOrNull(json['outBps']);
    final total =
        parseDoubleOrNull(json['trafficBps']) ??
        (inB != null && outB != null ? inB + outB : (inB ?? outB));
    return TopologyEdgeModel(
      id: parseIntOrNull(json['id']) ?? 0,
      sourceDeviceId:
          parseIntOrNull(json['sourceDeviceId'] ?? json['source']) ?? 0,
      targetDeviceId:
          parseIntOrNull(json['targetDeviceId'] ?? json['target']) ?? 0,
      sourceDeviceName: json['sourceDeviceName'] as String?,
      targetDeviceName: json['targetDeviceName'] as String?,
      sourceInterfaceId: parseIntOrNull(json['sourceInterfaceId']),
      targetInterfaceId: parseIntOrNull(json['targetInterfaceId']),
      sourceInterfaceName: json['sourceInterfaceName'] as String?,
      targetInterfaceName: json['targetInterfaceName'] as String?,
      sourceInterfaceSpeed: parseDoubleOrNull(json['sourceInterfaceSpeed']),
      targetInterfaceSpeed: parseDoubleOrNull(json['targetInterfaceSpeed']),
      sourceInterfaceStatus: json['sourceInterfaceStatus'] as String?,
      targetInterfaceStatus: json['targetInterfaceStatus'] as String?,
      inBps: inB,
      outBps: outB,
      trafficBps: total,
      trafficLabel:
          (json['trafficLabel'] as String?) ??
          (total != null ? formatBps(total) : null),
      speedBps: parseDoubleOrNull(json['speedBps']),
      utilization: parseDoubleOrNull(json['utilization']),
      linkType: (json['linkType'] as String?) ?? 'ethernet',
      discoveryMethod: (json['discoveryMethod'] as String?) ?? 'manual',
      confidence: parseDoubleOrNull(json['confidence']) ?? 1,
      confirmed: json['confirmed'] == true,
      status: (json['status'] as String?) ?? 'unknown',
      stale: json['stale'] == true,
      lastSeenAt: json['lastSeenAt'] as String?,
    );
  }

  TopologyEdgeModel withTrafficUpdate({
    double? inBps,
    double? outBps,
    String? sourceInterfaceStatus,
    String? targetInterfaceStatus,
    double? sourceInterfaceSpeed,
    double? targetInterfaceSpeed,
  }) {
    final nextIn = inBps ?? this.inBps;
    final nextOut = outBps ?? this.outBps;
    final nextSrcStatus = sourceInterfaceStatus ?? this.sourceInterfaceStatus;
    final nextTgtStatus = targetInterfaceStatus ?? this.targetInterfaceStatus;
    final nextSrcSpeed = sourceInterfaceSpeed ?? this.sourceInterfaceSpeed;
    final nextTgtSpeed = targetInterfaceSpeed ?? this.targetInterfaceSpeed;

    final total = nextIn != null && nextOut != null
        ? nextIn + nextOut
        : (nextIn ?? nextOut ?? trafficBps);
    final effectiveSpeed = speedBps ?? _minPositive(nextSrcSpeed, nextTgtSpeed);
    final peak = nextIn != null || nextOut != null
        ? ((nextIn ?? 0) > (nextOut ?? 0) ? (nextIn ?? 0) : (nextOut ?? 0))
        : null;
    final nextUtil =
        (effectiveSpeed != null && effectiveSpeed > 0 && peak != null)
        ? (peak / effectiveSpeed).clamp(0.0, 1.0)
        : utilization;
    final nextStatus = (nextSrcStatus == 'down' || nextTgtStatus == 'down')
        ? 'down'
        : 'up';

    return TopologyEdgeModel(
      id: id,
      sourceDeviceId: sourceDeviceId,
      targetDeviceId: targetDeviceId,
      sourceDeviceName: sourceDeviceName,
      targetDeviceName: targetDeviceName,
      sourceInterfaceId: sourceInterfaceId,
      targetInterfaceId: targetInterfaceId,
      sourceInterfaceName: sourceInterfaceName,
      targetInterfaceName: targetInterfaceName,
      sourceInterfaceSpeed: nextSrcSpeed,
      targetInterfaceSpeed: nextTgtSpeed,
      sourceInterfaceStatus: nextSrcStatus,
      targetInterfaceStatus: nextTgtStatus,
      inBps: nextIn,
      outBps: nextOut,
      trafficBps: total,
      trafficLabel: total != null ? formatBps(total) : trafficLabel,
      speedBps: effectiveSpeed,
      utilization: nextUtil,
      linkType: linkType,
      discoveryMethod: discoveryMethod,
      confidence: confidence,
      confirmed: confirmed,
      status: nextStatus,
      stale: stale,
      lastSeenAt: lastSeenAt,
    );
  }

  static double? _minPositive(double? a, double? b) {
    final vals = <double>[if (a != null && a > 0) a, if (b != null && b > 0) b];
    if (vals.isEmpty) return null;
    return vals.reduce((x, y) => x < y ? x : y);
  }
}

@immutable
class TopologyPathModel {
  const TopologyPathModel({required this.deviceIds, required this.edgeIds});

  final List<int> deviceIds;
  final List<int> edgeIds;

  factory TopologyPathModel.fromJson(Map<String, dynamic> json) {
    return TopologyPathModel(
      deviceIds: asIntList(json['deviceIds']),
      edgeIds: asIntList(json['edgeIds']),
    );
  }
}

@immutable
class TopologyPluginActionModel {
  const TopologyPluginActionModel({
    required this.pluginId,
    required this.pluginName,
    required this.action,
    required this.label,
    this.effect = 'read',
  });

  final int pluginId;
  final String pluginName;
  final String action;
  final String label;
  final String effect;

  factory TopologyPluginActionModel.fromJson(Map<String, dynamic> json) {
    return TopologyPluginActionModel(
      pluginId: parseIntOrNull(json['pluginId']) ?? 0,
      pluginName: (json['pluginName'] as String?) ?? 'Plugin',
      action: (json['action'] as String?) ?? '',
      label: (json['label'] ?? json['title'] ?? json['action'] ?? '')
          .toString(),
      effect: (json['effect'] as String?) ?? 'read',
    );
  }
}

@immutable
class TopologyViewPrefs {
  const TopologyViewPrefs({
    this.showTrafficLabels = true,
    this.showIpLabels = true,
    this.showHealthBadges = true,
    this.colorByUtilization = true,
    this.showIsolated = true,
  });

  final bool showTrafficLabels;
  final bool showIpLabels;
  final bool showHealthBadges;
  final bool colorByUtilization;
  final bool showIsolated;

  TopologyViewPrefs copyWith({
    bool? showTrafficLabels,
    bool? showIpLabels,
    bool? showHealthBadges,
    bool? colorByUtilization,
    bool? showIsolated,
  }) {
    return TopologyViewPrefs(
      showTrafficLabels: showTrafficLabels ?? this.showTrafficLabels,
      showIpLabels: showIpLabels ?? this.showIpLabels,
      showHealthBadges: showHealthBadges ?? this.showHealthBadges,
      colorByUtilization: colorByUtilization ?? this.colorByUtilization,
      showIsolated: showIsolated ?? this.showIsolated,
    );
  }
}
