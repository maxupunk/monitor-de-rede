import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/features/servers/models/system_database_models.dart';

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
      state = state.copyWith(
        loadingInfo: false,
        error: describeError(
          err,
          fallback: 'Não foi possível ler as informações do banco.',
        ),
      );
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
      state = state.copyWith(
        loadingUsage: false,
        error: describeError(err, fallback: 'Falha ao medir o uso do banco.'),
      );
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
      state = state.copyWith(
        actionLoading: false,
        error: describeError(err, fallback: 'Erro ao limpar histórico.'),
      );
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
      state = state.copyWith(
        actionLoading: false,
        error: describeError(err, fallback: 'Erro ao apagar itens.'),
      );
      rethrow;
    }
  }
}

final systemDatabaseProvider =
    NotifierProvider<SystemDatabaseNotifier, SystemDatabaseState>(
      SystemDatabaseNotifier.new,
    );
