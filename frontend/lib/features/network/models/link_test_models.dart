import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';

bool isLinkTestActiveStatus(String status) =>
    status == 'queued' ||
    status == 'running' ||
    status == 'preparing' ||
    status == 'pending';

@immutable
class LinkTestSpecModel {
  const LinkTestSpecModel({
    this.durationS = 10,
    this.direction = 'both',
    this.streams = 4,
    this.limitMbps,
  });

  final int durationS;
  final String direction;
  final int streams;
  final double? limitMbps;

  factory LinkTestSpecModel.fromJson(Map<String, dynamic> json) {
    return LinkTestSpecModel(
      durationS: parseIntOrNull(json['durationS']) ?? 10,
      direction: (json['direction'] as String?) ?? 'both',
      streams: parseIntOrNull(json['streams']) ?? 4,
      limitMbps: parseDoubleOrNull(json['limitMbps']),
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
    'durationS': durationS,
    'direction': direction,
    'streams': streams,
    if (limitMbps != null && limitMbps! > 0) 'limitMbps': limitMbps,
  };
}

@immutable
class LinkTestEndpointModel {
  const LinkTestEndpointModel({
    required this.kind,
    required this.name,
    this.deviceId,
    this.agentId,
    this.ip,
    this.platform,
  });

  final String kind;
  final int? deviceId;
  final int? agentId;
  final String name;
  final String? ip;
  final String? platform;

  factory LinkTestEndpointModel.fromJson(Map<String, dynamic> json) {
    return LinkTestEndpointModel(
      kind: (json['kind'] as String?) ?? 'device',
      deviceId: parseIntOrNull(json['deviceId']),
      agentId: parseIntOrNull(json['agentId']),
      name: (json['name'] as String?) ?? 'Ponto',
      ip: json['ip'] as String?,
      platform: json['platform'] as String?,
    );
  }
}

@immutable
class LinkTestStageModel {
  const LinkTestStageModel({
    required this.key,
    required this.label,
    required this.status,
    this.message,
    this.runId,
  });

  final String key;
  final String label;
  final String status;
  final String? message;
  final int? runId;

  factory LinkTestStageModel.fromJson(Map<String, dynamic> json) {
    return LinkTestStageModel(
      key: (json['key'] as String?) ?? '',
      label: (json['label'] as String?) ?? '',
      status: (json['status'] as String?) ?? 'pending',
      message: json['message'] as String?,
      runId: parseIntOrNull(json['runId']),
    );
  }
}

@immutable
class LinkTestSampleModel {
  const LinkTestSampleModel({
    required this.flow,
    required this.bps,
    required this.tMs,
    this.atMs,
  });

  final String flow;
  final double bps;
  final int tMs;
  final int? atMs;

  factory LinkTestSampleModel.fromJson(Map<String, dynamic> json) {
    return LinkTestSampleModel(
      flow: (json['flow'] as String?) ?? 'forward',
      bps: parseDoubleOrNull(json['bps']) ?? 0,
      tMs: parseIntOrNull(json['tMs']) ?? 0,
      atMs: parseIntOrNull(json['atMs']),
    );
  }
}

@immutable
class LinkTestHealthSampleModel {
  const LinkTestHealthSampleModel({
    required this.deviceId,
    required this.atMs,
    required this.phase,
    this.cpu,
    this.memory,
  });

  final int deviceId;
  final int atMs;
  final double? cpu;
  final double? memory;
  final String phase;

  factory LinkTestHealthSampleModel.fromJson(Map<String, dynamic> json) {
    return LinkTestHealthSampleModel(
      deviceId: parseIntOrNull(json['deviceId']) ?? 0,
      atMs: parseIntOrNull(json['atMs']) ?? 0,
      cpu: parseDoubleOrNull(json['cpu']),
      memory: parseDoubleOrNull(json['memory']),
      phase: (json['phase'] as String?) ?? 'during',
    );
  }
}

@immutable
class LinkTestFixModel {
  const LinkTestFixModel({
    required this.kind,
    required this.deviceId,
    required this.deviceName,
    required this.message,
    this.pluginId,
    this.pluginSlug,
    this.pluginName,
    this.transport,
    this.action,
    this.engine,
    this.role,
    this.packages = const <String>[],
  });

  final String kind;
  final int deviceId;
  final String deviceName;
  final String message;
  final int? pluginId;
  final String? pluginSlug;
  final String? pluginName;
  final String? transport;
  final String? action;
  final String? engine;
  final String? role;
  final List<String> packages;

  factory LinkTestFixModel.fromJson(Map<String, dynamic> json) {
    return LinkTestFixModel(
      kind: (json['kind'] as String?) ?? '',
      deviceId: parseIntOrNull(json['deviceId']) ?? 0,
      deviceName: (json['deviceName'] as String?) ?? '',
      message: (json['message'] as String?) ?? '',
      pluginId: parseIntOrNull(json['pluginId']),
      pluginSlug: json['pluginSlug'] as String?,
      pluginName: json['pluginName'] as String?,
      transport: json['transport'] as String?,
      action: json['action'] as String?,
      engine: json['engine'] as String?,
      role: json['role'] as String?,
      packages: asStringList(json['packages']),
    );
  }
}

@immutable
class LinkTestLegModel {
  const LinkTestLegModel({
    required this.seq,
    required this.kind,
    required this.status,
    required this.a,
    required this.b,
    required this.durationS,
    this.runnable = true,
    this.engine,
    this.serverSide,
    this.edgeId,
    this.edgeIds = const <int>[],
    this.viaNames = const <String>[],
    this.forwardBps,
    this.reverseBps,
    this.retransmits,
    this.error,
    this.stages = const <LinkTestStageModel>[],
    this.issues = const <String>[],
    this.fixes = const <LinkTestFixModel>[],
    this.samples = const <LinkTestSampleModel>[],
    this.health = const <LinkTestHealthSampleModel>[],
  });

  final int seq;
  final String kind;
  final String status;
  final LinkTestEndpointModel a;
  final LinkTestEndpointModel b;
  final int durationS;
  final bool runnable;
  final String? engine;
  final String? serverSide;
  final int? edgeId;
  final List<int> edgeIds;
  final List<String> viaNames;
  final double? forwardBps;
  final double? reverseBps;
  final int? retransmits;
  final String? error;
  final List<LinkTestStageModel> stages;
  final List<String> issues;
  final List<LinkTestFixModel> fixes;
  final List<LinkTestSampleModel> samples;
  final List<LinkTestHealthSampleModel> health;

  factory LinkTestLegModel.fromJson(Map<String, dynamic> json) {
    return LinkTestLegModel(
      seq: parseIntOrNull(json['seq']) ?? 0,
      kind: (json['kind'] as String?) ?? 'link',
      status: (json['status'] as String?) ?? 'pending',
      a: LinkTestEndpointModel.fromJson(asJsonMap(json['a'])),
      b: LinkTestEndpointModel.fromJson(asJsonMap(json['b'])),
      durationS: parseIntOrNull(json['durationS']) ?? 10,
      runnable: (json['runnable'] as bool?) ?? true,
      engine: json['engine'] as String?,
      serverSide: json['serverSide'] as String?,
      edgeId: parseIntOrNull(json['edgeId']),
      edgeIds: asIntList(json['edgeIds']),
      viaNames: asStringList(json['viaNames']),
      forwardBps: parseDoubleOrNull(json['forwardBps']),
      reverseBps: parseDoubleOrNull(json['reverseBps']),
      retransmits: parseIntOrNull(json['retransmits']),
      error: json['error'] as String?,
      stages: asJsonMapList(
        json['stages'],
      ).map(LinkTestStageModel.fromJson).toList(growable: false),
      issues: asStringList(json['issues']),
      fixes: asJsonMapList(
        json['fixes'],
      ).map(LinkTestFixModel.fromJson).toList(growable: false),
      samples: asJsonMapList(
        json['samples'],
      ).map(LinkTestSampleModel.fromJson).toList(growable: false),
      health: asJsonMapList(
        json['health'],
      ).map(LinkTestHealthSampleModel.fromJson).toList(growable: false),
    );
  }
}

@immutable
class LinkTestSummaryModel {
  const LinkTestSummaryModel({
    this.bottleneckLegSeq,
    this.minForwardBps,
    this.minReverseBps,
  });

  final int? bottleneckLegSeq;
  final double? minForwardBps;
  final double? minReverseBps;

  static LinkTestSummaryModel? tryFromJson(Object? raw) {
    if (raw is! Map) return null;
    final json = Map<String, dynamic>.from(raw);
    return LinkTestSummaryModel(
      bottleneckLegSeq: parseIntOrNull(json['bottleneckLegSeq']),
      minForwardBps: parseDoubleOrNull(json['minForwardBps']),
      minReverseBps: parseDoubleOrNull(json['minReverseBps']),
    );
  }
}

@immutable
class LinkTestModel {
  const LinkTestModel({
    required this.id,
    required this.kind,
    required this.status,
    required this.origin,
    required this.spec,
    required this.legs,
    required this.createdAt,
    this.scheduleId,
    this.linkId,
    this.sourceDeviceId,
    this.targetDeviceId,
    this.summary,
    this.error,
    this.startedAt,
    this.finishedAt,
  });

  final int id;
  final String kind;
  final String status;
  final String origin;
  final int? scheduleId;
  final int? linkId;
  final int? sourceDeviceId;
  final int? targetDeviceId;
  final LinkTestSpecModel spec;
  final List<LinkTestLegModel> legs;
  final LinkTestSummaryModel? summary;
  final String? error;
  final String createdAt;
  final String? startedAt;
  final String? finishedAt;

  bool get isActive => isLinkTestActiveStatus(status);

  String get title {
    if (legs.isNotEmpty) {
      final first = legs.first.a.name;
      final last = legs.last.b.name;
      return '$first -> $last';
    }
    return 'Teste #$id';
  }

  static LinkTestModel? tryFromJson(Object? raw) {
    if (raw is! Map) return null;
    final json = Map<String, dynamic>.from(raw);
    final id = parseIntOrNull(json['id']);
    if (id == null || json['kind'] is! String || json['status'] is! String) {
      return null;
    }
    return LinkTestModel(
      id: id,
      kind: json['kind'] as String,
      status: json['status'] as String,
      origin: (json['origin'] as String?) ?? 'manual',
      scheduleId: parseIntOrNull(json['scheduleId']),
      linkId: parseIntOrNull(json['linkId']),
      sourceDeviceId: parseIntOrNull(json['sourceDeviceId']),
      targetDeviceId: parseIntOrNull(json['targetDeviceId']),
      spec: LinkTestSpecModel.fromJson(asJsonMap(json['spec'])),
      legs: asJsonMapList(
        json['legs'],
      ).map(LinkTestLegModel.fromJson).toList(growable: false),
      summary: LinkTestSummaryModel.tryFromJson(json['summary']),
      error: json['error'] as String?,
      createdAt: (json['createdAt'] as String?) ?? '',
      startedAt: json['startedAt'] as String?,
      finishedAt: json['finishedAt'] as String?,
    );
  }
}

@immutable
class LinkTestPlanModel {
  const LinkTestPlanModel({
    required this.kind,
    required this.legs,
    required this.estimatedSeconds,
    this.warnings = const <String>[],
    this.blocking = const <String>[],
    this.fixes = const <LinkTestFixModel>[],
  });

  final String kind;
  final List<LinkTestLegModel> legs;
  final int estimatedSeconds;
  final List<String> warnings;
  final List<String> blocking;
  final List<LinkTestFixModel> fixes;

  bool get isReady => blocking.isEmpty && legs.any((leg) => leg.runnable);

  factory LinkTestPlanModel.fromJson(Map<String, dynamic> json) {
    return LinkTestPlanModel(
      kind: (json['kind'] as String?) ?? 'link',
      legs: asJsonMapList(
        json['legs'],
      ).map(LinkTestLegModel.fromJson).toList(growable: false),
      estimatedSeconds: parseIntOrNull(json['estimatedSeconds']) ?? 0,
      warnings: asStringList(json['warnings']),
      blocking: asStringList(json['blocking']),
      fixes: asJsonMapList(
        json['fixes'],
      ).map(LinkTestFixModel.fromJson).toList(growable: false),
    );
  }
}

@immutable
class LinkTestScheduleModel {
  const LinkTestScheduleModel({
    required this.id,
    required this.name,
    required this.kind,
    required this.spec,
    required this.intervalMinutes,
    required this.enabled,
    this.linkId,
    this.sourceDeviceId,
    this.targetDeviceId,
    this.windowStart,
    this.windowEnd,
    this.lastRunAt,
    this.nextRunAt,
    this.lastTestId,
    this.lastStatus,
  });

  final int id;
  final String name;
  final String kind;
  final int? linkId;
  final int? sourceDeviceId;
  final int? targetDeviceId;
  final LinkTestSpecModel spec;
  final int intervalMinutes;
  final String? windowStart;
  final String? windowEnd;
  final bool enabled;
  final String? lastRunAt;
  final String? nextRunAt;
  final int? lastTestId;
  final String? lastStatus;

  factory LinkTestScheduleModel.fromJson(Map<String, dynamic> json) {
    return LinkTestScheduleModel(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] as String?) ?? 'Agendamento',
      kind: (json['kind'] as String?) ?? 'link',
      linkId: parseIntOrNull(json['linkId']),
      sourceDeviceId: parseIntOrNull(json['sourceDeviceId']),
      targetDeviceId: parseIntOrNull(json['targetDeviceId']),
      spec: LinkTestSpecModel.fromJson(asJsonMap(json['spec'])),
      intervalMinutes: parseIntOrNull(json['intervalMinutes']) ?? 60,
      windowStart: json['windowStart'] as String?,
      windowEnd: json['windowEnd'] as String?,
      enabled: (json['enabled'] as bool?) ?? true,
      lastRunAt: json['lastRunAt'] as String?,
      nextRunAt: json['nextRunAt'] as String?,
      lastTestId: parseIntOrNull(json['lastTestId']),
      lastStatus: json['lastStatus'] as String?,
    );
  }
}

@immutable
class LinkTestRequestDraft {
  const LinkTestRequestDraft({
    required this.kind,
    this.linkId,
    this.sourceDeviceId,
    this.targetDeviceId,
  });

  final String kind;
  final int? linkId;
  final int? sourceDeviceId;
  final int? targetDeviceId;
}
