import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/core/models/operations_models.dart';

@immutable
class MaintenanceWindowsState {
  const MaintenanceWindowsState({
    this.windows = const <MaintenanceWindow>[],
    this.loading = false,
    this.error,
  });

  final List<MaintenanceWindow> windows;
  final bool loading;
  final String? error;

  MaintenanceWindowsState copyWith({
    List<MaintenanceWindow>? windows,
    bool? loading,
    String? error,
    bool clearError = false,
  }) {
    return MaintenanceWindowsState(
      windows: windows ?? this.windows,
      loading: loading ?? this.loading,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class MaintenanceWindowsNotifier extends Notifier<MaintenanceWindowsState> {
  @override
  MaintenanceWindowsState build() => const MaintenanceWindowsState();

  Future<void> fetchWindows() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/maintenance-windows');
      state = state.copyWith(
        windows: asJsonList<MaintenanceWindow>(raw, MaintenanceWindow.fromJson),
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(loading: false, error: describeError(err));
    }
  }

  Future<bool> createWindow(Map<String, dynamic> payload) async {
    state = state.copyWith(clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<Map<String, dynamic>>('/maintenance-windows', data: payload);
      state = state.copyWith(
        windows: <MaintenanceWindow>[
          ...state.windows,
          MaintenanceWindow.fromJson(raw),
        ],
      );
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  Future<bool> updateWindow(int id, Map<String, dynamic> payload) async {
    state = state.copyWith(clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .put<Map<String, dynamic>>('/maintenance-windows/$id', data: payload);
      final updated = MaintenanceWindow.fromJson(raw);
      state = state.copyWith(
        windows: state.windows
            .map((w) => w.id == id ? updated : w)
            .toList(growable: false),
      );
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  Future<bool> deleteWindow(int id) async {
    state = state.copyWith(clearError: true);
    try {
      await ref
          .read(apiClientProvider)
          .delete<dynamic>('/maintenance-windows/$id');
      state = state.copyWith(
        windows: state.windows.where((w) => w.id != id).toList(growable: false),
      );
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }
}

final maintenanceWindowsProvider =
    NotifierProvider<MaintenanceWindowsNotifier, MaintenanceWindowsState>(
      MaintenanceWindowsNotifier.new,
    );
