import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

@immutable
class LogEntry {
  const LogEntry({
    required this.id,
    this.deviceId,
    this.deviceName,
    required this.sourceIp,
    required this.receivedAt,
    this.deviceTime,
    this.facility,
    this.severity,
    this.severityLabel,
    this.hostname,
    this.appName,
    this.pid,
    this.topics = const <String>[],
    required this.message,
    this.templateHash,
  });

  final int id;
  final int? deviceId;
  final String? deviceName;
  final String sourceIp;
  final String receivedAt;
  final String? deviceTime;
  final int? facility;
  final int? severity;
  final String? severityLabel;
  final String? hostname;
  final String? appName;
  final int? pid;
  final List<String> topics;
  final String message;
  final String? templateHash;

  SemanticTone get severityTone {
    final s = severity;
    if (s == null) return SemanticTone.secondary;
    if (s <= 3) return SemanticTone.error;
    if (s == 4) return SemanticTone.warning;
    if (s <= 6) return SemanticTone.info;
    return SemanticTone.secondary;
  }

  factory LogEntry.fromJson(Map<String, dynamic> json) {
    return LogEntry(
      id: parseIntOrNull(json['id']) ?? 0,
      deviceId: parseIntOrNull(json['deviceId']),
      deviceName: json['deviceName'] as String?,
      sourceIp: (json['sourceIp'] ?? '').toString(),
      receivedAt: (json['receivedAt'] ?? '').toString(),
      deviceTime: json['deviceTime'] as String?,
      facility: parseIntOrNull(json['facility']),
      severity: parseIntOrNull(json['severity']),
      severityLabel: json['severityLabel'] as String?,
      hostname: json['hostname'] as String?,
      appName: json['appName'] as String?,
      pid: parseIntOrNull(json['pid']),
      topics: asStringList(json['topics']),
      message: (json['message'] ?? '').toString(),
      templateHash: json['templateHash'] as String?,
    );
  }
}

@immutable
class LogTemplateInfo {
  const LogTemplateInfo({
    required this.templateHash,
    required this.template,
    required this.example,
    this.category,
    this.confidence,
    this.model,
    required this.confirmed,
    this.alertTemplate,
    required this.alertRegex,
  });

  final String templateHash;
  final String template;
  final String example;
  final String? category;
  final int? confidence;
  final String? model;
  final bool confirmed;
  final String? alertTemplate;
  final String alertRegex;

  static LogTemplateInfo? tryFromJson(Object? raw) {
    if (raw is! Map) return null;
    final json = asJsonMap(raw);
    final hash = json['templateHash'] as String?;
    if (hash == null || hash.isEmpty) return null;
    return LogTemplateInfo(
      templateHash: hash,
      template: (json['template'] ?? '').toString(),
      example: (json['example'] ?? '').toString(),
      category: json['category'] as String?,
      confidence: parseIntOrNull(json['confidence']),
      model: json['model'] as String?,
      confirmed: json['confirmed'] == true,
      alertTemplate: json['alertTemplate'] as String?,
      alertRegex: (json['alertRegex'] ?? '').toString(),
    );
  }
}

@immutable
class LogSourceEntry {
  const LogSourceEntry({
    required this.bindKey,
    required this.sourceIp,
    this.hostname,
    required this.kind,
    this.deviceId,
    this.deviceName,
    this.masked = false,
    this.messageCount = 0,
    this.droppedCount = 0,
    this.lastMessage,
    this.lastSeenAt,
    this.candidates = const <int>[],
  });

  final String bindKey;
  final String sourceIp;
  final String? hostname;
  final String kind;
  final int? deviceId;
  final String? deviceName;
  final bool masked;
  final int messageCount;
  final int droppedCount;
  final String? lastMessage;
  final String? lastSeenAt;
  final List<int> candidates;

  factory LogSourceEntry.fromJson(Map<String, dynamic> json) {
    final rawCandidates = json['candidates'];
    final parsedCandidates = rawCandidates is List
        ? rawCandidates
              .map(
                (e) => e is Map ? parseIntOrNull(e['id']) : parseIntOrNull(e),
              )
              .whereType<int>()
              .toList(growable: false)
        : const <int>[];
    return LogSourceEntry(
      bindKey: (json['bindKey'] ?? json['sourceIp'] ?? '').toString(),
      sourceIp: (json['sourceIp'] ?? '').toString(),
      hostname: json['hostname'] as String?,
      kind: (json['kind'] ?? 'unknown').toString(),
      deviceId: parseIntOrNull(json['deviceId']),
      deviceName: json['deviceName'] as String?,
      masked: json['masked'] == true,
      messageCount: parseIntOrNull(json['messageCount']) ?? 0,
      droppedCount: parseIntOrNull(json['droppedCount']) ?? 0,
      lastMessage: json['lastMessage'] as String?,
      lastSeenAt: json['lastSeenAt'] as String?,
      candidates: parsedCandidates,
    );
  }
}

@immutable
class LogNatDiagnostics {
  const LogNatDiagnostics({
    this.unresolvedMaskedCount = 0,
    this.maskedIps = const <String>[],
    this.explanation,
  });

  final int unresolvedMaskedCount;
  final List<String> maskedIps;
  final String? explanation;

  factory LogNatDiagnostics.fromJson(Map<String, dynamic> json) {
    return LogNatDiagnostics(
      unresolvedMaskedCount: parseIntOrNull(json['unresolvedMaskedCount']) ?? 0,
      maskedIps: asStringList(json['maskedIps']),
      explanation: json['explanation'] as String?,
    );
  }
}

@immutable
class SetupSnippet {
  const SetupSnippet({
    required this.id,
    required this.label,
    required this.commands,
    this.notes,
  });

  final String id;
  final String label;
  final String commands;
  final String? notes;

  factory SetupSnippet.fromJson(Map<String, dynamic> json) {
    final rawCmds = json['commands'] ?? json['snippet'] ?? json['code'] ?? '';
    final commands = rawCmds is List
        ? asStringList(rawCmds).join('\n')
        : rawCmds.toString();
    return SetupSnippet(
      id: (json['id'] ?? json['os'] ?? '').toString(),
      label: (json['label'] ?? json['title'] ?? json['name'] ?? '').toString(),
      commands: commands,
      notes: (json['note'] ?? json['notes'] ?? json['description']) as String?,
    );
  }
}

@immutable
class SetupGuide {
  const SetupGuide({
    required this.serverAddress,
    required this.port,
    required this.snippets,
  });

  final String serverAddress;
  final int port;
  final List<SetupSnippet> snippets;

  factory SetupGuide.fromJson(Map<String, dynamic> json) {
    return SetupGuide(
      serverAddress: (json['serverAddress'] ?? '').toString(),
      port: parseIntOrNull(json['port']) ?? 514,
      snippets: asJsonList<SetupSnippet>(
        json['snippets'],
        SetupSnippet.fromJson,
      ),
    );
  }
}

@immutable
class ProvisionHintsResponse {
  const ProvisionHintsResponse({
    this.serverAddress,
    this.serverPort = 514,
    this.accessMode,
    this.operatingSystem,
    this.sshOpen = false,
    this.telnetOpen = false,
    this.macAddress,
    this.layer2Reachable = false,
    this.savedCredentials = const <String>[],
  });

  final String? serverAddress;
  final int serverPort;
  final String? accessMode;
  final String? operatingSystem;
  final bool sshOpen;
  final bool telnetOpen;
  final String? macAddress;
  final bool layer2Reachable;
  final List<String> savedCredentials;

  factory ProvisionHintsResponse.fromJson(Map<String, dynamic> json) {
    final rawCreds = json['savedCredentials'] is List
        ? json['savedCredentials'] as List
        : const [];
    return ProvisionHintsResponse(
      serverAddress: json['serverAddress'] as String?,
      serverPort: parseIntOrNull(json['serverPort']) ?? 514,
      accessMode: json['accessMode'] as String?,
      operatingSystem: json['operatingSystem'] as String?,
      sshOpen: json['sshOpen'] == true,
      telnetOpen: json['telnetOpen'] == true,
      macAddress: json['macAddress'] as String?,
      layer2Reachable: json['layer2Reachable'] == true,
      savedCredentials: rawCreds
          .map(
            (e) => e is Map
                ? (e['transport'] ?? e['kind'] ?? '').toString()
                : e.toString(),
          )
          .where((e) => e.isNotEmpty)
          .toList(growable: false),
    );
  }
}

@immutable
class ProvisionLoggingResponse {
  const ProvisionLoggingResponse({
    this.identifiedHostname,
    this.commands = const <String>[],
    this.transcript = '',
    this.confirmed,
    this.addressSaved = false,
    this.persistenceWarning,
  });

  final String? identifiedHostname;
  final List<String> commands;
  final String transcript;
  final bool? confirmed;
  final bool addressSaved;
  final String? persistenceWarning;

  factory ProvisionLoggingResponse.fromJson(Map<String, dynamic> json) {
    return ProvisionLoggingResponse(
      identifiedHostname: json['identifiedHostname'] as String?,
      commands: asStringList(json['commands']),
      transcript: (json['transcript'] ?? '').toString(),
      confirmed: json['confirmed'] as bool?,
      addressSaved: json['addressSaved'] == true,
      persistenceWarning: json['persistenceWarning'] as String?,
    );
  }
}

@immutable
class LogFilters {
  const LogFilters({
    this.deviceId,
    this.severity,
    this.hours = 24,
    this.search = '',
    this.category,
  });

  final int? deviceId;
  final int? severity;
  final int? hours;
  final String search;
  final String? category;

  LogFilters copyWith({
    int? deviceId,
    bool clearDevice = false,
    int? severity,
    bool clearSeverity = false,
    int? hours,
    bool clearHours = false,
    String? search,
    String? category,
    bool clearCategory = false,
  }) {
    return LogFilters(
      deviceId: clearDevice ? null : (deviceId ?? this.deviceId),
      severity: clearSeverity ? null : (severity ?? this.severity),
      hours: clearHours ? null : (hours ?? this.hours),
      search: search ?? this.search,
      category: clearCategory ? null : (category ?? this.category),
    );
  }
}

const List<(int, String)> kSyslogSeverityOptions = <(int, String)>[
  (2, 'Crítico e acima'),
  (3, 'Erro e acima'),
  (4, 'Aviso e acima'),
  (6, 'Informação e acima'),
  (7, 'Tudo (incl. depuração)'),
];

const List<(int, String)> kSyslogWindowOptions = <(int, String)>[
  (1, 'Última hora'),
  (6, '6 horas'),
  (24, '24 horas'),
  (168, '7 dias'),
];

const List<(String, String)> kLogCategories = <(String, String)>[
  ('auth_failure', 'Falha de autenticação'),
  ('link_change', 'Mudança de enlace'),
  ('dhcp', 'DHCP'),
  ('wireless', 'Wi-Fi / Wireless'),
  ('firewall', 'Firewall'),
  ('routing', 'Roteamento'),
  ('system', 'Sistema'),
  ('hardware', 'Hardware'),
  ('vpn', 'VPN'),
  ('noise', 'Ruído / Informativo'),
];

String logCategoryLabel(String? category) {
  if (category == null || category.isEmpty) return 'Sem categoria';
  for (final (key, label) in kLogCategories) {
    if (key == category) return label;
  }
  return category;
}
