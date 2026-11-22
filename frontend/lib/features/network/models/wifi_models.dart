import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

@immutable
class WifiDistanceModel {
  const WifiDistanceModel({
    required this.meters,
    required this.minM,
    required this.maxM,
    required this.calibrated,
    required this.environment,
  });

  final double meters;
  final double minM;
  final double maxM;
  final bool calibrated;
  final String environment;

  static WifiDistanceModel? tryFromJson(Object? raw) {
    if (raw is! Map) return null;
    final json = Map<String, dynamic>.from(raw);
    final meters = parseDoubleOrNull(json['meters']);
    if (meters == null) return null;
    return WifiDistanceModel(
      meters: meters,
      minM: parseDoubleOrNull(json['minM']) ?? meters,
      maxM: parseDoubleOrNull(json['maxM']) ?? meters,
      calibrated: json['calibrated'] == true,
      environment: (json['environment'] as String?) ?? 'office',
    );
  }
}

@immutable
class WifiStationModel {
  const WifiStationModel({
    required this.mac,
    required this.signalDbm,
    required this.signalGrade,
    required this.health,
    required this.linkScore,
    this.randomizedMac = false,
    this.vendor,
    this.ip,
    this.hostname,
    this.ssid,
    this.bssid,
    this.radio,
    this.band,
    this.channel,
    this.channelWidthMhz,
    this.frequencyMhz,
    this.txPowerDbm,
    this.noiseDbm,
    this.snrDb,
    this.rxRateBps,
    this.txRateBps,
    this.txRetriesPct,
    this.connectedS,
    this.healthReason,
    this.distance,
  });

  final String mac;
  final bool randomizedMac;
  final String? vendor;
  final String? ip;
  final String? hostname;
  final String? ssid;
  final String? bssid;
  final String? radio;
  final String? band;
  final int? channel;
  final int? channelWidthMhz;
  final int? frequencyMhz;
  final int? txPowerDbm;
  final int signalDbm;
  final int? noiseDbm;
  final int? snrDb;
  final double? rxRateBps;
  final double? txRateBps;
  final double? txRetriesPct;
  final int? connectedS;
  final String signalGrade;
  final String health;
  final String? healthReason;
  final double linkScore;
  final WifiDistanceModel? distance;

  factory WifiStationModel.fromJson(Map<String, dynamic> json) {
    return WifiStationModel(
      mac: (json['mac'] as String?) ?? '',
      randomizedMac: json['randomizedMac'] == true,
      vendor: json['vendor'] as String?,
      ip: json['ip'] as String?,
      hostname: json['hostname'] as String?,
      ssid: json['ssid'] as String?,
      bssid: json['bssid'] as String?,
      radio: json['radio'] as String?,
      band: json['band'] as String?,
      channel: parseIntOrNull(json['channel']),
      channelWidthMhz: parseIntOrNull(json['channelWidthMhz']),
      frequencyMhz: parseIntOrNull(json['frequencyMhz']),
      txPowerDbm: parseIntOrNull(json['txPowerDbm']),
      signalDbm: parseIntOrNull(json['signalDbm']) ?? -70,
      noiseDbm: parseIntOrNull(json['noiseDbm']),
      snrDb: parseIntOrNull(json['snrDb']),
      rxRateBps: parseDoubleOrNull(json['rxRateBps']),
      txRateBps: parseDoubleOrNull(json['txRateBps']),
      txRetriesPct: parseDoubleOrNull(json['txRetriesPct']),
      connectedS: parseIntOrNull(json['connectedS']),
      signalGrade: (json['signalGrade'] as String?) ?? 'good',
      health: (json['health'] as String?) ?? 'good',
      healthReason: json['healthReason'] as String?,
      linkScore: parseDoubleOrNull(json['linkScore']) ?? 0.5,
      distance: WifiDistanceModel.tryFromJson(json['distance']),
    );
  }
}

@immutable
class WifiDeviceSnapshotModel {
  const WifiDeviceSnapshotModel({
    required this.deviceId,
    required this.deviceName,
    required this.status,
    required this.stations,
    this.readAt,
    this.error,
    this.canPing = false,
  });

  final int deviceId;
  final String deviceName;
  final String status;
  final String? readAt;
  final String? error;
  final List<WifiStationModel> stations;
  final bool canPing;

  static WifiDeviceSnapshotModel? tryFromJson(Map<String, dynamic> json) {
    final deviceId = parseIntOrNull(json['deviceId']);
    if (deviceId == null || json['stations'] is! List) return null;
    return WifiDeviceSnapshotModel(
      deviceId: deviceId,
      deviceName: (json['deviceName'] as String?) ?? 'AP #$deviceId',
      status: (json['status'] as String?) ?? 'ok',
      readAt: json['readAt'] as String?,
      error: json['error'] as String?,
      stations: asJsonMapList(
        json['stations'],
      ).map(WifiStationModel.fromJson).toList(growable: false),
      canPing: json['canPing'] == true,
    );
  }
}

@immutable
class WifiStationRow {
  const WifiStationRow({
    required this.deviceId,
    required this.deviceName,
    required this.canPing,
    required this.station,
  });

  final int deviceId;
  final String deviceName;
  final bool canPing;
  final WifiStationModel station;

  String get displayName =>
      (station.hostname != null && station.hostname!.trim().isNotEmpty)
      ? station.hostname!.trim()
      : (station.vendor != null && station.vendor!.trim().isNotEmpty)
      ? station.vendor!.trim()
      : station.mac;
}

@immutable
class WifiWatchedClientModel {
  const WifiWatchedClientModel({
    required this.id,
    required this.mac,
    required this.name,
    required this.state,
    this.randomizedMac = false,
    this.deviceId,
    this.deviceName,
    this.lastSeenAt,
  });

  final int id;
  final String mac;
  final String name;
  final bool randomizedMac;
  final String state;
  final int? deviceId;
  final String? deviceName;
  final String? lastSeenAt;

  factory WifiWatchedClientModel.fromJson(Map<String, dynamic> json) {
    return WifiWatchedClientModel(
      id: parseIntOrNull(json['id']) ?? 0,
      mac: (json['mac'] as String?) ?? '',
      name: (json['name'] as String?) ?? '',
      randomizedMac: json['randomizedMac'] == true,
      state: (json['state'] as String?) ?? 'offline',
      deviceId: parseIntOrNull(json['deviceId']),
      deviceName: json['deviceName'] as String?,
      lastSeenAt: json['lastSeenAt'] as String?,
    );
  }
}

@immutable
class WifiRoamEventModel {
  const WifiRoamEventModel({
    required this.id,
    required this.mac,
    required this.fromDeviceName,
    required this.toDeviceName,
    required this.occurredAt,
    this.fromDeviceId,
    this.toDeviceId,
    this.signalBeforeDbm,
    this.signalAfterDbm,
    this.pingPong = false,
  });

  final int id;
  final String mac;
  final int? fromDeviceId;
  final String fromDeviceName;
  final int? toDeviceId;
  final String toDeviceName;
  final int? signalBeforeDbm;
  final int? signalAfterDbm;
  final String occurredAt;
  final bool pingPong;

  static WifiRoamEventModel? tryFromJson(Map<String, dynamic> json) {
    final id = parseIntOrNull(json['id']);
    final mac = json['mac'] as String?;
    if (id == null || mac == null) return null;
    return WifiRoamEventModel(
      id: id,
      mac: mac,
      fromDeviceId: parseIntOrNull(json['fromDeviceId']),
      fromDeviceName: (json['fromDeviceName'] as String?) ?? 'AP anterior',
      toDeviceId: parseIntOrNull(json['toDeviceId']),
      toDeviceName: (json['toDeviceName'] as String?) ?? 'Novo AP',
      signalBeforeDbm: parseIntOrNull(json['signalBeforeDbm']),
      signalAfterDbm: parseIntOrNull(json['signalAfterDbm']),
      occurredAt: (json['occurredAt'] as String?) ?? '',
      pingPong: json['pingPong'] == true,
    );
  }
}

@immutable
class WifiPingResultModel {
  const WifiPingResultModel({
    required this.deviceId,
    required this.target,
    required this.sent,
    required this.received,
    required this.lossPct,
    required this.grade,
    this.avgMs,
  });

  final int deviceId;
  final String target;
  final int sent;
  final int received;
  final double lossPct;
  final double? avgMs;
  final String grade;

  factory WifiPingResultModel.fromJson(Map<String, dynamic> json) {
    return WifiPingResultModel(
      deviceId: parseIntOrNull(json['deviceId']) ?? 0,
      target: (json['target'] as String?) ?? '',
      sent: parseIntOrNull(json['sent']) ?? 0,
      received: parseIntOrNull(json['received']) ?? 0,
      lossPct: parseDoubleOrNull(json['lossPct']) ?? 0,
      avgMs: parseDoubleOrNull(json['avgMs']),
      grade: (json['grade'] as String?) ?? 'good',
    );
  }
}

@immutable
class WifiSettingsModel {
  const WifiSettingsModel({
    this.liveIntervalS = 5,
    this.backgroundIntervalS = 60,
    this.environment = 'office',
    this.roamingRetentionDays = 30,
    this.pingPongCount = 4,
    this.pingPongWindowMin = 10,
    this.apEnvironments = const <Map<String, dynamic>>[],
  });

  final int liveIntervalS;
  final int backgroundIntervalS;
  final String environment;
  final int roamingRetentionDays;
  final int pingPongCount;
  final int pingPongWindowMin;
  final List<Map<String, dynamic>> apEnvironments;

  factory WifiSettingsModel.fromJson(Map<String, dynamic> json) {
    return WifiSettingsModel(
      liveIntervalS: parseIntOrNull(json['liveIntervalS']) ?? 5,
      backgroundIntervalS: parseIntOrNull(json['backgroundIntervalS']) ?? 60,
      environment: (json['environment'] as String?) ?? 'office',
      roamingRetentionDays: parseIntOrNull(json['roamingRetentionDays']) ?? 30,
      pingPongCount: parseIntOrNull(json['pingPongCount']) ?? 4,
      pingPongWindowMin: parseIntOrNull(json['pingPongWindowMin']) ?? 10,
      apEnvironments: asJsonMapList(json['apEnvironments']),
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
    'liveIntervalS': liveIntervalS,
    'backgroundIntervalS': backgroundIntervalS,
    'environment': environment,
    'apEnvironments': apEnvironments,
    'roamingRetentionDays': roamingRetentionDays,
    'pingPongCount': pingPongCount,
    'pingPongWindowMin': pingPongWindowMin,
  };
}
