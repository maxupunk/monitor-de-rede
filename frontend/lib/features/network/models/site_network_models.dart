import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';

@immutable
class SiteModel {
  const SiteModel({
    required this.id,
    required this.name,
    this.location,
    this.description,
    this.devicesCount = 0,
    this.networksCount = 0,
    this.createdAt,
    this.updatedAt,
  });

  final int id;
  final String name;
  final String? location;
  final String? description;
  final int devicesCount;
  final int networksCount;
  final String? createdAt;
  final String? updatedAt;

  factory SiteModel.fromJson(Map<String, dynamic> json) {
    return SiteModel(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] as String?) ?? 'Site',
      location: json['location'] as String?,
      description: json['description'] as String?,
      devicesCount:
          parseIntOrNull(json['devicesCount'] ?? json['deviceCount']) ?? 0,
      networksCount:
          parseIntOrNull(json['networksCount'] ?? json['networkCount']) ?? 0,
      createdAt: json['createdAt'] as String?,
      updatedAt: json['updatedAt'] as String?,
    );
  }
}

@immutable
class NetworkModel {
  const NetworkModel({
    required this.id,
    required this.name,
    required this.cidr,
    this.siteId,
    this.probeId,
    this.gateway,
    this.vlanId,
    this.description,
    this.scanEnabled = false,
    this.scanInterval,
    this.lastScanAt,
    this.nextScanAt,
    this.scannable = true,
    this.usableHosts,
    this.site,
    this.createdAt,
    this.updatedAt,
  });

  final int id;
  final int? siteId;
  final int? probeId;
  final String name;
  final String cidr;
  final String? gateway;
  final int? vlanId;
  final String? description;
  final bool scanEnabled;
  final int? scanInterval;
  final String? lastScanAt;
  final String? nextScanAt;
  final bool scannable;
  final int? usableHosts;
  final DeviceRef? site;
  final String? createdAt;
  final String? updatedAt;

  factory NetworkModel.fromJson(Map<String, dynamic> json) {
    return NetworkModel(
      id: parseIntOrNull(json['id']) ?? 0,
      siteId: parseIntOrNull(json['siteId']),
      probeId: parseIntOrNull(json['probeId']),
      name: (json['name'] as String?) ?? 'Rede',
      cidr: (json['cidr'] as String?) ?? '',
      gateway: json['gateway'] as String?,
      vlanId: parseIntOrNull(json['vlanId']),
      description: json['description'] as String?,
      scanEnabled: json['scanEnabled'] == true,
      scanInterval: parseIntOrNull(json['scanInterval']),
      lastScanAt: (json['lastScanAt'] ?? json['lastScannedAt']) as String?,
      nextScanAt: json['nextScanAt'] as String?,
      scannable: (json['scannable'] as bool?) ?? true,
      usableHosts: parseIntOrNull(json['usableHosts']),
      site: DeviceRef.tryFromJson(json['site']),
      createdAt: json['createdAt'] as String?,
      updatedAt: json['updatedAt'] as String?,
    );
  }
}

@immutable
class NetworkScanResult {
  const NetworkScanResult({
    required this.message,
    required this.alreadyQueued,
    required this.runId,
    required this.runStatus,
    required this.usableHosts,
    required this.truncated,
  });

  final String message;
  final bool alreadyQueued;
  final int runId;
  final String runStatus;
  final int usableHosts;
  final bool truncated;

  factory NetworkScanResult.fromJson(Map<String, dynamic> json) {
    final run = asJsonMap(json['run']);
    return NetworkScanResult(
      message: (json['message'] as String?) ?? 'Varredura agendada.',
      alreadyQueued: json['alreadyQueued'] == true,
      runId: parseIntOrNull(run['id']) ?? 0,
      runStatus: (run['status'] as String?) ?? 'pending',
      usableHosts: parseIntOrNull(json['usableHosts']) ?? 0,
      truncated: json['truncated'] == true,
    );
  }
}

@immutable
class ProbeModel {
  const ProbeModel({
    required this.id,
    required this.name,
    required this.status,
    this.location,
    this.ipAddress,
    this.probeType,
    this.version,
    this.lastHeartbeatAt,
    this.createdAt,
  });

  final int id;
  final String name;
  final String? location;
  final String? ipAddress;
  final String? probeType;
  final String? version;
  final String status;
  final String? lastHeartbeatAt;
  final String? createdAt;

  bool get isVpnProbe =>
      probeType == 'vpn_probe' || name.toLowerCase().contains('vpn');

  factory ProbeModel.fromJson(Map<String, dynamic> json) {
    return ProbeModel(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] as String?) ?? 'Sonda',
      location: json['location'] as String?,
      ipAddress: json['ipAddress'] as String?,
      probeType: json['probeType'] as String?,
      version: json['version'] as String?,
      status: (json['status'] as String?) ?? 'offline',
      lastHeartbeatAt:
          (json['lastHeartbeatAt'] ?? json['lastSeenAt']) as String?,
      createdAt: json['createdAt'] as String?,
    );
  }

  ProbeModel copyWith({String? name, String? status, String? lastHeartbeatAt}) {
    return ProbeModel(
      id: id,
      name: name ?? this.name,
      location: location,
      ipAddress: ipAddress,
      probeType: probeType,
      version: version,
      status: status ?? this.status,
      lastHeartbeatAt: lastHeartbeatAt ?? this.lastHeartbeatAt,
      createdAt: createdAt,
    );
  }
}

@immutable
class DiscoveryRunModel {
  const DiscoveryRunModel({
    required this.id,
    required this.networkId,
    required this.status,
    required this.devicesFound,
    this.networkName,
    this.probeId,
    this.cidr,
    this.startedAt,
    this.finishedAt,
    this.error,
  });

  final int id;
  final int networkId;
  final String? networkName;
  final int? probeId;
  final String status;
  final int devicesFound;
  final String? cidr;
  final String? startedAt;
  final String? finishedAt;
  final String? error;

  factory DiscoveryRunModel.fromJson(Map<String, dynamic> json) {
    return DiscoveryRunModel(
      id: parseIntOrNull(json['id']) ?? 0,
      networkId: parseIntOrNull(json['networkId']) ?? 0,
      networkName: json['networkName'] as String?,
      probeId: parseIntOrNull(json['probeId']),
      status: (json['status'] as String?) ?? 'completed',
      devicesFound:
          parseIntOrNull(json['devicesFound'] ?? json['hostsFound']) ?? 0,
      cidr: json['cidr'] as String?,
      startedAt: json['startedAt'] as String?,
      finishedAt: json['finishedAt'] as String?,
      error: json['error'] as String?,
    );
  }
}

@immutable
class StreamedDiscoveryHost {
  const StreamedDiscoveryHost({
    required this.ipAddress,
    this.macAddress,
    this.hostname,
    this.mdnsName,
    this.vendor,
    this.deviceType,
    this.openPorts = const <int>[],
    this.confidence = 0,
    this.data = const <String, dynamic>{},
  });

  final String ipAddress;
  final String? macAddress;
  final String? hostname;
  final String? mdnsName;
  final String? vendor;
  final String? deviceType;
  final List<int> openPorts;
  final int confidence;
  final Map<String, dynamic> data;

  String get bestName => (hostname != null && hostname!.trim().isNotEmpty)
      ? hostname!.trim()
      : (mdnsName != null && mdnsName!.trim().isNotEmpty)
      ? mdnsName!.trim()
      : ipAddress;

  String? get normalizedMac {
    final trimmed = macAddress?.trim();
    return (trimmed != null && trimmed.isNotEmpty) ? trimmed : null;
  }

  /// Linha 2 do card de descoberta: `IP · MAC` quando há hostname no título,
  /// ou apenas o `MAC` quando o próprio IP já ocupa o título.
  String? get addressSubtitle {
    final mac = normalizedMac;
    if (bestName != ipAddress) {
      return <String>[ipAddress, ?mac].join(' · ');
    }
    return mac;
  }

  /// Extrai o nome do sistema operacional reportado em [data], se disponível.
  String? get detectedOsLabel {
    for (final key in const <String>['operatingSystem', 'osName', 'os']) {
      final raw = data[key];
      if (raw is String && raw.trim().isNotEmpty) {
        return raw.trim();
      }
      if (raw is Map) {
        for (final subKey in const <String>['name', 'label', 'family']) {
          final nested = raw[subKey];
          if (nested is String && nested.trim().isNotEmpty) {
            return nested.trim();
          }
        }
      }
    }
    return null;
  }

  /// Linha 3 do card de descoberta: sistema operacional e/ou fabricante sem duplicar.
  String? get systemOrVendorLabel {
    final os = detectedOsLabel;
    final cleanVendor = (vendor != null && vendor!.trim().isNotEmpty)
        ? vendor!.trim()
        : null;
    if (os != null && cleanVendor != null) {
      final osLower = os.toLowerCase();
      final vendorLower = cleanVendor.toLowerCase();
      if (osLower.contains(vendorLower)) return os;
      if (vendorLower.contains(osLower)) return cleanVendor;
      return '$cleanVendor · $os';
    }
    return os ?? cleanVendor;
  }

  /// Linha 4 do card de descoberta: resumo compacto de portas abertas.
  String? get openPortsSummary {
    if (openPorts.isEmpty) return null;
    final listed = openPorts.take(6).join(', ');
    final extra = openPorts.length > 6 ? ' +${openPorts.length - 6}' : '';
    return 'Portas: $listed$extra';
  }

  factory StreamedDiscoveryHost.fromJson(Map<String, dynamic> json) {
    return StreamedDiscoveryHost(
      ipAddress: (json['ipAddress'] as String?) ?? '',
      macAddress: json['macAddress'] as String?,
      hostname: json['hostname'] as String?,
      mdnsName: json['mdnsName'] as String?,
      vendor: json['vendor'] as String?,
      deviceType: json['deviceType'] as String?,
      openPorts: asIntList(json['openPorts']),
      confidence: parseIntOrNull(json['confidence']) ?? 0,
      data: asJsonMap(json['data']),
    );
  }
}

@immutable
class NetworkConflictModel {
  const NetworkConflictModel({
    required this.id,
    required this.conflictType,
    required this.ipAddress,
    required this.macAddresses,
    required this.vendors,
    required this.severity,
    required this.description,
    required this.detectedAt,
    this.affectedDeviceId,
    this.affectedDeviceName,
  });

  final String id;
  final String conflictType;
  final String ipAddress;
  final List<String> macAddresses;
  final int? affectedDeviceId;
  final String? affectedDeviceName;
  final List<String> vendors;
  final String severity;
  final String description;
  final String detectedAt;

  factory NetworkConflictModel.fromJson(Map<String, dynamic> json) {
    return NetworkConflictModel(
      id: (json['id'] ?? '').toString(),
      conflictType: (json['conflictType'] as String?) ?? 'ipCollision',
      ipAddress: (json['ipAddress'] as String?) ?? '',
      macAddresses: asStringList(json['macAddresses']),
      affectedDeviceId: parseIntOrNull(json['affectedDeviceId']),
      affectedDeviceName: json['affectedDeviceName'] as String?,
      vendors: asStringList(json['vendors']),
      severity: (json['severity'] as String?) ?? 'warning',
      description: (json['description'] as String?) ?? '',
      detectedAt: (json['detectedAt'] as String?) ?? '',
    );
  }
}

@immutable
class DiscoveryEnvironmentModel {
  const DiscoveryEnvironmentModel({
    required this.isHostMode,
    required this.containerized,
  });

  final bool isHostMode;
  final bool containerized;

  factory DiscoveryEnvironmentModel.fromJson(Map<String, dynamic> json) {
    return DiscoveryEnvironmentModel(
      isHostMode: json['isHostMode'] == true,
      containerized: json['containerized'] == true,
    );
  }
}

@immutable
class ScanSessionState {
  const ScanSessionState({
    this.runId,
    this.networkId,
    this.status = 'idle',
    this.phase = 'idle',
    this.progressCurrent = 0,
    this.progressTotal = 0,
    this.hosts = const <StreamedDiscoveryHost>[],
    this.logs = const <String>[],
    this.error,
    this.startedAt,
    this.finishedAt,
  });

  final int? runId;
  final int? networkId;
  final String status;
  final String phase;
  final int progressCurrent;
  final int progressTotal;
  final List<StreamedDiscoveryHost> hosts;
  final List<String> logs;
  final String? error;
  final String? startedAt;
  final String? finishedAt;

  bool get isActive => status == 'running' || status == 'pending';

  double get progressPercent {
    if (status == 'completed') return 100;
    if (progressTotal <= 0) return 0;
    return ((progressCurrent / progressTotal) * 100).clamp(0, 100);
  }

  static ScanSessionState? tryFromJson(Map<String, dynamic> json) {
    if (json['status'] is! String || json['hosts'] is! List) return null;
    final parsedHosts =
        asJsonMapList(
            json['hosts'],
          ).map(StreamedDiscoveryHost.fromJson).toList()
          ..sort((a, b) => compareIpAddresses(a.ipAddress, b.ipAddress));
    return ScanSessionState(
      runId: parseIntOrNull(json['runId']),
      networkId: parseIntOrNull(json['networkId']),
      status: json['status'] as String,
      phase: (json['phase'] as String?) ?? 'idle',
      progressCurrent: parseIntOrNull(json['progressCurrent']) ?? 0,
      progressTotal: parseIntOrNull(json['progressTotal']) ?? 0,
      hosts: List<StreamedDiscoveryHost>.unmodifiable(parsedHosts),
      logs: asStringList(json['logs']),
      error: json['error'] as String?,
      startedAt: json['startedAt'] as String?,
      finishedAt: json['finishedAt'] as String?,
    );
  }
}

/// Ordenação numérica de endereços IPv4 (ex.: `.2` antes de `.10`).
int compareIpAddresses(String a, String b) {
  final partsA = a.split('.').map(int.tryParse).toList();
  final partsB = b.split('.').map(int.tryParse).toList();
  if (partsA.length == 4 &&
      partsB.length == 4 &&
      partsA.every((e) => e != null) &&
      partsB.every((e) => e != null)) {
    for (var i = 0; i < 4; i++) {
      final diff = partsA[i]! - partsB[i]!;
      if (diff != 0) return diff;
    }
    return 0;
  }
  return a.compareTo(b);
}
