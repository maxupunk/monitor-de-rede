import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
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
      final list = raw is List
          ? raw
                .whereType<Map>()
                .map(
                  (e) =>
                      MaintenanceWindow.fromJson(Map<String, dynamic>.from(e)),
                )
                .toList(growable: false)
          : const <MaintenanceWindow>[];
      state = state.copyWith(windows: list, loading: false);
    } catch (err) {
      state = state.copyWith(loading: false, error: err.toString());
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
      state = state.copyWith(error: err.toString());
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
      state = state.copyWith(error: err.toString());
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
      state = state.copyWith(error: err.toString());
      return false;
    }
  }
}

final maintenanceWindowsProvider =
    NotifierProvider<MaintenanceWindowsNotifier, MaintenanceWindowsState>(
      MaintenanceWindowsNotifier.new,
    );
