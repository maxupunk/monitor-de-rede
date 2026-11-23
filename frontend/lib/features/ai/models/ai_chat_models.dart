import 'package:flutter/material.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Risco de uma ferramenta ou ação da IA (`AiToolRisk`).
enum AiToolRisk {
  low('low', 'Risco baixo', SemanticTone.success, Icons.verified_user_outlined),
  medium('medium', 'Risco médio', SemanticTone.warning, Icons.shield_outlined),
  high('high', 'Risco alto', SemanticTone.error, Icons.gpp_maybe_outlined);

  const AiToolRisk(this.id, this.label, this.tone, this.icon);

  final String id;
  final String label;
  final SemanticTone tone;
  final IconData icon;

  static AiToolRisk? tryParse(Object? raw) {
    final text = raw?.toString();
    for (final risk in AiToolRisk.values) {
      if (risk.id == text) return risk;
    }
    return null;
  }
}

/// Tipo de recurso marcado com `@` no chat (`AiMentionKind`).
/// Nota: `source` usa `SemanticTone.info` (`SemanticTone.plugin` é exclusivo de Plugins).
enum AiMentionKind {
  device('device', 'Dispositivo', SemanticTone.primary, Icons.router_outlined),
  monitor(
    'monitor',
    'Monitor',
    SemanticTone.secondary,
    Icons.monitor_heart_outlined,
  ),
  container(
    'container',
    'Container',
    SemanticTone.info,
    Icons.widgets_outlined,
  ),
  source('source', 'Fonte', SemanticTone.info, Icons.storage_outlined);

  const AiMentionKind(this.id, this.label, this.tone, this.icon);

  final String id;
  final String label;
  final SemanticTone tone;
  final IconData icon;

  static AiMentionKind fromRaw(Object? raw) {
    final text = raw?.toString();
    for (final kind in AiMentionKind.values) {
      if (kind.id == text) return kind;
    }
    return AiMentionKind.device;
  }
}

/// Recurso marcado com `@` (`AiMention`).
@immutable
class AiMention {
  const AiMention({
    required this.kind,
    required this.id,
    required this.label,
    this.detail,
  });

  final AiMentionKind kind;
  final String id;
  final String label;
  final String? detail;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'kind': kind.id,
    'id': id,
    'label': label,
    if (detail != null) 'detail': detail,
  };

  factory AiMention.fromJson(Map<String, dynamic> json) {
    return AiMention(
      kind: AiMentionKind.fromRaw(json['kind']),
      id: (json['id'] ?? '').toString(),
      label: (json['label'] ?? '').toString(),
      detail: json['detail'] as String?,
    );
  }
}

/// Ponto de gráfico embutido no chat (`AiChartPoint`).
@immutable
class AiChartPoint {
  const AiChartPoint({required this.time, required this.value, this.label});

  final String time;
  final double value;
  final String? label;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'time': time,
    'value': value,
    if (label != null) 'label': label,
  };

  factory AiChartPoint.fromJson(Map<String, dynamic> json) {
    return AiChartPoint(
      time: (json['time'] ?? json['x'] ?? '').toString(),
      value:
          parseDoubleOrNull(json['value']) ?? parseDoubleOrNull(json['y']) ?? 0,
      label: json['label'] as String?,
    );
  }
}

@immutable
class AiChartSeries {
  const AiChartSeries({
    required this.id,
    required this.label,
    required this.points,
  });

  final String id;
  final String label;
  final List<AiChartPoint> points;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'id': id,
    'label': label,
    'points': points.map((p) => p.toJson()).toList(growable: false),
  };

  factory AiChartSeries.fromJson(Map<String, dynamic> json) {
    return AiChartSeries(
      id: (json['id'] ?? '').toString(),
      label: (json['label'] ?? json['id'] ?? '').toString(),
      points: asJsonList<AiChartPoint>(json['points'], AiChartPoint.fromJson),
    );
  }
}

/// Gráfico produzido por ferramenta da IA (`AiChart`).
@immutable
class AiChart {
  const AiChart({
    required this.title,
    this.subtitle,
    required this.unit,
    this.xAxis = 'time',
    required this.series,
    this.avgValue,
  });

  final String title;
  final String? subtitle;
  final String
  unit; // 'latency' | 'bandwidth' | 'percentage' | 'bytes' | 'generic'
  final String xAxis;
  final List<AiChartSeries> series;
  final double? avgValue;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'title': title,
    'subtitle': subtitle,
    'unit': unit,
    'xAxis': xAxis,
    'series': series.map((s) => s.toJson()).toList(growable: false),
    'avgValue': avgValue,
  };

  factory AiChart.fromJson(Map<String, dynamic> json) {
    return AiChart(
      title: (json['title'] as String?) ?? '',
      subtitle: json['subtitle'] as String?,
      unit: (json['unit'] as String?) ?? 'generic',
      xAxis: (json['xAxis'] as String?) ?? 'time',
      series: asJsonList<AiChartSeries>(json['series'], AiChartSeries.fromJson),
      avgValue: parseDoubleOrNull(json['avgValue']),
    );
  }
}

enum AiToolStatus { running, done, error, awaiting, cancelled }

@immutable
class AiToolCallState {
  const AiToolCallState({
    required this.id,
    required this.name,
    this.arguments = const <String, dynamic>{},
    this.result,
    this.chart,
    this.summary,
    required this.status,
    this.autoApproved = false,
    this.risk,
    this.reason,
  });

  final String id;
  final String name;
  final Map<String, dynamic> arguments;
  final Map<String, dynamic>? result;
  final AiChart? chart;
  final String? summary;
  final AiToolStatus status;
  final bool autoApproved;
  final AiToolRisk? risk;
  final String? reason;

  AiToolCallState copyWith({
    Map<String, dynamic>? result,
    AiChart? chart,
    String? summary,
    AiToolStatus? status,
    bool? autoApproved,
    AiToolRisk? risk,
    String? reason,
  }) {
    return AiToolCallState(
      id: id,
      name: name,
      arguments: arguments,
      result: result ?? this.result,
      chart: chart ?? this.chart,
      summary: summary ?? this.summary,
      status: status ?? this.status,
      autoApproved: autoApproved ?? this.autoApproved,
      risk: risk ?? this.risk,
      reason: reason ?? this.reason,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
    'id': id,
    'name': name,
    'arguments': arguments,
    if (result != null) 'result': result,
    if (chart != null) 'chart': chart!.toJson(),
    if (summary != null) 'summary': summary,
    'status': status.name,
    if (autoApproved) 'autoApproved': true,
    if (risk != null) 'risk': risk!.id,
    if (reason != null) 'reason': reason,
  };

  factory AiToolCallState.fromJson(Map<String, dynamic> json) {
    final rawStatus = json['status']?.toString();
    final status = AiToolStatus.values.firstWhere(
      (s) => s.name == rawStatus,
      orElse: () => AiToolStatus.done,
    );
    return AiToolCallState(
      id: (json['id'] ?? '').toString(),
      name: (json['name'] ?? '').toString(),
      arguments: asJsonMap(json['arguments']),
      result: json['result'] is Map ? asJsonMap(json['result']) : null,
      chart: json['chart'] is Map
          ? AiChart.fromJson(asJsonMap(json['chart']))
          : null,
      summary: json['summary'] as String?,
      status: status,
      autoApproved: parseBool(json['autoApproved']),
      risk: AiToolRisk.tryParse(json['risk']),
      reason: json['reason'] as String?,
    );
  }
}

@immutable
class AiUsageInfo {
  const AiUsageInfo({
    this.promptTokens = 0,
    this.completionTokens = 0,
    this.model,
    this.contextTokens,
    this.contextWindow,
    this.contextWindowReported,
    this.generationMs,
    this.durationMs,
    this.cachedTokens,
    this.toolGroups,
  });

  final int promptTokens;
  final int completionTokens;
  final String? model;
  final int? contextTokens;
  final int? contextWindow;
  final bool? contextWindowReported;
  final int? generationMs;
  final int? durationMs;
  final int? cachedTokens;
  final List<String>? toolGroups;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'promptTokens': promptTokens,
    'completionTokens': completionTokens,
    if (model != null) 'model': model,
    if (contextTokens != null) 'contextTokens': contextTokens,
    if (contextWindow != null) 'contextWindow': contextWindow,
    if (contextWindowReported != null)
      'contextWindowReported': contextWindowReported,
    if (generationMs != null) 'generationMs': generationMs,
    if (durationMs != null) 'durationMs': durationMs,
    if (cachedTokens != null) 'cachedTokens': cachedTokens,
    if (toolGroups != null) 'toolGroups': toolGroups,
  };

  factory AiUsageInfo.fromJson(Map<String, dynamic> json) {
    int? positive(Object? v) {
      final n = parseIntOrNull(v);
      return (n != null && n > 0) ? n : null;
    }

    final rawGroups = json['toolGroups'];
    return AiUsageInfo(
      promptTokens: positive(json['promptTokens']) ?? 0,
      completionTokens: positive(json['completionTokens']) ?? 0,
      model: json['model'] is String && (json['model'] as String).isNotEmpty
          ? json['model'] as String
          : null,
      contextTokens: positive(json['contextTokens']),
      contextWindow: positive(json['contextWindow']),
      contextWindowReported: json['contextWindowReported'] as bool?,
      generationMs: positive(json['generationMs']),
      durationMs: positive(json['durationMs']),
      cachedTokens: positive(json['cachedTokens']),
      toolGroups: rawGroups is List
          ? rawGroups.whereType<String>().toList(growable: false)
          : null,
    );
  }
}

@immutable
class AiCompaction {
  const AiCompaction({
    required this.summary,
    required this.tokensBefore,
    required this.tokensAfter,
    this.summarized = true,
  });

  final String summary;
  final int tokensBefore;
  final int tokensAfter;
  final bool summarized;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'summary': summary,
    'tokensBefore': tokensBefore,
    'tokensAfter': tokensAfter,
    'summarized': summarized,
  };

  factory AiCompaction.fromJson(Map<String, dynamic> json) {
    return AiCompaction(
      summary: (json['summary'] as String?) ?? '',
      tokensBefore: parseIntOrNull(json['tokensBefore']) ?? 0,
      tokensAfter: parseIntOrNull(json['tokensAfter']) ?? 0,
      summarized: parseBool(json['summarized'], fallback: true),
    );
  }
}

@immutable
class AiNotice {
  const AiNotice({required this.message, this.settings = false, this.key});

  final String message;
  final bool settings;
  final String? key;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'message': message,
    'settings': settings,
    if (key != null) 'key': key,
  };

  factory AiNotice.fromJson(Map<String, dynamic> json) {
    return AiNotice(
      message: (json['message'] as String?) ?? '',
      settings: parseBool(json['settings']),
      key: json['key'] as String?,
    );
  }
}

@immutable
class AiRating {
  const AiRating({required this.value, this.reason});

  final String value; // 'up' | 'down'
  final String? reason;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'value': value,
    if (reason != null && reason!.isNotEmpty) 'reason': reason,
  };

  factory AiRating.fromJson(Map<String, dynamic> json) {
    return AiRating(
      value: (json['value'] as String?) ?? 'up',
      reason: json['reason'] as String?,
    );
  }
}

@immutable
class AskedScreen {
  const AskedScreen({
    required this.id,
    required this.label,
    required this.icon,
  });

  final String id;
  final String label;
  final String icon;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'id': id,
    'label': label,
    'icon': icon,
  };

  factory AskedScreen.fromJson(Map<String, dynamic> json) {
    return AskedScreen(
      id: (json['id'] as String?) ?? '',
      label: (json['label'] as String?) ?? '',
      icon: (json['icon'] as String?) ?? 'mdi-view-dashboard-outline',
    );
  }
}

@immutable
class AiDisplayMessage {
  const AiDisplayMessage({
    required this.id,
    required this.role,
    required this.content,
    this.compaction,
    this.toolCalls = const <AiToolCallState>[],
    this.mentions = const <AiMention>[],
    this.screen,
    this.rating,
    this.usage,
    this.isStreaming = false,
    this.error,
    this.notices = const <AiNotice>[],
    this.roundStart,
  });

  final String id;
  final String role; // 'user' | 'assistant'
  final String content;
  final AiCompaction? compaction;
  final List<AiToolCallState> toolCalls;
  final List<AiMention> mentions;
  final AskedScreen? screen;
  final AiRating? rating;
  final AiUsageInfo? usage;
  final bool isStreaming;
  final String? error;
  final List<AiNotice> notices;
  final int? roundStart;

  AiDisplayMessage copyWith({
    String? content,
    AiCompaction? compaction,
    List<AiToolCallState>? toolCalls,
    List<AiMention>? mentions,
    AskedScreen? screen,
    AiRating? rating,
    bool clearRating = false,
    AiUsageInfo? usage,
    bool? isStreaming,
    String? error,
    bool clearError = false,
    List<AiNotice>? notices,
    int? roundStart,
  }) {
    return AiDisplayMessage(
      id: id,
      role: role,
      content: content ?? this.content,
      compaction: compaction ?? this.compaction,
      toolCalls: toolCalls ?? this.toolCalls,
      mentions: mentions ?? this.mentions,
      screen: screen ?? this.screen,
      rating: clearRating ? null : (rating ?? this.rating),
      usage: usage ?? this.usage,
      isStreaming: isStreaming ?? this.isStreaming,
      error: clearError ? null : (error ?? this.error),
      notices: notices ?? this.notices,
      roundStart: roundStart ?? this.roundStart,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
    'id': id,
    'role': role,
    'content': content,
    if (compaction != null) 'compaction': compaction!.toJson(),
    if (toolCalls.isNotEmpty)
      'toolCalls': toolCalls.map((t) => t.toJson()).toList(growable: false),
    if (mentions.isNotEmpty)
      'mentions': mentions.map((m) => m.toJson()).toList(growable: false),
    if (screen != null) 'screen': screen!.toJson(),
    if (rating != null) 'rating': rating!.toJson(),
    if (usage != null) 'usage': usage!.toJson(),
    if (error != null) 'error': error,
    if (notices.isNotEmpty)
      'notices': notices.map((n) => n.toJson()).toList(growable: false),
  };

  factory AiDisplayMessage.fromJson(Map<String, dynamic> json) {
    return AiDisplayMessage(
      id: (json['id'] ?? DateTime.now().microsecondsSinceEpoch.toString())
          .toString(),
      role: (json['role'] as String?) ?? 'assistant',
      content: (json['content'] as String?) ?? '',
      compaction: json['compaction'] is Map
          ? AiCompaction.fromJson(asJsonMap(json['compaction']))
          : null,
      toolCalls: asJsonList<AiToolCallState>(
        json['toolCalls'],
        AiToolCallState.fromJson,
      ),
      mentions: asJsonList<AiMention>(json['mentions'], AiMention.fromJson),
      screen: json['screen'] is Map
          ? AskedScreen.fromJson(asJsonMap(json['screen']))
          : null,
      rating: json['rating'] is Map
          ? AiRating.fromJson(asJsonMap(json['rating']))
          : null,
      usage: json['usage'] is Map
          ? AiUsageInfo.fromJson(asJsonMap(json['usage']))
          : null,
      isStreaming: false,
      error: json['error'] as String?,
      notices: asJsonList<AiNotice>(json['notices'], AiNotice.fromJson),
    );
  }
}

/// Pedido de aprovação pendente vindo do SSE (`ai:approval_pending`).
@immutable
class AiPendingApproval {
  const AiPendingApproval({
    required this.callId,
    required this.userId,
    required this.tool,
    required this.summary,
    this.risk,
    this.reason,
    this.createdAt,
  });

  final String callId;
  final int userId;
  final String tool;
  final String summary;
  final AiToolRisk? risk;
  final String? reason;
  final String? createdAt;

  factory AiPendingApproval.fromJson(Map<String, dynamic> json) {
    return AiPendingApproval(
      callId: (json['callId'] ?? '').toString(),
      userId: parseIntOrNull(json['userId']) ?? 0,
      tool: (json['tool'] as String?) ?? '',
      summary: (json['summary'] as String?) ?? '',
      risk: AiToolRisk.tryParse(json['risk']),
      reason: json['reason'] as String?,
      createdAt: json['createdAt'] as String?,
    );
  }
}

/// Conversa persistida (`AiConversationSummary` / `AiConversationDetail`).
@immutable
class AiConversationSummary {
  const AiConversationSummary({
    required this.id,
    required this.title,
    required this.updatedAt,
    required this.messageCount,
  });

  final String id;
  final String title;
  final String updatedAt;
  final int messageCount;

  factory AiConversationSummary.fromJson(Map<String, dynamic> json) {
    return AiConversationSummary(
      id: (json['id'] ?? '').toString(),
      title: (json['title'] as String?) ?? 'Nova conversa',
      updatedAt: (json['updatedAt'] as String?) ?? '',
      messageCount: parseIntOrNull(json['messageCount']) ?? 0,
    );
  }
}

@immutable
class AiConversationDetail {
  const AiConversationDetail({
    required this.id,
    required this.title,
    required this.updatedAt,
    required this.messages,
  });

  final String id;
  final String title;
  final String updatedAt;
  final List<AiDisplayMessage> messages;

  factory AiConversationDetail.fromJson(Map<String, dynamic> json) {
    return AiConversationDetail(
      id: (json['id'] ?? '').toString(),
      title: (json['title'] as String?) ?? 'Nova conversa',
      updatedAt: (json['updatedAt'] as String?) ?? '',
      messages: asJsonList<AiDisplayMessage>(
        json['messages'],
        AiDisplayMessage.fromJson,
      ),
    );
  }
}

@immutable
class AiDataExportSummary {
  const AiDataExportSummary({
    required this.conversations,
    required this.messages,
    required this.users,
    required this.deviceRuns,
    this.oldest,
    this.newest,
  });

  final int conversations;
  final int messages;
  final int users;
  final int deviceRuns;
  final String? oldest;
  final String? newest;

  factory AiDataExportSummary.fromJson(Map<String, dynamic> json) {
    return AiDataExportSummary(
      conversations: parseIntOrNull(json['conversations']) ?? 0,
      messages: parseIntOrNull(json['messages']) ?? 0,
      users: parseIntOrNull(json['users']) ?? 0,
      deviceRuns: parseIntOrNull(json['deviceRuns']) ?? 0,
      oldest: json['oldest'] as String?,
      newest: json['newest'] as String?,
    );
  }
}

/// Resumo de incidente proativo (`AiIncidentSummary`).
@immutable
class AiIncidentSummary {
  const AiIncidentSummary({
    required this.text,
    this.cause,
    this.recommendation,
    this.model,
    this.generatedAt,
  });

  final String text;
  final String? cause;
  final String? recommendation;
  final String? model;
  final String? generatedAt;

  factory AiIncidentSummary.fromJson(Map<String, dynamic> json) {
    return AiIncidentSummary(
      text: (json['text'] ?? json['summary'] ?? '').toString(),
      cause: json['cause'] as String?,
      recommendation: json['recommendation'] as String?,
      model: json['model'] as String?,
      generatedAt: (json['generatedAt'] ?? json['createdAt']) as String?,
    );
  }
}

/// Digest periódico da IA (`AiDigest`).
@immutable
class AiDigest {
  const AiDigest({
    required this.title,
    required this.summary,
    this.period,
    this.highlights = const <String>[],
    this.createdAt,
  });

  final String title;
  final String summary;
  final String? period;
  final List<String> highlights;
  final String? createdAt;

  factory AiDigest.fromJson(Map<String, dynamic> json) {
    final periodHours = parseIntOrNull(json['periodHours']);
    return AiDigest(
      title: (json['title'] as String?) ?? 'Resumo da rede',
      summary: (json['text'] ?? json['summary'] ?? json['content'] ?? '')
          .toString(),
      period:
          (json['period'] as String?) ??
          (periodHours != null ? '${periodHours}h' : null),
      highlights: asStringList(json['highlights']),
      createdAt: (json['generatedAt'] ?? json['createdAt']) as String?,
    );
  }
}

/// Estado do modo "Aceitar automaticamente" para acesso da IA a um equipamento (`AutoAcceptState`).
@immutable
class AutoAcceptState {
  const AutoAcceptState({
    required this.active,
    this.expiresAt,
    this.termsVersion = '1',
    this.terms = '',
  });

  final bool active;
  final String? expiresAt;
  final String termsVersion;
  final String terms;

  factory AutoAcceptState.fromJson(Map<String, dynamic> json) {
    return AutoAcceptState(
      active: parseBool(json['active']),
      expiresAt: json['expiresAt'] as String?,
      termsVersion: (json['termsVersion'] ?? '1').toString(),
      terms: (json['terms'] as String?) ?? '',
    );
  }
}
