import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/servers/models/storage_models.dart';
import 'package:netmonitor_app/features/servers/utils/collection_helpers.dart';

@immutable
class StoragesState {
  const StoragesState({
    this.storages = const <StorageDestinationResponse>[],
    this.meta,
    this.loading = false,
    this.loaded = false,
    this.error,
  });

  final List<StorageDestinationResponse> storages;
  final StoragesMetaResponse? meta;
  final bool loading;
  final bool loaded;
  final String? error;

  StoragesState copyWith({
    List<StorageDestinationResponse>? storages,
    StoragesMetaResponse? meta,
    bool? loading,
    bool? loaded,
    String? error,
    bool clearError = false,
  }) {
    return StoragesState(
      storages: storages ?? this.storages,
      meta: meta ?? this.meta,
      loading: loading ?? this.loading,
      loaded: loaded ?? this.loaded,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

/// Estado de destinos de armazenamento de backup (`GET /api/storages`).
/// Recarregado por SSE via `storages:updated` quando já aberto.
class StoragesNotifier extends Notifier<StoragesState> {
  @override
  StoragesState build() => const StoragesState();

  ApiClient get _api => ref.read(apiClientProvider);

  Future<bool> fetchStorages() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await _api.get<List<dynamic>>('/storages');
      final parsed = asJsonList(raw, StorageDestinationResponse.fromJson);
      state = state.copyWith(
        storages: List<StorageDestinationResponse>.unmodifiable(parsed),
        loading: false,
        loaded: true,
      );
      return true;
    } catch (err) {
      state = state.copyWith(
        loading: false,
        error: describeError(
          err,
          fallback: 'Erro ao carregar os destinos de armazenamento',
        ),
      );
      return false;
    }
  }

  void refreshIfLoaded() {
    if (state.loaded) {
      fetchStorages();
    }
  }

  Future<StoragesMetaResponse?> fetchMeta() async {
    if (state.meta != null) return state.meta;
    try {
      final raw = await _api.get<Map<String, dynamic>>('/storages/meta');
      final meta = StoragesMetaResponse.fromJson(raw);
      state = state.copyWith(meta: meta);
      return meta;
    } catch (_) {
      return null;
    }
  }

  Future<StorageDestinationDetail> fetchDetail(int id) async {
    final raw = await _api.get<Map<String, dynamic>>('/storages/$id');
    return StorageDestinationDetail.fromJson(raw);
  }

  Future<StorageDestinationDetail> save(
    int? id,
    StorageDestinationInput input,
  ) async {
    final raw = id == null
        ? await _api.post<Map<String, dynamic>>(
            '/storages',
            data: input.toJson(),
          )
        : await _api.put<Map<String, dynamic>>(
            '/storages/$id',
            data: input.toJson(),
          );
    final saved = StorageDestinationDetail.fromJson(raw);
    final summary = saved.toSummary();
    state = state.copyWith(
      storages: upsertSortedByName<StorageDestinationResponse>(
        state.storages,
        summary,
        idOf: (s) => s.id,
        nameOf: (s) => s.name,
      ),
    );
    return saved;
  }

  Future<bool> remove(int id) async {
    state = state.copyWith(clearError: true);
    try {
      await _api.delete<dynamic>('/storages/$id');
      state = state.copyWith(
        storages: List<StorageDestinationResponse>.unmodifiable(
          state.storages.where((s) => s.id != id),
        ),
      );
      return true;
    } catch (err) {
      state = state.copyWith(
        error: describeError(err, fallback: 'Erro ao excluir o destino'),
      );
      return false;
    }
  }

  Future<StorageTestResponse> testDraft({
    int? id,
    required String provider,
    required Map<String, dynamic> config,
  }) async {
    final raw = await _api.post<Map<String, dynamic>>(
      '/storages/test',
      data: <String, dynamic>{
        'id': ?id,
        'provider': provider,
        'config': config,
      },
    );
    return StorageTestResponse.fromJson(raw);
  }

  Future<StorageTestResponse> testSaved(int id) async {
    final raw = await _api.post<Map<String, dynamic>>('/storages/$id/test');
    return StorageTestResponse.fromJson(raw);
  }

  Future<StorageBrowseResponse> browse(
    int id,
    String path, {
    String? cursor,
  }) async {
    final raw = await _api.get<Map<String, dynamic>>(
      '/storages/$id/browse',
      queryParameters: <String, dynamic>{
        'path': path,
        if (cursor != null && cursor.isNotEmpty) 'cursor': cursor,
      },
    );
    return StorageBrowseResponse.fromJson(raw);
  }

  Future<DownloadResult> downloadObject(int id, String key) {
    return _api.downloadBytes(
      '/storages/$id/download',
      queryParameters: <String, dynamic>{'key': key},
    );
  }

  Future<void> deleteObject(
    int id,
    String key, {
    required bool directory,
  }) async {
    await _api.delete<dynamic>(
      '/storages/$id/objects',
      queryParameters: <String, dynamic>{
        'key': key,
        'directory': directory.toString(),
      },
    );
  }
}

final storagesProvider = NotifierProvider<StoragesNotifier, StoragesState>(
  StoragesNotifier.new,
);
