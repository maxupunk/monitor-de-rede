import 'package:flutter/material.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Modos de automação da IA (`AiAutomationMode`).
enum AiAutomationMode {
  off('off', 'Desligado'),
  confirm('confirm', 'Pedir confirmação'),
  auto('auto', 'Automático');

  const AiAutomationMode(this.id, this.label);

  final String id;
  final String label;

  static AiAutomationMode fromRaw(
    Object? raw, {
    AiAutomationMode fallback = AiAutomationMode.confirm,
  }) {
    final value = raw?.toString();
    for (final mode in AiAutomationMode.values) {
      if (mode.id == value) return mode;
    }
    return fallback;
  }
}

/// Estilo de resposta da IA (`AiResponseStyle`).
enum AiResponseStyle {
  concise('concise', 'Direto'),
  normal('normal', 'Normal');

  const AiResponseStyle(this.id, this.label);

  final String id;
  final String label;

  static AiResponseStyle fromRaw(Object? raw) => raw?.toString() == 'normal'
      ? AiResponseStyle.normal
      : AiResponseStyle.concise;
}

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
  source('source', 'Fonte', SemanticTone.plugin, Icons.storage_outlined);

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

/// Configurações proativas da IA (`AiProactiveSettings`).
@immutable
class AiProactiveSettings {
  const AiProactiveSettings({
    this.incidentSummaries = false,
    this.incidentMinSeverity = 'critical',
    this.maxSummariesPerHour = 6,
    this.digest = 'off',
    this.digestHour = 8,
    this.skipQuietDigest = true,
  });

  final bool incidentSummaries;
  final String incidentMinSeverity;
  final int maxSummariesPerHour;
  final String digest; // 'off' | 'daily' | 'weekly'
  final int digestHour;
  final bool skipQuietDigest;

  AiProactiveSettings copyWith({
    bool? incidentSummaries,
    String? incidentMinSeverity,
    int? maxSummariesPerHour,
    String? digest,
    int? digestHour,
    bool? skipQuietDigest,
  }) {
    return AiProactiveSettings(
      incidentSummaries: incidentSummaries ?? this.incidentSummaries,
      incidentMinSeverity: incidentMinSeverity ?? this.incidentMinSeverity,
      maxSummariesPerHour: maxSummariesPerHour ?? this.maxSummariesPerHour,
      digest: digest ?? this.digest,
      digestHour: digestHour ?? this.digestHour,
      skipQuietDigest: skipQuietDigest ?? this.skipQuietDigest,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
    'incidentSummaries': incidentSummaries,
    'incidentMinSeverity': incidentMinSeverity,
    'maxSummariesPerHour': maxSummariesPerHour,
    'digest': digest,
    'digestHour': digestHour,
    'skipQuietDigest': skipQuietDigest,
  };

  factory AiProactiveSettings.fromJson(Map<String, dynamic> json) {
    return AiProactiveSettings(
      incidentSummaries: json['incidentSummaries'] == true,
      incidentMinSeverity:
          (json['incidentMinSeverity'] as String?) ?? 'critical',
      maxSummariesPerHour: (json['maxSummariesPerHour'] as num?)?.toInt() ?? 6,
      digest: (json['digest'] as String?) ?? 'off',
      digestHour: (json['digestHour'] as num?)?.toInt() ?? 8,
      skipQuietDigest: json['skipQuietDigest'] != false,
    );
  }
}

/// Funcionalidades do Decisor (`DeciderFeatures`).
@immutable
class DeciderFeatures {
  const DeciderFeatures({
    this.chatTools = true,
    this.deviceIdentity = true,
    this.interfaces = true,
    this.logEvents = false,
    this.incidentTriage = 'off',
    this.actionReview = true,
  });

  final bool chatTools;
  final bool deviceIdentity;
  final bool interfaces;
  final bool logEvents;
  final String incidentTriage; // 'off' | 'shadow' | 'enforce'
  final bool actionReview;

  DeciderFeatures copyWith({
    bool? chatTools,
    bool? deviceIdentity,
    bool? interfaces,
    bool? logEvents,
    String? incidentTriage,
    bool? actionReview,
  }) {
    return DeciderFeatures(
      chatTools: chatTools ?? this.chatTools,
      deviceIdentity: deviceIdentity ?? this.deviceIdentity,
      interfaces: interfaces ?? this.interfaces,
      logEvents: logEvents ?? this.logEvents,
      incidentTriage: incidentTriage ?? this.incidentTriage,
      actionReview: actionReview ?? this.actionReview,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
    'chatTools': chatTools,
    'deviceIdentity': deviceIdentity,
    'interfaces': interfaces,
    'logEvents': logEvents,
    'incidentTriage': incidentTriage,
    'actionReview': actionReview,
  };

  factory DeciderFeatures.fromJson(Map<String, dynamic> json) {
    return DeciderFeatures(
      chatTools: json['chatTools'] != false,
      deviceIdentity: json['deviceIdentity'] != false,
      interfaces: json['interfaces'] != false,
      logEvents: json['logEvents'] == true,
      incidentTriage: (json['incidentTriage'] as String?) ?? 'off',
      actionReview: json['actionReview'] != false,
    );
  }
}

/// Configurações do Decisor rápido (`AiDeciderSettings`).
@immutable
class AiDeciderSettings {
  const AiDeciderSettings({
    this.enabled = false,
    this.provider = 'ollaya',
    this.customProtocol = 'systemone',
    this.baseUrl = 'http://ollaya:11435',
    this.apiKey,
    this.accountId,
    this.model = 'netmonitor/decima-network:v15',
    this.minConfidence = 60,
    this.timeoutMs = 4000,
    this.features = const DeciderFeatures(),
  });

  final bool enabled;
  final String provider;
  final String customProtocol;
  final String baseUrl;
  final String? apiKey;
  final String? accountId;
  final String model;
  final int minConfidence;
  final int timeoutMs;
  final DeciderFeatures features;

  AiDeciderSettings copyWith({
    bool? enabled,
    String? provider,
    String? customProtocol,
    String? baseUrl,
    String? apiKey,
    bool clearApiKey = false,
    String? accountId,
    bool clearAccountId = false,
    String? model,
    int? minConfidence,
    int? timeoutMs,
    DeciderFeatures? features,
  }) {
    return AiDeciderSettings(
      enabled: enabled ?? this.enabled,
      provider: provider ?? this.provider,
      customProtocol: customProtocol ?? this.customProtocol,
      baseUrl: baseUrl ?? this.baseUrl,
      apiKey: clearApiKey ? null : (apiKey ?? this.apiKey),
      accountId: clearAccountId ? null : (accountId ?? this.accountId),
      model: model ?? this.model,
      minConfidence: minConfidence ?? this.minConfidence,
      timeoutMs: timeoutMs ?? this.timeoutMs,
      features: features ?? this.features,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
    'enabled': enabled,
    'provider': provider,
    'customProtocol': customProtocol,
    'baseUrl': baseUrl,
    if (apiKey != null && apiKey!.isNotEmpty) 'apiKey': apiKey,
    if (accountId != null && accountId!.isNotEmpty) 'accountId': accountId,
    'model': model,
    'minConfidence': minConfidence,
    'timeoutMs': timeoutMs,
    'features': features.toJson(),
  };

  factory AiDeciderSettings.fromJson(Map<String, dynamic> json) {
    final rawFeatures = json['features'];
    return AiDeciderSettings(
      enabled: json['enabled'] == true,
      provider: (json['provider'] as String?) ?? 'ollaya',
      customProtocol: (json['customProtocol'] as String?) ?? 'systemone',
      baseUrl: (json['baseUrl'] as String?) ?? 'http://ollaya:11435',
      apiKey: json['apiKey'] as String?,
      accountId: json['accountId'] as String?,
      model: (json['model'] as String?) ?? 'netmonitor/decima-network:v15',
      minConfidence: (json['minConfidence'] as num?)?.toInt() ?? 60,
      timeoutMs: (json['timeoutMs'] as num?)?.toInt() ?? 4000,
      features: rawFeatures is Map
          ? DeciderFeatures.fromJson(Map<String, dynamic>.from(rawFeatures))
          : const DeciderFeatures(),
    );
  }
}

/// Configurações gerais do Assistente IA (`AiSettings`).
@immutable
class AiSettings {
  const AiSettings({
    this.enabled = false,
    this.activeDriver = 'ollama',
    this.opencodeBaseUrl = 'https://opencode.ai/zen/v1',
    this.opencodeApiKey,
    this.opencodeModel = 'muse-spark-1.3-contributor-free',
    this.openrouterApiKey,
    this.openrouterModel = 'openrouter/free',
    this.ollamaBaseUrl = 'http://ollama:11434/v1',
    this.ollamaModel = '',
    this.ollamaNumCtx = 16384,
    this.ollamaReasoning = false,
    this.ollamaNetmonitorTrained,
    this.allowActiveTools = true,
    this.requireToolConfirmation = true,
    this.actionMode = AiAutomationMode.off,
    this.containerActions = AiAutomationMode.confirm,
    this.responseStyle = AiResponseStyle.concise,
    this.proactive = const AiProactiveSettings(),
    this.decider = const AiDeciderSettings(),
    this.customSystemPrompt,
  });

  final bool enabled;
  final String activeDriver; // 'ollama' | 'openrouter' | 'opencode'
  final String? opencodeBaseUrl;
  final String? opencodeApiKey;
  final String? opencodeModel;
  final String? openrouterApiKey;
  final String? openrouterModel;
  final String? ollamaBaseUrl;
  final String? ollamaModel;
  final int? ollamaNumCtx;
  final bool ollamaReasoning;
  final bool? ollamaNetmonitorTrained;
  final bool allowActiveTools;
  final bool requireToolConfirmation;
  final AiAutomationMode actionMode;
  final AiAutomationMode containerActions;
  final AiResponseStyle responseStyle;
  final AiProactiveSettings proactive;
  final AiDeciderSettings decider;
  final String? customSystemPrompt;

  String get currentModelLabel {
    switch (activeDriver) {
      case 'openrouter':
        return openrouterModel ?? 'openrouter/free';
      case 'opencode':
        return opencodeModel ?? 'muse-spark-1.3-contributor-free';
      default:
        return ollamaModel?.isNotEmpty == true ? ollamaModel! : 'Ollama';
    }
  }

  AiAutomationMode get activeToolsMode {
    if (!allowActiveTools) return AiAutomationMode.off;
    return requireToolConfirmation
        ? AiAutomationMode.confirm
        : AiAutomationMode.auto;
  }

  AiSettings copyWith({
    bool? enabled,
    String? activeDriver,
    String? opencodeBaseUrl,
    String? opencodeApiKey,
    String? opencodeModel,
    String? openrouterApiKey,
    String? openrouterModel,
    String? ollamaBaseUrl,
    String? ollamaModel,
    int? ollamaNumCtx,
    bool? ollamaReasoning,
    bool? ollamaNetmonitorTrained,
    bool clearOllamaNetmonitorTrained = false,
    bool? allowActiveTools,
    bool? requireToolConfirmation,
    AiAutomationMode? actionMode,
    AiAutomationMode? containerActions,
    AiResponseStyle? responseStyle,
    AiProactiveSettings? proactive,
    AiDeciderSettings? decider,
    String? customSystemPrompt,
  }) {
    return AiSettings(
      enabled: enabled ?? this.enabled,
      activeDriver: activeDriver ?? this.activeDriver,
      opencodeBaseUrl: opencodeBaseUrl ?? this.opencodeBaseUrl,
      opencodeApiKey: opencodeApiKey ?? this.opencodeApiKey,
      opencodeModel: opencodeModel ?? this.opencodeModel,
      openrouterApiKey: openrouterApiKey ?? this.openrouterApiKey,
      openrouterModel: openrouterModel ?? this.openrouterModel,
      ollamaBaseUrl: ollamaBaseUrl ?? this.ollamaBaseUrl,
      ollamaModel: ollamaModel ?? this.ollamaModel,
      ollamaNumCtx: ollamaNumCtx ?? this.ollamaNumCtx,
      ollamaReasoning: ollamaReasoning ?? this.ollamaReasoning,
      ollamaNetmonitorTrained: clearOllamaNetmonitorTrained
          ? null
          : (ollamaNetmonitorTrained ?? this.ollamaNetmonitorTrained),
      allowActiveTools: allowActiveTools ?? this.allowActiveTools,
      requireToolConfirmation:
          requireToolConfirmation ?? this.requireToolConfirmation,
      actionMode: actionMode ?? this.actionMode,
      containerActions: containerActions ?? this.containerActions,
      responseStyle: responseStyle ?? this.responseStyle,
      proactive: proactive ?? this.proactive,
      decider: decider ?? this.decider,
      customSystemPrompt: customSystemPrompt ?? this.customSystemPrompt,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
    'enabled': enabled,
    'activeDriver': activeDriver,
    'opencodeBaseUrl': opencodeBaseUrl,
    'opencodeApiKey': opencodeApiKey,
    'opencodeModel': opencodeModel,
    'openrouterApiKey': openrouterApiKey,
    'openrouterModel': openrouterModel,
    'ollamaBaseUrl': ollamaBaseUrl,
    'ollamaModel': ollamaModel,
    'ollamaNumCtx': ollamaNumCtx,
    'ollamaReasoning': ollamaReasoning,
    'ollamaNetmonitorTrained': ollamaNetmonitorTrained,
    'allowActiveTools': allowActiveTools,
    'requireToolConfirmation': requireToolConfirmation,
    'actionMode': actionMode.id,
    'containerActions': containerActions.id,
    'responseStyle': responseStyle.id,
    'proactive': proactive.toJson(),
    'decider': decider.toJson(),
    'customSystemPrompt': customSystemPrompt,
  };

  factory AiSettings.fromJson(Map<String, dynamic> json) {
    final rawProactive = json['proactive'];
    final rawDecider = json['decider'];
    var openrouterModel = json['openrouterModel'] as String?;
    if (openrouterModel == 'meta-llama/llama-3.3-70b-instruct:free') {
      openrouterModel = 'openrouter/free';
    }
    return AiSettings(
      enabled: json['enabled'] == true,
      activeDriver: (json['activeDriver'] as String?) ?? 'ollama',
      opencodeBaseUrl:
          (json['opencodeBaseUrl'] as String?) ?? 'https://opencode.ai/zen/v1',
      opencodeApiKey: json['opencodeApiKey'] as String?,
      opencodeModel:
          (json['opencodeModel'] as String?) ??
          'muse-spark-1.3-contributor-free',
      openrouterApiKey: json['openrouterApiKey'] as String?,
      openrouterModel: openrouterModel ?? 'openrouter/free',
      ollamaBaseUrl:
          (json['ollamaBaseUrl'] as String?) ?? 'http://ollama:11434/v1',
      ollamaModel: (json['ollamaModel'] as String?) ?? '',
      ollamaNumCtx: (json['ollamaNumCtx'] as num?)?.toInt() ?? 16384,
      ollamaReasoning: json['ollamaReasoning'] == true,
      ollamaNetmonitorTrained: json['ollamaNetmonitorTrained'] as bool?,
      allowActiveTools: json['allowActiveTools'] != false,
      requireToolConfirmation: json['requireToolConfirmation'] != false,
      actionMode: AiAutomationMode.fromRaw(
        json['actionMode'],
        fallback: AiAutomationMode.off,
      ),
      containerActions: AiAutomationMode.fromRaw(
        json['containerActions'],
        fallback: AiAutomationMode.confirm,
      ),
      responseStyle: AiResponseStyle.fromRaw(json['responseStyle']),
      proactive: rawProactive is Map
          ? AiProactiveSettings.fromJson(
              Map<String, dynamic>.from(rawProactive),
            )
          : const AiProactiveSettings(),
      decider: rawDecider is Map
          ? AiDeciderSettings.fromJson(Map<String, dynamic>.from(rawDecider))
          : const AiDeciderSettings(),
      customSystemPrompt: json['customSystemPrompt'] as String?,
    );
  }
}

@immutable
class AiTestConnectionResponse {
  const AiTestConnectionResponse({
    required this.success,
    required this.latencyMs,
    required this.message,
    this.model,
  });

  final bool success;
  final int latencyMs;
  final String message;
  final String? model;

  factory AiTestConnectionResponse.fromJson(Map<String, dynamic> json) {
    return AiTestConnectionResponse(
      success: json['success'] == true,
      latencyMs: (json['latencyMs'] as num?)?.toInt() ?? 0,
      message: (json['message'] as String?) ?? '',
      model: json['model'] as String?,
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
          (json['value'] as num?)?.toDouble() ??
          (json['y'] as num?)?.toDouble() ??
          0,
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
    final rawPoints = json['points'];
    return AiChartSeries(
      id: (json['id'] ?? '').toString(),
      label: (json['label'] ?? json['id'] ?? '').toString(),
      points: rawPoints is List
          ? rawPoints
                .whereType<Map>()
                .map((p) => AiChartPoint.fromJson(Map<String, dynamic>.from(p)))
                .toList(growable: false)
          : const <AiChartPoint>[],
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
    final rawSeries = json['series'];
    return AiChart(
      title: (json['title'] as String?) ?? '',
      subtitle: json['subtitle'] as String?,
      unit: (json['unit'] as String?) ?? 'generic',
      xAxis: (json['xAxis'] as String?) ?? 'time',
      series: rawSeries is List
          ? rawSeries
                .whereType<Map>()
                .map(
                  (s) => AiChartSeries.fromJson(Map<String, dynamic>.from(s)),
                )
                .toList(growable: false)
          : const <AiChartSeries>[],
      avgValue: (json['avgValue'] as num?)?.toDouble(),
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
      arguments: json['arguments'] is Map
          ? Map<String, dynamic>.from(json['arguments'] as Map)
          : const <String, dynamic>{},
      result: json['result'] is Map
          ? Map<String, dynamic>.from(json['result'] as Map)
          : null,
      chart: json['chart'] is Map
          ? AiChart.fromJson(Map<String, dynamic>.from(json['chart'] as Map))
          : null,
      summary: json['summary'] as String?,
      status: status,
      autoApproved: json['autoApproved'] == true,
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
      if (v is num && v.isFinite && v > 0) return v.toInt();
      return null;
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
      tokensBefore: (json['tokensBefore'] as num?)?.toInt() ?? 0,
      tokensAfter: (json['tokensAfter'] as num?)?.toInt() ?? 0,
      summarized: json['summarized'] != false,
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
      settings: json['settings'] == true,
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
    final rawTools = json['toolCalls'];
    final rawMentions = json['mentions'];
    final rawNotices = json['notices'];
    return AiDisplayMessage(
      id: (json['id'] ?? DateTime.now().microsecondsSinceEpoch.toString())
          .toString(),
      role: (json['role'] as String?) ?? 'assistant',
      content: (json['content'] as String?) ?? '',
      compaction: json['compaction'] is Map
          ? AiCompaction.fromJson(
              Map<String, dynamic>.from(json['compaction'] as Map),
            )
          : null,
      toolCalls: rawTools is List
          ? rawTools
                .whereType<Map>()
                .map(
                  (t) => AiToolCallState.fromJson(Map<String, dynamic>.from(t)),
                )
                .toList(growable: false)
          : const <AiToolCallState>[],
      mentions: rawMentions is List
          ? rawMentions
                .whereType<Map>()
                .map((m) => AiMention.fromJson(Map<String, dynamic>.from(m)))
                .toList(growable: false)
          : const <AiMention>[],
      screen: json['screen'] is Map
          ? AskedScreen.fromJson(
              Map<String, dynamic>.from(json['screen'] as Map),
            )
          : null,
      rating: json['rating'] is Map
          ? AiRating.fromJson(Map<String, dynamic>.from(json['rating'] as Map))
          : null,
      usage: json['usage'] is Map
          ? AiUsageInfo.fromJson(
              Map<String, dynamic>.from(json['usage'] as Map),
            )
          : null,
      isStreaming: false,
      error: json['error'] as String?,
      notices: rawNotices is List
          ? rawNotices
                .whereType<Map>()
                .map((n) => AiNotice.fromJson(Map<String, dynamic>.from(n)))
                .toList(growable: false)
          : const <AiNotice>[],
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
      userId: (json['userId'] as num?)?.toInt() ?? 0,
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
      messageCount: (json['messageCount'] as num?)?.toInt() ?? 0,
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
    final rawMessages = json['messages'];
    return AiConversationDetail(
      id: (json['id'] ?? '').toString(),
      title: (json['title'] as String?) ?? 'Nova conversa',
      updatedAt: (json['updatedAt'] as String?) ?? '',
      messages: rawMessages is List
          ? rawMessages
                .whereType<Map>()
                .map(
                  (m) =>
                      AiDisplayMessage.fromJson(Map<String, dynamic>.from(m)),
                )
                .toList(growable: false)
          : const <AiDisplayMessage>[],
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
      conversations: (json['conversations'] as num?)?.toInt() ?? 0,
      messages: (json['messages'] as num?)?.toInt() ?? 0,
      users: (json['users'] as num?)?.toInt() ?? 0,
      deviceRuns: (json['deviceRuns'] as num?)?.toInt() ?? 0,
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

/// Triagem do Decisor para alertas (`DeciderTriage`).
@immutable
class DeciderTriage {
  const DeciderTriage({
    required this.classification,
    required this.confidence,
    this.suppressed = false,
    this.reason,
  });

  final String classification; // 'actionable' | 'symptom' | 'transient'
  final double confidence;
  final bool suppressed;
  final String? reason;

  factory DeciderTriage.fromJson(Map<String, dynamic> json) {
    return DeciderTriage(
      classification: (json['classification'] ?? json['label'] ?? 'actionable')
          .toString(),
      confidence: (json['confidence'] as num?)?.toDouble() ?? 0,
      suppressed: json['suppressed'] == true,
      reason: json['reason'] as String?,
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
    final rawHighlights = json['highlights'];
    return AiDigest(
      title: (json['title'] as String?) ?? 'Resumo da rede',
      summary: (json['summary'] ?? json['content'] ?? '').toString(),
      period: json['period'] as String?,
      highlights: rawHighlights is List
          ? rawHighlights.map((e) => e.toString()).toList(growable: false)
          : const <String>[],
      createdAt: json['createdAt'] as String?,
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
      active: json['active'] == true,
      expiresAt: json['expiresAt'] as String?,
      termsVersion: (json['termsVersion'] ?? '1').toString(),
      terms: (json['terms'] as String?) ?? '',
    );
  }
}

/// Progresso de download de modelo via SSE-over-POST (`ModelPullProgress`).
@immutable
class ModelPullProgress {
  const ModelPullProgress({
    required this.status,
    this.digest,
    this.total,
    this.completed,
    this.percentage,
    this.done = false,
    this.error,
  });

  final String status;
  final String? digest;
  final int? total;
  final int? completed;
  final double? percentage;
  final bool done;
  final String? error;

  factory ModelPullProgress.fromJson(Map<String, dynamic> json) {
    return ModelPullProgress(
      status: (json['status'] as String?) ?? '',
      digest: json['digest'] as String?,
      total: (json['total'] as num?)?.toInt(),
      completed: (json['completed'] as num?)?.toInt(),
      percentage: (json['percentage'] as num?)?.toDouble(),
      done: json['done'] == true,
      error: json['error'] as String?,
    );
  }
}

/// Modelo listado em provedores (Ollama / OpenRouter / OpenCode).
@immutable
class AiProviderModelItem {
  const AiProviderModelItem({
    required this.id,
    required this.name,
    this.description,
    this.contextLength,
    this.sizeBytes,
    this.isFree = false,
    this.supportsTools = true,
    this.installed = false,
    this.recommended = false,
    this.vendor,
  });

  final String id;
  final String name;
  final String? description;
  final int? contextLength;
  final int? sizeBytes;
  final bool isFree;
  final bool supportsTools;
  final bool installed;
  final bool recommended;
  final String? vendor;

  factory AiProviderModelItem.fromOllamaInstalled(Map<String, dynamic> json) {
    final name = (json['name'] ?? json['id'] ?? '').toString();
    return AiProviderModelItem(
      id: name,
      name: name,
      sizeBytes: (json['size'] as num?)?.toInt(),
      contextLength: (json['contextLength'] as num?)?.toInt(),
      isFree: true,
      supportsTools: json['supportsTools'] != false,
      installed: true,
    );
  }

  factory AiProviderModelItem.fromOllamaRecommended(Map<String, dynamic> json) {
    final name = (json['name'] ?? json['id'] ?? '').toString();
    return AiProviderModelItem(
      id: name,
      name: (json['label'] as String?) ?? name,
      description: json['description'] as String?,
      contextLength: (json['contextLength'] as num?)?.toInt(),
      isFree: true,
      supportsTools: true,
      recommended: true,
    );
  }

  factory AiProviderModelItem.fromRemote(Map<String, dynamic> json) {
    final id = (json['id'] ?? json['name'] ?? '').toString();
    final parts = id.split('/');
    return AiProviderModelItem(
      id: id,
      name: (json['name'] as String?) ?? id,
      description: json['description'] as String?,
      contextLength: (json['contextLength'] as num?)?.toInt(),
      isFree:
          json['isFree'] == true ||
          id.endsWith(':free') ||
          id.contains('-free'),
      supportsTools: json['supportsTools'] != false,
      vendor: parts.length > 1 ? parts.first : null,
    );
  }
}

@immutable
class AiProviderModelsCatalog {
  const AiProviderModelsCatalog({
    this.models = const <AiProviderModelItem>[],
    this.isOnline = false,
    this.errorMessage,
  });

  final List<AiProviderModelItem> models;
  final bool isOnline;
  final String? errorMessage;
}

/// Catálogo de provedores e modelos do Decisor (`DeciderProviderView`, `DeciderModelsResponse`).
@immutable
class DeciderProviderView {
  const DeciderProviderView({
    required this.id,
    required this.label,
    required this.description,
    this.protocol,
    required this.defaultBaseUrl,
    required this.defaultModel,
    this.models = const <String>[],
    required this.requiresKey,
    required this.requiresAccount,
    this.keyUrl,
    required this.cloud,
    required this.managesModels,
    required this.suggestedTimeoutMs,
  });

  final String id;
  final String label;
  final String description;
  final String? protocol;
  final String defaultBaseUrl;
  final String defaultModel;
  final List<String> models;
  final bool requiresKey;
  final bool requiresAccount;
  final String? keyUrl;
  final bool cloud;
  final bool managesModels;
  final int suggestedTimeoutMs;

  factory DeciderProviderView.fromJson(Map<String, dynamic> json) {
    final rawModels = json['models'];
    return DeciderProviderView(
      id: (json['id'] as String?) ?? '',
      label: (json['label'] as String?) ?? '',
      description: (json['description'] as String?) ?? '',
      protocol: json['protocol'] as String?,
      defaultBaseUrl: (json['defaultBaseUrl'] as String?) ?? '',
      defaultModel: (json['defaultModel'] as String?) ?? '',
      models: rawModels is List
          ? rawModels.map((e) => e.toString()).toList(growable: false)
          : const <String>[],
      requiresKey: json['requiresKey'] == true,
      requiresAccount: json['requiresAccount'] == true,
      keyUrl: json['keyUrl'] as String?,
      cloud: json['cloud'] == true,
      managesModels: json['managesModels'] == true,
      suggestedTimeoutMs: (json['suggestedTimeoutMs'] as num?)?.toInt() ?? 4000,
    );
  }
}

@immutable
class DeciderModelSpecView {
  const DeciderModelSpecView({
    required this.language,
    this.downloadMb,
    required this.suggestedTimeoutMs,
    this.recommended = false,
  });

  final String language; // 'multilingual' | 'english'
  final int? downloadMb;
  final int suggestedTimeoutMs;
  final bool recommended;

  factory DeciderModelSpecView.fromJson(Map<String, dynamic> json) {
    return DeciderModelSpecView(
      language: (json['language'] as String?) ?? 'multilingual',
      downloadMb: (json['downloadMb'] as num?)?.toInt(),
      suggestedTimeoutMs: (json['suggestedTimeoutMs'] as num?)?.toInt() ?? 4000,
      recommended: json['recommended'] == true,
    );
  }
}

@immutable
class DeciderDecisionsScoreView {
  const DeciderDecisionsScoreView({required this.overall, required this.p95Ms});

  final double overall;
  final double p95Ms;

  factory DeciderDecisionsScoreView.fromJson(Map<String, dynamic> json) {
    return DeciderDecisionsScoreView(
      overall: (json['overall'] as num?)?.toDouble() ?? 0,
      p95Ms: (json['p95Ms'] as num?)?.toDouble() ?? 0,
    );
  }
}

@immutable
class DeciderModelOption {
  const DeciderModelOption({
    required this.name,
    this.installed = false,
    this.recommended = false,
    this.spec,
    this.decisions,
  });

  final String name;
  final bool installed;
  final bool recommended;
  final DeciderModelSpecView? spec;
  final DeciderDecisionsScoreView? decisions;

  factory DeciderModelOption.fromJson(Map<String, dynamic> json) {
    return DeciderModelOption(
      name: (json['name'] as String?) ?? '',
      installed: json['installed'] == true,
      recommended: json['recommended'] == true,
      spec: json['spec'] is Map
          ? DeciderModelSpecView.fromJson(
              Map<String, dynamic>.from(json['spec'] as Map),
            )
          : null,
      decisions: json['decisions'] is Map
          ? DeciderDecisionsScoreView.fromJson(
              Map<String, dynamic>.from(json['decisions'] as Map),
            )
          : null,
    );
  }
}

@immutable
class DeciderModelsResponse {
  const DeciderModelsResponse({
    this.online = false,
    this.managesModels = false,
    this.errorMessage,
    this.installed = const <String>[],
    this.loaded = const <String>[],
    this.options = const <DeciderModelOption>[],
  });

  final bool online;
  final bool managesModels;
  final String? errorMessage;
  final List<String> installed;
  final List<String> loaded;
  final List<DeciderModelOption> options;

  DeciderModelsResponse copyWith({List<String>? loaded}) {
    return DeciderModelsResponse(
      online: online,
      managesModels: managesModels,
      errorMessage: errorMessage,
      installed: installed,
      loaded: loaded ?? this.loaded,
      options: options,
    );
  }

  factory DeciderModelsResponse.fromJson(Map<String, dynamic> json) {
    return DeciderModelsResponse(
      online: json['online'] == true,
      managesModels: json['managesModels'] == true,
      errorMessage: json['errorMessage'] as String?,
      installed: json['installed'] is List
          ? (json['installed'] as List)
                .map((e) => e.toString())
                .toList(growable: false)
          : const <String>[],
      loaded: json['loaded'] is List
          ? (json['loaded'] as List)
                .map((e) => e.toString())
                .toList(growable: false)
          : const <String>[],
      options: json['options'] is List
          ? (json['options'] as List)
                .whereType<Map>()
                .map(
                  (e) =>
                      DeciderModelOption.fromJson(Map<String, dynamic>.from(e)),
                )
                .toList(growable: false)
          : const <DeciderModelOption>[],
    );
  }
}

@immutable
class DeciderGroupScore {
  const DeciderGroupScore({
    required this.group,
    required this.confidence,
    this.selected = false,
  });

  final String group;
  final int confidence;
  final bool selected;

  factory DeciderGroupScore.fromJson(Map<String, dynamic> json) {
    final rawConf = (json['confidence'] as num?)?.toDouble() ?? 0;
    final conf = rawConf <= 1.0 && rawConf > 0
        ? (rawConf * 100).round()
        : rawConf.round();
    return DeciderGroupScore(
      group: (json['group'] ?? json['name'] ?? '').toString(),
      confidence: conf,
      selected: json['selected'] == true,
    );
  }
}

@immutable
class TestDeciderResponse {
  const TestDeciderResponse({
    required this.success,
    required this.online,
    required this.modelInstalled,
    required this.latencyMs,
    required this.message,
    this.model,
    this.groups = const <DeciderGroupScore>[],
  });

  final bool success;
  final bool online;
  final bool modelInstalled;
  final int latencyMs;
  final String message;
  final String? model;
  final List<DeciderGroupScore> groups;

  factory TestDeciderResponse.fromJson(Map<String, dynamic> json) {
    return TestDeciderResponse(
      success: json['success'] == true,
      online: json['online'] == true,
      modelInstalled: json['modelInstalled'] == true,
      latencyMs: (json['latencyMs'] as num?)?.toInt() ?? 0,
      message: (json['message'] as String?) ?? '',
      model: json['model'] as String?,
      groups: json['groups'] is List
          ? (json['groups'] as List)
                .whereType<Map>()
                .map(
                  (e) =>
                      DeciderGroupScore.fromJson(Map<String, dynamic>.from(e)),
                )
                .toList(growable: false)
          : const <DeciderGroupScore>[],
    );
  }
}

@immutable
class DeciderModelState {
  const DeciderModelState({
    required this.model,
    required this.baseUrl,
    required this.state,
    this.loadMs,
    this.message,
  });

  final String model;
  final String baseUrl;
  final String state; // 'loading' | 'ready' | 'failed'
  final int? loadMs;
  final String? message;

  static DeciderModelState? tryFromJson(Map<String, dynamic> json) {
    final model = json['model'] as String?;
    final baseUrl = json['baseUrl'] as String?;
    final st = json['state'] as String?;
    if (model == null || baseUrl == null) return null;
    if (st != 'loading' && st != 'ready' && st != 'failed') return null;
    return DeciderModelState(
      model: model,
      baseUrl: baseUrl,
      state: st!,
      loadMs: (json['loadMs'] as num?)?.toInt(),
      message: json['message'] as String?,
    );
  }
}

// --- Benchmarks (Chat + Decider) ---

@immutable
class ChatBenchmarkDomain {
  const ChatBenchmarkDomain({required this.id, required this.label});

  final String id;
  final String label;

  factory ChatBenchmarkDomain.fromJson(Map<String, dynamic> json) {
    return ChatBenchmarkDomain(
      id: (json['id'] as String?) ?? '',
      label: (json['label'] as String?) ?? '',
    );
  }
}

@immutable
class ChatBenchmarkDomainScore {
  const ChatBenchmarkDomainScore({required this.domain, required this.score});

  final String domain;
  final double score;

  factory ChatBenchmarkDomainScore.fromJson(Map<String, dynamic> json) {
    return ChatBenchmarkDomainScore(
      domain: (json['domain'] as String?) ?? '',
      score: (json['score'] as num?)?.toDouble() ?? 0,
    );
  }
}

@immutable
class ChatBenchmarkSplitView {
  const ChatBenchmarkSplitView({
    required this.overall,
    required this.ciLow,
    required this.ciHigh,
    required this.tools,
    required this.response,
    required this.safety,
    required this.turnP50Ms,
    required this.turnP95Ms,
    required this.tokensPerSec,
    required this.perfectRatio,
    required this.errors,
    required this.fit,
    this.domains = const <ChatBenchmarkDomainScore>[],
  });

  final double overall;
  final double ciLow;
  final double ciHigh;
  final double tools;
  final double response;
  final double safety;
  final double turnP50Ms;
  final double turnP95Ms;
  final double tokensPerSec;
  final double perfectRatio;
  final int errors;
  final bool fit;
  final List<ChatBenchmarkDomainScore> domains;

  factory ChatBenchmarkSplitView.fromJson(Map<String, dynamic> json) {
    return ChatBenchmarkSplitView(
      overall: (json['overall'] as num?)?.toDouble() ?? 0,
      ciLow: (json['ciLow'] as num?)?.toDouble() ?? 0,
      ciHigh: (json['ciHigh'] as num?)?.toDouble() ?? 0,
      tools: (json['tools'] as num?)?.toDouble() ?? 0,
      response: (json['response'] as num?)?.toDouble() ?? 0,
      safety: (json['safety'] as num?)?.toDouble() ?? 0,
      turnP50Ms: (json['turnP50Ms'] as num?)?.toDouble() ?? 0,
      turnP95Ms: (json['turnP95Ms'] as num?)?.toDouble() ?? 0,
      tokensPerSec: (json['tokensPerSec'] as num?)?.toDouble() ?? 0,
      perfectRatio: (json['perfectRatio'] as num?)?.toDouble() ?? 0,
      errors: (json['errors'] as num?)?.toInt() ?? 0,
      fit: json['fit'] == true,
      domains: json['domains'] is List
          ? (json['domains'] as List)
                .whereType<Map>()
                .map(
                  (e) => ChatBenchmarkDomainScore.fromJson(
                    Map<String, dynamic>.from(e),
                  ),
                )
                .toList(growable: false)
          : const <ChatBenchmarkDomainScore>[],
    );
  }
}

@immutable
class ChatBenchmarkRow {
  const ChatBenchmarkRow({
    required this.model,
    this.label,
    this.quantization,
    this.downloadMb,
    this.recommended = false,
    this.bundled = false,
    this.fit = false,
    required this.bench,
    this.blind,
  });

  final String model;
  final String? label;
  final String? quantization;
  final int? downloadMb;
  final bool recommended;
  final bool bundled;
  final bool fit;
  final ChatBenchmarkSplitView bench;
  final ChatBenchmarkSplitView? blind;

  factory ChatBenchmarkRow.fromJson(Map<String, dynamic> json) {
    return ChatBenchmarkRow(
      model: (json['model'] as String?) ?? '',
      label: json['label'] as String?,
      quantization: json['quantization'] as String?,
      downloadMb: (json['downloadMb'] as num?)?.toInt(),
      recommended: json['recommended'] == true,
      bundled: json['bundled'] == true,
      fit: json['fit'] == true,
      bench: json['bench'] is Map
          ? ChatBenchmarkSplitView.fromJson(
              Map<String, dynamic>.from(json['bench'] as Map),
            )
          : const ChatBenchmarkSplitView(
              overall: 0,
              ciLow: 0,
              ciHigh: 0,
              tools: 0,
              response: 0,
              safety: 0,
              turnP50Ms: 0,
              turnP95Ms: 0,
              tokensPerSec: 0,
              perfectRatio: 0,
              errors: 0,
              fit: false,
            ),
      blind: json['blind'] is Map
          ? ChatBenchmarkSplitView.fromJson(
              Map<String, dynamic>.from(json['blind'] as Map),
            )
          : null,
    );
  }
}

@immutable
class ChatBenchmarkReport {
  const ChatBenchmarkReport({
    required this.measuredOn,
    required this.hardware,
    required this.numCtx,
    required this.timeoutMs,
    required this.minScore,
    required this.minSafety,
    required this.minTools,
    required this.benchCases,
    required this.blindCases,
    this.domains = const <ChatBenchmarkDomain>[],
    this.models = const <ChatBenchmarkRow>[],
  });

  final String measuredOn;
  final String hardware;
  final int numCtx;
  final int timeoutMs;
  final double minScore;
  final double minSafety;
  final double minTools;
  final int benchCases;
  final int blindCases;
  final List<ChatBenchmarkDomain> domains;
  final List<ChatBenchmarkRow> models;

  factory ChatBenchmarkReport.fromJson(Map<String, dynamic> json) {
    return ChatBenchmarkReport(
      measuredOn: (json['measuredOn'] as String?) ?? '',
      hardware: (json['hardware'] as String?) ?? '',
      numCtx: (json['numCtx'] as num?)?.toInt() ?? 16384,
      timeoutMs: (json['timeoutMs'] as num?)?.toInt() ?? 30000,
      minScore: (json['minScore'] as num?)?.toDouble() ?? 0.85,
      minSafety: (json['minSafety'] as num?)?.toDouble() ?? 0.9,
      minTools: (json['minTools'] as num?)?.toDouble() ?? 0.85,
      benchCases: (json['benchCases'] as num?)?.toInt() ?? 0,
      blindCases: (json['blindCases'] as num?)?.toInt() ?? 0,
      domains: json['domains'] is List
          ? (json['domains'] as List)
                .whereType<Map>()
                .map(
                  (e) => ChatBenchmarkDomain.fromJson(
                    Map<String, dynamic>.from(e),
                  ),
                )
                .toList(growable: false)
          : const <ChatBenchmarkDomain>[],
      models: json['models'] is List
          ? (json['models'] as List)
                .whereType<Map>()
                .map(
                  (e) =>
                      ChatBenchmarkRow.fromJson(Map<String, dynamic>.from(e)),
                )
                .toList(growable: false)
          : const <ChatBenchmarkRow>[],
    );
  }
}

@immutable
class DeciderBenchmarkTask {
  const DeciderBenchmarkTask({required this.id, required this.label});

  final String id;
  final String label;

  factory DeciderBenchmarkTask.fromJson(Map<String, dynamic> json) {
    return DeciderBenchmarkTask(
      id: (json['id'] as String?) ?? '',
      label: (json['label'] as String?) ?? '',
    );
  }
}

@immutable
class DeciderBenchmarkTaskScore {
  const DeciderBenchmarkTaskScore({required this.task, required this.score});

  final String task;
  final double score;

  factory DeciderBenchmarkTaskScore.fromJson(Map<String, dynamic> json) {
    return DeciderBenchmarkTaskScore(
      task: (json['task'] as String?) ?? '',
      score: (json['score'] as num?)?.toDouble() ?? 0,
    );
  }
}

@immutable
class DeciderBenchmarkRow {
  const DeciderBenchmarkRow({
    required this.model,
    required this.overall,
    required this.ciLow,
    required this.ciHigh,
    required this.p50Ms,
    required this.p95Ms,
    required this.overTimeout,
    this.recommended = false,
    this.bundled = false,
    this.latencyFrom,
    this.tasks = const <DeciderBenchmarkTaskScore>[],
  });

  final String model;
  final double overall;
  final double ciLow;
  final double ciHigh;
  final double p50Ms;
  final double p95Ms;
  final double overTimeout;
  final bool recommended;
  final bool bundled;
  final String? latencyFrom;
  final List<DeciderBenchmarkTaskScore> tasks;

  factory DeciderBenchmarkRow.fromJson(Map<String, dynamic> json) {
    return DeciderBenchmarkRow(
      model: (json['model'] as String?) ?? '',
      overall: (json['overall'] as num?)?.toDouble() ?? 0,
      ciLow: (json['ciLow'] as num?)?.toDouble() ?? 0,
      ciHigh: (json['ciHigh'] as num?)?.toDouble() ?? 0,
      p50Ms: (json['p50Ms'] as num?)?.toDouble() ?? 0,
      p95Ms: (json['p95Ms'] as num?)?.toDouble() ?? 0,
      overTimeout: (json['overTimeout'] as num?)?.toDouble() ?? 0,
      recommended: json['recommended'] == true,
      bundled: json['bundled'] == true,
      latencyFrom: json['latencyFrom'] as String?,
      tasks: json['tasks'] is List
          ? (json['tasks'] as List)
                .whereType<Map>()
                .map(
                  (e) => DeciderBenchmarkTaskScore.fromJson(
                    Map<String, dynamic>.from(e),
                  ),
                )
                .toList(growable: false)
          : const <DeciderBenchmarkTaskScore>[],
    );
  }
}

@immutable
class DeciderBenchmarkReport {
  const DeciderBenchmarkReport({
    required this.measuredOn,
    required this.hardware,
    required this.cases,
    required this.timeoutMs,
    this.tasks = const <DeciderBenchmarkTask>[],
    this.models = const <DeciderBenchmarkRow>[],
  });

  final String measuredOn;
  final String hardware;
  final int cases;
  final int timeoutMs;
  final List<DeciderBenchmarkTask> tasks;
  final List<DeciderBenchmarkRow> models;

  factory DeciderBenchmarkReport.fromJson(Map<String, dynamic> json) {
    return DeciderBenchmarkReport(
      measuredOn: (json['measuredOn'] as String?) ?? '',
      hardware: (json['hardware'] as String?) ?? '',
      cases: (json['cases'] as num?)?.toInt() ?? 0,
      timeoutMs: (json['timeoutMs'] as num?)?.toInt() ?? 4000,
      tasks: json['tasks'] is List
          ? (json['tasks'] as List)
                .whereType<Map>()
                .map(
                  (e) => DeciderBenchmarkTask.fromJson(
                    Map<String, dynamic>.from(e),
                  ),
                )
                .toList(growable: false)
          : const <DeciderBenchmarkTask>[],
      models: json['models'] is List
          ? (json['models'] as List)
                .whereType<Map>()
                .map(
                  (e) => DeciderBenchmarkRow.fromJson(
                    Map<String, dynamic>.from(e),
                  ),
                )
                .toList(growable: false)
          : const <DeciderBenchmarkRow>[],
    );
  }
}
