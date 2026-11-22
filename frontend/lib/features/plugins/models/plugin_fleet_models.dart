import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_manifest_models.dart';

@immutable
class PluginApp {
  const PluginApp({
    required this.id,
    required this.slug,
    required this.title,
    this.icon = 'mdi-apps',
    this.description,
    this.members = 0,
  });

  final int id;
  final String slug;
  final String title;
  final String icon;
  final String? description;
  final int members;

  factory PluginApp.fromJson(Map<String, dynamic> json) {
    return PluginApp(
      id: parseIntOrNull(json['id']) ?? 0,
      slug: (json['slug'] as String?) ?? '',
      title: (json['title'] as String?) ?? '',
      icon: (json['icon'] as String?) ?? 'mdi-apps',
      description: json['description'] as String?,
      members: parseIntOrNull(json['members']) ?? 0,
    );
  }
}

@immutable
class FleetMember {
  const FleetMember({
    required this.deviceId,
    required this.deviceName,
    required this.ipAddress,
    this.platform,
    this.firmware,
    this.compatibility = 'likely',
    this.reasons = const <String>[],
    this.credentialsReady = true,
    this.missingTransport,
    this.status,
    this.settings,
    this.card = const <String, dynamic>{},
    this.error,
  });

  final int deviceId;
  final String deviceName;
  final String ipAddress;
  final String? platform;
  final String? firmware;
  final String compatibility;
  final List<String> reasons;
  final bool credentialsReady;
  final String? missingTransport;
  final String? status;
  final Map<String, dynamic>? settings;
  final Map<String, dynamic> card;
  final String? error;

  FleetMember copyWith({Map<String, dynamic>? settings}) {
    return FleetMember(
      deviceId: deviceId,
      deviceName: deviceName,
      ipAddress: ipAddress,
      platform: platform,
      firmware: firmware,
      compatibility: compatibility,
      reasons: reasons,
      credentialsReady: credentialsReady,
      missingTransport: missingTransport,
      status: status,
      settings: settings ?? this.settings,
      card: card,
      error: error,
    );
  }

  factory FleetMember.fromJson(Map<String, dynamic> json) {
    return FleetMember(
      deviceId: parseIntOrNull(json['deviceId']) ?? 0,
      deviceName: (json['deviceName'] ?? json['name'] ?? '').toString(),
      ipAddress: ((json['ipAddress'] ?? json['ip']) as String?) ?? '',
      platform: json['platform'] as String?,
      firmware: json['firmware'] as String?,
      compatibility:
          ((json['compatibility'] ?? json['compat']) as String?) ?? 'likely',
      reasons: asStringList(json['reasons']),
      credentialsReady: parseBool(json['credentialsReady'], fallback: true),
      missingTransport:
          (json['missingTransport'] ?? json['missingAccess']) as String?,
      status: json['status'] as String?,
      settings: json['settings'] is Map ? asJsonMap(json['settings']) : null,
      card: asJsonMap(json['card']),
      error: json['error'] as String?,
    );
  }
}

@immutable
class FleetCandidate {
  const FleetCandidate({
    required this.deviceId,
    required this.deviceName,
    required this.ipAddress,
    this.compatibility = 'possible',
    this.reasons = const <String>[],
  });

  final int deviceId;
  final String deviceName;
  final String ipAddress;
  final String compatibility;
  final List<String> reasons;

  bool get isIncompatible => compatibility == 'incompatible';

  factory FleetCandidate.fromJson(Map<String, dynamic> json) {
    return FleetCandidate(
      deviceId: parseIntOrNull(json['deviceId']) ?? 0,
      deviceName: (json['deviceName'] ?? json['name'] ?? '').toString(),
      ipAddress: ((json['ipAddress'] ?? json['ip']) as String?) ?? '',
      compatibility:
          ((json['compatibility'] ?? json['compat']) as String?) ?? 'possible',
      reasons: asStringList(json['reasons']),
    );
  }
}

@immutable
class BatchDevice {
  const BatchDevice({
    required this.deviceId,
    required this.deviceName,
    required this.status,
    this.runId,
    this.output,
    this.error,
    this.missingTransport,
  });

  final int deviceId;
  final String deviceName;
  final String status;
  final int? runId;
  final Object? output;
  final String? error;
  final String? missingTransport;

  factory BatchDevice.fromJson(Map<String, dynamic> json) {
    return BatchDevice(
      deviceId: parseIntOrNull(json['deviceId']) ?? 0,
      deviceName: (json['deviceName'] ?? json['name'] ?? '').toString(),
      status: (json['status'] as String?) ?? 'pending',
      runId: parseIntOrNull(json['runId']),
      output: json['output'],
      error: json['error'] as String?,
      missingTransport:
          (json['missingTransport'] ?? json['missingAccess']) as String?,
    );
  }
}

@immutable
class PluginBatchView {
  const PluginBatchView({
    required this.id,
    required this.pluginId,
    required this.action,
    required this.title,
    required this.status,
    this.devices = const <BatchDevice>[],
    this.result,
    this.hasPatch = false,
    this.error,
    this.createdAt = '',
    this.finishedAt,
  });

  final int id;
  final int pluginId;
  final String action;
  final String title;
  final String status;
  final List<BatchDevice> devices;
  final Object? result;
  final bool hasPatch;
  final String? error;
  final String createdAt;
  final String? finishedAt;

  bool get isRunning => status == 'running' || status == 'pending';

  factory PluginBatchView.fromJson(Map<String, dynamic> json) {
    return PluginBatchView(
      id: parseIntOrNull(json['id']) ?? 0,
      pluginId: parseIntOrNull(json['pluginId']) ?? 0,
      action: (json['action'] as String?) ?? '',
      title: (json['title'] as String?) ?? (json['action'] as String?) ?? '',
      status: (json['status'] as String?) ?? 'running',
      devices: asJsonList<BatchDevice>(json['devices'], BatchDevice.fromJson),
      result: json['result'],
      hasPatch: parseBool(json['hasPatch']),
      error: json['error'] as String?,
      createdAt: (json['createdAt'] as String?) ?? '',
      finishedAt: json['finishedAt'] as String?,
    );
  }
}

@immutable
class FleetView {
  const FleetView({
    required this.plugin,
    this.settings,
    this.members = const <FleetMember>[],
    this.candidates = const <FleetCandidate>[],
    this.batches = const <PluginBatchView>[],
  });

  final PluginSummary plugin;
  final Map<String, dynamic>? settings;
  final List<FleetMember> members;
  final List<FleetCandidate> candidates;
  final List<PluginBatchView> batches;

  FleetView copyWith({
    Map<String, dynamic>? settings,
    List<FleetMember>? members,
    List<FleetCandidate>? candidates,
    List<PluginBatchView>? batches,
  }) {
    return FleetView(
      plugin: plugin,
      settings: settings ?? this.settings,
      members: members ?? this.members,
      candidates: candidates ?? this.candidates,
      batches: batches ?? this.batches,
    );
  }

  factory FleetView.fromJson(Map<String, dynamic> json) {
    return FleetView(
      plugin: PluginSummary.fromJson(asJsonMap(json['plugin'])),
      settings: json['settings'] is Map ? asJsonMap(json['settings']) : null,
      members: asJsonList<FleetMember>(json['members'], FleetMember.fromJson),
      candidates: asJsonList<FleetCandidate>(
        json['candidates'],
        FleetCandidate.fromJson,
      ),
      batches: asJsonList<PluginBatchView>(
        json['batches'],
        PluginBatchView.fromJson,
      ),
    );
  }
}
