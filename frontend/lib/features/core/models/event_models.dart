import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

export 'package:netmonitor_app/core/models/pagination.dart';

@immutable
class HistoricalEventRecord {
  const HistoricalEventRecord({
    required this.id,
    required this.type,
    required this.timestamp,
    required this.data,
    this.severity,
    this.title,
    this.message,
  });

  final int id;
  final String type;
  final String timestamp;
  final Map<String, dynamic> data;
  final String? severity;
  final String? title;
  final String? message;

  factory HistoricalEventRecord.fromJson(Map<String, dynamic> json) {
    final payload = json['data'] is Map
        ? asJsonMap(json['data'])
        : (json['payload'] is Map
              ? asJsonMap(json['payload'])
              : const <String, dynamic>{});
    return HistoricalEventRecord(
      id: parseIntOrNull(json['id']) ?? 0,
      type: (json['type'] ?? json['eventType'] ?? 'event').toString(),
      timestamp: (json['timestamp'] ?? json['createdAt'] ?? '').toString(),
      data: payload,
      severity: (json['severity'] ?? payload['severity']) as String?,
      title: (json['title'] ?? payload['title'] ?? payload['name']) as String?,
      message: (json['message'] ?? payload['message']) as String?,
    );
  }
}

@immutable
class EventPresentation {
  const EventPresentation({
    required this.label,
    required this.category,
    required this.icon,
    required this.tone,
    required this.summary,
  });

  final String label;
  final String category;
  final IconData icon;
  final SemanticTone tone;
  final String summary;

  static EventPresentation describe(String type, Map<String, dynamic> data) {
    final prefix = type.contains(':') ? type.split(':').first : type;
    final status = data['status']?.toString().toLowerCase() ?? '';
    final severity = data['severity']?.toString().toLowerCase() ?? '';

    final (
      String label,
      String category,
      IconData icon,
      SemanticTone tone,
    ) = switch (type) {
      'alert:triggered' => (
        'Alerta disparado',
        'Alertas',
        Icons.notifications_active_outlined,
        severity == 'critical' ? SemanticTone.error : SemanticTone.warning,
      ),
      'alert:resolved' => (
        'Alerta resolvido',
        'Alertas',
        Icons.check_circle_outline,
        SemanticTone.success,
      ),
      'alert:acknowledged' => (
        'Alerta reconhecido',
        'Alertas',
        Icons.done_all,
        SemanticTone.info,
      ),
      'alert:silenced' => (
        'Alerta silenciado',
        'Alertas',
        Icons.notifications_off_outlined,
        SemanticTone.info,
      ),
      'monitor:result' || 'monitor:checked' => (
        'Verificação de monitor',
        'Monitores',
        Icons.monitor_heart_outlined,
        (status == 'down' || status == 'offline')
            ? SemanticTone.error
            : (status == 'warning'
                  ? SemanticTone.warning
                  : SemanticTone.success),
      ),
      'device:status' || 'device:updated' => (
        'Estado de dispositivo',
        'Dispositivos',
        Icons.devices_outlined,
        status == 'offline' ? SemanticTone.error : SemanticTone.success,
      ),
      'backup:failed' => (
        'Falha de backup',
        'Backup',
        Icons.backup_outlined,
        SemanticTone.error,
      ),
      'maintenance_windows:updated' => (
        'Janela de manutenção',
        'Operação',
        Icons.build_circle_outlined,
        SemanticTone.info,
      ),
      'scheduled_task:updated' => (
        'Tarefa agendada',
        'Operação',
        Icons.event_repeat_outlined,
        status == 'failed' ? SemanticTone.error : SemanticTone.info,
      ),
      'areas:updated' => (
        'Áreas do sistema',
        'Sistema',
        Icons.apps,
        SemanticTone.primary,
      ),
      _ => (
        type,
        switch (prefix) {
          'alert' || 'alert_rule' => 'Alertas',
          'monitor' || 'metric' => 'Monitores',
          'device' || 'interface' => 'Dispositivos',
          'docker' || 'deploy' => 'Servidores',
          'vpn' => 'VPN',
          'plugin' => 'Plugins',
          'ai' || 'decider' => 'IA',
          _ => 'Sistema',
        },
        Icons.bolt_outlined,
        SemanticTone.primary,
      ),
    };

    final summary = data['message']?.toString().isNotEmpty == true
        ? data['message'].toString()
        : (data['title']?.toString().isNotEmpty == true
              ? data['title'].toString()
              : (data['monitorName'] ??
                        data['deviceName'] ??
                        data['name'] ??
                        type)
                    .toString());

    return EventPresentation(
      label: label,
      category: category,
      icon: icon,
      tone: tone,
      summary: summary,
    );
  }
}
