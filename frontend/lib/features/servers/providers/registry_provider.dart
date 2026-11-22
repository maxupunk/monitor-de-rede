import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/servers/models/registry_models.dart';

@immutable
class RegistryState {
  const RegistryState({
    this.overview,
    this.tokens = const <RegistryTokenView>[],
    this.loadingTokens = false,
    this.tokensLoaded = false,
    this.justCreatedToken,
    this.exceptions = const <RegistryExceptionView>[],
    this.loadingExceptions = false,
    this.exceptionsLoaded = false,
    this.loading = false,
    this.error,
    this.revision = 0,
  });

  final RegistryOverview? overview;
  final List<RegistryTokenView> tokens;
  final bool loadingTokens;
  final bool tokensLoaded;
  final RegistryTokenCreated? justCreatedToken;
  final List<RegistryExceptionView> exceptions;
  final bool loadingExceptions;
  final bool exceptionsLoaded;
  final bool loading;
  final String? error;
  final int revision;

  RegistryState copyWith({
    RegistryOverview? overview,
    List<RegistryTokenView>? tokens,
    bool? loadingTokens,
    bool? tokensLoaded,
    RegistryTokenCreated? justCreatedToken,
    bool clearJustCreatedToken = false,
    List<RegistryExceptionView>? exceptions,
    bool? loadingExceptions,
    bool? exceptionsLoaded,
    bool? loading,
    String? error,
    bool clearError = false,
    int? revision,
  }) {
    return RegistryState(
      overview: overview ?? this.overview,
      tokens: tokens ?? this.tokens,
      loadingTokens: loadingTokens ?? this.loadingTokens,
      tokensLoaded: tokensLoaded ?? this.tokensLoaded,
      justCreatedToken: clearJustCreatedToken
          ? null
          : (justCreatedToken ?? this.justCreatedToken),
      exceptions: exceptions ?? this.exceptions,
      loadingExceptions: loadingExceptions ?? this.loadingExceptions,
      exceptionsLoaded: exceptionsLoaded ?? this.exceptionsLoaded,
      loading: loading ?? this.loading,
      error: clearError ? null : (error ?? this.error),
      revision: revision ?? this.revision,
    );
  }
}

/// Estado do Container Registry da central (ADR 022 / ADR 023 — 15 endpoints).
/// Atualizado por SSE (`registry:changed`) sem polling.
class RegistryNotifier extends Notifier<RegistryState> {
  @override
  RegistryState build() => const RegistryState();

  ApiClient get _api => ref.read(apiClientProvider);

  Future<void> fetchOverview() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await _api.get<Map<String, dynamic>>('/registry');
      state = state.copyWith(
        overview: RegistryOverview.fromJson(raw),
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(
        loading: false,
        error: describeError(err, fallback: 'Erro ao carregar o registry'),
      );
    }
  }

  void refreshIfLoaded() {
    state = state.copyWith(revision: state.revision + 1);
    if (state.overview != null) {
      fetchOverview();
    }
    if (state.tokensLoaded) {
      fetchTokens();
    }
    if (state.exceptionsLoaded) {
      fetchExceptions();
    }
  }

  Future<RegistryRepositoryDetail> repositoryDetail(int repositoryId) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '/registry/repositories/$repositoryId',
    );
    return RegistryRepositoryDetail.fromJson(raw);
  }

  Future<List<RegistryFindingView>> versionFindings(int versionId) async {
    final raw = await _api.get<List<dynamic>>(
      '/registry/versions/$versionId/findings',
    );
    return List<RegistryFindingView>.unmodifiable(
      asJsonList(raw, RegistryFindingView.fromJson),
    );
  }

  Future<List<RegistryFindingView>> rescanVersion(int versionId) async {
    final raw = await _api.post<List<dynamic>>(
      '/registry/versions/$versionId/scan',
      data: const <String, dynamic>{},
    );
    return List<RegistryFindingView>.unmodifiable(
      asJsonList(raw, RegistryFindingView.fromJson),
    );
  }

  Future<void> pinVersion(int versionId, {required bool pinned}) async {
    await _api.post<dynamic>(
      '/registry/versions/$versionId/pin',
      data: <String, dynamic>{'pinned': pinned},
    );
  }

  Future<void> removeVersion(int versionId) async {
    await _api.delete<dynamic>('/registry/versions/$versionId');
  }

  Future<void> saveSettings(RegistrySettings settings) async {
    await _api.put<Map<String, dynamic>>(
      '/registry/settings',
      data: settings.toJson(),
    );
    await fetchOverview();
  }

  Future<int> collectGarbage() async {
    final raw = await _api.post<Map<String, dynamic>>(
      '/registry/collect',
      data: const <String, dynamic>{},
    );
    await fetchOverview();
    return parseIntOrNull(raw['freedBytes']) ?? 0;
  }

  Future<void> syncAdvisories() async {
    await _api.post<dynamic>(
      '/registry/advisories/sync',
      data: const <String, dynamic>{},
    );
  }

  Future<List<RegistryTokenView>> fetchTokens() async {
    state = state.copyWith(loadingTokens: true);
    try {
      final raw = await _api.get<List<dynamic>>('/registry/tokens');
      final list = List<RegistryTokenView>.unmodifiable(
        asJsonList(raw, RegistryTokenView.fromJson),
      );
      state = state.copyWith(
        tokens: list,
        loadingTokens: false,
        tokensLoaded: true,
      );
      return list;
    } catch (_) {
      state = state.copyWith(loadingTokens: false);
      return state.tokens;
    }
  }

  Future<RegistryTokenCreated> createToken({
    required String name,
    required List<String> repositories,
    required List<String> actions,
    required int days,
  }) async {
    final raw = await _api.post<Map<String, dynamic>>(
      '/registry/tokens',
      data: <String, dynamic>{
        'name': name,
        'scope': <String, dynamic>{
          'repositories': repositories,
          'actions': actions,
        },
        'days': days,
      },
    );
    final created = RegistryTokenCreated.fromJson(raw);
    state = state.copyWith(justCreatedToken: created);
    await fetchTokens();
    return created;
  }

  void clearJustCreatedToken() {
    state = state.copyWith(clearJustCreatedToken: true);
  }

  Future<void> revokeToken(int id) async {
    await _api.delete<dynamic>('/registry/tokens/$id');
    await fetchTokens();
  }

  Future<List<RegistryExceptionView>> fetchExceptions() async {
    state = state.copyWith(loadingExceptions: true);
    try {
      final raw = await _api.get<List<dynamic>>('/registry/exceptions');
      final list = List<RegistryExceptionView>.unmodifiable(
        asJsonList(raw, RegistryExceptionView.fromJson),
      );
      state = state.copyWith(
        exceptions: list,
        loadingExceptions: false,
        exceptionsLoaded: true,
      );
      return list;
    } catch (_) {
      state = state.copyWith(loadingExceptions: false);
      return state.exceptions;
    }
  }

  Future<void> addException({
    int? applicationId,
    required String advisory,
    required String reason,
    required int days,
  }) async {
    await _api.post<dynamic>(
      '/registry/exceptions',
      data: <String, dynamic>{
        'applicationId': applicationId,
        'advisory': advisory,
        'reason': reason,
        'days': days,
      },
    );
    await fetchExceptions();
  }

  Future<void> removeException(int id) async {
    await _api.delete<dynamic>('/registry/exceptions/$id');
    await fetchExceptions();
  }
}

final registryProvider = NotifierProvider<RegistryNotifier, RegistryState>(
  RegistryNotifier.new,
);
