import 'dart:ui';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';

/// Repositório completo dos endpoints REST e SSE-over-POST do Assistente IA & Decisor.
class AiRepository {
  const AiRepository(this._api);

  final ApiClient _api;

  // --- Settings & Models ---

  Future<AiSettings> fetchSettings() async {
    final json = await _api.get<Map<String, dynamic>>('/ai/settings');
    return AiSettings.fromJson(json);
  }

  Future<AiSettings> saveSettings(AiSettings settings) async {
    final json = await _api.put<Map<String, dynamic>>(
      '/ai/settings',
      data: settings.toJson(),
    );
    return AiSettings.fromJson(json);
  }

  Future<AiTestConnectionResponse> testConnection({
    required String driver,
    String? baseUrl,
    String? apiKey,
    required String model,
  }) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/ai/test-connection',
      data: <String, dynamic>{
        'driver': driver,
        'baseUrl': ?baseUrl,
        'apiKey': ?apiKey,
        'model': model,
      },
      timeout: const Duration(seconds: 45),
    );
    return AiTestConnectionResponse.fromJson(json);
  }

  Future<AiProviderModelsCatalog> fetchOllamaModels({String? baseUrl}) async {
    final json = await _api.get<Map<String, dynamic>>(
      '/ai/ollama/models',
      queryParameters: baseUrl != null && baseUrl.trim().isNotEmpty
          ? <String, dynamic>{'base_url': baseUrl.trim()}
          : null,
    );
    final items = <AiProviderModelItem>[];
    final seen = <String>{};
    for (final raw in asJsonMapList(json['installed'])) {
      final item = AiProviderModelItem.fromOllamaInstalled(raw);
      seen.add(item.id);
      items.add(item);
    }
    for (final raw in asJsonMapList(json['recommended'])) {
      final item = AiProviderModelItem.fromOllamaRecommended(raw);
      if (seen.contains(item.id)) continue;
      items.add(item);
    }
    return AiProviderModelsCatalog(
      models: List<AiProviderModelItem>.unmodifiable(items),
      isOnline: parseBool(json['isOnline']),
      errorMessage: json['errorMessage'] as String?,
    );
  }

  Future<AiProviderModelsCatalog> fetchOpenRouterModels({
    String? apiKey,
  }) async {
    final params = <String, dynamic>{};
    if (apiKey != null && apiKey.isNotEmpty && apiKey != '********') {
      params['api_key'] = apiKey;
    }
    final json = await _api.get<Map<String, dynamic>>(
      '/ai/openrouter/models',
      queryParameters: params.isEmpty ? null : params,
    );
    return AiProviderModelsCatalog(
      models: asJsonList<AiProviderModelItem>(
        json['models'],
        AiProviderModelItem.fromRemote,
      ),
      isOnline: parseBool(json['isOnline']),
      errorMessage: json['errorMessage'] as String?,
    );
  }

  Future<AiProviderModelsCatalog> fetchOpenCodeModels({
    String? apiKey,
    String? baseUrl,
  }) async {
    final params = <String, dynamic>{};
    if (apiKey != null && apiKey.isNotEmpty && apiKey != '********') {
      params['api_key'] = apiKey;
    }
    if (baseUrl != null && baseUrl.trim().isNotEmpty) {
      params['base_url'] = baseUrl.trim();
    }
    final json = await _api.get<Map<String, dynamic>>(
      '/ai/opencode/models',
      queryParameters: params.isEmpty ? null : params,
    );
    return AiProviderModelsCatalog(
      models: asJsonList<AiProviderModelItem>(
        json['models'],
        AiProviderModelItem.fromRemote,
      ),
      isOnline: parseBool(json['isOnline']),
      errorMessage: json['errorMessage'] as String?,
    );
  }

  /// Streaming SSE-over-POST de download de modelo Ollama (`POST /api/ai/ollama/pull`).
  Stream<ModelPullProgress> pullOllamaModel({
    required String model,
    String? baseUrl,
    void Function(VoidCallback cancel)? onCancelAvailable,
  }) {
    return _api
        .streamSsePost(
          '/ai/ollama/pull',
          body: <String, dynamic>{
            'model': model.trim(),
            if (baseUrl != null && baseUrl.trim().isNotEmpty)
              'baseUrl': baseUrl.trim(),
          },
          onCancelAvailable: onCancelAvailable,
        )
        .map(ModelPullProgress.fromJson);
  }

  // --- Chat Streaming, Mentions, Tools & Approvals ---

  /// Streaming SSE-over-POST do chat (`POST /api/ai/chat/stream`).
  Stream<Map<String, dynamic>> streamChat({
    required Map<String, dynamic> body,
    void Function(VoidCallback cancel)? onCancelAvailable,
  }) {
    return _api.streamSsePost(
      '/ai/chat/stream',
      body: body,
      onCancelAvailable: onCancelAvailable,
    );
  }

  Future<List<AiMention>> fetchMentions(String query) async {
    final raw = await _api.get<List<dynamic>>(
      '/ai/mentions',
      queryParameters: <String, dynamic>{'q': query},
    );
    return asJsonList<AiMention>(raw, AiMention.fromJson);
  }

  Future<({Map<String, dynamic> result, AiChart? chart})> executeTool({
    required String name,
    required Map<String, dynamic> arguments,
    required String conversationKey,
    required String callId,
  }) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/ai/tools/execute',
      data: <String, dynamic>{
        'name': name,
        'arguments': arguments,
        'conversationKey': conversationKey,
        'callId': callId,
      },
      timeout: const Duration(minutes: 15),
    );
    return (
      result: asJsonMap(json['result']),
      chart: json['chart'] is Map
          ? AiChart.fromJson(asJsonMap(json['chart']))
          : null,
    );
  }

  Future<({Map<String, dynamic> result, AiChart? chart})> confirmApproval(
    String callId,
  ) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/ai/approvals/${Uri.encodeComponent(callId)}/confirm',
      data: const <String, dynamic>{},
      timeout: const Duration(minutes: 15),
    );
    return (
      result: asJsonMap(json['result']),
      chart: json['chart'] is Map
          ? AiChart.fromJson(asJsonMap(json['chart']))
          : null,
    );
  }

  Future<void> dismissApproval(String callId) async {
    await _api.post<dynamic>(
      '/ai/approvals/${Uri.encodeComponent(callId)}/dismiss',
      data: const <String, dynamic>{},
    );
  }

  // --- Conversations ---

  Future<List<AiConversationSummary>> fetchConversations() async {
    final raw = await _api.get<List<dynamic>>('/ai/conversations');
    return asJsonList<AiConversationSummary>(
      raw,
      AiConversationSummary.fromJson,
    );
  }

  Future<AiConversationSummary> createConversation({
    required String title,
    required List<AiDisplayMessage> messages,
  }) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/ai/conversations',
      data: <String, dynamic>{
        'title': title,
        'messages': messages.map((m) => m.toJson()).toList(growable: false),
      },
    );
    return AiConversationSummary.fromJson(json);
  }

  Future<AiConversationDetail> fetchConversation(String id) async {
    final json = await _api.get<Map<String, dynamic>>('/ai/conversations/$id');
    return AiConversationDetail.fromJson(json);
  }

  Future<AiConversationSummary> updateConversation({
    required String id,
    required String title,
    required List<AiDisplayMessage> messages,
  }) async {
    final json = await _api.put<Map<String, dynamic>>(
      '/ai/conversations/$id',
      data: <String, dynamic>{
        'title': title,
        'messages': messages.map((m) => m.toJson()).toList(growable: false),
      },
    );
    return AiConversationSummary.fromJson(json);
  }

  Future<void> deleteConversation(String id) async {
    await _api.delete<dynamic>('/ai/conversations/$id');
  }

  // --- Data Export ---

  Future<AiDataExportSummary> fetchExportSummary({int? days}) async {
    final json = await _api.get<Map<String, dynamic>>(
      '/ai/data-export/summary',
      queryParameters: days != null ? <String, dynamic>{'days': days} : null,
    );
    return AiDataExportSummary.fromJson(json);
  }

  Future<DownloadResult> downloadExportZip({int? days}) {
    return _api.downloadBytes(
      '/ai/data-export',
      queryParameters: days != null ? <String, dynamic>{'days': days} : null,
    );
  }

  // --- Decider ---

  Future<AiDeciderSettings> fetchDeciderSettings() async {
    final json = await _api.get<Map<String, dynamic>>('/ai/decider');
    return AiDeciderSettings.fromJson(json);
  }

  Future<AiDeciderSettings> saveDeciderSettings(
    AiDeciderSettings settings,
  ) async {
    final json = await _api.put<Map<String, dynamic>>(
      '/ai/decider',
      data: settings.toJson(),
    );
    return AiDeciderSettings.fromJson(json);
  }

  Future<List<DeciderProviderView>> fetchDeciderProviders() async {
    final raw = await _api.get<List<dynamic>>('/ai/decider/providers');
    return asJsonList<DeciderProviderView>(raw, DeciderProviderView.fromJson);
  }

  Future<DeciderModelsResponse> fetchDeciderModels(
    AiDeciderSettings decider,
  ) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/ai/decider/models',
      data: <String, dynamic>{'decider': decider.toJson()},
    );
    return DeciderModelsResponse.fromJson(json);
  }

  Future<void> warmUpDeciderModel(AiDeciderSettings decider) async {
    await _api.post<dynamic>(
      '/ai/decider/load',
      data: <String, dynamic>{'decider': decider.toJson()},
    );
  }

  Future<TestDeciderResponse> testDecider({
    required AiDeciderSettings decider,
    String? question,
  }) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/ai/decider/test',
      data: <String, dynamic>{
        'decider': decider.toJson(),
        'question': question?.trim().isNotEmpty == true
            ? question!.trim()
            : null,
      },
      timeout: const Duration(seconds: 30),
    );
    return TestDeciderResponse.fromJson(json);
  }

  Stream<ModelPullProgress> pullDeciderModel({
    required AiDeciderSettings decider,
    void Function(VoidCallback cancel)? onCancelAvailable,
  }) {
    return _api
        .streamSsePost(
          '/ai/decider/pull',
          body: <String, dynamic>{'decider': decider.toJson()},
          onCancelAvailable: onCancelAvailable,
        )
        .map(ModelPullProgress.fromJson);
  }

  // --- Benchmarks ---

  Future<ChatBenchmarkReport> fetchChatBenchmark() async {
    final json = await _api.get<Map<String, dynamic>>('/ai/chat/benchmark');
    return ChatBenchmarkReport.fromJson(json);
  }

  Future<DeciderBenchmarkReport> fetchDeciderBenchmark() async {
    final json = await _api.get<Map<String, dynamic>>('/ai/decider/benchmark');
    return DeciderBenchmarkReport.fromJson(json);
  }

  // --- Proactive Incident Summary ---

  Future<AiIncidentSummary?> generateAlertSummary(int alertId) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/ai/alerts/$alertId/summary',
      timeout: const Duration(seconds: 60),
    );
    final raw = json['summary'];
    if (raw is Map) {
      return AiIncidentSummary.fromJson(asJsonMap(raw));
    }
    return null;
  }

  // --- Auto-accept for DeviceAccess (ADR 012 — desacoplado de `features/plugins/`) ---

  Future<AutoAcceptState> fetchAutoAccept({
    required String conversationKey,
    required int deviceId,
  }) async {
    final json = await _api.get<Map<String, dynamic>>(
      '/plugin-auto-accept',
      queryParameters: <String, dynamic>{
        'conversationKey': conversationKey,
        'deviceId': deviceId,
      },
    );
    return AutoAcceptState.fromJson(json);
  }

  Future<AutoAcceptState> enableAutoAccept({
    required String conversationKey,
    required int deviceId,
  }) async {
    final json = await _api.post<Map<String, dynamic>>(
      '/plugin-auto-accept',
      data: <String, dynamic>{
        'conversationKey': conversationKey,
        'deviceId': deviceId,
        'acceptTerms': true,
      },
    );
    return AutoAcceptState.fromJson(json);
  }

  Future<AutoAcceptState> disableAutoAccept({
    required String conversationKey,
    required int deviceId,
  }) async {
    final json = await _api.delete<Map<String, dynamic>>(
      '/plugin-auto-accept',
      data: <String, dynamic>{
        'conversationKey': conversationKey,
        'deviceId': deviceId,
      },
    );
    return AutoAcceptState.fromJson(json);
  }

  /// Cancela imediatamente quaisquer execuções em andamento no equipamento ao desligar
  /// o modo "Aceitar automaticamente", sem importar `features/plugins/` (ADR 012 / ADR 017).
  Future<void> cancelRunningDeviceRuns(int deviceId) async {
    try {
      final raw = await _api.get<dynamic>('/devices/$deviceId/plugins');
      final runs = asJsonMapList(asJsonMap(raw)['runs']);
      for (final run in runs) {
        if (run['status'] != 'running') continue;
        final runId = parseIntOrNull(run['id']);
        if (runId == null || runId <= 0) continue;
        try {
          await _api.post<dynamic>(
            '/plugin-runs/$runId/cancel',
            data: const <String, dynamic>{},
          );
        } catch (_) {}
      }
    } catch (_) {}
  }
}

final aiRepositoryProvider = Provider<AiRepository>((ref) {
  return AiRepository(ref.watch(apiClientProvider));
});
