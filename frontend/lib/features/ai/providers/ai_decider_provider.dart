import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/ai/data/ai_repository.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/utils/ai_chat_logic.dart';

// ============================================================================
// AI Decider (`stores/aiDecider.ts`)
// ============================================================================

@immutable
class AiDeciderState {
  const AiDeciderState({
    this.saved,
    this.providers = const <DeciderProviderView>[],
    this.models,
    this.testResult,
    this.modelState,
    this.loading = false,
    this.saving = false,
    this.loadingModels = false,
    this.testing = false,
    this.pulling = false,
    this.pullProgress,
    this.error,
  });

  final AiDeciderSettings? saved;
  final List<DeciderProviderView> providers;
  final DeciderModelsResponse? models;
  final TestDeciderResponse? testResult;
  final DeciderModelState? modelState;
  final bool loading;
  final bool saving;
  final bool loadingModels;
  final bool testing;
  final bool pulling;
  final ModelPullProgress? pullProgress;
  final String? error;

  DeciderProviderView? providerById(String id) {
    for (final item in providers) {
      if (item.id == id) return item;
    }
    return null;
  }

  DeciderModelOption? optionFor(String model) {
    final target = model.trim().toLowerCase();
    for (final opt in models?.options ?? const <DeciderModelOption>[]) {
      if (opt.name.toLowerCase() == target) return opt;
    }
    return null;
  }

  bool isInstalled(String model) =>
      optionFor(model)?.installed ??
      isModelInstalled(models?.installed ?? const <String>[], model);

  bool isInMemory(String model) =>
      isModelInstalled(models?.loaded ?? const <String>[], model);

  AiDeciderState copyWith({
    AiDeciderSettings? saved,
    List<DeciderProviderView>? providers,
    DeciderModelsResponse? models,
    TestDeciderResponse? testResult,
    DeciderModelState? modelState,
    bool? loading,
    bool? saving,
    bool? loadingModels,
    bool? testing,
    bool? pulling,
    ModelPullProgress? pullProgress,
    bool clearPullProgress = false,
    String? error,
    bool clearError = false,
  }) {
    return AiDeciderState(
      saved: saved ?? this.saved,
      providers: providers ?? this.providers,
      models: models ?? this.models,
      testResult: testResult ?? this.testResult,
      modelState: modelState ?? this.modelState,
      loading: loading ?? this.loading,
      saving: saving ?? this.saving,
      loadingModels: loadingModels ?? this.loadingModels,
      testing: testing ?? this.testing,
      pulling: pulling ?? this.pulling,
      pullProgress: clearPullProgress
          ? null
          : (pullProgress ?? this.pullProgress),
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class AiDeciderNotifier extends Notifier<AiDeciderState> {
  VoidCallback? _cancelPull;

  @override
  AiDeciderState build() {
    ref.onDispose(() => _cancelPull?.call());
    return const AiDeciderState();
  }

  AiRepository get _repo => ref.read(aiRepositoryProvider);

  Future<void> loadAll() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final results = await Future.wait<dynamic>(<Future<dynamic>>[
        _repo.fetchDeciderSettings(),
        _repo.fetchDeciderProviders(),
      ]);
      final settings = results[0] as AiDeciderSettings;
      final providers = results[1] as List<DeciderProviderView>;
      state = state.copyWith(
        saved: settings,
        providers: providers,
        loading: false,
      );
      await loadModels(settings);
    } catch (err) {
      state = state.copyWith(
        loading: false,
        error: describeError(
          err,
          fallback: 'Falha ao carregar configurações do Decisor.',
        ),
      );
    }
  }

  Future<String?> save(AiDeciderSettings decider) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      final saved = await _repo.saveDeciderSettings(decider);
      state = state.copyWith(saved: saved, saving: false);
      return null;
    } catch (err) {
      final msg = describeError(err, fallback: 'Falha ao salvar o Decisor.');
      state = state.copyWith(saving: false, error: msg);
      return msg;
    }
  }

  Future<void> loadModels(AiDeciderSettings decider) async {
    state = state.copyWith(loadingModels: true);
    try {
      final response = await _repo.fetchDeciderModels(decider);
      state = state.copyWith(models: response, loadingModels: false);
    } catch (err) {
      state = state.copyWith(
        loadingModels: false,
        models: DeciderModelsResponse(
          online: false,
          managesModels: decider.provider == 'ollaya',
          errorMessage: describeError(
            err,
            fallback: 'Falha ao consultar provedor.',
          ),
        ),
      );
    }
  }

  void applyModelState(Map<String, dynamic> data) {
    final parsed = DeciderModelState.tryFromJson(data);
    if (parsed == null) return;
    var nextModels = state.models;
    if (parsed.state == 'ready' &&
        nextModels != null &&
        !state.isInMemory(parsed.model)) {
      nextModels = nextModels.copyWith(
        loaded: <String>[...nextModels.loaded, parsed.model],
      );
    }
    state = state.copyWith(modelState: parsed, models: nextModels);
  }

  Future<void> warmUp(AiDeciderSettings decider) async {
    state = state.copyWith(
      modelState: DeciderModelState(
        model: decider.model,
        baseUrl: decider.baseUrl,
        state: 'loading',
      ),
    );
    try {
      await _repo.warmUpDeciderModel(decider);
    } catch (err) {
      state = state.copyWith(
        modelState: DeciderModelState(
          model: decider.model,
          baseUrl: decider.baseUrl,
          state: 'failed',
          message: describeError(err, fallback: 'Falha ao carregar modelo.'),
        ),
      );
    }
  }

  Future<TestDeciderResponse> test(
    AiDeciderSettings decider, {
    String? question,
  }) async {
    state = state.copyWith(testing: true);
    try {
      final result = await _repo.testDecider(
        decider: decider,
        question: question,
      );
      state = state.copyWith(testResult: result, testing: false);
      return result;
    } catch (err) {
      final result = TestDeciderResponse(
        success: false,
        online: false,
        modelInstalled: false,
        latencyMs: 0,
        message: describeError(err, fallback: 'Falha ao testar o Decisor.'),
      );
      state = state.copyWith(testResult: result, testing: false);
      return result;
    }
  }

  Future<bool> pull(AiDeciderSettings decider, {String? question}) async {
    if (state.pulling) return false;
    state = state.copyWith(
      pulling: true,
      pullProgress: const ModelPullProgress(
        status: 'Iniciando download...',
        percentage: 0,
      ),
    );
    try {
      await for (final progress in _repo.pullDeciderModel(
        decider: decider,
        onCancelAvailable: (cancel) => _cancelPull = cancel,
      )) {
        state = state.copyWith(pullProgress: progress);
        if (progress.error != null && progress.error!.isNotEmpty) {
          state = state.copyWith(pulling: false);
          return false;
        }
      }
      state = state.copyWith(pulling: false, clearPullProgress: true);
      await loadModels(decider);
      await test(decider, question: question);
      return true;
    } catch (err) {
      state = state.copyWith(
        pulling: false,
        pullProgress: ModelPullProgress(
          status: 'Erro no download',
          done: true,
          error: describeError(
            err,
            fallback: 'Falha durante o download do modelo.',
          ),
        ),
      );
      return false;
    } finally {
      _cancelPull = null;
    }
  }

  void cancelPull() {
    _cancelPull?.call();
    _cancelPull = null;
    state = state.copyWith(
      pulling: false,
      pullProgress: const ModelPullProgress(
        status: 'Download cancelado',
        done: true,
        error: 'Cancelado.',
      ),
    );
  }
}

final aiDeciderProvider = NotifierProvider<AiDeciderNotifier, AiDeciderState>(
  AiDeciderNotifier.new,
);

// ============================================================================
// Benchmarks & Proactive Alert Feedback
// ============================================================================

@immutable
class AiBenchmarkState {
  const AiBenchmarkState({
    this.chatReport,
    this.deciderReport,
    this.loadingChat = false,
    this.loadingDecider = false,
    this.chatError,
    this.deciderError,
  });

  final ChatBenchmarkReport? chatReport;
  final DeciderBenchmarkReport? deciderReport;
  final bool loadingChat;
  final bool loadingDecider;
  final String? chatError;
  final String? deciderError;

  AiBenchmarkState copyWith({
    ChatBenchmarkReport? chatReport,
    DeciderBenchmarkReport? deciderReport,
    bool? loadingChat,
    bool? loadingDecider,
    String? chatError,
    bool clearChatError = false,
    String? deciderError,
    bool clearDeciderError = false,
  }) {
    return AiBenchmarkState(
      chatReport: chatReport ?? this.chatReport,
      deciderReport: deciderReport ?? this.deciderReport,
      loadingChat: loadingChat ?? this.loadingChat,
      loadingDecider: loadingDecider ?? this.loadingDecider,
      chatError: clearChatError ? null : (chatError ?? this.chatError),
      deciderError: clearDeciderError
          ? null
          : (deciderError ?? this.deciderError),
    );
  }
}

class AiBenchmarkNotifier extends Notifier<AiBenchmarkState> {
  @override
  AiBenchmarkState build() => const AiBenchmarkState();

  Future<void> loadChat({bool force = false}) async {
    if (!force && (state.chatReport != null || state.loadingChat)) return;
    state = state.copyWith(loadingChat: true, clearChatError: true);
    try {
      final report = await ref.read(aiRepositoryProvider).fetchChatBenchmark();
      state = state.copyWith(chatReport: report, loadingChat: false);
    } catch (err) {
      state = state.copyWith(
        loadingChat: false,
        chatError: describeError(
          err,
          fallback: 'Falha ao carregar benchmark do Chat.',
        ),
      );
    }
  }

  Future<void> loadDecider({bool force = false}) async {
    if (!force && (state.deciderReport != null || state.loadingDecider)) return;
    state = state.copyWith(loadingDecider: true, clearDeciderError: true);
    try {
      final report = await ref
          .read(aiRepositoryProvider)
          .fetchDeciderBenchmark();
      state = state.copyWith(deciderReport: report, loadingDecider: false);
    } catch (err) {
      state = state.copyWith(
        loadingDecider: false,
        deciderError: describeError(
          err,
          fallback: 'Falha ao carregar benchmark do Decisor.',
        ),
      );
    }
  }
}

final aiBenchmarkProvider =
    NotifierProvider<AiBenchmarkNotifier, AiBenchmarkState>(
      AiBenchmarkNotifier.new,
    );

/// Armazena triagens do Decisor e resumos proativos de incidentes recebidos via SSE.
@immutable
class AiAlertFeedbackState {
  const AiAlertFeedbackState({
    this.triages = const <int, DeciderTriage>{},
    this.summaries = const <int, AiIncidentSummary>{},
  });

  final Map<int, DeciderTriage> triages;
  final Map<int, AiIncidentSummary> summaries;
}

class AiAlertFeedbackNotifier extends Notifier<AiAlertFeedbackState> {
  @override
  AiAlertFeedbackState build() => const AiAlertFeedbackState();

  int? _extractAlertId(Map<String, dynamic> data) =>
      parseIntOrNull(data['alertEventId'] ?? data['id']);

  void applyDeciderTriage(Map<String, dynamic> data) {
    final id = _extractAlertId(data);
    final raw = data['deciderTriage'];
    if (id == null || raw is! Map) return;
    state = AiAlertFeedbackState(
      triages: <int, DeciderTriage>{
        ...state.triages,
        id: DeciderTriage.fromJson(asJsonMap(raw)),
      },
      summaries: state.summaries,
    );
  }

  void applyAiSummary(Map<String, dynamic> data) {
    final id = _extractAlertId(data);
    final raw = data['aiSummary'];
    if (id == null || raw is! Map) return;
    state = AiAlertFeedbackState(
      triages: state.triages,
      summaries: <int, AiIncidentSummary>{
        ...state.summaries,
        id: AiIncidentSummary.fromJson(asJsonMap(raw)),
      },
    );
  }

  Future<AiIncidentSummary?> requestSummary(int alertId) async {
    final summary = await ref
        .read(aiRepositoryProvider)
        .generateAlertSummary(alertId);
    if (summary != null) {
      state = AiAlertFeedbackState(
        triages: state.triages,
        summaries: <int, AiIncidentSummary>{
          ...state.summaries,
          alertId: summary,
        },
      );
    }
    return summary;
  }
}

final aiAlertFeedbackProvider =
    NotifierProvider<AiAlertFeedbackNotifier, AiAlertFeedbackState>(
      AiAlertFeedbackNotifier.new,
    );
