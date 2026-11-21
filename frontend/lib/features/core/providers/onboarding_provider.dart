import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/core/models/settings_models.dart';

@immutable
class OnboardingState {
  const OnboardingState({
    this.status,
    this.loading = false,
    this.saving = false,
    this.showWizard = false,
    this.dismissedInSession = false,
    this.checked = false,
  });

  final OnboardingStatus? status;
  final bool loading;
  final bool saving;
  final bool showWizard;
  final bool dismissedInSession;
  final bool checked;

  OnboardingState copyWith({
    OnboardingStatus? status,
    bool? loading,
    bool? saving,
    bool? showWizard,
    bool? dismissedInSession,
    bool? checked,
  }) {
    return OnboardingState(
      status: status ?? this.status,
      loading: loading ?? this.loading,
      saving: saving ?? this.saving,
      showWizard: showWizard ?? this.showWizard,
      dismissedInSession: dismissedInSession ?? this.dismissedInSession,
      checked: checked ?? this.checked,
    );
  }
}

class OnboardingNotifier extends Notifier<OnboardingState> {
  @override
  OnboardingState build() => const OnboardingState();

  Future<OnboardingStatus?> fetchStatus() async {
    state = state.copyWith(loading: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>('/settings/onboarding');
      final parsed = OnboardingStatus.fromJson(raw);
      state = state.copyWith(status: parsed, loading: false);
      return parsed;
    } catch (_) {
      state = state.copyWith(loading: false);
      return null;
    }
  }

  Future<bool> checkAndOpenIfNeeded() async {
    if (state.checked || state.dismissedInSession) return false;
    final auth = ref.read(authProvider);
    if (!auth.isAuthenticated || !auth.canWrite) return false;

    state = state.copyWith(checked: true);
    final st = await fetchStatus();
    if (st != null && st.needsOnboarding && !st.completed) {
      state = state.copyWith(showWizard: true);
      return true;
    }
    return false;
  }

  void openWizard() {
    state = state.copyWith(showWizard: true);
    fetchStatus();
  }

  void dismissWizard({bool persistOnServer = false}) {
    state = state.copyWith(showWizard: false, dismissedInSession: true);
    if (persistOnServer) {
      completeOnboarding();
    }
  }

  void markIntentChosen() {
    if (state.status != null) {
      state = state.copyWith(
        status: state.status!.copyWith(intentChosen: true),
      );
    }
  }

  Future<bool> completeOnboarding() async {
    state = state.copyWith(saving: true);
    try {
      await ref
          .read(apiClientProvider)
          .post<dynamic>('/settings/onboarding/complete');
      state = state.copyWith(
        saving: false,
        status: state.status?.copyWith(completed: true, needsOnboarding: false),
      );
      return true;
    } catch (_) {
      state = state.copyWith(saving: false);
      return false;
    }
  }
}

final onboardingProvider =
    NotifierProvider<OnboardingNotifier, OnboardingState>(
      OnboardingNotifier.new,
    );
