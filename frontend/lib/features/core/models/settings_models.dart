import 'package:flutter/foundation.dart';

@immutable
class OnboardingStatus {
  const OnboardingStatus({
    required this.completed,
    this.completedAt,
    required this.needsOnboarding,
    required this.intentChosen,
    required this.sitesCount,
    required this.networksCount,
    required this.dnsServersCount,
    required this.vpnConfigured,
    this.detectedLanIp,
    this.detectedPublicIp,
  });

  final bool completed;
  final String? completedAt;
  final bool needsOnboarding;
  final bool intentChosen;
  final int sitesCount;
  final int networksCount;
  final int dnsServersCount;
  final bool vpnConfigured;
  final String? detectedLanIp;
  final String? detectedPublicIp;

  OnboardingStatus copyWith({
    bool? completed,
    String? completedAt,
    bool? needsOnboarding,
    bool? intentChosen,
  }) {
    return OnboardingStatus(
      completed: completed ?? this.completed,
      completedAt: completedAt ?? this.completedAt,
      needsOnboarding: needsOnboarding ?? this.needsOnboarding,
      intentChosen: intentChosen ?? this.intentChosen,
      sitesCount: sitesCount,
      networksCount: networksCount,
      dnsServersCount: dnsServersCount,
      vpnConfigured: vpnConfigured,
      detectedLanIp: detectedLanIp,
      detectedPublicIp: detectedPublicIp,
    );
  }

  factory OnboardingStatus.fromJson(Map<String, dynamic> json) {
    return OnboardingStatus(
      completed: json['completed'] == true,
      completedAt: json['completedAt'] as String?,
      needsOnboarding: json['needsOnboarding'] == true,
      intentChosen: json['intentChosen'] == true,
      sitesCount: (json['sitesCount'] as num?)?.toInt() ?? 0,
      networksCount: (json['networksCount'] as num?)?.toInt() ?? 0,
      dnsServersCount: (json['dnsServersCount'] as num?)?.toInt() ?? 0,
      vpnConfigured: json['vpnConfigured'] == true,
      detectedLanIp: json['detectedLanIp'] as String?,
      detectedPublicIp: json['detectedPublicIp'] as String?,
    );
  }
}

@immutable
class SystemPreferences {
  const SystemPreferences({
    this.defaultPingIntervalSeconds = 60,
    this.defaultSnmpCommunity = 'public',
    this.autoDiscoveryEnabled = true,
  });

  final int defaultPingIntervalSeconds;
  final String defaultSnmpCommunity;
  final bool autoDiscoveryEnabled;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'defaultPingIntervalSeconds': defaultPingIntervalSeconds,
    'defaultSnmpCommunity': defaultSnmpCommunity,
    'autoDiscoveryEnabled': autoDiscoveryEnabled,
  };

  factory SystemPreferences.fromJson(Map<String, dynamic> json) {
    return SystemPreferences(
      defaultPingIntervalSeconds:
          (json['defaultPingIntervalSeconds'] as num?)?.toInt() ?? 60,
      defaultSnmpCommunity: (json['defaultSnmpCommunity'] ?? 'public')
          .toString(),
      autoDiscoveryEnabled: json['autoDiscoveryEnabled'] != false,
    );
  }
}

@immutable
class ServerAddressEntry {
  const ServerAddressEntry({
    required this.id,
    required this.kind,
    required this.label,
    required this.description,
    this.value,
    this.detected,
    required this.overridden,
    required this.source,
    required this.https,
  });

  final String id;
  final String kind;
  final String label;
  final String description;
  final String? value;
  final String? detected;
  final bool overridden;
  final String source;
  final bool https;

  factory ServerAddressEntry.fromJson(Map<String, dynamic> json) {
    return ServerAddressEntry(
      id: (json['id'] ?? '').toString(),
      kind: (json['kind'] ?? 'custom').toString(),
      label: (json['label'] ?? '').toString(),
      description: (json['description'] ?? '').toString(),
      value: json['value'] as String?,
      detected: json['detected'] as String?,
      overridden: json['overridden'] == true,
      source: (json['source'] ?? '').toString(),
      https: json['https'] == true,
    );
  }
}

@immutable
class SnmpProfileSensor {
  const SnmpProfileSensor({
    required this.key,
    required this.label,
    required this.oid,
    this.unit,
    this.scale,
    this.category,
    this.dataType = 'float',
    this.description,
  });

  final String key;
  final String label;
  final String oid;
  final String? unit;
  final double? scale;
  final String? category;
  final String dataType;
  final String? description;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'key': key,
    'label': label,
    'oid': oid,
    if (unit != null && unit!.isNotEmpty) 'unit': unit,
    if (scale != null) 'scale': scale,
    if (category != null && category!.isNotEmpty) 'category': category,
    'dataType': dataType,
    if (description != null && description!.isNotEmpty)
      'description': description,
  };

  factory SnmpProfileSensor.fromJson(Map<String, dynamic> json) {
    return SnmpProfileSensor(
      key: (json['key'] ?? '').toString(),
      label: (json['label'] ?? '').toString(),
      oid: (json['oid'] ?? '').toString(),
      unit: json['unit'] as String?,
      scale: (json['scale'] as num?)?.toDouble(),
      category: json['category'] as String?,
      dataType: (json['dataType'] ?? 'float').toString(),
      description: json['description'] as String?,
    );
  }
}

@immutable
class SnmpDeviceProfile {
  const SnmpDeviceProfile({
    required this.id,
    required this.name,
    required this.vendor,
    required this.category,
    this.sysObjectIdPrefix,
    this.sysDescrPattern,
    required this.sensors,
    this.isBuiltin = false,
  });

  final String id;
  final String name;
  final String vendor;
  final String category;
  final String? sysObjectIdPrefix;
  final String? sysDescrPattern;
  final List<SnmpProfileSensor> sensors;
  final bool isBuiltin;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'id': id,
    'name': name,
    'vendor': vendor,
    'category': category,
    if (sysObjectIdPrefix != null && sysObjectIdPrefix!.isNotEmpty)
      'sysObjectIdPrefix': sysObjectIdPrefix,
    if (sysDescrPattern != null && sysDescrPattern!.isNotEmpty)
      'sysDescrPattern': sysDescrPattern,
    'sensors': sensors.map((s) => s.toJson()).toList(growable: false),
  };

  factory SnmpDeviceProfile.fromJson(Map<String, dynamic> json) {
    final rawSensors = json['sensors'] is List
        ? json['sensors'] as List
        : const [];
    return SnmpDeviceProfile(
      id: (json['id'] ?? '').toString(),
      name: (json['name'] ?? '').toString(),
      vendor: (json['vendor'] ?? '').toString(),
      category: (json['category'] ?? '').toString(),
      sysObjectIdPrefix: json['sysObjectIdPrefix'] as String?,
      sysDescrPattern: json['sysDescrPattern'] as String?,
      sensors: rawSensors
          .whereType<Map>()
          .map((e) => SnmpProfileSensor.fromJson(Map<String, dynamic>.from(e)))
          .toList(growable: false),
      isBuiltin: json['isBuiltin'] == true,
    );
  }
}

@immutable
class OperatingSystemOption {
  const OperatingSystemOption({
    required this.id,
    required this.label,
    required this.supportsSyslog,
    required this.supportsMacTelnet,
    this.vpnProfile,
  });

  final String id;
  final String label;
  final bool supportsSyslog;
  final bool supportsMacTelnet;
  final String? vpnProfile;

  factory OperatingSystemOption.fromJson(Map<String, dynamic> json) {
    return OperatingSystemOption(
      id: (json['id'] ?? '').toString(),
      label: (json['label'] ?? json['id'] ?? '').toString(),
      supportsSyslog: json['supportsSyslog'] == true,
      supportsMacTelnet: json['supportsMacTelnet'] == true,
      vpnProfile: json['vpnProfile'] as String?,
    );
  }
}

@immutable
class VendorRegistryStatus {
  const VendorRegistryStatus({
    required this.entries,
    this.updatedAt,
    required this.builtinEntries,
    required this.stale,
    required this.autoUpdate,
    required this.updating,
    required this.sources,
  });

  final int entries;
  final String? updatedAt;
  final int builtinEntries;
  final bool stale;
  final bool autoUpdate;
  final bool updating;
  final List<String> sources;

  factory VendorRegistryStatus.fromJson(Map<String, dynamic> json) {
    final rawSources = json['sources'] is List
        ? json['sources'] as List
        : const [];
    return VendorRegistryStatus(
      entries: (json['entries'] as num?)?.toInt() ?? 0,
      updatedAt: json['updatedAt'] as String?,
      builtinEntries: (json['builtinEntries'] as num?)?.toInt() ?? 0,
      stale: json['stale'] == true,
      autoUpdate: json['autoUpdate'] == true,
      updating: json['updating'] == true,
      sources: rawSources.map((e) => e.toString()).toList(growable: false),
    );
  }
}

@immutable
class MacVendorLookup {
  const MacVendorLookup({
    this.vendor,
    this.organization,
    required this.source,
    required this.locallyAdministered,
  });

  final String? vendor;
  final String? organization;
  final String source;
  final bool locallyAdministered;

  factory MacVendorLookup.fromJson(Map<String, dynamic> json) {
    return MacVendorLookup(
      vendor: json['vendor'] as String?,
      organization: json['organization'] as String?,
      source: (json['source'] ?? 'none').toString(),
      locallyAdministered: json['locallyAdministered'] == true,
    );
  }
}

@immutable
class PushStatusModel {
  const PushStatusModel({
    required this.enabled,
    required this.configured,
    required this.subscriptionCount,
  });

  final bool enabled;
  final bool configured;
  final int subscriptionCount;

  factory PushStatusModel.fromJson(Map<String, dynamic> json) {
    return PushStatusModel(
      enabled: json['enabled'] == true,
      configured: json['configured'] == true,
      subscriptionCount: (json['subscriptionCount'] as num?)?.toInt() ?? 0,
    );
  }
}

@immutable
class DatabaseSizeInfo {
  const DatabaseSizeInfo({
    required this.engine,
    required this.totalBytes,
    this.logsBytes,
    this.path,
  });

  final String engine;
  final int totalBytes;
  final int? logsBytes;
  final String? path;

  factory DatabaseSizeInfo.fromJson(Map<String, dynamic> json) {
    return DatabaseSizeInfo(
      engine: (json['engine'] ?? json['driver'] ?? 'database').toString(),
      totalBytes:
          ((json['totalBytes'] ?? json['sizeBytes'] ?? json['size']) as num?)
              ?.toInt() ??
          0,
      logsBytes: (json['logsBytes'] as num?)?.toInt(),
      path: json['path'] as String?,
    );
  }
}

@immutable
class DatabaseTableUsage {
  const DatabaseTableUsage({
    required this.name,
    required this.rows,
    required this.bytes,
    this.database,
  });

  final String name;
  final int rows;
  final int bytes;
  final String? database;

  factory DatabaseTableUsage.fromJson(Map<String, dynamic> json) {
    return DatabaseTableUsage(
      name: (json['name'] ?? json['table'] ?? '').toString(),
      rows: (json['rows'] as num?)?.toInt() ?? 0,
      bytes:
          ((json['bytes'] ?? json['totalBytes'] ?? json['sizeBytes']) as num?)
              ?.toInt() ??
          0,
      database: json['database'] as String?,
    );
  }
}

@immutable
class DatabaseUsageModel {
  const DatabaseUsageModel({required this.totalBytes, required this.tables});

  final int totalBytes;
  final List<DatabaseTableUsage> tables;

  factory DatabaseUsageModel.fromJson(Map<String, dynamic> json) {
    final tables = <DatabaseTableUsage>[];
    if (json['tables'] is List) {
      for (final item in json['tables'] as List) {
        if (item is Map) {
          tables.add(
            DatabaseTableUsage.fromJson(Map<String, dynamic>.from(item)),
          );
        }
      }
    } else if (json['databases'] is List) {
      for (final db in json['databases'] as List) {
        if (db is Map && db['tables'] is List) {
          final dbName = db['name']?.toString();
          for (final t in db['tables'] as List) {
            if (t is Map) {
              final map = Map<String, dynamic>.from(t);
              if (dbName != null) map['database'] = dbName;
              tables.add(DatabaseTableUsage.fromJson(map));
            }
          }
        }
      }
    }
    final total =
        (json['totalBytes'] as num?)?.toInt() ??
        tables.fold<int>(0, (acc, t) => acc + t.bytes);
    return DatabaseUsageModel(
      totalBytes: total,
      tables: List.unmodifiable(tables),
    );
  }
}
