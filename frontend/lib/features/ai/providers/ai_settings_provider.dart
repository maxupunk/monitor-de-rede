import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/features/ai/data/ai_repository.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';

@immutable
class AiSettingsState {
  const AiSettingsState({
    this.settings,
    this.ollamaCatalog,
    this.openRouterCatalog,
    this.openCodeCatalog,
    this.loading = false,
    this.saving = false,
    this.testing = false,
    this.testResult,
    this.pulling = false,
    this.pullProgress,
    this.pullingModel,
    this.error,
  });

  final AiSettings? settings;
  final AiProviderModelsCatalog? ollamaCatalog;
  final AiProviderModelsCatalog? openRouterCatalog;
  final AiProviderModelsCatalog? openCodeCatalog;
  final bool loading;
  final bool saving;
  final bool testing;
  final AiTestConnectionResponse? testResult;
  final bool pulling;
  final ModelPullProgress? pullProgress;
  final String? pullingModel;
  final String? error;

  AiSettingsState copyWith({
    AiSettings? settings,
    AiProviderModelsCatalog? ollamaCatalog,
    AiProviderModelsCatalog? openRouterCatalog,
    AiProviderModelsCatalog? openCodeCatalog,
    bool? loading,
    bool? saving,
    bool? testing,
    AiTestConnectionResponse? testResult,
    bool clearTestResult = false,
    bool? pulling,
    ModelPullProgress? pullProgress,
    bool clearPullProgress = false,
    String? pullingModel,
    String? error,
    bool clearError = false,
  }) {
    return AiSettingsState(
      settings: settings ?? this.settings,
      ollamaCatalog: ollamaCatalog ?? this.ollamaCatalog,
      openRouterCatalog: openRouterCatalog ?? this.openRouterCatalog,
      openCodeCatalog: openCodeCatalog ?? this.openCodeCatalog,
      loading: loading ?? this.loading,
      saving: saving ?? this.saving,
      testing: testing ?? this.testing,
      testResult: clearTestResult ? null : (testResult ?? this.testResult),
      pulling: pulling ?? this.pulling,
      pullProgress: clearPullProgress
          ? null
          : (pullProgress ?? this.pullProgress),
      pullingModel: pullingModel ?? this.pullingModel,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class AiSettingsNotifier extends Notifier<AiSettingsState> {
  VoidCallback? _cancelPull;

  @override
  AiSettingsState build() {
    ref.onDispose(() => _cancelPull?.call());
    return const AiSettingsState();
  }

  AiRepository get _repo => ref.read(aiRepositoryProvider);

  Future<void> load() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final settings = await _repo.fetchSettings();
      state = state.copyWith(settings: settings, loading: false);
    } catch (err) {
      state = state.copyWith(
        loading: false,
        error: describeError(
          err,
          fallback: 'Falha ao carregar configurações de IA.',
        ),
      );
    }
  }

  Future<bool> save(AiSettings settings) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      final saved = await _repo.saveSettings(settings);
      state = state.copyWith(settings: saved, saving: false);
      return true;
    } catch (err) {
      state = state.copyWith(
        saving: false,
        error: describeError(
          err,
          fallback: 'Falha ao salvar configurações de IA.',
        ),
      );
      return false;
    }
  }

  Future<AiTestConnectionResponse?> testConnection({
    required String driver,
    String? baseUrl,
    String? apiKey,
    required String model,
  }) async {
    state = state.copyWith(testing: true, clearError: true);
    try {
      final res = await _repo.testConnection(
        driver: driver,
        baseUrl: baseUrl,
        apiKey: apiKey,
        model: model,
      );
      state = state.copyWith(testing: false, testResult: res);
      return res;
    } catch (err) {
      final res = AiTestConnectionResponse(
        success: false,
        latencyMs: 0,
        message: describeError(err, fallback: 'Falha ao testar a conexão.'),
      );
      state = state.copyWith(testing: false, testResult: res);
      return res;
    }
  }

  Future<void> loadOllamaModels({String? baseUrl}) async {
    try {
      final catalog = await _repo.fetchOllamaModels(baseUrl: baseUrl);
      state = state.copyWith(ollamaCatalog: catalog);
    } catch (err) {
      state = state.copyWith(
        ollamaCatalog: AiProviderModelsCatalog(
          isOnline: false,
          errorMessage: describeError(err, fallback: 'Ollama indisponível.'),
        ),
      );
    }
  }

  Future<void> loadOpenRouterModels({String? apiKey}) async {
    try {
      final catalog = await _repo.fetchOpenRouterModels(apiKey: apiKey);
      state = state.copyWith(openRouterCatalog: catalog);
    } catch (err) {
      state = state.copyWith(
        openRouterCatalog: AiProviderModelsCatalog(
          isOnline: false,
          errorMessage: describeError(
            err,
            fallback: 'Falha ao listar modelos OpenRouter.',
          ),
        ),
      );
    }
  }

  Future<void> loadOpenCodeModels({String? apiKey, String? baseUrl}) async {
    try {
      final catalog = await _repo.fetchOpenCodeModels(
        apiKey: apiKey,
        baseUrl: baseUrl,
      );
      state = state.copyWith(openCodeCatalog: catalog);
    } catch (err) {
      state = state.copyWith(
        openCodeCatalog: AiProviderModelsCatalog(
          isOnline: false,
          errorMessage: describeError(
            err,
            fallback: 'Falha ao listar modelos OpenCode.',
          ),
        ),
      );
    }
  }

  Future<bool> pullOllamaModel(String model, {String? baseUrl}) async {
    if (state.pulling) return false;
    state = state.copyWith(
      pulling: true,
      pullingModel: model,
      pullProgress: const ModelPullProgress(
        status: 'Iniciando download...',
        percentage: 0,
      ),
    );
    _pullCancelled = false;
    try {
      await for (final progress in _repo.pullOllamaModel(
        model: model,
        baseUrl: baseUrl,
        onCancelAvailable: (cancel) => _cancelPull = cancel,
      )) {
        if (_pullCancelled) return false;
        state = state.copyWith(pullProgress: progress);
        if (progress.error != null && progress.error!.isNotEmpty) {
          state = state.copyWith(pulling: false);
          return false;
        }
      }
      if (_pullCancelled) return false;
      state = state.copyWith(pulling: false, clearPullProgress: true);
      await loadOllamaModels(baseUrl: baseUrl);
      return true;
    } catch (err) {
      if (_pullCancelled) return false;
      state = state.copyWith(
        pulling: false,
        pullProgress: ModelPullProgress(
          status: 'Erro no download',
          done: true,
          error: describeError(err, fallback: 'Falha ao baixar modelo.'),
        ),
      );
      return false;
    } finally {
      _cancelPull = null;
    }
  }

  bool _pullCancelled = false;

  void cancelPull() {
    _pullCancelled = true;
    _cancelPull?.call();
    _cancelPull = null;
    state = state.copyWith(
      pulling: false,
      pullProgress: const ModelPullProgress(
        status: 'Download cancelado',
        done: true,
        error: 'Cancelado pelo usuário.',
      ),
    );
  }

  Future<AiDataExportSummary> fetchExportSummary({int? days}) =>
      _repo.fetchExportSummary(days: days);

  Future<DownloadResult> downloadExportZip({int? days}) =>
      _repo.downloadExportZip(days: days);
}

final aiSettingsProvider =
    NotifierProvider<AiSettingsNotifier, AiSettingsState>(
      AiSettingsNotifier.new,
    );
