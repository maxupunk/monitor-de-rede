import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';

/// Contexto da tela ativa anexado ao chat (`stores/aiScreen.ts`).
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
