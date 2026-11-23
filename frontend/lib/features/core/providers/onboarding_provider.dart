import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/core/models/settings_models.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/features/core/providers/settings_provider.dart';

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

  Future<Map<String, dynamic>?> fetchVpnPreflight() async {
    try {
      return await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>('/vpn/preflight');
    } catch (_) {
      return null;
    }
  }

  /// Persiste todas as escolhas do assistente inicial de forma transacional por domínio.
  /// Retorna o token do primeiro agente remoto criado (se solicitado).
  Future<String?> commitInitialSetup({
    required SystemPreferences preferences,
    String? lanAddressOverride,
    required bool enableNetwork,
    String? siteName,
    String? networkName,
    String? networkCidr,
    String? dnsName,
    String? dnsIp,
    List<String> selectedSaas = const <String>[],
    required bool enableServers,
    String? agentName,
    required bool enableLocalNotifications,
  }) async {
    final api = ref.read(apiClientProvider);
    final settingsNotifier = ref.read(coreSettingsProvider.notifier);

    await settingsNotifier.savePreferences(preferences);

    final lan = (lanAddressOverride ?? '').trim();
    if (lan.isNotEmpty) {
      if (ref.read(coreSettingsProvider).addresses.isEmpty) {
        await settingsNotifier.fetchServerAddresses();
      }
      final existing = ref.read(coreSettingsProvider);
      final existingOverrides = <String, String>{
        for (final entry in existing.addresses)
          if (entry.overridden &&
              entry.value != null &&
              entry.value!.isNotEmpty &&
              entry.kind != 'custom')
            entry.id: entry.value!,
      };
      final existingCustom = <Map<String, dynamic>>[
        for (final entry in existing.addresses)
          if (entry.kind == 'custom' &&
              entry.value != null &&
              entry.value!.isNotEmpty)
            <String, dynamic>{
              'id': entry.id,
              'label': entry.label,
              'value': entry.value,
            },
      ];
      existingOverrides['lan'] = lan;
      await settingsNotifier.saveServerAddresses(
        overrides: existingOverrides,
        custom: existingCustom,
        preferredId: existing.preferredAddressId ?? 'lan',
        domainHttps: existing.domainHttps,
      );
    }

    if (enableNetwork) {
      int? createdSiteId;
      final cleanSite = (siteName ?? '').trim();
      if (cleanSite.isNotEmpty) {
        final siteRes = await api
            .post<Map<String, dynamic>>(
              '/sites',
              data: <String, dynamic>{'name': cleanSite},
            )
            .catchError((_) => <String, dynamic>{});
        createdSiteId = parseIntOrNull(siteRes['id']);
      }
      final cleanNet = (networkName ?? '').trim();
      final cleanCidr = (networkCidr ?? '').trim();
      if (cleanNet.isNotEmpty && cleanCidr.isNotEmpty) {
        await api
            .post<dynamic>(
              '/networks',
              data: <String, dynamic>{
                'name': cleanNet,
                'cidr': cleanCidr,
                'siteId': ?createdSiteId,
              },
            )
            .catchError((_) => null);
      }
      final cleanDnsIp = (dnsIp ?? '').trim();
      final cleanDnsName = (dnsName ?? '').trim();
      if (cleanDnsIp.isNotEmpty) {
        await api
            .post<dynamic>(
              '/dns/servers',
              data: <String, dynamic>{
                'name': cleanDnsName.isEmpty ? cleanDnsIp : cleanDnsName,
                'address': cleanDnsIp,
              },
            )
            .catchError((_) => null);
      }
      if (selectedSaas.isNotEmpty) {
        await ref
            .read(monitorsProvider.notifier)
            .provisionSaasPresets(selectedSaas)
            .catchError((_) => <String, dynamic>{});
      }
    }

    String? createdAgentToken;
    final cleanAgent = (agentName ?? '').trim();
    if (enableServers && cleanAgent.isNotEmpty) {
      final agentRes = await api
          .post<Map<String, dynamic>>(
            '/agents',
            data: <String, dynamic>{'name': cleanAgent},
          )
          .catchError((_) => <String, dynamic>{});
      final enrollment = agentRes['enrollment'] is Map
          ? asJsonMap(agentRes['enrollment'])
          : const <String, dynamic>{};
      final commands = enrollment['commands'] is Map
          ? asJsonMap(enrollment['commands'])
          : const <String, dynamic>{};
      createdAgentToken =
          (commands['docker'] ?? enrollment['code'] ?? agentRes['token'])
              as String?;
    }

    await settingsNotifier.setLocalNotificationsEnabled(
      enableLocalNotifications,
    );
    await completeOnboarding();
    return createdAgentToken;
  }

  Future<bool> checkAndOpenIfNeeded() async {
    if (state.checked || state.dismissedInSession) return false;
    final auth = ref.read(authProvider);
    if (!auth.isAuthenticated || !auth.isAdmin) return false;

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
