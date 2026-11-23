import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/features/ai/data/ai_repository.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';

// ============================================================================
// AI Conversations (`stores/aiConversations.ts`)
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

  /// Persiste a conversa informada (ou ativa no momento da chamada) de forma
  /// serializada. O `targetId` é capturado sincronamente antes de entrar na
  /// fila para evitar gravar na conversa errada após troca rápida de chat.
  Future<void> persistCurrent(
    List<AiDisplayMessage> messages, {
    String? targetId,
    bool updateActiveOnCreate = true,
  }) {
    final capturedId = targetId ?? state.activeId;
    _writeQueue = _writeQueue.then(
      (_) => _persistInternal(
        messages,
        targetId: capturedId,
        updateActiveOnCreate: updateActiveOnCreate,
      ),
    );
    return _writeQueue;
  }

  Future<void> _persistInternal(
    List<AiDisplayMessage> messages, {
    required String? targetId,
    required bool updateActiveOnCreate,
  }) async {
    if (messages.isEmpty) return;
    final title = _deriveTitle(messages);
    try {
      if (targetId == null) {
        final created = await _repo.createConversation(
          title: title,
          messages: messages,
        );
        state = state.copyWith(
          activeId: updateActiveOnCreate && state.activeId == null
              ? created.id
              : state.activeId,
          summaries: <AiConversationSummary>[
            created,
            ...state.summaries.where((s) => s.id != created.id),
          ],
          clearSaveError: true,
        );
      } else {
        final existingTitle = state.summaries
            .where((s) => s.id == targetId)
            .map((s) => s.title)
            .firstOrNull;
        final updated = await _repo.updateConversation(
          id: targetId,
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
        saveError: describeError(err, fallback: 'Falha ao salvar a conversa.'),
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
// AI Pending Approvals (`stores/aiApprovals.ts`, SSE-driven)
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

/// Helper rápido para saber se o usuário atual é o destinatário de aprovações pendentes.
final myAiPendingApprovalsProvider = Provider<List<AiPendingApproval>>((ref) {
  final userId = ref.watch(authProvider).user?.id;
  return ref.watch(aiApprovalsProvider).mine(userId);
});
