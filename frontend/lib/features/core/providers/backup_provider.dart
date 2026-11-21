import 'dart:convert';

import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/core/models/admin_models.dart';

@immutable
class BackupUploadProgress {
  const BackupUploadProgress({
    required this.phase,
    required this.sentBytes,
    required this.totalBytes,
  });

  final String phase; // 'preview' | 'restore'
  final int sentBytes;
  final int totalBytes;

  double? get fraction =>
      totalBytes > 0 ? (sentBytes / totalBytes).clamp(0.0, 1.0) : null;
}

@immutable
class BackupState {
  const BackupState({
    this.plan,
    this.storages = const <StorageOptionRef>[],
    this.loading = false,
    this.running = false,
    this.exporting = false,
    this.restoring = false,
    this.pendingBytes,
    this.pendingName,
    this.pendingCounts,
    this.upload,
    this.fullDownloadReceivedBytes,
    this.lastRestore,
    this.error,
  });

  final SystemBackupPlan? plan;
  final List<StorageOptionRef> storages;
  final bool loading;
  final bool running;
  final bool exporting;
  final bool restoring;
  final Uint8List? pendingBytes;
  final String? pendingName;
  final BackupCountsResponse? pendingCounts;
  final BackupUploadProgress? upload;
  final int? fullDownloadReceivedBytes;
  final BackupRestoreResponse? lastRestore;
  final String? error;

  bool get isDownloadingFull => fullDownloadReceivedBytes != null;

  BackupState copyWith({
    SystemBackupPlan? plan,
    List<StorageOptionRef>? storages,
    bool? loading,
    bool? running,
    bool? exporting,
    bool? restoring,
    Uint8List? pendingBytes,
    String? pendingName,
    BackupCountsResponse? pendingCounts,
    bool clearPending = false,
    BackupUploadProgress? upload,
    bool clearUpload = false,
    int? fullDownloadReceivedBytes,
    bool clearFullDownload = false,
    BackupRestoreResponse? lastRestore,
    String? error,
    bool clearError = false,
  }) {
    return BackupState(
      plan: plan ?? this.plan,
      storages: storages ?? this.storages,
      loading: loading ?? this.loading,
      running: running ?? this.running,
      exporting: exporting ?? this.exporting,
      restoring: restoring ?? this.restoring,
      pendingBytes: clearPending ? null : (pendingBytes ?? this.pendingBytes),
      pendingName: clearPending ? null : (pendingName ?? this.pendingName),
      pendingCounts: clearPending
          ? null
          : (pendingCounts ?? this.pendingCounts),
      upload: clearUpload ? null : (upload ?? this.upload),
      fullDownloadReceivedBytes: clearFullDownload
          ? null
          : (fullDownloadReceivedBytes ?? this.fullDownloadReceivedBytes),
      lastRestore: lastRestore ?? this.lastRestore,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class BackupNotifier extends Notifier<BackupState> {
  CancelToken? _fullDownloadCancel;

  @override
  BackupState build() {
    ref.onDispose(() => _fullDownloadCancel?.cancel());
    return const BackupState();
  }

  Future<void> fetchPlan() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final api = ref.read(apiClientProvider);
      final planRaw = await api.get<Map<String, dynamic>>('/backup/system');
      final storagesRaw = await api
          .get<dynamic>('/storages')
          .catchError((_) => const <dynamic>[]);
      final storageList = storagesRaw is List
          ? storagesRaw
                .whereType<Map>()
                .map(
                  (e) =>
                      StorageOptionRef.fromJson(Map<String, dynamic>.from(e)),
                )
                .toList(growable: false)
          : const <StorageOptionRef>[];
      state = state.copyWith(
        plan: SystemBackupPlan.fromJson(planRaw),
        storages: storageList,
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(loading: false, error: err.toString());
    }
  }

  void refreshPlanIfLoaded() {
    if (state.plan != null) {
      fetchPlan();
    }
  }

  Future<SystemBackupPlan> savePlan({
    required int? storageDestinationId,
    required bool backupEnabled,
    required int backupIntervalHours,
    required int backupRetention,
  }) async {
    final raw = await ref
        .read(apiClientProvider)
        .put<Map<String, dynamic>>(
          '/backup/system',
          data: <String, dynamic>{
            'storageDestinationId': storageDestinationId,
            'backupEnabled': backupEnabled,
            'backupIntervalHours': backupIntervalHours,
            'backupRetention': backupRetention,
          },
        );
    final saved = SystemBackupPlan.fromJson(raw);
    state = state.copyWith(plan: saved);
    return saved;
  }

  Future<Map<String, dynamic>> runNow() async {
    state = state.copyWith(running: true, clearError: true);
    try {
      final res = await ref
          .read(apiClientProvider)
          .post<Map<String, dynamic>>('/backup/system/run');
      return res;
    } finally {
      state = state.copyWith(running: false);
      fetchPlan();
    }
  }

  Future<List<SystemBackupCopy>> listCopies({int? storageId}) async {
    final raw = await ref
        .read(apiClientProvider)
        .get<dynamic>(
          '/backup/system/copies',
          queryParameters: storageId != null
              ? <String, dynamic>{'storageDestinationId': storageId}
              : null,
        );
    if (raw is List) {
      return raw
          .whereType<Map>()
          .map((e) => SystemBackupCopy.fromJson(Map<String, dynamic>.from(e)))
          .toList(growable: false);
    }
    return const <SystemBackupCopy>[];
  }

  Future<BackupCountsResponse> previewCopy(
    int storageDestinationId,
    String key,
  ) async {
    final raw = await ref
        .read(apiClientProvider)
        .post<Map<String, dynamic>>(
          '/backup/system/copies/preview',
          data: <String, dynamic>{
            'storageDestinationId': storageDestinationId,
            'key': key,
          },
        );
    return BackupCountsResponse.fromJson(raw);
  }

  Future<BackupCountsResponse> restoreCopy(
    int storageDestinationId,
    String key,
  ) async {
    final raw = await ref
        .read(apiClientProvider)
        .post<Map<String, dynamic>>(
          '/backup/system/copies/restore',
          data: <String, dynamic>{
            'storageDestinationId': storageDestinationId,
            'key': key,
          },
        );
    return BackupCountsResponse.fromJson(raw);
  }

  /// Exporta JSON de configuração (`GET /api/backup/export`).
  Future<({Uint8List bytes, String filename})?> exportConfig() async {
    state = state.copyWith(exporting: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>('/backup/export');
      final encoded = const JsonEncoder.withIndent('  ').convert(raw);
      final stamp = DateTime.now()
          .toUtc()
          .toIso8601String()
          .substring(0, 19)
          .replaceAll(RegExp(r'[-:]'), '')
          .replaceAll('T', '-');
      return (
        bytes: Uint8List.fromList(utf8.encode(encoded)),
        filename: 'netmonitor-backup-$stamp.json',
      );
    } catch (err) {
      state = state.copyWith(error: err.toString());
      return null;
    } finally {
      state = state.copyWith(exporting: false);
    }
  }

  /// Baixa backup completo (`.json.gz`) com progresso e cancelamento (`GET /api/backup/full`).
  Future<DownloadResult?> downloadFull() async {
    if (state.isDownloadingFull) return null;
    _fullDownloadCancel = CancelToken();
    state = state.copyWith(fullDownloadReceivedBytes: 0, clearError: true);
    try {
      final result = await ref
          .read(apiClientProvider)
          .downloadBytes(
            '/backup/full',
            cancelToken: _fullDownloadCancel,
            onReceiveProgress: (received, _) {
              state = state.copyWith(fullDownloadReceivedBytes: received);
            },
          );
      return result;
    } on DioException catch (e) {
      if (e.type != DioExceptionType.cancel) {
        state = state.copyWith(error: 'Erro ao baixar o backup completo.');
      }
      return null;
    } catch (err) {
      state = state.copyWith(error: err.toString());
      return null;
    } finally {
      _fullDownloadCancel = null;
      state = state.copyWith(clearFullDownload: true);
    }
  }

  void cancelFullDownload() {
    _fullDownloadCancel?.cancel('Cancelado pelo operador');
  }

  /// Envia bytes crus para `POST /api/backup/preview` (NÃO multipart!).
  Future<bool> previewUploadedFile(Uint8List bytes, String filename) async {
    state = state.copyWith(
      clearPending: true,
      clearError: true,
      upload: BackupUploadProgress(
        phase: 'preview',
        sentBytes: 0,
        totalBytes: bytes.length,
      ),
    );
    try {
      final raw = await ref
          .read(apiClientProvider)
          .postRawBytes<Map<String, dynamic>>(
            '/backup/preview',
            bytes,
            onSendProgress: (sent, total) {
              state = state.copyWith(
                upload: BackupUploadProgress(
                  phase: 'preview',
                  sentBytes: sent,
                  totalBytes: total > 0 ? total : bytes.length,
                ),
              );
            },
          );
      state = state.copyWith(
        pendingBytes: bytes,
        pendingName: filename,
        pendingCounts: BackupCountsResponse.fromJson(raw),
        clearUpload: true,
      );
      return true;
    } catch (err) {
      state = state.copyWith(clearUpload: true, error: err.toString());
      return false;
    }
  }

  void clearPendingFile() {
    state = state.copyWith(clearPending: true, clearError: true);
  }

  /// Envia bytes crus para `POST /api/backup/restore` (NÃO multipart!).
  Future<BackupRestoreResponse?> restorePendingFile() async {
    final bytes = state.pendingBytes;
    if (bytes == null) return null;
    state = state.copyWith(
      restoring: true,
      clearError: true,
      upload: BackupUploadProgress(
        phase: 'restore',
        sentBytes: 0,
        totalBytes: bytes.length,
      ),
    );
    try {
      final raw = await ref
          .read(apiClientProvider)
          .postRawBytes<Map<String, dynamic>>(
            '/backup/restore',
            bytes,
            onSendProgress: (sent, total) {
              state = state.copyWith(
                upload: BackupUploadProgress(
                  phase: 'restore',
                  sentBytes: sent,
                  totalBytes: total > 0 ? total : bytes.length,
                ),
              );
            },
          );
      final parsed = BackupRestoreResponse.fromJson(raw);
      state = state.copyWith(
        restoring: false,
        clearPending: true,
        clearUpload: true,
        lastRestore: parsed,
      );
      return parsed;
    } catch (err) {
      state = state.copyWith(
        restoring: false,
        clearUpload: true,
        error: err.toString(),
      );
      return null;
    }
  }
}

final backupProvider = NotifierProvider<BackupNotifier, BackupState>(
  BackupNotifier.new,
);
