import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/features/ai/data/ai_repository.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_conversations_provider.dart';
import 'package:netmonitor_app/features/ai/providers/ai_screen_provider.dart';
import 'package:netmonitor_app/features/ai/utils/ai_chat_logic.dart';

String _randomConversationKey() {
  final rng = math.Random.secure();
  String hex(int count) => List<String>.generate(
    count,
    (_) => rng.nextInt(16).toRadixString(16),
  ).join();
  return '${hex(8)}-${hex(4)}-4${hex(3)}-${(8 + rng.nextInt(4)).toRadixString(16)}${hex(3)}-${hex(12)}';
}

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

  Future<List<AiMention>> searchMentions(String query) =>
      _repo.fetchMentions(query);

  void toggleCompactNext() {
    state = state.copyWith(compactNext: !state.compactNext);
  }

  int _streamGeneration = 0;
  int _cancelledGeneration = 0;

  void stopStreaming({bool switchingConversation = false}) {
    _cancelledGeneration = _streamGeneration;
    _cancelStream?.call();
    _cancelStream = null;
    if (!state.isStreaming) return;
    final updated = state.messages
        .map((m) {
          if (!m.isStreaming) return m;
          return interruptRunningTools(m);
        })
        .toList(growable: false);
    final capturedId = ref.read(aiConversationsProvider).activeId;
    state = state.copyWith(messages: updated, isStreaming: false);
    ref
        .read(aiConversationsProvider.notifier)
        .persistCurrent(
          updated,
          targetId: capturedId,
          updateActiveOnCreate: !switchingConversation,
        );
  }

  void newConversation({AiChatContext context = const AiChatContext()}) {
    stopStreaming(switchingConversation: true);
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
    stopStreaming(switchingConversation: true);
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
    await _repo.cancelRunningDeviceRuns(deviceId);
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

    final myGen = ++_streamGeneration;
    final myConversationKey = state.conversationKey;
    final capturedConvId = ref.read(aiConversationsProvider).activeId;
    bool isStaleOrCancelled() =>
        _cancelledGeneration == myGen ||
        _streamGeneration != myGen ||
        state.conversationKey != myConversationKey;

    var streamEndedProperly = false;
    try {
      await for (final frame in _repo.streamChat(
        body: payload,
        onCancelAvailable: (cancel) {
          if (!isStaleOrCancelled()) {
            _cancelStream = cancel;
          } else {
            cancel();
          }
        },
      )) {
        if (isStaleOrCancelled()) break;
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

      if (!streamEndedProperly && !isStaleOrCancelled()) {
        final interrupted = state.messages
            .map((m) {
              if (m.id != assistantId) return m;
              return interruptRunningTools(m).copyWith(
                error:
                    'A resposta foi interrompida antes de terminar. Tente de novo.',
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
      if (!isStaleOrCancelled()) {
        final errMsg = describeError(
          err,
          fallback: 'Falha na comunicação com a IA.',
        );
        final failed = state.messages
            .map((m) {
              if (m.id != assistantId) return m;
              return interruptRunningTools(
                m,
              ).copyWith(error: errMsg, isStreaming: false);
            })
            .toList(growable: false);
        state = state.copyWith(
          messages: failed,
          isStreaming: false,
          streamError: errMsg,
        );
      }
    } finally {
      if (_streamGeneration == myGen) {
        _cancelStream = null;
      }
      if (!isStaleOrCancelled()) {
        if (state.isStreaming) {
          state = state.copyWith(isStreaming: false);
        }
        await ref
            .read(aiConversationsProvider.notifier)
            .persistCurrent(state.messages, targetId: capturedConvId);
      }
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
            'error': describeError(
              err,
              fallback: 'Falha ao executar ferramenta.',
            ),
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
    } catch (_) {}
    await ref
        .read(aiConversationsProvider.notifier)
        .persistCurrent(state.messages);
  }

  /// Sincroniza decisões de aprovação vindas do SSE (`ai:approval_resolved`).
  void applyApprovalDecision(Map<String, dynamic> data) {
    final callId = data['callId']?.toString();
    final decision = data['decision']?.toString();
    if (callId == null || decision == null) return;
    if (decision == 'expired') return;
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
