import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

@immutable
class AlertRuleCondition {
  const AlertRuleCondition({
    required this.field,
    required this.operator,
    required this.value,
    this.pattern,
    this.windowSeconds,
    this.minSeverity,
  });

  final String field;
  final String operator;
  final Object value;
  final String? pattern;
  final int? windowSeconds;
  final int? minSeverity;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'field': field,
    'operator': operator,
    'value': value,
    if (pattern != null && pattern!.isNotEmpty) 'pattern': pattern,
    if (windowSeconds != null) 'windowSeconds': windowSeconds,
    if (minSeverity != null) 'minSeverity': minSeverity,
  };

  static String normalizeOperator(Object? raw) {
    final op = (raw ?? 'eq').toString().trim();
    return switch (op) {
      '==' => 'eq',
      '!=' => 'neq',
      '>' => 'gt',
      '>=' => 'gte',
      '<' => 'lt',
      '<=' => 'lte',
      'matches' => 'contains',
      '' => 'eq',
      _ => op,
    };
  }

  factory AlertRuleCondition.fromJson(Map<String, dynamic> json) {
    return AlertRuleCondition(
      field: (json['field'] ?? 'status').toString(),
      operator: normalizeOperator(json['operator']),
      value: json['value'] ?? '',
      pattern: json['pattern'] as String?,
      windowSeconds: parseIntOrNull(json['windowSeconds']),
      minSeverity: parseIntOrNull(json['minSeverity']),
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
      id: parseIntOrNull(json['id']) ?? 0,
      siteId: parseIntOrNull(json['siteId']),
      deviceId: parseIntOrNull(json['deviceId']),
      monitorId: parseIntOrNull(json['monitorId']),
      name: (json['name'] ?? '').toString(),
      type: (json['type'] ?? 'custom').toString(),
      templateKey: json['templateKey'] as String?,
      condition: json['condition'] is Map
          ? AlertRuleCondition.fromJson(asJsonMap(json['condition']))
          : const AlertRuleCondition(
              field: 'status',
              operator: 'eq',
              value: 'down',
            ),
      severity: (json['severity'] ?? 'warning').toString(),
      durationSeconds: parseIntOrNull(json['durationSeconds']) ?? 60,
      recoveryWindowSeconds: parseIntOrNull(json['recoveryWindowSeconds']) ?? 0,
      flapThreshold: parseIntOrNull(json['flapThreshold']) ?? 0,
      flapWindowSeconds: parseIntOrNull(json['flapWindowSeconds']) ?? 300,
      notificationCooldownSeconds:
          parseIntOrNull(json['notificationCooldownSeconds']) ?? 0,
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
          ? AlertRuleCondition.fromJson(asJsonMap(json['condition']))
          : const AlertRuleCondition(
              field: 'status',
              operator: 'eq',
              value: 'down',
            ),
      severity: (json['severity'] ?? 'warning').toString(),
      durationSeconds: parseIntOrNull(json['durationSeconds']) ?? 60,
      recommended: json['recommended'] == true,
      applied: json['applied'] == true,
      ruleId: parseIntOrNull(json['ruleId']),
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

  Map<String, dynamic>? get aiSummary =>
      data?['aiSummary'] is Map ? asJsonMap(data!['aiSummary']) : null;

  Map<String, dynamic>? get deciderTriage =>
      data?['deciderTriage'] is Map ? asJsonMap(data!['deciderTriage']) : null;

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
    final dev = json['device'] is Map ? asJsonMap(json['device']) : null;
    final mon = json['monitor'] is Map ? asJsonMap(json['monitor']) : null;
    return AlertEvent(
      id: parseIntOrNull(json['id'] ?? json['alertEventId']) ?? 0,
      alertRuleId: parseIntOrNull(json['alertRuleId']),
      deviceId: parseIntOrNull(json['deviceId']),
      monitorId: parseIntOrNull(json['monitorId']),
      severity: (json['severity'] ?? 'warning').toString(),
      status: (json['status'] ?? 'active').toString(),
      title: (json['title'] ?? json['ruleName'] ?? 'Alerta').toString(),
      message: (json['message'] ?? '').toString(),
      data: json['data'] is Map ? asJsonMap(json['data']) : null,
      acknowledgedBy: json['acknowledgedBy'] as String?,
      silencedUntil: json['silencedUntil'] as String?,
      deviceName: (json['deviceName'] ?? dev?['name']) as String?,
      monitorName: (json['monitorName'] ?? mon?['name']) as String?,
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
      oscillations: parseIntOrNull(json['oscillations']) ?? 0,
      episodes: parseIntOrNull(json['episodes']) ?? 0,
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
      confidence: parseDoubleOrNull(json['confidence']) ?? 0,
      explanation: (json['explanation'] ?? '').toString(),
      impactedDevicesCount: parseIntOrNull(json['impactedDevicesCount']) ?? 0,
      totalAlertsCount: parseIntOrNull(json['totalAlertsCount']) ?? 0,
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
    return RootCauseAnalysisSummary(
      activeClusters: asJsonList<IncidentCluster>(
        json['activeClusters'],
        IncidentCluster.fromJson,
      ),
      totalActiveIncidents: parseIntOrNull(json['totalActiveIncidents']) ?? 0,
      totalCorrelatedAlerts: parseIntOrNull(json['totalCorrelatedAlerts']) ?? 0,
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
        ? asJsonMap(json['primaryCause'])
        : null;
    return AlertCorrelation(
      windowSeconds: parseIntOrNull(json['windowSeconds']) ?? 300,
      causalCategoryLabel:
          (json['causalCategoryLabel'] ??
                  json['causalCategory'] ??
                  'Correlação')
              .toString(),
      confidence: parseDoubleOrNull(json['confidence']) ?? 0,
      explanation: (json['explanation'] ?? '').toString(),
      impactedDevicesCount: parseIntOrNull(json['impactedDevicesCount']) ?? 0,
      correlationCount: parseIntOrNull(json['correlationCount']) ?? 0,
      primaryCauseTitle: primary?['title'] as String?,
      dependencyChain: asJsonMapList(json['dependencyChain']),
      impactedDevices: asJsonMapList(json['impactedDevices']),
      relatedEvents: asJsonMapList(json['relatedEvents']),
    );
  }
}
