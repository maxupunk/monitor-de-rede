import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/ai/models/ai_settings_models.dart';

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
      chatTools: parseBool(json['chatTools'], fallback: true),
      deviceIdentity: parseBool(json['deviceIdentity'], fallback: true),
      interfaces: parseBool(json['interfaces'], fallback: true),
      logEvents: parseBool(json['logEvents']),
      incidentTriage: (json['incidentTriage'] as String?) ?? 'off',
      actionReview: parseBool(json['actionReview'], fallback: true),
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
    return AiDeciderSettings(
      enabled: parseBool(json['enabled']),
      provider: (json['provider'] as String?) ?? 'ollaya',
      customProtocol: (json['customProtocol'] as String?) ?? 'systemone',
      baseUrl: (json['baseUrl'] as String?) ?? 'http://ollaya:11435',
      apiKey: json['apiKey'] as String?,
      accountId: json['accountId'] as String?,
      model: (json['model'] as String?) ?? 'netmonitor/decima-network:v15',
      minConfidence: parseIntOrNull(json['minConfidence']) ?? 60,
      timeoutMs: parseIntOrNull(json['timeoutMs']) ?? 4000,
      features: json['features'] is Map
          ? DeciderFeatures.fromJson(asJsonMap(json['features']))
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
    return switch (activeDriver) {
      'openrouter' => openrouterModel ?? 'openrouter/free',
      'opencode' => opencodeModel ?? 'muse-spark-1.3-contributor-free',
      _ => ollamaModel?.isNotEmpty == true ? ollamaModel! : 'Ollama',
    };
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
    var openrouterModel = json['openrouterModel'] as String?;
    if (openrouterModel == 'meta-llama/llama-3.3-70b-instruct:free') {
      openrouterModel = 'openrouter/free';
    }
    return AiSettings(
      enabled: parseBool(json['enabled']),
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
      ollamaNumCtx: parseIntOrNull(json['ollamaNumCtx']) ?? 16384,
      ollamaReasoning: parseBool(json['ollamaReasoning']),
      ollamaNetmonitorTrained: json['ollamaNetmonitorTrained'] as bool?,
      allowActiveTools: parseBool(json['allowActiveTools'], fallback: true),
      requireToolConfirmation: parseBool(
        json['requireToolConfirmation'],
        fallback: true,
      ),
      actionMode: AiAutomationMode.fromRaw(
        json['actionMode'],
        fallback: AiAutomationMode.off,
      ),
      containerActions: AiAutomationMode.fromRaw(
        json['containerActions'],
        fallback: AiAutomationMode.confirm,
      ),
      responseStyle: AiResponseStyle.fromRaw(json['responseStyle']),
      proactive: json['proactive'] is Map
          ? AiProactiveSettings.fromJson(asJsonMap(json['proactive']))
          : const AiProactiveSettings(),
      decider: json['decider'] is Map
          ? AiDeciderSettings.fromJson(asJsonMap(json['decider']))
          : const AiDeciderSettings(),
      customSystemPrompt: json['customSystemPrompt'] as String?,
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
    final actionable = parseDoubleOrNull(json['actionable']);
    final symptom = parseDoubleOrNull(json['symptom']);
    final transient = parseDoubleOrNull(json['transient']);
    if (actionable != null || symptom != null || transient != null) {
      final a = actionable ?? 0.0;
      final s = symptom ?? 0.0;
      final t = transient ?? 0.0;
      String cls = 'actionable';
      double score = a;
      if (s >= score && s > a) {
        cls = 'symptom';
        score = s;
      }
      if (t > score) {
        cls = 'transient';
        score = t;
      }
      final normalized = score > 1.0 ? score / 100.0 : score;
      return DeciderTriage(
        classification: cls,
        confidence: normalized.clamp(0.0, 1.0),
        suppressed: parseBool(json['suppressed']),
        reason: json['reason'] as String?,
      );
    }
    final rawConf = parseDoubleOrNull(json['confidence']) ?? 0;
    return DeciderTriage(
      classification: (json['classification'] ?? json['label'] ?? 'actionable')
          .toString(),
      confidence: rawConf > 1.0 ? (rawConf / 100.0).clamp(0.0, 1.0) : rawConf,
      suppressed: parseBool(json['suppressed']),
      reason: json['reason'] as String?,
    );
  }
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
    final parsedModels = <String>[];
    if (rawModels is List) {
      for (final item in rawModels) {
        if (item is String && item.isNotEmpty) {
          parsedModels.add(item);
        } else if (item is Map) {
          final name = (item['name'] ?? item['id'])?.toString() ?? '';
          if (name.isNotEmpty) parsedModels.add(name);
        }
      }
    }
    return DeciderProviderView(
      id: (json['id'] as String?) ?? '',
      label: (json['label'] as String?) ?? '',
      description: (json['description'] as String?) ?? '',
      protocol: json['protocol'] as String?,
      defaultBaseUrl: (json['defaultBaseUrl'] as String?) ?? '',
      defaultModel: (json['defaultModel'] as String?) ?? '',
      models: List<String>.unmodifiable(parsedModels),
      requiresKey: parseBool(json['requiresKey']),
      requiresAccount: parseBool(json['requiresAccount']),
      keyUrl: json['keyUrl'] as String?,
      cloud: parseBool(json['cloud']),
      managesModels: parseBool(json['managesModels']),
      suggestedTimeoutMs: parseIntOrNull(json['suggestedTimeoutMs']) ?? 4000,
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
      downloadMb: parseIntOrNull(json['downloadMb']),
      suggestedTimeoutMs: parseIntOrNull(json['suggestedTimeoutMs']) ?? 4000,
      recommended: parseBool(json['recommended']),
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
      overall: parseDoubleOrNull(json['overall']) ?? 0,
      p95Ms: parseDoubleOrNull(json['p95Ms']) ?? 0,
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
    final spec = json['spec'] is Map
        ? DeciderModelSpecView.fromJson(asJsonMap(json['spec']))
        : null;
    return DeciderModelOption(
      name: (json['name'] as String?) ?? '',
      installed: parseBool(json['installed']),
      recommended:
          parseBool(json['recommended']) || (spec?.recommended ?? false),
      spec: spec,
      decisions: json['decisions'] is Map
          ? DeciderDecisionsScoreView.fromJson(asJsonMap(json['decisions']))
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
      online: parseBool(json['online']),
      managesModels: parseBool(json['managesModels']),
      errorMessage: json['errorMessage'] as String?,
      installed: asStringList(json['installed']),
      loaded: asStringList(json['loaded']),
      options: asJsonList<DeciderModelOption>(
        json['options'],
        DeciderModelOption.fromJson,
      ),
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
    final rawConf =
        parseDoubleOrNull(json['probability'] ?? json['confidence']) ?? 0;
    final conf = rawConf <= 1.0 && rawConf > 0
        ? (rawConf * 100).round()
        : rawConf.round();
    return DeciderGroupScore(
      group: (json['id'] ?? json['group'] ?? json['name'] ?? '').toString(),
      confidence: conf,
      selected: parseBool(json['selected']),
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
      success: parseBool(json['success']),
      online: parseBool(json['online']),
      modelInstalled: parseBool(json['modelInstalled']),
      latencyMs: parseIntOrNull(json['latencyMs']) ?? 0,
      message: (json['message'] as String?) ?? '',
      model: json['model'] as String?,
      groups: asJsonList<DeciderGroupScore>(
        json['groups'],
        DeciderGroupScore.fromJson,
      ),
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
      loadMs: parseIntOrNull(json['loadMs']),
      message: json['message'] as String?,
    );
  }
}
