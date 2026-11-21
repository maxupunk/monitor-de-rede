import 'package:flutter/foundation.dart';

@immutable
class AlertRuleCondition {
  const AlertRuleCondition({
    required this.field,
    required this.operator,
    required this.value,
  });

  final String field;
  final String operator;
  final Object value;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'field': field,
    'operator': operator,
    'value': value,
  };

  factory AlertRuleCondition.fromJson(Map<String, dynamic> json) {
    return AlertRuleCondition(
      field: (json['field'] ?? 'status').toString(),
      operator: (json['operator'] ?? '==').toString(),
      value: json['value'] ?? '',
    );
  }
}

@immutable
class AlertRule {
  const AlertRule({
    required this.id,
    this.siteId,
    this.deviceId,
    this.monitorId,
    required this.name,
    required this.type,
    this.templateKey,
    required this.condition,
    required this.severity,
    required this.durationSeconds,
    this.recoveryWindowSeconds = 0,
    this.flapThreshold = 0,
    this.flapWindowSeconds = 300,
    this.notificationCooldownSeconds = 0,
    this.inhibitWhenParentDown = false,
    required this.isEnabled,
  });

  final int id;
  final int? siteId;
  final int? deviceId;
  final int? monitorId;
  final String name;
  final String type;
  final String? templateKey;
  final AlertRuleCondition condition;
  final String severity;
  final int durationSeconds;
  final int recoveryWindowSeconds;
  final int flapThreshold;
  final int flapWindowSeconds;
  final int notificationCooldownSeconds;
  final bool inhibitWhenParentDown;
  final bool isEnabled;

  factory AlertRule.fromJson(Map<String, dynamic> json) {
    final enabled =
        (json['isEnabled'] as bool?) ?? (json['enabled'] as bool?) ?? true;
    return AlertRule(
      id: (json['id'] as num?)?.toInt() ?? 0,
      siteId: (json['siteId'] as num?)?.toInt(),
      deviceId: (json['deviceId'] as num?)?.toInt(),
      monitorId: (json['monitorId'] as num?)?.toInt(),
      name: (json['name'] ?? '').toString(),
      type: (json['type'] ?? 'custom').toString(),
      templateKey: json['templateKey'] as String?,
      condition: json['condition'] is Map
          ? AlertRuleCondition.fromJson(
              Map<String, dynamic>.from(json['condition'] as Map),
            )
          : const AlertRuleCondition(
              field: 'status',
              operator: '==',
              value: 'down',
            ),
      severity: (json['severity'] ?? 'warning').toString(),
      durationSeconds: (json['durationSeconds'] as num?)?.toInt() ?? 60,
      recoveryWindowSeconds:
          (json['recoveryWindowSeconds'] as num?)?.toInt() ?? 0,
      flapThreshold: (json['flapThreshold'] as num?)?.toInt() ?? 0,
      flapWindowSeconds: (json['flapWindowSeconds'] as num?)?.toInt() ?? 300,
      notificationCooldownSeconds:
          (json['notificationCooldownSeconds'] as num?)?.toInt() ?? 0,
      inhibitWhenParentDown: json['inhibitWhenParentDown'] == true,
      isEnabled: enabled,
    );
  }
}

@immutable
class AlertRuleTemplate {
  const AlertRuleTemplate({
    required this.key,
    required this.name,
    required this.description,
    required this.category,
    this.area,
    required this.type,
    required this.condition,
    required this.severity,
    required this.durationSeconds,
    this.recommended = false,
    this.applied = false,
    this.ruleId,
    this.applicable = true,
  });

  final String key;
  final String name;
  final String description;
  final String category;
  final String? area;
  final String type;
  final AlertRuleCondition condition;
  final String severity;
  final int durationSeconds;
  final bool recommended;
  final bool applied;
  final int? ruleId;
  final bool applicable;

  factory AlertRuleTemplate.fromJson(Map<String, dynamic> json) {
    return AlertRuleTemplate(
      key: (json['key'] ?? '').toString(),
      name: (json['name'] ?? '').toString(),
      description: (json['description'] ?? '').toString(),
      category: (json['category'] ?? 'geral').toString(),
      area: json['area'] as String?,
      type: (json['type'] ?? 'custom').toString(),
      condition: json['condition'] is Map
          ? AlertRuleCondition.fromJson(
              Map<String, dynamic>.from(json['condition'] as Map),
            )
          : const AlertRuleCondition(
              field: 'status',
              operator: '==',
              value: 'down',
            ),
      severity: (json['severity'] ?? 'warning').toString(),
      durationSeconds: (json['durationSeconds'] as num?)?.toInt() ?? 60,
      recommended: json['recommended'] == true,
      applied: json['applied'] == true,
      ruleId: (json['ruleId'] as num?)?.toInt(),
      applicable: json['applicable'] != false,
    );
  }
}

@immutable
class AlertEvent {
  const AlertEvent({
    required this.id,
    this.alertRuleId,
    this.deviceId,
    this.monitorId,
    required this.severity,
    required this.status,
    required this.title,
    required this.message,
    this.data,
    this.acknowledgedBy,
    this.silencedUntil,
    this.deviceName,
    this.monitorName,
    this.startedAt,
    required this.createdAt,
    this.resolvedAt,
  });

  final int id;
  final int? alertRuleId;
  final int? deviceId;
  final int? monitorId;
  final String severity;
  final String status;
  final String title;
  final String message;
  final Map<String, dynamic>? data;
  final String? acknowledgedBy;
  final String? silencedUntil;
  final String? deviceName;
  final String? monitorName;
  final String? startedAt;
  final String createdAt;
  final String? resolvedAt;

  bool get isResolved => status == 'resolved';

  Map<String, dynamic>? get aiSummary => data?['aiSummary'] is Map
      ? Map<String, dynamic>.from(data!['aiSummary'] as Map)
      : null;

  Map<String, dynamic>? get deciderTriage => data?['deciderTriage'] is Map
      ? Map<String, dynamic>.from(data!['deciderTriage'] as Map)
      : null;

  AlertEvent copyWith({
    String? severity,
    String? status,
    String? title,
    String? message,
    Map<String, dynamic>? data,
    String? acknowledgedBy,
    String? silencedUntil,
    String? resolvedAt,
  }) {
    return AlertEvent(
      id: id,
      alertRuleId: alertRuleId,
      deviceId: deviceId,
      monitorId: monitorId,
      severity: severity ?? this.severity,
      status: status ?? this.status,
      title: title ?? this.title,
      message: message ?? this.message,
      data: data ?? this.data,
      acknowledgedBy: acknowledgedBy ?? this.acknowledgedBy,
      silencedUntil: silencedUntil ?? this.silencedUntil,
      deviceName: deviceName,
      monitorName: monitorName,
      startedAt: startedAt,
      createdAt: createdAt,
      resolvedAt: resolvedAt ?? this.resolvedAt,
    );
  }

  factory AlertEvent.fromJson(Map<String, dynamic> json) {
    final dev = json['device'] is Map
        ? Map<String, dynamic>.from(json['device'] as Map)
        : null;
    final mon = json['monitor'] is Map
        ? Map<String, dynamic>.from(json['monitor'] as Map)
        : null;
    return AlertEvent(
      id: ((json['id'] ?? json['alertEventId']) as num?)?.toInt() ?? 0,
      alertRuleId: (json['alertRuleId'] as num?)?.toInt(),
      deviceId: (json['deviceId'] as num?)?.toInt(),
      monitorId: (json['monitorId'] as num?)?.toInt(),
      severity: (json['severity'] ?? 'warning').toString(),
      status: (json['status'] ?? 'active').toString(),
      title: (json['title'] ?? json['ruleName'] ?? 'Alerta').toString(),
      message: (json['message'] ?? '').toString(),
      data: json['data'] is Map
          ? Map<String, dynamic>.from(json['data'] as Map)
          : null,
      acknowledgedBy: json['acknowledgedBy'] as String?,
      silencedUntil: json['silencedUntil'] as String?,
      deviceName: dev?['name'] as String?,
      monitorName: mon?['name'] as String?,
      startedAt: json['startedAt'] as String?,
      createdAt: (json['createdAt'] ?? json['startedAt'] ?? '').toString(),
      resolvedAt: json['resolvedAt'] as String?,
    );
  }
}

@immutable
class ScopeInstability {
  const ScopeInstability({
    required this.scopeKey,
    required this.oscillations,
    required this.episodes,
    required this.flapping,
    this.lastProblemAt,
  });

  final String scopeKey;
  final int oscillations;
  final int episodes;
  final bool flapping;
  final String? lastProblemAt;

  factory ScopeInstability.fromJson(Map<String, dynamic> json) {
    return ScopeInstability(
      scopeKey: (json['scopeKey'] ?? '').toString(),
      oscillations: (json['oscillations'] as num?)?.toInt() ?? 0,
      episodes: (json['episodes'] as num?)?.toInt() ?? 0,
      flapping: json['flapping'] == true,
      lastProblemAt: json['lastProblemAt'] as String?,
    );
  }
}

@immutable
class IncidentCluster {
  const IncidentCluster({
    required this.id,
    this.rootCauseDeviceName,
    required this.causalCategoryLabel,
    required this.confidence,
    required this.explanation,
    required this.impactedDevicesCount,
    required this.totalAlertsCount,
    required this.maxSeverity,
    this.startedAt,
  });

  final String id;
  final String? rootCauseDeviceName;
  final String causalCategoryLabel;
  final double confidence;
  final String explanation;
  final int impactedDevicesCount;
  final int totalAlertsCount;
  final String maxSeverity;
  final String? startedAt;

  factory IncidentCluster.fromJson(Map<String, dynamic> json) {
    return IncidentCluster(
      id: (json['id'] ?? '').toString(),
      rootCauseDeviceName: json['rootCauseDeviceName'] as String?,
      causalCategoryLabel:
          (json['causalCategoryLabel'] ?? json['causalCategory'] ?? '')
              .toString(),
      confidence: (json['confidence'] as num?)?.toDouble() ?? 0,
      explanation: (json['explanation'] ?? '').toString(),
      impactedDevicesCount:
          (json['impactedDevicesCount'] as num?)?.toInt() ?? 0,
      totalAlertsCount: (json['totalAlertsCount'] as num?)?.toInt() ?? 0,
      maxSeverity: (json['maxSeverity'] ?? 'warning').toString(),
      startedAt: json['startedAt'] as String?,
    );
  }
}

@immutable
class RootCauseAnalysisSummary {
  const RootCauseAnalysisSummary({
    required this.activeClusters,
    required this.totalActiveIncidents,
    required this.totalCorrelatedAlerts,
  });

  final List<IncidentCluster> activeClusters;
  final int totalActiveIncidents;
  final int totalCorrelatedAlerts;

  factory RootCauseAnalysisSummary.fromJson(Map<String, dynamic> json) {
    final raw = json['activeClusters'] is List
        ? json['activeClusters'] as List
        : const [];
    return RootCauseAnalysisSummary(
      activeClusters: raw
          .whereType<Map>()
          .map((e) => IncidentCluster.fromJson(Map<String, dynamic>.from(e)))
          .toList(growable: false),
      totalActiveIncidents:
          (json['totalActiveIncidents'] as num?)?.toInt() ?? 0,
      totalCorrelatedAlerts:
          (json['totalCorrelatedAlerts'] as num?)?.toInt() ?? 0,
    );
  }
}

@immutable
class AlertCorrelation {
  const AlertCorrelation({
    required this.windowSeconds,
    required this.causalCategoryLabel,
    required this.confidence,
    required this.explanation,
    required this.impactedDevicesCount,
    required this.correlationCount,
    this.primaryCauseTitle,
    this.dependencyChain = const <Map<String, dynamic>>[],
    this.impactedDevices = const <Map<String, dynamic>>[],
    this.relatedEvents = const <Map<String, dynamic>>[],
  });

  final int windowSeconds;
  final String causalCategoryLabel;
  final double confidence;
  final String explanation;
  final int impactedDevicesCount;
  final int correlationCount;
  final String? primaryCauseTitle;
  final List<Map<String, dynamic>> dependencyChain;
  final List<Map<String, dynamic>> impactedDevices;
  final List<Map<String, dynamic>> relatedEvents;

  factory AlertCorrelation.fromJson(Map<String, dynamic> json) {
    final primary = json['primaryCause'] is Map
        ? Map<String, dynamic>.from(json['primaryCause'] as Map)
        : null;
    List<Map<String, dynamic>> parseList(Object? raw) => raw is List
        ? raw
              .whereType<Map>()
              .map((e) => Map<String, dynamic>.from(e))
              .toList(growable: false)
        : const <Map<String, dynamic>>[];

    return AlertCorrelation(
      windowSeconds: (json['windowSeconds'] as num?)?.toInt() ?? 300,
      causalCategoryLabel:
          (json['causalCategoryLabel'] ??
                  json['causalCategory'] ??
                  'Correlação')
              .toString(),
      confidence: (json['confidence'] as num?)?.toDouble() ?? 0,
      explanation: (json['explanation'] ?? '').toString(),
      impactedDevicesCount:
          (json['impactedDevicesCount'] as num?)?.toInt() ?? 0,
      correlationCount: (json['correlationCount'] as num?)?.toInt() ?? 0,
      primaryCauseTitle: primary?['title'] as String?,
      dependencyChain: parseList(json['dependencyChain']),
      impactedDevices: parseList(json['impactedDevices']),
      relatedEvents: parseList(json['relatedEvents']),
    );
  }
}
