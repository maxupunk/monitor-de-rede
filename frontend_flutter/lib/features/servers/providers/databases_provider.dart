import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/servers/models/database_models.dart';

String databaseJobKey(String kind, int connectionId) => '$kind:$connectionId';

@immutable
class DatabasesState {
  const DatabasesState({
    this.connections = const <DatabaseConnectionResponse>[],
    this.jobs = const <String, DatabaseJobSnapshot>{},
    this.agents = const <AgentRouteOption>[],
    this.containers = const DatabaseContainersResponse(),
    this.containersLoading = false,
    this.containersError,
    this.loading = false,
    this.loaded = false,
    this.error,
  });

  final List<DatabaseConnectionResponse> connections;
  final Map<String, DatabaseJobSnapshot> jobs;
  final List<AgentRouteOption> agents;
  final DatabaseContainersResponse containers;
  final bool containersLoading;
  final String? containersError;
  final bool loading;
  final bool loaded;
  final String? error;

  DatabaseJobSnapshot? jobFor(String kind, int connectionId) =>
      jobs[databaseJobKey(kind, connectionId)];

  DatabasesState copyWith({
    List<DatabaseConnectionResponse>? connections,
    Map<String, DatabaseJobSnapshot>? jobs,
    List<AgentRouteOption>? agents,
    DatabaseContainersResponse? containers,
    bool? containersLoading,
    String? containersError,
    bool clearContainersError = false,
    bool? loading,
    bool? loaded,
    String? error,
    bool clearError = false,
  }) {
    return DatabasesState(
      connections: connections ?? this.connections,
      jobs: jobs ?? this.jobs,
      agents: agents ?? this.agents,
      containers: containers ?? this.containers,
      containersLoading: containersLoading ?? this.containersLoading,
      containersError: clearContainersError
          ? null
          : (containersError ?? this.containersError),
      loading: loading ?? this.loading,
      loaded: loaded ?? this.loaded,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

/// Estado de conexões de banco de dados & jobs de backup/restauração (ADR 013).
/// Andamento em tempo real exclusivamente por SSE (`database_jobs:updated`).
class DatabasesNotifier extends Notifier<DatabasesState> {
  @override
  DatabasesState build() => const DatabasesState();

  ApiClient get _api => ref.read(apiClientProvider);

  String _describe(Object err, String fallback) {
    if (err is ApiException && err.message.isNotEmpty) return err.message;
    if (err is NetworkException && err.message.isNotEmpty) return err.message;
    return fallback;
  }

  Future<bool> fetchConnections() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await _api.get<List<dynamic>>('/databases');
      final parsed = raw
          .whereType<Map>()
          .map(
            (e) => DatabaseConnectionResponse.fromJson(
              Map<String, dynamic>.from(e),
            ),
          )
          .toList(growable: false);
      state = state.copyWith(
        connections: List<DatabaseConnectionResponse>.unmodifiable(parsed),
        loading: false,
        loaded: true,
      );
      return true;
    } catch (err) {
      state = state.copyWith(
        loading: false,
        error: _describe(err, 'Erro ao carregar as conexões de banco'),
      );
      return false;
    }
  }

  void refreshIfLoaded() {
    if (state.loaded) {
      fetchConnections();
    }
  }

  Future<void> fetchAgents() async {
    try {
      final raw = await _api.get<List<dynamic>>('/databases/agents');
      final parsed = raw
          .whereType<Map>()
          .map((e) => AgentRouteOption.fromJson(Map<String, dynamic>.from(e)))
          .toList(growable: false);
      state = state.copyWith(
        agents: List<AgentRouteOption>.unmodifiable(parsed),
      );
    } catch (_) {
      state = state.copyWith(agents: const <AgentRouteOption>[]);
    }
  }

  Future<void> fetchContainers() async {
    state = state.copyWith(containersLoading: true, clearContainersError: true);
    try {
      final raw = await _api.get<Map<String, dynamic>>('/databases/containers');
      state = state.copyWith(
        containers: DatabaseContainersResponse.fromJson(raw),
        containersLoading: false,
      );
    } catch (err) {
      state = state.copyWith(
        containers: const DatabaseContainersResponse(),
        containersLoading: false,
        containersError: _describe(
          err,
          'Não foi possível consultar os contêineres',
        ),
      );
    }
  }

  Future<DatabaseContainerResolution> resolveContainer(
    String hostKey,
    String containerId,
  ) async {
    final raw = await _api.post<Map<String, dynamic>>(
      '/databases/containers/resolve',
      data: <String, dynamic>{'hostKey': hostKey, 'containerId': containerId},
    );
    return DatabaseContainerResolution.fromJson(raw);
  }

  void applyJob(Map<String, dynamic> payload) {
    final snapshot = DatabaseJobSnapshot.tryFromJson(payload);
    if (snapshot == null) return;
    _applyJobSnapshot(snapshot);
  }

  void _applyJobSnapshot(DatabaseJobSnapshot snapshot) {
    final key = databaseJobKey(snapshot.kind, snapshot.connectionId);
    final current = state.jobs[key];
    if (current != null &&
        current.id != snapshot.id &&
        current.startedAt.compareTo(snapshot.startedAt) > 0) {
      return;
    }
    state = state.copyWith(
      jobs: Map<String, DatabaseJobSnapshot>.unmodifiable(
        <String, DatabaseJobSnapshot>{...state.jobs, key: snapshot},
      ),
    );
  }

  Future<DatabaseConnectionResponse> save(
    int? id,
    DatabaseConnectionInput input,
  ) async {
    final raw = id == null
        ? await _api.post<Map<String, dynamic>>(
            '/databases',
            data: input.toJson(),
          )
        : await _api.put<Map<String, dynamic>>(
            '/databases/$id',
            data: input.toJson(),
          );
    final saved = DatabaseConnectionResponse.fromJson(raw);
    final next = List<DatabaseConnectionResponse>.from(state.connections);
    final idx = next.indexWhere((c) => c.id == saved.id);
    if (idx == -1) {
      next.add(saved);
      next.sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));
    } else {
      next[idx] = saved;
    }
    state = state.copyWith(
      connections: List<DatabaseConnectionResponse>.unmodifiable(next),
    );
    return saved;
  }

  Future<bool> remove(int id) async {
    state = state.copyWith(clearError: true);
    try {
      await _api.delete<dynamic>('/databases/$id');
      state = state.copyWith(
        connections: List<DatabaseConnectionResponse>.unmodifiable(
          state.connections.where((c) => c.id != id),
        ),
      );
      return true;
    } catch (err) {
      state = state.copyWith(
        error: _describe(err, 'Erro ao excluir a conexão'),
      );
      return false;
    }
  }

  Future<DatabaseProbeResponse> probe({
    int? id,
    required String engine,
    required String host,
    required int port,
    required String username,
    String? password,
    required String sslMode,
    int? viaProbeId,
  }) async {
    final raw = await _api.post<Map<String, dynamic>>(
      '/databases/probe',
      data: <String, dynamic>{
        'id': ?id,
        'engine': engine,
        'host': host,
        'port': port,
        'username': username,
        if (password != null && password.isNotEmpty) 'password': password,
        'sslMode': sslMode,
        'viaProbeId': viaProbeId,
      },
    );
    return DatabaseProbeResponse.fromJson(raw);
  }

  Future<DatabaseJobSnapshot?> runBackup(int id) async {
    final raw = await _api.post<Map<String, dynamic>>('/databases/$id/backups');
    final snap = DatabaseJobSnapshot.tryFromJson(raw);
    if (snap != null) _applyJobSnapshot(snap);
    return snap;
  }

  Future<List<DatabaseBackupResponse>> history(int id) async {
    final raw = await _api.get<List<dynamic>>('/databases/$id/backups');
    return List<DatabaseBackupResponse>.unmodifiable(
      raw.whereType<Map>().map(
        (e) => DatabaseBackupResponse.fromJson(Map<String, dynamic>.from(e)),
      ),
    );
  }

  Future<DatabaseJobSnapshot?> restore(
    int backupId,
    DatabaseRestoreInput input,
  ) async {
    final raw = await _api.post<Map<String, dynamic>>(
      '/databases/backups/$backupId/restore',
      data: input.toJson(),
    );
    final snap = DatabaseJobSnapshot.tryFromJson(raw);
    if (snap != null) _applyJobSnapshot(snap);
    return snap;
  }
}

final databasesProvider = NotifierProvider<DatabasesNotifier, DatabasesState>(
  DatabasesNotifier.new,
);

/// Estado do banco de dados interno do NetMonitor (`Settings -> Sistema -> DatabaseInfoCard`).
@immutable
class SystemDatabaseState {
  const SystemDatabaseState({
    this.info,
    this.usage,
    this.loadingInfo = false,
    this.loadingUsage = false,
    this.actionLoading = false,
    this.error,
  });

  final DatabaseInfo? info;
  final DatabaseUsage? usage;
  final bool loadingInfo;
  final bool loadingUsage;
  final bool actionLoading;
  final String? error;

  SystemDatabaseState copyWith({
    DatabaseInfo? info,
    DatabaseUsage? usage,
    bool? loadingInfo,
    bool? loadingUsage,
    bool? actionLoading,
    String? error,
    bool clearError = false,
  }) {
    return SystemDatabaseState(
      info: info ?? this.info,
      usage: usage ?? this.usage,
      loadingInfo: loadingInfo ?? this.loadingInfo,
      loadingUsage: loadingUsage ?? this.loadingUsage,
      actionLoading: actionLoading ?? this.actionLoading,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class SystemDatabaseNotifier extends Notifier<SystemDatabaseState> {
  @override
  SystemDatabaseState build() => const SystemDatabaseState();

  ApiClient get _api => ref.read(apiClientProvider);

  Future<void> fetchInfo() async {
    state = state.copyWith(loadingInfo: true, clearError: true);
    try {
      final raw = await _api.get<Map<String, dynamic>>(
        '/settings/database-size',
      );
      state = state.copyWith(
        info: DatabaseInfo.fromJson(raw),
        loadingInfo: false,
      );
    } catch (err) {
      state = state.copyWith(loadingInfo: false, error: err.toString());
    }
  }

  Future<DatabaseUsage?> fetchUsage() async {
    state = state.copyWith(loadingUsage: true, clearError: true);
    try {
      final raw = await _api.get<Map<String, dynamic>>(
        '/settings/database-usage',
        timeout: const Duration(seconds: 120),
      );
      final usage = DatabaseUsage.fromJson(raw);
      state = state.copyWith(usage: usage, loadingUsage: false);
      return usage;
    } catch (err) {
      state = state.copyWith(loadingUsage: false, error: err.toString());
      return null;
    }
  }

  Future<Map<String, dynamic>> clearHistory() async {
    state = state.copyWith(actionLoading: true, clearError: true);
    try {
      final raw = await _api.post<Map<String, dynamic>>(
        '/settings/clear-history',
        data: const <String, dynamic>{},
      );
      await fetchInfo();
      state = state.copyWith(actionLoading: false);
      return raw;
    } catch (err) {
      state = state.copyWith(actionLoading: false, error: err.toString());
      rethrow;
    }
  }

  Future<Map<String, dynamic>> clearAllItems() async {
    state = state.copyWith(actionLoading: true, clearError: true);
    try {
      final raw = await _api.post<Map<String, dynamic>>(
        '/settings/clear-all-items',
        data: const <String, dynamic>{},
      );
      await fetchInfo();
      state = state.copyWith(actionLoading: false);
      return raw;
    } catch (err) {
      state = state.copyWith(actionLoading: false, error: err.toString());
      rethrow;
    }
  }
}

final systemDatabaseProvider =
    NotifierProvider<SystemDatabaseNotifier, SystemDatabaseState>(
      SystemDatabaseNotifier.new,
    );
