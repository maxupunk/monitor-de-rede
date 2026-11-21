import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/servers/models/registry_models.dart';

@immutable
class RegistryState {
  const RegistryState({
    this.overview,
    this.loading = false,
    this.error,
    this.revision = 0,
  });

  final RegistryOverview? overview;
  final bool loading;
  final String? error;
  final int revision;

  RegistryState copyWith({
    RegistryOverview? overview,
    bool? loading,
    String? error,
    bool clearError = false,
    int? revision,
  }) {
    return RegistryState(
      overview: overview ?? this.overview,
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

  String _describe(Object err, String fallback) {
    if (err is ApiException && err.message.isNotEmpty) return err.message;
    if (err is NetworkException && err.message.isNotEmpty) return err.message;
    return fallback;
  }

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
        error: _describe(err, 'Erro ao carregar o registry'),
      );
    }
  }

  void refreshIfLoaded() {
    state = state.copyWith(revision: state.revision + 1);
    if (state.overview != null) {
      fetchOverview();
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
      raw.whereType<Map>().map(
        (e) => RegistryFindingView.fromJson(Map<String, dynamic>.from(e)),
      ),
    );
  }

  Future<List<RegistryFindingView>> rescanVersion(int versionId) async {
    final raw = await _api.post<List<dynamic>>(
      '/registry/versions/$versionId/scan',
      data: const <String, dynamic>{},
    );
    return List<RegistryFindingView>.unmodifiable(
      raw.whereType<Map>().map(
        (e) => RegistryFindingView.fromJson(Map<String, dynamic>.from(e)),
      ),
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
    return (raw['freedBytes'] as num?)?.toInt() ?? 0;
  }

  Future<void> syncAdvisories() async {
    await _api.post<dynamic>(
      '/registry/advisories/sync',
      data: const <String, dynamic>{},
    );
  }

  Future<List<RegistryTokenView>> fetchTokens() async {
    final raw = await _api.get<List<dynamic>>('/registry/tokens');
    return List<RegistryTokenView>.unmodifiable(
      raw.whereType<Map>().map(
        (e) => RegistryTokenView.fromJson(Map<String, dynamic>.from(e)),
      ),
    );
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
    return RegistryTokenCreated.fromJson(raw);
  }

  Future<void> revokeToken(int id) async {
    await _api.delete<dynamic>('/registry/tokens/$id');
  }

  Future<List<RegistryExceptionView>> fetchExceptions() async {
    final raw = await _api.get<List<dynamic>>('/registry/exceptions');
    return List<RegistryExceptionView>.unmodifiable(
      raw.whereType<Map>().map(
        (e) => RegistryExceptionView.fromJson(Map<String, dynamic>.from(e)),
      ),
    );
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
  }

  Future<void> removeException(int id) async {
    await _api.delete<dynamic>('/registry/exceptions/$id');
  }
}

final registryProvider = NotifierProvider<RegistryNotifier, RegistryState>(
  RegistryNotifier.new,
);
