import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

export 'package:netmonitor_app/core/utils/json_parsers.dart'
    show
        asIntList,
        asJsonMap,
        asJsonMapList,
        asStringList,
        parseDoubleOrNull,
        parseIntOrNull;
export 'package:netmonitor_app/shared/device_access/device_access_models.dart'
    show
        AgentRouteOption,
        DedicatedAccessInfo,
        DeviceAccessViewModel,
        DeviceCredentialView;

@immutable
class DeviceRef {
  const DeviceRef({required this.id, required this.name, this.cidr});

  final int id;
  final String name;
  final String? cidr;

  static DeviceRef? tryFromJson(Object? raw) {
    if (raw is! Map) return null;
    final json = Map<String, dynamic>.from(raw);
    final id = parseIntOrNull(json['id']);
    if (id == null) return null;
    return DeviceRef(
      id: id,
      name: (json['name'] as String?) ?? '#$id',
      cidr: json['cidr'] as String?,
    );
  }
}

@immutable
class DeviceModel {
  const DeviceModel({
    required this.id,
    required this.name,
    required this.type,
    required this.status,
    this.siteId,
    this.networkId,
    this.parentId,
    this.vendor,
    this.model,
    this.ipAddress,
    this.macAddress,
    this.isMonitored = true,
    this.snmpEnabled = false,
    this.snmpProfileId,
    this.snmpPollIntervalSeconds,
    this.accessMode,
    this.effectiveAccessMode,
    this.accessModeReason,
    this.operatingSystem,
    this.effectiveOperatingSystem,
    this.operatingSystemSource,
    this.systemKey,
    this.isSystem = false,
    this.site,
    this.parent,
    this.network,
    this.linkInterfaceId,
    this.linkInterfaceName,
    this.location,
    this.description,
    this.uptimeSeconds,
    this.lastSeenAt,
    this.createdAt,
    this.updatedAt,
  });

  final int id;
  final int? siteId;
  final int? networkId;
  final int? parentId;
  final String name;
  final String type;
  final String? vendor;
  final String? model;
  final String? ipAddress;
  final String? macAddress;
  final bool isMonitored;
  final String status;
  final bool snmpEnabled;

  /// A credencial SNMP (`/api/snmp/credentials`); `null` é a padrão.
  final int? snmpProfileId;
  final int? snmpPollIntervalSeconds;
  final String? accessMode;
  final String? effectiveAccessMode;
  final String? accessModeReason;
  final String? operatingSystem;
  final String? effectiveOperatingSystem;
  final String? operatingSystemSource;
  final String? systemKey;
  final bool isSystem;
  final DeviceRef? site;
  final DeviceRef? parent;
  final DeviceRef? network;
  final int? linkInterfaceId;
  final String? linkInterfaceName;
  final String? location;
  final String? description;
  final int? uptimeSeconds;
  final String? lastSeenAt;
  final String? createdAt;
  final String? updatedAt;

  bool get hasIp => ipAddress != null && ipAddress!.trim().isNotEmpty;

  factory DeviceModel.fromJson(Map<String, dynamic> json) {
    return DeviceModel(
      id: parseIntOrNull(json['id']) ?? 0,
      siteId: parseIntOrNull(json['siteId']),
      networkId: parseIntOrNull(json['networkId']),
      parentId: parseIntOrNull(json['parentId']),
      name: (json['name'] as String?) ?? 'Sem nome',
      type: (json['type'] as String?) ?? 'other',
      vendor: json['vendor'] as String?,
      model: json['model'] as String?,
      ipAddress: json['ipAddress'] as String?,
      macAddress: json['macAddress'] as String?,
      isMonitored: (json['isMonitored'] as bool?) ?? true,
      status: (json['status'] as String?) ?? 'unknown',
      snmpEnabled: (json['snmpEnabled'] as bool?) ?? false,
      snmpProfileId: parseIntOrNull(json['snmpProfileId']),
      snmpPollIntervalSeconds: parseIntOrNull(json['snmpPollIntervalSeconds']),
      accessMode: json['accessMode'] as String?,
      effectiveAccessMode: json['effectiveAccessMode'] as String?,
      accessModeReason: json['accessModeReason'] as String?,
      operatingSystem: json['operatingSystem'] as String?,
      effectiveOperatingSystem: json['effectiveOperatingSystem'] as String?,
      operatingSystemSource: json['operatingSystemSource'] as String?,
      systemKey: json['systemKey'] as String?,
      isSystem: json['isSystem'] == true,
      site: DeviceRef.tryFromJson(json['site']),
      parent: DeviceRef.tryFromJson(json['parent']),
      network: DeviceRef.tryFromJson(json['network']),
      linkInterfaceId: parseIntOrNull(json['linkInterfaceId']),
      linkInterfaceName: json['linkInterfaceName'] as String?,
      location: json['location'] as String?,
      description: json['description'] as String?,
      uptimeSeconds: parseIntOrNull(json['uptimeSeconds']),
      lastSeenAt: json['lastSeenAt'] as String?,
      createdAt: json['createdAt'] as String?,
      updatedAt: json['updatedAt'] as String?,
    );
  }

  DeviceModel copyWith({
    String? name,
    String? status,
    String? ipAddress,
    String? lastSeenAt,
    int? parentId,
  }) {
    return DeviceModel(
      id: id,
      siteId: siteId,
      networkId: networkId,
      parentId: parentId ?? this.parentId,
      name: name ?? this.name,
      type: type,
      vendor: vendor,
      model: model,
      ipAddress: ipAddress ?? this.ipAddress,
      macAddress: macAddress,
      isMonitored: isMonitored,
      status: status ?? this.status,
      snmpEnabled: snmpEnabled,
      snmpProfileId: snmpProfileId,
      snmpPollIntervalSeconds: snmpPollIntervalSeconds,
      accessMode: accessMode,
      effectiveAccessMode: effectiveAccessMode,
      accessModeReason: accessModeReason,
      operatingSystem: operatingSystem,
      effectiveOperatingSystem: effectiveOperatingSystem,
      operatingSystemSource: operatingSystemSource,
      systemKey: systemKey,
      isSystem: isSystem,
      site: site,
      parent: parent,
      network: network,
      linkInterfaceId: linkInterfaceId,
      linkInterfaceName: linkInterfaceName,
      location: location,
      description: description,
      uptimeSeconds: uptimeSeconds,
      lastSeenAt: lastSeenAt ?? this.lastSeenAt,
      createdAt: createdAt,
      updatedAt: updatedAt,
    );
  }
}

String? _normalizeInterfaceStatus(Object? raw) {
  if (raw == null) return null;
  if (raw is num) {
    return switch (raw.toInt()) {
      1 => 'up',
      2 => 'down',
      3 => 'testing',
      5 => 'dormant',
      6 => 'notPresent',
      7 => 'lowerLayerDown',
      _ => 'unknown',
    };
  }
  final text = raw.toString().trim();
  if (text.isEmpty) return null;
  final numeric = int.tryParse(text);
  if (numeric != null) return _normalizeInterfaceStatus(numeric);
  return switch (text.toLowerCase()) {
    'up' => 'up',
    'down' => 'down',
    'testing' => 'testing',
    'dormant' => 'dormant',
    'notpresent' => 'notPresent',
    'lowerlayerdown' => 'lowerLayerDown',
    'unknown' => 'unknown',
    _ => text,
  };
}

double? _parseInterfaceSpeedBps(Map<String, dynamic> json) {
  // Backend (`SnmpInterface.ifSpeed` / `DeviceInterfaceView.speed`) já envia
  // a velocidade negociada em bps (incorporando `ifHighSpeed * 1_000_000`).
  // Ainda assim suportamos payloads com `ifHighSpeed` / `highSpeed` / `speedMbps`.
  final highSpeedMbps = parseDoubleOrNull(
    json['ifHighSpeed'] ??
        json['highSpeed'] ??
        json['speedMbps'] ??
        json['if_high_speed'] ??
        json['speed_mbps'],
  );
  final rawBps = parseDoubleOrNull(
    json['ifSpeed'] ??
        json['speed'] ??
        json['speedBps'] ??
        json['if_speed'] ??
        json['speed_bps'],
  );
  if (highSpeedMbps != null && highSpeedMbps > 0) {
    if (rawBps == null || rawBps <= 0 || rawBps >= 4294967295) {
      return highSpeedMbps * 1000000;
    }
  }
  return rawBps;
}

@immutable
class DeviceInterfaceModel {
  const DeviceInterfaceModel({
    required this.id,
    required this.deviceId,
    this.snmpIndex,
    this.ifIndex,
    this.name,
    this.ifName,
    this.ifType,
    this.description,
    this.alias,
    this.adminStatus,
    this.operStatus,
    this.speed,
    this.macAddress,
    this.ipAddress,
    this.inBps,
    this.outBps,
    this.isMonitored = false,
  });

  final int id;
  final int deviceId;
  final int? snmpIndex;
  final int? ifIndex;
  final String? name;
  final String? ifName;
  final String? ifType;
  final String? description;
  final String? alias;
  final String? adminStatus;
  final String? operStatus;
  final double? speed;
  final String? macAddress;
  final String? ipAddress;
  final double? inBps;
  final double? outBps;
  final bool isMonitored;

  String get displayName {
    final primary = ifName?.trim();
    if (primary != null && primary.isNotEmpty) return primary;
    final fallback = name?.trim();
    if (fallback != null && fallback.isNotEmpty) return fallback;
    return 'if-${ifIndex ?? snmpIndex ?? id}';
  }

  /// Indica se a interface está operacionalmente ativa (`up`).
  bool get isUp => operStatus?.trim().toLowerCase() == 'up';

  /// Velocidade negociada em bps (`0` quando ausente ou inválida).
  double get negotiatedSpeedBps {
    final s = speed;
    return (s != null && s.isFinite && s > 0) ? s : 0;
  }

  /// Alias ou descrição complementar quando traz informação além do nome.
  String? get subtitleText {
    final cleanName = displayName.toLowerCase();
    final cleanAlias = alias?.trim();
    if (cleanAlias != null &&
        cleanAlias.isNotEmpty &&
        cleanAlias.toLowerCase() != cleanName) {
      return cleanAlias;
    }
    final cleanDesc = description?.trim();
    if (cleanDesc != null &&
        cleanDesc.isNotEmpty &&
        cleanDesc.toLowerCase() != cleanName) {
      return cleanDesc;
    }
    return null;
  }

  /// Ordena interfaces para seleção de uplink: UP primeiro (maior velocidade
  /// negociada primeiro), depois DOWN, desempatando por índice SNMP e nome.
  static int compareForUplinkSelection(
    DeviceInterfaceModel a,
    DeviceInterfaceModel b,
  ) {
    if (a.isUp != b.isUp) return a.isUp ? -1 : 1;
    final speedCmp = b.negotiatedSpeedBps.compareTo(a.negotiatedSpeedBps);
    if (speedCmp != 0) return speedCmp;
    final aIdx = a.ifIndex ?? a.snmpIndex ?? a.id;
    final bIdx = b.ifIndex ?? b.snmpIndex ?? b.id;
    final idxCmp = aIdx.compareTo(bIdx);
    if (idxCmp != 0) return idxCmp;
    return a.displayName.toLowerCase().compareTo(b.displayName.toLowerCase());
  }

  /// Retorna lista ordenada e deduplicada por `displayName` para uso seguro
  /// em menus suspensos (`DropdownButtonFormField`).
  static List<DeviceInterfaceModel> sortForUplinkSelection(
    Iterable<DeviceInterfaceModel> interfaces,
  ) {
    final sorted = interfaces.toList()..sort(compareForUplinkSelection);
    final seen = <String>{};
    final unique = <DeviceInterfaceModel>[];
    for (final iface in sorted) {
      if (seen.add(iface.displayName)) {
        unique.add(iface);
      }
    }
    return List<DeviceInterfaceModel>.unmodifiable(unique);
  }

  factory DeviceInterfaceModel.fromJson(Map<String, dynamic> json) {
    return DeviceInterfaceModel(
      id: parseIntOrNull(json['id']) ?? 0,
      deviceId: parseIntOrNull(json['deviceId']) ?? 0,
      snmpIndex: parseIntOrNull(json['snmpIndex']),
      ifIndex: parseIntOrNull(
        json['ifIndex'] ?? json['if_index'] ?? json['snmpIndex'],
      ),
      name: (json['name'] ?? json['ifName'] ?? json['if_name']) as String?,
      ifName: (json['ifName'] ?? json['if_name']) as String?,
      ifType: json['ifType']?.toString(),
      description:
          (json['description'] ?? json['ifDescr'] ?? json['if_descr'])
              as String?,
      alias: (json['alias'] ?? json['ifAlias'] ?? json['if_alias']) as String?,
      adminStatus: _normalizeInterfaceStatus(
        json['ifAdminStatus'] ??
            json['adminStatus'] ??
            json['if_admin_status'] ??
            json['admin_status'],
      ),
      operStatus: _normalizeInterfaceStatus(
        json['ifOperStatus'] ??
            json['operStatus'] ??
            json['if_oper_status'] ??
            json['oper_status'],
      ),
      speed: _parseInterfaceSpeedBps(json),
      macAddress: json['macAddress'] as String?,
      ipAddress: json['ipAddress'] as String?,
      inBps: parseDoubleOrNull(json['inBps']),
      outBps: parseDoubleOrNull(json['outBps']),
      isMonitored: json['isMonitored'] == true,
    );
  }

  DeviceInterfaceModel copyWith({
    String? operStatus,
    double? speed,
    double? inBps,
    double? outBps,
    bool? isMonitored,
  }) {
    return DeviceInterfaceModel(
      id: id,
      deviceId: deviceId,
      snmpIndex: snmpIndex,
      ifIndex: ifIndex,
      name: name,
      ifName: ifName,
      ifType: ifType,
      description: description,
      alias: alias,
      adminStatus: adminStatus,
      operStatus: operStatus ?? this.operStatus,
      speed: speed ?? this.speed,
      macAddress: macAddress,
      ipAddress: ipAddress,
      inBps: inBps ?? this.inBps,
      outBps: outBps ?? this.outBps,
      isMonitored: isMonitored ?? this.isMonitored,
    );
  }
}

@immutable
class DeviceMetricModel {
  const DeviceMetricModel({
    required this.id,
    required this.deviceId,
    required this.metricName,
    required this.metricValue,
    required this.createdAt,
    this.interfaceId,
    this.interfaceName,
    this.unit,
  });

  final int id;
  final int deviceId;
  final int? interfaceId;
  final String? interfaceName;
  final String metricName;
  final double metricValue;
  final String? unit;
  final String createdAt;

  factory DeviceMetricModel.fromJson(Map<String, dynamic> json) {
    return DeviceMetricModel(
      id: parseIntOrNull(json['id']) ?? 0,
      deviceId: parseIntOrNull(json['deviceId']) ?? 0,
      interfaceId: parseIntOrNull(json['interfaceId']),
      interfaceName: json['interfaceName'] as String?,
      metricName: (json['metricName'] ?? json['name'] ?? '').toString(),
      metricValue: parseDoubleOrNull(json['metricValue'] ?? json['value']) ?? 0,
      unit: json['unit'] as String?,
      createdAt: (json['createdAt'] ?? json['recordedAt'] ?? '').toString(),
    );
  }
}

@immutable
class DeviceMonitorSummary {
  const DeviceMonitorSummary({
    required this.id,
    required this.name,
    required this.type,
    required this.status,
    required this.enabled,
    this.target,
    this.latencyMs,
    this.intervalSeconds,
    this.lastCheckedAt,
  });

  final int id;
  final String name;
  final String type;
  final String status;
  final bool enabled;
  final String? target;
  final double? latencyMs;
  final int? intervalSeconds;
  final String? lastCheckedAt;

  factory DeviceMonitorSummary.fromJson(Map<String, dynamic> json) {
    return DeviceMonitorSummary(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] as String?) ?? 'Monitor',
      type: (json['type'] ?? json['kind'] ?? 'ping').toString(),
      status: (json['status'] ?? json['currentStatus'] ?? 'unknown').toString(),
      enabled: (json['isEnabled'] ?? json['enabled'] ?? true) == true,
      target: json['target'] as String?,
      latencyMs: parseDoubleOrNull(json['latencyMs'] ?? json['lastLatencyMs']),
      intervalSeconds: parseIntOrNull(
        json['intervalSeconds'] ?? json['interval'],
      ),
      lastCheckedAt: json['lastCheckedAt'] as String?,
    );
  }

  DeviceMonitorSummary copyWith({
    String? status,
    double? latencyMs,
    String? lastCheckedAt,
  }) {
    return DeviceMonitorSummary(
      id: id,
      name: name,
      type: type,
      status: status ?? this.status,
      enabled: enabled,
      target: target,
      latencyMs: latencyMs ?? this.latencyMs,
      intervalSeconds: intervalSeconds,
      lastCheckedAt: lastCheckedAt ?? this.lastCheckedAt,
    );
  }
}

@immutable
class DeviceEventModel {
  const DeviceEventModel({
    required this.id,
    required this.deviceId,
    required this.eventType,
    required this.severity,
    required this.message,
    required this.createdAt,
  });

  final int id;
  final int deviceId;
  final String eventType;
  final String severity;
  final String message;
  final String createdAt;

  factory DeviceEventModel.fromJson(Map<String, dynamic> json) {
    return DeviceEventModel(
      id: parseIntOrNull(json['id']) ?? 0,
      deviceId: parseIntOrNull(json['deviceId']) ?? 0,
      eventType: (json['eventType'] as String?) ?? 'event',
      severity: (json['severity'] as String?) ?? 'info',
      message: (json['message'] as String?) ?? '',
      createdAt: (json['createdAt'] as String?) ?? '',
    );
  }
}

@immutable
class DeviceCapabilitiesModel {
  const DeviceCapabilitiesModel({
    required this.deviceId,
    this.isSystem = false,
    this.snmpConfigured = false,
    this.snmpConnected = false,
    this.interfaces = false,
    this.events = false,
    this.logs = false,
    this.vpn = false,
    this.health = false,
    this.plugins = false,
    this.wifi = false,
    this.canSnmpScan = false,
    this.canSnmpCollect = false,
    this.canScanPorts = false,
    this.canEditIdentity = true,
    this.canCreateMonitor = true,
    this.reachMonitorBlockedReason,
    this.alertFields = const <String>[],
  });

  final int deviceId;
  final bool isSystem;
  final bool snmpConfigured;
  final bool snmpConnected;
  final bool interfaces;
  final bool events;
  final bool logs;
  final bool vpn;
  final bool health;
  final bool plugins;
  final bool wifi;
  final bool canSnmpScan;
  final bool canSnmpCollect;
  final bool canScanPorts;
  final bool canEditIdentity;
  final bool canCreateMonitor;
  final String? reachMonitorBlockedReason;
  final List<String> alertFields;

  factory DeviceCapabilitiesModel.fromJson(Map<String, dynamic> json) {
    return DeviceCapabilitiesModel(
      deviceId: parseIntOrNull(json['deviceId']) ?? 0,
      isSystem: json['isSystem'] == true,
      snmpConfigured: json['snmpConfigured'] == true,
      snmpConnected: json['snmpConnected'] == true,
      interfaces: json['interfaces'] == true,
      events: json['events'] == true,
      logs: json['logs'] == true,
      vpn: json['vpn'] == true,
      health: json['health'] == true,
      plugins: json['plugins'] == true,
      wifi: json['wifi'] == true,
      canSnmpScan: json['canSnmpScan'] == true,
      canSnmpCollect: json['canSnmpCollect'] == true,
      canScanPorts: json['canScanPorts'] == true,
      canEditIdentity: (json['canEditIdentity'] as bool?) ?? true,
      canCreateMonitor: (json['canCreateMonitor'] as bool?) ?? true,
      reachMonitorBlockedReason: json['reachMonitorBlockedReason'] as String?,
      alertFields: asStringList(json['alertFields']),
    );
  }
}

@immutable
class SnmpScanInterfaceItem {
  const SnmpScanInterfaceItem({
    required this.ifIndex,
    required this.ifName,
    this.ifDescr,
    this.ifAlias,
    this.macAddress,
    this.ifSpeed,
    this.ifAdminStatus,
    this.ifOperStatus,
    this.isMonitored = false,
  });

  final int ifIndex;
  final String ifName;
  final String? ifDescr;
  final String? ifAlias;
  final String? macAddress;
  final double? ifSpeed;
  final String? ifAdminStatus;
  final String? ifOperStatus;
  final bool isMonitored;

  factory SnmpScanInterfaceItem.fromJson(Map<String, dynamic> json) {
    return SnmpScanInterfaceItem(
      ifIndex: parseIntOrNull(json['ifIndex']) ?? 0,
      ifName: (json['ifName'] ?? json['name'] ?? 'if').toString(),
      ifDescr: json['ifDescr'] as String?,
      ifAlias: json['ifAlias'] as String?,
      macAddress: json['macAddress'] as String?,
      ifSpeed: parseDoubleOrNull(json['ifSpeed']),
      ifAdminStatus: json['ifAdminStatus'] as String?,
      ifOperStatus: json['ifOperStatus'] as String?,
      isMonitored: json['isMonitored'] == true,
    );
  }
}

@immutable
class DiscoveredSensorItem {
  const DiscoveredSensorItem({
    required this.key,
    required this.label,
    required this.oid,
    required this.unit,
    required this.category,
    required this.formattedValue,
    this.isMonitored = false,
  });

  final String key;
  final String label;
  final String oid;
  final String unit;
  final String category;
  final String formattedValue;
  final bool isMonitored;

  factory DiscoveredSensorItem.fromJson(Map<String, dynamic> json) {
    return DiscoveredSensorItem(
      key: (json['key'] as String?) ?? '',
      label: (json['label'] ?? json['name'] ?? json['key'] ?? '').toString(),
      oid: (json['oid'] as String?) ?? '',
      unit: (json['unit'] as String?) ?? '',
      category: (json['category'] as String?) ?? 'sensor',
      formattedValue: (json['formattedValue'] ?? json['value'] ?? '—')
          .toString(),
      isMonitored: json['isMonitored'] == true,
    );
  }
}

@immutable
class SnmpScanResultModel {
  const SnmpScanResultModel({
    required this.snmpResponded,
    this.sysName,
    this.sysDescr,
    this.sysUpTime,
    this.cpuUsagePercent,
    this.memoryUsedPercent,
    this.hasCpuMonitor = false,
    this.hasMemoryMonitor = false,
    this.matchedProfileName,
    this.interfaces = const <SnmpScanInterfaceItem>[],
    this.sensors = const <DiscoveredSensorItem>[],
    this.collectorErrors = const <String, String>{},
  });

  final bool snmpResponded;
  final String? sysName;
  final String? sysDescr;
  final int? sysUpTime;
  final double? cpuUsagePercent;
  final double? memoryUsedPercent;
  final bool hasCpuMonitor;
  final bool hasMemoryMonitor;
  final String? matchedProfileName;
  final List<SnmpScanInterfaceItem> interfaces;
  final List<DiscoveredSensorItem> sensors;
  final Map<String, String> collectorErrors;

  factory SnmpScanResultModel.fromJson(Map<String, dynamic> json) {
    final sys = asJsonMap(json['systemInfo']);
    final cpu = asJsonMap(json['cpuInfo']);
    final mem = asJsonMap(json['memoryInfo']);
    final prof = asJsonMap(json['matchedProfile']);
    final rawErrors = asJsonMap(json['collectorErrors']);
    return SnmpScanResultModel(
      snmpResponded: json['snmpResponded'] == true,
      sysName: sys['sysName'] as String?,
      sysDescr: sys['sysDescr'] as String?,
      sysUpTime: parseIntOrNull(sys['sysUpTime']),
      cpuUsagePercent: parseDoubleOrNull(cpu['usagePercent']),
      memoryUsedPercent: parseDoubleOrNull(mem['usedPercent']),
      hasCpuMonitor: json['hasCpuMonitor'] == true,
      hasMemoryMonitor: json['hasMemoryMonitor'] == true,
      matchedProfileName: prof['name'] as String?,
      interfaces: asJsonMapList(
        json['interfaces'],
      ).map(SnmpScanInterfaceItem.fromJson).toList(growable: false),
      sensors: asJsonMapList(
        json['sensors'],
      ).map(DiscoveredSensorItem.fromJson).toList(growable: false),
      collectorErrors: rawErrors.map((k, v) => MapEntry(k, v.toString())),
    );
  }
}

@immutable
class SnmpProfileModel {
  const SnmpProfileModel({
    required this.id,
    required this.name,
    required this.vendor,
    required this.category,
    this.sysObjectIdPrefix,
    this.sysDescrPattern,
    this.sensorCount = 0,
    this.isBuiltin = false,
  });

  final String id;
  final String name;
  final String vendor;
  final String category;
  final String? sysObjectIdPrefix;
  final String? sysDescrPattern;
  final int sensorCount;
  final bool isBuiltin;

  factory SnmpProfileModel.fromJson(Map<String, dynamic> json) {
    final sensors = json['sensors'] is List
        ? (json['sensors'] as List).length
        : 0;
    return SnmpProfileModel(
      id: (json['id'] ?? '').toString(),
      name: (json['name'] ?? '').toString(),
      vendor: (json['vendor'] ?? '').toString(),
      category: (json['category'] ?? 'generic').toString(),
      sysObjectIdPrefix: json['sysObjectIdPrefix'] as String?,
      sysDescrPattern: json['sysDescrPattern'] as String?,
      sensorCount: sensors,
      isBuiltin: json['isBuiltin'] == true,
    );
  }
}

@immutable
class OperatingSystemOption {
  const OperatingSystemOption({
    required this.id,
    required this.label,
    this.icon,
    this.supportsSyslog = false,
  });

  final String id;
  final String label;
  final String? icon;
  final bool supportsSyslog;

  factory OperatingSystemOption.fromJson(Map<String, dynamic> json) {
    return OperatingSystemOption(
      id: (json['id'] ?? json['slug'] ?? '').toString(),
      label: (json['label'] ?? json['name'] ?? '').toString(),
      icon: json['icon'] as String?,
      supportsSyslog: json['supportsSyslog'] == true,
    );
  }
}

@immutable
class IdentifyResultModel {
  const IdentifyResultModel({
    this.operatingSystem,
    this.source,
    this.vendor,
    this.model,
    this.evidence,
  });

  final String? operatingSystem;
  final String? source;
  final String? vendor;
  final String? model;
  final String? evidence;

  factory IdentifyResultModel.fromJson(Map<String, dynamic> json) {
    return IdentifyResultModel(
      operatingSystem: (json['operatingSystem'] ?? json['system']) as String?,
      source: json['source'] as String?,
      vendor: json['vendor'] as String?,
      model: json['model'] as String?,
      evidence: (json['evidence'] ?? json['detail'] ?? json['sysDescr'])
          ?.toString(),
    );
  }
}

@immutable
class BandwidthLatencyPoint {
  const BandwidthLatencyPoint({
    required this.time,
    required this.timestamp,
    required this.bwBps,
    required this.latency,
  });

  final String time;
  final int timestamp;
  final double bwBps;
  final double latency;

  factory BandwidthLatencyPoint.fromJson(Map<String, dynamic> json) {
    return BandwidthLatencyPoint(
      time: (json['time'] as String?) ?? '',
      timestamp: parseIntOrNull(json['timestamp']) ?? 0,
      bwBps: parseDoubleOrNull(json['bwBps']) ?? 0,
      latency: parseDoubleOrNull(json['latency']) ?? 0,
    );
  }
}

@immutable
class BandwidthLatencyResponse {
  const BandwidthLatencyResponse({
    required this.timeframe,
    required this.samples,
    required this.currentBw,
    required this.peakBw,
    required this.currentLatency,
    required this.avgLatency,
    required this.correlationScore,
    required this.hasSaturationCorrelation,
  });

  final String timeframe;
  final List<BandwidthLatencyPoint> samples;
  final double currentBw;
  final double peakBw;
  final double currentLatency;
  final double avgLatency;
  final double correlationScore;
  final bool hasSaturationCorrelation;

  factory BandwidthLatencyResponse.fromJson(Map<String, dynamic> json) {
    return BandwidthLatencyResponse(
      timeframe: (json['timeframe'] as String?) ?? '1h',
      samples: asJsonMapList(
        json['samples'],
      ).map(BandwidthLatencyPoint.fromJson).toList(growable: false),
      currentBw: parseDoubleOrNull(json['currentBw']) ?? 0,
      peakBw: parseDoubleOrNull(json['peakBw']) ?? 0,
      currentLatency: parseDoubleOrNull(json['currentLatency']) ?? 0,
      avgLatency: parseDoubleOrNull(json['avgLatency']) ?? 0,
      correlationScore: parseDoubleOrNull(json['correlationScore']) ?? 0,
      hasSaturationCorrelation: json['hasSaturationCorrelation'] == true,
    );
  }
}

@immutable
class MonitorUptimeModel {
  const MonitorUptimeModel({
    required this.monitorId,
    required this.hours,
    required this.uptimePercentage,
    required this.totalChecks,
    required this.upChecks,
    required this.downChecks,
    this.avgLatencyMs,
  });

  final int monitorId;
  final int hours;
  final double uptimePercentage;
  final int totalChecks;
  final int upChecks;
  final int downChecks;
  final double? avgLatencyMs;

  factory MonitorUptimeModel.fromJson(Map<String, dynamic> json) {
    return MonitorUptimeModel(
      monitorId: parseIntOrNull(json['monitorId']) ?? 0,
      hours: parseIntOrNull(json['hours']) ?? 24,
      uptimePercentage: parseDoubleOrNull(json['uptimePercentage']) ?? 0,
      totalChecks: parseIntOrNull(json['totalChecks']) ?? 0,
      upChecks: parseIntOrNull(json['upChecks']) ?? 0,
      downChecks: parseIntOrNull(json['downChecks']) ?? 0,
      avgLatencyMs: parseDoubleOrNull(json['avgLatencyMs']),
    );
  }
}
