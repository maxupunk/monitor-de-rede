import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

@immutable
class MaintenanceWindow {
  const MaintenanceWindow({
    required this.id,
    this.siteId,
    this.deviceId,
    required this.name,
    this.description,
    required this.startsAt,
    required this.endsAt,
    this.createdBy,
    required this.createdAt,
    required this.updatedAt,
  });

  final int id;
  final int? siteId;
  final int? deviceId;
  final String name;
  final String? description;
  final String startsAt;
  final String endsAt;
  final int? createdBy;
  final String createdAt;
  final String updatedAt;

  String statusAt(DateTime now) {
    final start = parseDateOrNull(startsAt);
    final end = parseDateOrNull(endsAt);
    if (start == null || end == null) return 'scheduled';
    if (now.isBefore(start)) return 'scheduled';
    if (now.isAfter(end)) return 'ended';
    return 'active';
  }

  factory MaintenanceWindow.fromJson(Map<String, dynamic> json) {
    return MaintenanceWindow(
      id: parseIntOrNull(json['id']) ?? 0,
      siteId: parseIntOrNull(json['siteId']),
      deviceId: parseIntOrNull(json['deviceId']),
      name: (json['name'] ?? '').toString(),
      description: json['description'] as String?,
      startsAt: (json['startsAt'] ?? json['startTime'] ?? '').toString(),
      endsAt: (json['endsAt'] ?? json['endTime'] ?? '').toString(),
      createdBy: parseIntOrNull(json['createdBy']),
      createdAt: (json['createdAt'] ?? '').toString(),
      updatedAt: (json['updatedAt'] ?? '').toString(),
    );
  }
}

@immutable
class ScheduledTask {
  const ScheduledTask({
    required this.id,
    required this.name,
    this.description,
    required this.tool,
    required this.arguments,
    required this.cron,
    required this.scheduleLabel,
    required this.enabled,
    required this.origin,
    this.nextRunAt,
    this.lastRunAt,
    this.lastStatus,
    this.lastMessage,
    this.ownerId,
    this.ownerName,
    this.running = false,
    required this.createdAt,
    required this.updatedAt,
  });

  final int id;
  final String name;
  final String? description;
  final String tool;
  final Map<String, dynamic> arguments;
  final String cron;
  final String scheduleLabel;
  final bool enabled;
  final String origin;
  final String? nextRunAt;
  final String? lastRunAt;
  final String? lastStatus;
  final String? lastMessage;
  final int? ownerId;
  final String? ownerName;
  final bool running;
  final String createdAt;
  final String updatedAt;

  factory ScheduledTask.fromJson(Map<String, dynamic> json) {
    return ScheduledTask(
      id: parseIntOrNull(json['id']) ?? 0,
      name: (json['name'] ?? '').toString(),
      description: json['description'] as String?,
      tool: (json['tool'] ?? '').toString(),
      arguments: asJsonMap(json['arguments']),
      cron: (json['cron'] ?? '').toString(),
      scheduleLabel: (json['scheduleLabel'] ?? json['cron'] ?? '').toString(),
      enabled: json['enabled'] != false,
      origin: (json['origin'] ?? 'user').toString(),
      nextRunAt: json['nextRunAt'] as String?,
      lastRunAt: json['lastRunAt'] as String?,
      lastStatus: json['lastStatus'] as String?,
      lastMessage: json['lastMessage'] as String?,
      ownerId: parseIntOrNull(json['ownerId']),
      ownerName: json['ownerName'] as String?,
      running: json['running'] == true,
      createdAt: (json['createdAt'] ?? '').toString(),
      updatedAt: (json['updatedAt'] ?? '').toString(),
    );
  }
}

@immutable
class SchedulableTool {
  const SchedulableTool({
    required this.name,
    required this.description,
    required this.group,
    required this.groupLabel,
    required this.kind,
    required this.adminOnly,
    required this.parameters,
  });

  final String name;
  final String description;
  final String group;
  final String groupLabel;
  final String kind;
  final bool adminOnly;
  final Map<String, dynamic> parameters;

  factory SchedulableTool.fromJson(Map<String, dynamic> json) {
    return SchedulableTool(
      name: (json['name'] ?? '').toString(),
      description: (json['description'] ?? '').toString(),
      group: (json['group'] ?? 'general').toString(),
      groupLabel: (json['groupLabel'] ?? json['group'] ?? 'Geral').toString(),
      kind: (json['kind'] ?? 'read').toString(),
      adminOnly: json['adminOnly'] == true,
      parameters: asJsonMap(json['parameters']),
    );
  }
}

@immutable
class ScheduledTaskPreview {
  const ScheduledTaskPreview({
    required this.valid,
    required this.errors,
    this.scheduleLabel,
    required this.nextRuns,
    this.summary,
    this.risk,
    this.riskReason,
  });

  final bool valid;
  final List<String> errors;
  final String? scheduleLabel;
  final List<String> nextRuns;
  final String? summary;
  final String? risk;
  final String? riskReason;

  factory ScheduledTaskPreview.fromJson(Map<String, dynamic> json) {
    return ScheduledTaskPreview(
      valid: json['valid'] == true,
      errors: asStringList(json['errors']),
      scheduleLabel: json['scheduleLabel'] as String?,
      nextRuns: asStringList(json['nextRuns']),
      summary: json['summary'] as String?,
      risk: json['risk'] as String?,
      riskReason: json['riskReason'] as String?,
    );
  }
}

@immutable
class ScheduledTaskRun {
  const ScheduledTaskRun({
    required this.id,
    required this.taskId,
    required this.trigger,
    required this.status,
    required this.startedAt,
    this.finishedAt,
    this.durationMs,
    this.message,
    this.result,
  });

  final int id;
  final int taskId;
  final String trigger;
  final String status;
  final String startedAt;
  final String? finishedAt;
  final int? durationMs;
  final String? message;
  final Object? result;

  factory ScheduledTaskRun.fromJson(Map<String, dynamic> json) {
    return ScheduledTaskRun(
      id: parseIntOrNull(json['id']) ?? 0,
      taskId: parseIntOrNull(json['taskId']) ?? 0,
      trigger: (json['trigger'] ?? 'schedule').toString(),
      status: (json['status'] ?? 'unknown').toString(),
      startedAt: (json['startedAt'] ?? '').toString(),
      finishedAt: json['finishedAt'] as String?,
      durationMs: parseIntOrNull(json['durationMs']),
      message: json['message'] as String?,
      result: json['result'],
    );
  }
}
