import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

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
      sitesCount: parseIntOrNull(json['sitesCount']) ?? 0,
      networksCount: parseIntOrNull(json['networksCount']) ?? 0,
      dnsServersCount: parseIntOrNull(json['dnsServersCount']) ?? 0,
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
          parseIntOrNull(json['defaultPingIntervalSeconds']) ?? 60,
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
    this.url,
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

  /// A URL desta central por este caminho, com a porta certa (calculada pelo
  /// servidor, a mesma regra dos comandos de instalação do agente).
  final String? url;

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
      url: json['url'] as String?,
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
      scale: parseDoubleOrNull(json['scale']),
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
    return SnmpDeviceProfile(
      id: (json['id'] ?? '').toString(),
      name: (json['name'] ?? '').toString(),
      vendor: (json['vendor'] ?? '').toString(),
      category: (json['category'] ?? '').toString(),
      sysObjectIdPrefix: json['sysObjectIdPrefix'] as String?,
      sysDescrPattern: json['sysDescrPattern'] as String?,
      sensors: asJsonList<SnmpProfileSensor>(
        json['sensors'],
        SnmpProfileSensor.fromJson,
      ),
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
      subscriptionCount:
          parseIntOrNull(
            json['userSubscriptions'] ??
                json['totalSubscriptions'] ??
                json['subscriptionCount'],
          ) ??
          0,
    );
  }
}
