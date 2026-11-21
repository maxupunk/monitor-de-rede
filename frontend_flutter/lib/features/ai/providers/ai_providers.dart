import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/ai/data/ai_repository.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/utils/ai_chat_logic.dart';

String _randomConversationKey() {
  final rng = math.Random.secure();
  String hex(int count) => List<String>.generate(
    count,
    (_) => rng.nextInt(16).toRadixString(16),
  ).join();
  return '${hex(8)}-${hex(4)}-4${hex(3)}-${(8 + rng.nextInt(4)).toRadixString(16)}${hex(3)}-${hex(12)}';
}

String _describeAiError(Object err, String fallback) {
  if (err is ApiException && err.message.isNotEmpty) return err.message;
  if (err is NetworkException && err.message.isNotEmpty) return err.message;
  return fallback;
}

// ============================================================================
// 1. AI Screen Context (`stores/aiScreen.ts`)
// ============================================================================

@immutable
class AiScreenState {
  const AiScreenState({
    this.activeScreen,
    this.detail,
    this.dismissedKey,
    this.entityId,
  });

  final AskedScreen? activeScreen;
  final String? detail;
  final String? dismissedKey;
  final int? entityId;

  String? get screenKey {
    if (activeScreen == null) return null;
    return entityId != null
        ? '${activeScreen!.id}:$entityId'
        : activeScreen!.id;
  }

  bool get isAttached =>
      activeScreen != null &&
      (dismissedKey == null || dismissedKey != screenKey);

  AiScreenState copyWith({
    AskedScreen? activeScreen,
    bool clearActiveScreen = false,
    String? detail,
    bool clearDetail = false,
    String? dismissedKey,
    bool clearDismissedKey = false,
    int? entityId,
    bool clearEntityId = false,
  }) {
    return AiScreenState(
      activeScreen: clearActiveScreen
          ? null
          : (activeScreen ?? this.activeScreen),
      detail: clearDetail ? null : (detail ?? this.detail),
      dismissedKey: clearDismissedKey
          ? null
          : (dismissedKey ?? this.dismissedKey),
      entityId: clearEntityId ? null : (entityId ?? this.entityId),
    );
  }
}

class AiScreenNotifier extends Notifier<AiScreenState> {
  @override
  AiScreenState build() => const AiScreenState();

  void setRouteScreen(AskedScreen? screen, {int? entityId}) {
    final prevKey = state.screenKey;
    final nextKey = screen == null
        ? null
        : (entityId != null ? '${screen.id}:$entityId' : screen.id);
    state = state.copyWith(
      activeScreen: screen,
      clearActiveScreen: screen == null,
      entityId: entityId,
      clearEntityId: entityId == null,
      clearDetail: prevKey != nextKey,
      clearDismissedKey: prevKey != nextKey,
    );
  }

  void setDetail(String? detail) {
    state = state.copyWith(detail: detail, clearDetail: detail == null);
  }

  void dismissCurrent() {
    if (state.screenKey != null) {
      state = state.copyWith(dismissedKey: state.screenKey);
    }
  }

  void restoreCurrent() {
    state = state.copyWith(clearDismissedKey: true);
  }
}

final aiScreenProvider = NotifierProvider<AiScreenNotifier, AiScreenState>(
  AiScreenNotifier.new,
);

// ============================================================================
// 2. AI Settings & Model Pull (`stores/ai.ts` settings slice)
// ============================================================================

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
        error: _describeAiError(err, 'Falha ao carregar configurações de IA.'),
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
        error: _describeAiError(err, 'Falha ao salvar configurações de IA.'),
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
        message: _describeAiError(err, 'Falha ao testar a conexão.'),
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
          errorMessage: _describeAiError(err, 'Ollama indisponível.'),
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
          errorMessage: _describeAiError(
            err,
            'Falha ao listar modelos OpenRouter.',
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
          errorMessage: _describeAiError(
            err,
            'Falha ao listar modelos OpenCode.',
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
    try {
      await for (final progress in _repo.pullOllamaModel(
        model: model,
        baseUrl: baseUrl,
        onCancelAvailable: (cancel) => _cancelPull = cancel,
      )) {
        state = state.copyWith(pullProgress: progress);
        if (progress.error != null && progress.error!.isNotEmpty) {
          state = state.copyWith(pulling: false);
          return false;
        }
      }
      state = state.copyWith(pulling: false, clearPullProgress: true);
      await loadOllamaModels(baseUrl: baseUrl);
      return true;
    } catch (err) {
      state = state.copyWith(
        pulling: false,
        pullProgress: ModelPullProgress(
          status: 'Erro no download',
          done: true,
          error: _describeAiError(err, 'Falha ao baixar modelo.'),
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
        error: 'Cancelado pelo usuário.',
      ),
    );
  }
}

final aiSettingsProvider =
    NotifierProvider<AiSettingsNotifier, AiSettingsState>(
      AiSettingsNotifier.new,
    );

// ============================================================================
// 3. AI Conversations (`stores/aiConversations.ts`)
// ============================================================================

@immutable
class AiConversationsState {
  const AiConversationsState({
    this.summaries = const <AiConversationSummary>[],
    this.activeId,
    this.loading = false,
    this.saveError,
  });

  final List<AiConversationSummary> summaries;
  final String? activeId;
  final bool loading;
  final String? saveError;

  AiConversationsState copyWith({
    List<AiConversationSummary>? summaries,
    String? activeId,
    bool clearActiveId = false,
    bool? loading,
    String? saveError,
    bool clearSaveError = false,
  }) {
    return AiConversationsState(
      summaries: summaries ?? this.summaries,
      activeId: clearActiveId ? null : (activeId ?? this.activeId),
      loading: loading ?? this.loading,
      saveError: clearSaveError ? null : (saveError ?? this.saveError),
    );
  }
}

class AiConversationsNotifier extends Notifier<AiConversationsState> {
  Future<void> _writeQueue = Future<void>.value();

  @override
  AiConversationsState build() => const AiConversationsState();

  AiRepository get _repo => ref.read(aiRepositoryProvider);

  Future<void> fetchAll() async {
    state = state.copyWith(loading: true, clearSaveError: true);
    try {
      final list = await _repo.fetchConversations();
      state = state.copyWith(summaries: list, loading: false);
    } catch (_) {
      state = state.copyWith(loading: false);
    }
  }

  Future<AiConversationDetail?> open(String id) async {
    try {
      final detail = await _repo.fetchConversation(id);
      state = state.copyWith(activeId: detail.id);
      return detail;
    } catch (_) {
      return null;
    }
  }

  void startNew() {
    state = state.copyWith(clearActiveId: true, clearSaveError: true);
  }

  /// Persiste a conversa atual de forma serializada (evita creates duplicados).
  Future<void> persistCurrent(List<AiDisplayMessage> messages) {
    _writeQueue = _writeQueue.then((_) => _persistInternal(messages));
    return _writeQueue;
  }

  Future<void> _persistInternal(List<AiDisplayMessage> messages) async {
    if (messages.isEmpty) return;
    final title = _deriveTitle(messages);
    try {
      final activeId = state.activeId;
      if (activeId == null) {
        final created = await _repo.createConversation(
          title: title,
          messages: messages,
        );
        state = state.copyWith(
          activeId: created.id,
          summaries: <AiConversationSummary>[
            created,
            ...state.summaries.where((s) => s.id != created.id),
          ],
          clearSaveError: true,
        );
      } else {
        final existingTitle = state.summaries
            .where((s) => s.id == activeId)
            .map((s) => s.title)
            .firstOrNull;
        final updated = await _repo.updateConversation(
          id: activeId,
          title: existingTitle?.isNotEmpty == true ? existingTitle! : title,
          messages: messages,
        );
        state = state.copyWith(
          summaries: <AiConversationSummary>[
            updated,
            ...state.summaries.where((s) => s.id != updated.id),
          ],
          clearSaveError: true,
        );
      }
    } catch (err) {
      state = state.copyWith(
        saveError: _describeAiError(err, 'Falha ao salvar a conversa.'),
      );
    }
  }

  Future<bool> rename(String id, String newTitle) async {
    try {
      final detail = await _repo.fetchConversation(id);
      final updated = await _repo.updateConversation(
        id: id,
        title: newTitle.trim(),
        messages: detail.messages,
      );
      state = state.copyWith(
        summaries: state.summaries
            .map((s) => s.id == id ? updated : s)
            .toList(growable: false),
      );
      return true;
    } catch (_) {
      return false;
    }
  }

  Future<bool> remove(String id) async {
    try {
      await _repo.deleteConversation(id);
      state = state.copyWith(
        summaries: state.summaries
            .where((s) => s.id != id)
            .toList(growable: false),
        clearActiveId: state.activeId == id,
      );
      return true;
    } catch (_) {
      return false;
    }
  }

  String _deriveTitle(List<AiDisplayMessage> messages) {
    for (final m in messages) {
      if (m.role == 'user' && m.content.trim().isNotEmpty) {
        final clean = m.content.trim().replaceAll(RegExp(r'\s+'), ' ');
        return clean.length > 60 ? '${clean.substring(0, 60)}…' : clean;
      }
    }
    return 'Nova conversa';
  }
}

final aiConversationsProvider =
    NotifierProvider<AiConversationsNotifier, AiConversationsState>(
      AiConversationsNotifier.new,
    );

// ============================================================================
// 4. AI Pending Approvals (`stores/aiApprovals.ts`, SSE-driven)
// ============================================================================

@immutable
class AiApprovalsState {
  const AiApprovalsState({
    this.pending = const <AiPendingApproval>[],
    this.resolving = const <String>{},
  });

  final List<AiPendingApproval> pending;
  final Set<String> resolving;

  List<AiPendingApproval> mine(int? userId) {
    if (userId == null) return const <AiPendingApproval>[];
    return pending
        .where((item) => item.userId == userId)
        .toList(growable: false);
  }

  AiApprovalsState copyWith({
    List<AiPendingApproval>? pending,
    Set<String>? resolving,
  }) {
    return AiApprovalsState(
      pending: pending ?? this.pending,
      resolving: resolving ?? this.resolving,
    );
  }
}

class AiApprovalsNotifier extends Notifier<AiApprovalsState> {
  @override
  AiApprovalsState build() => const AiApprovalsState();

  void reset() {
    state = const AiApprovalsState();
  }

  void applyPending(Map<String, dynamic> data) {
    final item = AiPendingApproval.fromJson(data);
    if (item.callId.isEmpty) return;
    if (state.pending.any((existing) => existing.callId == item.callId)) return;
    state = state.copyWith(
      pending: List<AiPendingApproval>.unmodifiable(<AiPendingApproval>[
        ...state.pending,
        item,
      ]),
    );
  }

  void applyResolved(Map<String, dynamic> data) {
    final callId = data['callId']?.toString();
    if (callId == null || callId.isEmpty) return;
    state = state.copyWith(
      pending: state.pending
          .where((item) => item.callId != callId)
          .toList(growable: false),
      resolving: state.resolving.where((id) => id != callId).toSet(),
    );
  }

  Future<bool> confirm(String callId) async {
    state = state.copyWith(resolving: <String>{...state.resolving, callId});
    try {
      await ref.read(aiRepositoryProvider).confirmApproval(callId);
      applyResolved(<String, dynamic>{'callId': callId});
      return true;
    } catch (_) {
      state = state.copyWith(
        resolving: state.resolving.where((id) => id != callId).toSet(),
      );
      return false;
    }
  }

  Future<bool> dismiss(String callId) async {
    state = state.copyWith(resolving: <String>{...state.resolving, callId});
    try {
      await ref.read(aiRepositoryProvider).dismissApproval(callId);
      applyResolved(<String, dynamic>{'callId': callId});
      return true;
    } catch (_) {
      state = state.copyWith(
        resolving: state.resolving.where((id) => id != callId).toSet(),
      );
      return false;
    }
  }
}

final aiApprovalsProvider =
    NotifierProvider<AiApprovalsNotifier, AiApprovalsState>(
      AiApprovalsNotifier.new,
    );

// ============================================================================
// 5. AI Chat Session (`stores/ai.ts` chat slice)
// ============================================================================

@immutable
class AiChatContext {
  const AiChatContext({
    this.deviceId,
    this.deviceName,
    this.monitorId,
    this.alertId,
    this.ruleId,
  });

  final int? deviceId;
  final String? deviceName;
  final int? monitorId;
  final int? alertId;
  final int? ruleId;
}

@immutable
class AiChatState {
  const AiChatState({
    required this.conversationKey,
    this.messages = const <AiDisplayMessage>[],
    this.isStreaming = false,
    this.compactNext = false,
    this.streamError,
    this.context = const AiChatContext(),
    this.autoAccept,
    this.draftText,
    this.draftMentions = const <AiMention>[],
    this.draftRevision = 0,
  });

  final String conversationKey;
  final List<AiDisplayMessage> messages;
  final bool isStreaming;
  final bool compactNext;
  final String? streamError;
  final AiChatContext context;
  final AutoAcceptState? autoAccept;
  final String? draftText;
  final List<AiMention> draftMentions;
  final int draftRevision;

  int? get deviceContextId => context.deviceId;

  bool hasTool(String callId) {
    return messages.any((m) => m.toolCalls.any((t) => t.id == callId));
  }

  AiChatState copyWith({
    String? conversationKey,
    List<AiDisplayMessage>? messages,
    bool? isStreaming,
    bool? compactNext,
    String? streamError,
    bool clearStreamError = false,
    AiChatContext? context,
    AutoAcceptState? autoAccept,
    bool clearAutoAccept = false,
    String? draftText,
    List<AiMention>? draftMentions,
    int? draftRevision,
  }) {
    return AiChatState(
      conversationKey: conversationKey ?? this.conversationKey,
      messages: messages ?? this.messages,
      isStreaming: isStreaming ?? this.isStreaming,
      compactNext: compactNext ?? this.compactNext,
      streamError: clearStreamError ? null : (streamError ?? this.streamError),
      context: context ?? this.context,
      autoAccept: clearAutoAccept ? null : (autoAccept ?? this.autoAccept),
      draftText: draftText ?? this.draftText,
      draftMentions: draftMentions ?? this.draftMentions,
      draftRevision: draftRevision ?? this.draftRevision,
    );
  }
}

class AiChatNotifier extends Notifier<AiChatState> {
  VoidCallback? _cancelStream;
  int _idSeq = 0;

  @override
  AiChatState build() {
    ref.onDispose(() => _cancelStream?.call());
    return AiChatState(conversationKey: _randomConversationKey());
  }

  AiRepository get _repo => ref.read(aiRepositoryProvider);

  String _nextId(String prefix) =>
      '$prefix-${DateTime.now().microsecondsSinceEpoch}-${++_idSeq}';

  void toggleCompactNext() {
    state = state.copyWith(compactNext: !state.compactNext);
  }

  void stopStreaming() {
    _cancelStream?.call();
    _cancelStream = null;
    if (!state.isStreaming) return;
    final updated = state.messages
        .map((m) {
          if (!m.isStreaming) return m;
          return interruptRunningTools(m);
        })
        .toList(growable: false);
    state = state.copyWith(messages: updated, isStreaming: false);
    ref.read(aiConversationsProvider.notifier).persistCurrent(updated);
  }

  void newConversation({AiChatContext context = const AiChatContext()}) {
    stopStreaming();
    ref.read(aiConversationsProvider.notifier).startNew();
    state = AiChatState(
      conversationKey: _randomConversationKey(),
      context: context,
    );
    if (context.deviceId != null) {
      refreshAutoAccept(context.deviceId!);
    }
  }

  Future<void> openConversation(String id) async {
    stopStreaming();
    final detail = await ref.read(aiConversationsProvider.notifier).open(id);
    if (detail == null) return;
    state = AiChatState(
      conversationKey: _randomConversationKey(),
      messages: detail.messages,
    );
  }

  /// Abre o drawer da IA com uma pergunta pronta sobre um recurso (`AiAskButton`).
  Future<void> askAbout(
    String prompt, {
    AiChatContext context = const AiChatContext(),
  }) async {
    newConversation(context: context);
    ref.read(aiDrawerOpenProvider.notifier).state = true;
    await sendMessage(prompt);
  }

  Future<void> refreshAutoAccept(int deviceId) async {
    try {
      final res = await _repo.fetchAutoAccept(
        conversationKey: state.conversationKey,
        deviceId: deviceId,
      );
      state = state.copyWith(autoAccept: res);
    } catch (_) {
      // Sem permissão ou recurso indisponível.
    }
  }

  Future<bool> enableAutoAccept(int deviceId) async {
    try {
      final res = await _repo.enableAutoAccept(
        conversationKey: state.conversationKey,
        deviceId: deviceId,
      );
      state = state.copyWith(autoAccept: res);
      return res.active;
    } catch (_) {
      return false;
    }
  }

  Future<void> disableAutoAccept(int deviceId) async {
    stopStreaming();
    try {
      final res = await _repo.disableAutoAccept(
        conversationKey: state.conversationKey,
        deviceId: deviceId,
      );
      state = state.copyWith(autoAccept: res);
    } catch (_) {
      state = state.copyWith(clearAutoAccept: true);
    }
  }

  /// Após cadastrar credencial faltante a partir de um cartão de ferramenta.
  Future<void> continueAfterAccess({
    required String transport,
    required String deviceName,
  }) async {
    final upper = transport.toUpperCase();
    await sendMessage(
      'Cadastrei o acesso $upper de $deviceName; pode tentar de novo.',
    );
  }

  Future<void> sendMessage(
    String content, {
    List<AiMention> mentions = const <AiMention>[],
    AskedScreen? chosenScreen,
    String? screenDetail,
    int? overrideDeviceId,
    int? overrideMonitorId,
  }) async {
    final trimmed = content.trim();
    if (trimmed.isEmpty || state.isStreaming) return;

    final screenState = ref.read(aiScreenProvider);
    final effectiveScreen =
        chosenScreen ??
        (screenState.isAttached ? screenState.activeScreen : null);

    // Determina tela anterior se mudou no meio da conversa.
    String? previousScreenId;
    if (effectiveScreen != null && chosenScreen == null) {
      for (var i = state.messages.length - 1; i >= 0; i--) {
        if (state.messages[i].role == 'user' &&
            state.messages[i].screen != null) {
          final prev = state.messages[i].screen!.id;
          if (prev != effectiveScreen.id) previousScreenId = prev;
          break;
        }
      }
    }

    final userMsg = AiDisplayMessage(
      id: _nextId('u'),
      role: 'user',
      content: trimmed,
      mentions: mentions,
      screen: effectiveScreen,
    );
    final assistantId = _nextId('a');
    final assistantMsg = AiDisplayMessage(
      id: assistantId,
      role: 'assistant',
      content: '',
      isStreaming: true,
    );

    final withUser = <AiDisplayMessage>[...state.messages, userMsg];
    final history = buildChatHistory(withUser);
    final compact = state.compactNext;

    state = state.copyWith(
      messages: <AiDisplayMessage>[...withUser, assistantMsg],
      isStreaming: true,
      compactNext: false,
      clearStreamError: true,
    );

    final effectiveDeviceId = overrideDeviceId ?? state.context.deviceId;
    final effectiveMonitorId = overrideMonitorId ?? state.context.monitorId;

    final payload = <String, dynamic>{
      'messages': history.messages,
      'deviceId': ?effectiveDeviceId,
      'monitorId': ?effectiveMonitorId,
      if (state.context.alertId != null) 'alertId': state.context.alertId,
      if (state.context.ruleId != null) 'ruleId': state.context.ruleId,
      'mentions': mentions.map((m) => m.toJson()).toList(growable: false),
      if (effectiveScreen != null)
        'screen': <String, dynamic>{
          'id': effectiveScreen.id,
          'detail': (screenDetail ?? screenState.detail)?.trim(),
          'previous': ?previousScreenId,
        },
      'summary': history.summary,
      'contextHint': history.contextHint,
      'compact': compact,
      'conversationKey': state.conversationKey,
    };

    var streamEndedProperly = false;
    try {
      await for (final frame in _repo.streamChat(
        body: payload,
        onCancelAvailable: (cancel) => _cancelStream = cancel,
      )) {
        final type = frame['type']?.toString();
        if (type == 'contextCompacted') {
          state = state.copyWith(
            messages: applyCompaction(state.messages, history.ids, frame),
          );
          continue;
        }
        if (type == 'done' || type == 'error') {
          streamEndedProperly = true;
        }
        final updated = state.messages
            .map((m) {
              if (m.id != assistantId) return m;
              return applyChatEvent(m, frame);
            })
            .toList(growable: false);

        state = state.copyWith(
          messages: updated,
          streamError: type == 'error'
              ? frame['message']?.toString()
              : state.streamError,
          isStreaming: type == 'done' || type == 'error'
              ? false
              : state.isStreaming,
        );
      }

      if (!streamEndedProperly) {
        final interrupted = state.messages
            .map((m) {
              if (m.id != assistantId) return m;
              return interruptRunningTools(m).copyWith(
                error: 'A resposta foi interrompida antes de terminar. Tente de novo.',
                isStreaming: false,
              );
            })
            .toList(growable: false);
        state = state.copyWith(
          messages: interrupted,
          isStreaming: false,
          streamError:
              'A resposta foi interrompida antes de terminar. Tente de novo.',
        );
      }
    } catch (err) {
      final errMsg = _describeAiError(err, 'Falha na comunicação com a IA.');
      final failed = state.messages
          .map((m) {
            if (m.id != assistantId) return m;
            return interruptRunningTools(m)
                .copyWith(error: errMsg, isStreaming: false);
          })
          .toList(growable: false);
      state = state.copyWith(
        messages: failed,
        isStreaming: false,
        streamError: errMsg,
      );
    } finally {
      _cancelStream = null;
      if (state.isStreaming) {
        state = state.copyWith(isStreaming: false);
      }
      await ref
          .read(aiConversationsProvider.notifier)
          .persistCurrent(state.messages);
    }
  }

  /// Confirma ou cancela uma ferramenta em estado `awaiting`.
  Future<void> confirmTool(String messageId, String callId) async {
    AiToolCallState? target;
    for (final msg in state.messages) {
      if (msg.id == messageId) {
        for (final tool in msg.toolCalls) {
          if (tool.id == callId) target = tool;
        }
      }
    }
    if (target == null) return;

    _patchTool(
      messageId,
      callId,
      (t) => t.copyWith(status: AiToolStatus.running),
    );
    try {
      final outcome = await _repo.executeTool(
        name: target.name,
        arguments: target.arguments,
        conversationKey: state.conversationKey,
        callId: callId,
      );
      _patchTool(
        messageId,
        callId,
        (t) => t.copyWith(
          result: outcome.result,
          chart: outcome.chart,
          status: outcome.result['error'] != null
              ? AiToolStatus.error
              : AiToolStatus.done,
        ),
      );
    } catch (err) {
      _patchTool(
        messageId,
        callId,
        (t) => t.copyWith(
          result: <String, dynamic>{
            'error': _describeAiError(err, 'Falha ao executar ferramenta.'),
          },
          status: AiToolStatus.error,
        ),
      );
    }
    await ref
        .read(aiConversationsProvider.notifier)
        .persistCurrent(state.messages);
  }

  Future<void> cancelTool(String messageId, String callId) async {
    _patchTool(
      messageId,
      callId,
      (t) => t.copyWith(status: AiToolStatus.cancelled),
    );
    try {
      await _repo.dismissApproval(callId);
    } catch (_) {
      // Pode já ter sido resolvido.
    }
    await ref
        .read(aiConversationsProvider.notifier)
        .persistCurrent(state.messages);
  }

  /// Sincroniza decisões de aprovação vindas do SSE (`ai:approval_resolved`).
  void applyApprovalDecision(Map<String, dynamic> data) {
    final callId = data['callId']?.toString();
    final decision = data['decision']?.toString();
    if (callId == null || decision == null) return;
    if (decision == 'expired') return; // Cartão no chat continua confirmável.
    final nextStatus = decision == 'confirmed'
        ? AiToolStatus.done
        : AiToolStatus.cancelled;
    final updated = state.messages
        .map((m) {
          if (!m.toolCalls.any((t) => t.id == callId)) return m;
          return m.copyWith(
            toolCalls: m.toolCalls
                .map(
                  (t) => t.id == callId && t.status == AiToolStatus.awaiting
                      ? t.copyWith(status: nextStatus)
                      : t,
                )
                .toList(growable: false),
          );
        })
        .toList(growable: false);
    state = state.copyWith(messages: updated);
  }

  void rateMessage(String messageId, String value, [String? reason]) {
    final updated = state.messages
        .map((m) {
          if (m.id != messageId) return m;
          final next = toggleRating(m.rating, value, reason);
          return m.copyWith(rating: next, clearRating: next == null);
        })
        .toList(growable: false);
    state = state.copyWith(messages: updated);
    ref.read(aiConversationsProvider.notifier).persistCurrent(updated);
  }

  void rewind(String userMessageId) {
    stopStreaming();
    final rewound = rewindTo(state.messages, userMessageId);
    if (rewound == null) return;
    state = state.copyWith(
      messages: rewound.kept,
      draftText: rewound.content,
      draftMentions: rewound.mentions,
      draftRevision: state.draftRevision + 1,
    );
    ref.read(aiConversationsProvider.notifier).persistCurrent(rewound.kept);
  }

  void _patchTool(
    String messageId,
    String callId,
    AiToolCallState Function(AiToolCallState) transform,
  ) {
    final updated = state.messages
        .map((m) {
          if (m.id != messageId) return m;
          return m.copyWith(
            toolCalls: m.toolCalls
                .map((t) => t.id == callId ? transform(t) : t)
                .toList(growable: false),
          );
        })
        .toList(growable: false);
    state = state.copyWith(messages: updated);
  }
}

final aiChatProvider = NotifierProvider<AiChatNotifier, AiChatState>(
  AiChatNotifier.new,
);

// ============================================================================
// 6. AI Decider (`stores/aiDecider.ts`)
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
        error: _describeAiError(
          err,
          'Falha ao carregar configurações do Decisor.',
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
      final msg = _describeAiError(err, 'Falha ao salvar o Decisor.');
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
          errorMessage: _describeAiError(err, 'Falha ao consultar provedor.'),
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
          message: _describeAiError(err, 'Falha ao carregar modelo.'),
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
        message: _describeAiError(err, 'Falha ao testar o Decisor.'),
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
          error: _describeAiError(err, 'Falha durante o download do modelo.'),
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
// 7. Benchmarks & Data Export & Proactive Alert Feedback
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
        chatError: _describeAiError(
          err,
          'Falha ao carregar benchmark do Chat.',
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
        deciderError: _describeAiError(
          err,
          'Falha ao carregar benchmark do Decisor.',
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

  int? _extractAlertId(Map<String, dynamic> data) {
    final raw = data['alertEventId'] ?? data['id'];
    return raw is num ? raw.toInt() : null;
  }

  void applyDeciderTriage(Map<String, dynamic> data) {
    final id = _extractAlertId(data);
    final raw = data['deciderTriage'];
    if (id == null || raw is! Map) return;
    state = AiAlertFeedbackState(
      triages: <int, DeciderTriage>{
        ...state.triages,
        id: DeciderTriage.fromJson(Map<String, dynamic>.from(raw)),
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
        id: AiIncidentSummary.fromJson(Map<String, dynamic>.from(raw)),
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

/// Helper rápido para saber se o usuário atual é o destinatário de aprovações pendentes.
final myAiPendingApprovalsProvider = Provider<List<AiPendingApproval>>((ref) {
  final userId = ref.watch(authProvider).user?.id;
  return ref.watch(aiApprovalsProvider).mine(userId);
});
