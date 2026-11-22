import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_manifest_models.dart';
import 'package:netmonitor_app/shared/device_access/device_access_models.dart'
    hide CredentialView;

/// Subtipo compatível de `DeviceCredentialView` que preserva o parâmetro nomeado
/// `dedicated` (`DedicatedAccessInfo?`) para consumidores existentes.
@immutable
class CredentialView extends DeviceCredentialView {
  const CredentialView({
    required super.kind,
    super.username = '',
    super.port,
    super.https = false,
    super.storage = 'ask',
    super.viaProbeId,
    super.hasSecret = false,
    super.sessionActive = false,
    super.sessionExpiresAt,
    DedicatedAccessInfo? dedicated,
    DedicatedAccessInfo? dedicatedInfo,
    super.dedicatedFlag = false,
    super.fingerprint,
    super.updatedAt,
  }) : super(dedicatedInfo: dedicatedInfo ?? dedicated);

  factory CredentialView.fromJson(Map<String, dynamic> json) {
    final base = DeviceCredentialView.fromJson(json);
    return CredentialView(
      kind: base.kind,
      username: base.username,
      port: base.port,
      https: base.https,
      storage: base.storage,
      viaProbeId: base.viaProbeId,
      hasSecret: base.hasSecret,
      sessionActive: base.sessionActive,
      sessionExpiresAt: base.sessionExpiresAt,
      dedicatedInfo: base.dedicatedInfo,
      dedicatedFlag: base.dedicatedFlag,
      fingerprint: base.fingerprint,
      updatedAt: base.updatedAt,
    );
  }
}

/// Entrada da transcrição de uma execução de plugin (`TranscriptEntry`).
@immutable
class TranscriptEntry {
  const TranscriptEntry({
    required this.seq,
    required this.kind,
    required this.request,
    this.response,
    this.durationMs = 0,
    this.ok = true,
  });

  final int seq;
  final String
  kind; // 'ssh' | 'telnet' | 'http' | 'log' | 'denied' | 'error' | 'step:*'
  final String request;
  final String? response;
  final int durationMs;
  final bool ok;

  factory TranscriptEntry.fromJson(Map<String, dynamic> json) {
    return TranscriptEntry(
      seq: parseIntOrNull(json['seq']) ?? 0,
      kind: (json['kind'] as String?) ?? 'log',
      request: (json['request'] as String?) ?? '',
      response: json['response'] as String?,
      durationMs: parseIntOrNull(json['durationMs']) ?? 0,
      ok: parseBool(json['ok'], fallback: true),
    );
  }
}

@immutable
class PluginRunView {
  const PluginRunView({
    required this.id,
    required this.pluginId,
    this.pluginName,
    required this.deviceId,
    required this.action,
    this.origin = 'user',
    required this.status,
    this.params = const <String, dynamic>{},
    this.output,
    this.transcript = const <TranscriptEntry>[],
    this.error,
    this.createdAt = '',
    this.finishedAt,
  });

  final int id;
  final int pluginId;
  final String? pluginName;
  final int deviceId;
  final String action;
  final String origin;
  final String status; // 'running' | 'succeeded' | 'failed' | 'cancelled'
  final Map<String, dynamic> params;
  final Object? output;
  final List<TranscriptEntry> transcript;
  final String? error;
  final String createdAt;
  final String? finishedAt;

  bool get isFinished => status != 'running' && status != 'queued';

  PluginRunView copyWith({
    String? pluginName,
    String? status,
    Object? output,
    List<TranscriptEntry>? transcript,
    String? error,
    String? finishedAt,
  }) {
    return PluginRunView(
      id: id,
      pluginId: pluginId,
      pluginName: pluginName ?? this.pluginName,
      deviceId: deviceId,
      action: action,
      origin: origin,
      status: status ?? this.status,
      params: params,
      output: output ?? this.output,
      transcript: transcript ?? this.transcript,
      error: error ?? this.error,
      createdAt: createdAt,
      finishedAt: finishedAt ?? this.finishedAt,
    );
  }

  factory PluginRunView.fromJson(Map<String, dynamic> json) {
    return PluginRunView(
      id: parseIntOrNull(json['id']) ?? 0,
      pluginId: parseIntOrNull(json['pluginId']) ?? 0,
      pluginName: json['pluginName'] as String?,
      deviceId: parseIntOrNull(json['deviceId']) ?? 0,
      action: (json['action'] as String?) ?? '',
      origin: (json['origin'] as String?) ?? 'user',
      status: (json['status'] as String?) ?? 'running',
      params: asJsonMap(json['params']),
      output: json['output'],
      transcript: asJsonList<TranscriptEntry>(
        json['transcript'],
        TranscriptEntry.fromJson,
      ),
      error: json['error'] as String?,
      createdAt: (json['createdAt'] as String?) ?? '',
      finishedAt: json['finishedAt'] as String?,
    );
  }
}

@immutable
class DevicePluginItem {
  const DevicePluginItem({
    required this.plugin,
    required this.installed,
    required this.compatibility,
    this.reasons = const <String>[],
    this.deviceSettings,
  });

  final PluginSummary plugin;
  final bool installed;
  final String
  compatibility; // 'validated' | 'likely' | 'possible' | 'incompatible'
  final List<String> reasons;
  final Map<String, dynamic>? deviceSettings;

  DevicePluginItem copyWith({Map<String, dynamic>? deviceSettings}) {
    return DevicePluginItem(
      plugin: plugin,
      installed: installed,
      compatibility: compatibility,
      reasons: reasons,
      deviceSettings: deviceSettings ?? this.deviceSettings,
    );
  }

  factory DevicePluginItem.fromJson(Map<String, dynamic> json) {
    return DevicePluginItem(
      plugin: PluginSummary.fromJson(asJsonMap(json['plugin'])),
      installed: parseBool(json['installed']),
      compatibility:
          ((json['compatibility'] ?? json['compat']) as String?) ?? 'possible',
      reasons: asStringList(json['reasons']),
      deviceSettings: json['deviceSettings'] is Map
          ? asJsonMap(json['deviceSettings'])
          : null,
    );
  }
}

@immutable
class DevicePluginsView {
  const DevicePluginsView({
    required this.deviceId,
    required this.platform,
    this.firmware,
    this.plugins = const <DevicePluginItem>[],
    this.credentials = const <DeviceCredentialView>[],
    this.runs = const <PluginRunView>[],
    this.agents = const <AgentRouteOption>[],
  });

  final int deviceId;
  final String platform;
  final String? firmware;
  final List<DevicePluginItem> plugins;
  final List<DeviceCredentialView> credentials;
  final List<PluginRunView> runs;
  final List<AgentRouteOption> agents;

  DevicePluginsView copyWith({
    String? firmware,
    List<DevicePluginItem>? plugins,
    List<DeviceCredentialView>? credentials,
    List<PluginRunView>? runs,
  }) {
    return DevicePluginsView(
      deviceId: deviceId,
      platform: platform,
      firmware: firmware ?? this.firmware,
      plugins: plugins ?? this.plugins,
      credentials: credentials ?? this.credentials,
      runs: runs ?? this.runs,
      agents: agents,
    );
  }

  factory DevicePluginsView.fromJson(Map<String, dynamic> json) {
    return DevicePluginsView(
      deviceId: parseIntOrNull(json['deviceId']) ?? 0,
      platform: (json['platform'] as String?) ?? 'unknown',
      firmware: json['firmware'] as String?,
      plugins: asJsonList<DevicePluginItem>(
        json['plugins'],
        DevicePluginItem.fromJson,
      ),
      credentials: asJsonList<CredentialView>(
        json['credentials'],
        CredentialView.fromJson,
      ),
      runs: asJsonList<PluginRunView>(json['runs'], PluginRunView.fromJson),
      agents: asJsonList<AgentRouteOption>(
        json['agents'],
        AgentRouteOption.fromJson,
      ),
    );
  }
}

@immutable
class ProgressSample {
  const ProgressSample({
    required this.tMs,
    required this.flow,
    required this.bps,
  });

  final int tMs;
  final String flow; // 'tx' | 'rx'
  final double bps;
}

/// Pedido de aprovação de acesso individual (`plugin:approval_required`).
@immutable
class PluginApprovalRequest {
  const PluginApprovalRequest({
    required this.key,
    required this.runId,
    required this.deviceId,
    required this.deviceName,
    required this.pluginName,
    required this.action,
    required this.transport,
    required this.effect,
    required this.summary,
    this.reason,
    required this.expiresAt,
  });

  final String key;
  final int runId;
  final int deviceId;
  final String deviceName;
  final String pluginName;
  final String action;
  final String transport;
  final String effect; // 'read' | 'write'
  final String summary;
  final String? reason;
  final String expiresAt;

  bool get isWrite => effect == 'write';

  static PluginApprovalRequest? tryFromJson(Map<String, dynamic> json) {
    final key = json['key'] as String?;
    final runId = parseIntOrNull(json['runId']);
    final req = json['request'];
    if (key == null || key.isEmpty || runId == null || req is! Map) return null;
    final reqMap = asJsonMap(req);
    return PluginApprovalRequest(
      key: key,
      runId: runId,
      deviceId: parseIntOrNull(json['deviceId']) ?? 0,
      deviceName: (json['deviceName'] as String?) ?? '',
      pluginName: (json['pluginName'] as String?) ?? '',
      action: (json['action'] as String?) ?? '',
      transport: (reqMap['transport'] as String?) ?? 'ssh',
      effect: (reqMap['effect'] as String?) ?? 'read',
      summary: (reqMap['summary'] ?? reqMap['command'] ?? '').toString(),
      reason: reqMap['reason'] as String?,
      expiresAt: (json['expiresAt'] as String?) ?? '',
    );
  }
}
