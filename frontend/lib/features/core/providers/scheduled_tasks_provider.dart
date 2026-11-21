import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/core/models/event_models.dart';
import 'package:netmonitor_app/features/core/models/operations_models.dart';

@immutable
class ScheduledTasksState {
  const ScheduledTasksState({
    this.tasks = const <ScheduledTask>[],
    this.catalog = const <SchedulableTool>[],
    this.loaded = false,
    this.loading = false,
    this.catalogLoading = false,
    this.lastEventTaskId,
    this.lastEventStatus,
    this.lastEventSeq = 0,
    this.error,
  });

  final List<ScheduledTask> tasks;
  final List<SchedulableTool> catalog;
  final bool loaded;
  final bool loading;
  final bool catalogLoading;
  final int? lastEventTaskId;
  final String? lastEventStatus;
  final int lastEventSeq;
  final String? error;

  SchedulableTool? toolOf(String name) {
    for (final t in catalog) {
      if (t.name == name) return t;
    }
    return null;
  }

  ScheduledTasksState copyWith({
    List<ScheduledTask>? tasks,
    List<SchedulableTool>? catalog,
    bool? loaded,
    bool? loading,
    bool? catalogLoading,
    int? lastEventTaskId,
    String? lastEventStatus,
    int? lastEventSeq,
    String? error,
    bool clearError = false,
  }) {
    return ScheduledTasksState(
      tasks: tasks ?? this.tasks,
      catalog: catalog ?? this.catalog,
      loaded: loaded ?? this.loaded,
      loading: loading ?? this.loading,
      catalogLoading: catalogLoading ?? this.catalogLoading,
      lastEventTaskId: lastEventTaskId ?? this.lastEventTaskId,
      lastEventStatus: lastEventStatus ?? this.lastEventStatus,
      lastEventSeq: lastEventSeq ?? this.lastEventSeq,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class ScheduledTasksNotifier extends Notifier<ScheduledTasksState> {
  @override
  ScheduledTasksState build() => const ScheduledTasksState();

  Future<void> fetchTasks({bool quiet = false}) async {
    if (!quiet) {
      state = state.copyWith(loading: true, clearError: true);
    }
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/scheduled-tasks');
      final list = raw is List
          ? raw
                .whereType<Map>()
                .map(
                  (e) => ScheduledTask.fromJson(Map<String, dynamic>.from(e)),
                )
                .toList(growable: false)
          : const <ScheduledTask>[];
      state = state.copyWith(tasks: list, loaded: true, loading: false);
    } catch (err) {
      state = state.copyWith(loading: false, error: err.toString());
    }
  }

  Future<void> fetchCatalog() async {
    if (state.catalog.isNotEmpty || state.catalogLoading) return;
    state = state.copyWith(catalogLoading: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/scheduled-tasks/catalog');
      final list = raw is List
          ? raw
                .whereType<Map>()
                .map(
                  (e) => SchedulableTool.fromJson(Map<String, dynamic>.from(e)),
                )
                .toList(growable: false)
          : const <SchedulableTool>[];
      state = state.copyWith(catalog: list, catalogLoading: false);
    } catch (err) {
      state = state.copyWith(catalogLoading: false, error: err.toString());
    }
  }

  Future<ScheduledTaskPreview> preview({
    required String tool,
    required Map<String, dynamic> arguments,
    required String cron,
  }) async {
    final raw = await ref
        .read(apiClientProvider)
        .post<Map<String, dynamic>>(
          '/scheduled-tasks/preview',
          data: <String, dynamic>{
            'tool': tool,
            'arguments': arguments,
            'cron': cron,
          },
        );
    return ScheduledTaskPreview.fromJson(raw);
  }

  void _replace(ScheduledTask saved) {
    final idx = state.tasks.indexWhere((t) => t.id == saved.id);
    if (idx >= 0) {
      final copy = List<ScheduledTask>.from(state.tasks);
      copy[idx] = saved;
      state = state.copyWith(tasks: copy);
    } else {
      final copy = <ScheduledTask>[...state.tasks, saved]
        ..sort((a, b) => a.name.toLowerCase().compareTo(b.name.toLowerCase()));
      state = state.copyWith(tasks: copy);
    }
  }

  Future<ScheduledTask> create(Map<String, dynamic> input) async {
    final raw = await ref
        .read(apiClientProvider)
        .post<Map<String, dynamic>>('/scheduled-tasks', data: input);
    final saved = ScheduledTask.fromJson(raw);
    _replace(saved);
    return saved;
  }

  Future<ScheduledTask> update(int id, Map<String, dynamic> input) async {
    final raw = await ref
        .read(apiClientProvider)
        .put<Map<String, dynamic>>('/scheduled-tasks/$id', data: input);
    final saved = ScheduledTask.fromJson(raw);
    _replace(saved);
    return saved;
  }

  Future<ScheduledTask> setEnabled(int id, bool enabled) async {
    final raw = await ref
        .read(apiClientProvider)
        .put<Map<String, dynamic>>(
          '/scheduled-tasks/$id/enabled',
          data: <String, dynamic>{'enabled': enabled},
        );
    final saved = ScheduledTask.fromJson(raw);
    _replace(saved);
    return saved;
  }

  Future<void> remove(int id) async {
    await ref.read(apiClientProvider).delete<dynamic>('/scheduled-tasks/$id');
    state = state.copyWith(
      tasks: state.tasks.where((t) => t.id != id).toList(growable: false),
    );
  }

  Future<int?> runNow(int id) async {
    final raw = await ref
        .read(apiClientProvider)
        .post<Map<String, dynamic>>(
          '/scheduled-tasks/$id/run',
          data: const <String, dynamic>{},
        );
    return (raw['runId'] as num?)?.toInt();
  }

  Future<({List<ScheduledTaskRun> runs, PaginationMeta? meta})> fetchRuns(
    int taskId, {
    int page = 1,
    int limit = 25,
  }) async {
    final raw = await ref
        .read(apiClientProvider)
        .get<dynamic>(
          '/scheduled-tasks/$taskId/runs',
          queryParameters: <String, dynamic>{'page': page, 'limit': limit},
        );
    if (raw is Map) {
      final list = raw['data'] is List ? raw['data'] as List : const [];
      return (
        runs: list
            .whereType<Map>()
            .map((e) => ScheduledTaskRun.fromJson(Map<String, dynamic>.from(e)))
            .toList(growable: false),
        meta: raw['meta'] is Map
            ? PaginationMeta.fromJson(
                Map<String, dynamic>.from(raw['meta'] as Map),
              )
            : null,
      );
    }
    if (raw is List) {
      return (
        runs: raw
            .whereType<Map>()
            .map((e) => ScheduledTaskRun.fromJson(Map<String, dynamic>.from(e)))
            .toList(growable: false),
        meta: null,
      );
    }
    return (runs: const <ScheduledTaskRun>[], meta: null);
  }

  void applyUpdated(Map<String, dynamic> data) {
    final id = (data['id'] as num?)?.toInt();
    if (id == null) return;
    state = state.copyWith(
      lastEventTaskId: id,
      lastEventStatus: data['status']?.toString(),
      lastEventSeq: state.lastEventSeq + 1,
    );
    if (state.loaded) {
      fetchTasks(quiet: true);
    }
  }
}

final scheduledTasksProvider =
    NotifierProvider<ScheduledTasksNotifier, ScheduledTasksState>(
      ScheduledTasksNotifier.new,
    );
