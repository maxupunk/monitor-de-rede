import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/core/models/admin_models.dart';

@immutable
class UsersState {
  const UsersState({
    this.users = const <ManagedUser>[],
    this.loading = false,
    this.saving = false,
    this.error,
  });

  final List<ManagedUser> users;
  final bool loading;
  final bool saving;
  final String? error;

  UsersState copyWith({
    List<ManagedUser>? users,
    bool? loading,
    bool? saving,
    String? error,
    bool clearError = false,
  }) {
    return UsersState(
      users: users ?? this.users,
      loading: loading ?? this.loading,
      saving: saving ?? this.saving,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class UsersNotifier extends Notifier<UsersState> {
  @override
  UsersState build() => const UsersState();

  List<ManagedUser> _sorted(List<ManagedUser> list) {
    final copy = List<ManagedUser>.from(list)
      ..sort(
        (a, b) => a.fullName.toLowerCase().compareTo(b.fullName.toLowerCase()),
      );
    return copy;
  }

  Future<void> fetchUsers() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await ref.read(apiClientProvider).get<dynamic>('/users');
      state = state.copyWith(
        users: _sorted(asJsonList<ManagedUser>(raw, ManagedUser.fromJson)),
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(loading: false, error: describeError(err));
    }
  }

  Future<bool> createUser(Map<String, dynamic> payload) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<Map<String, dynamic>>('/users', data: payload);
      final created = ManagedUser.fromJson(raw);
      state = state.copyWith(
        users: _sorted(<ManagedUser>[...state.users, created]),
        saving: false,
      );
      return true;
    } catch (err) {
      state = state.copyWith(saving: false, error: describeError(err));
      return false;
    }
  }

  Future<bool> updateUser(int id, Map<String, dynamic> payload) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .put<Map<String, dynamic>>('/users/$id', data: payload);
      final updated = ManagedUser.fromJson(raw);
      state = state.copyWith(
        users: _sorted(
          state.users.map((u) => u.id == id ? updated : u).toList(),
        ),
        saving: false,
      );
      return true;
    } catch (err) {
      state = state.copyWith(saving: false, error: describeError(err));
      return false;
    }
  }

  Future<bool> deleteUser(int id) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      await ref.read(apiClientProvider).delete<dynamic>('/users/$id');
      state = state.copyWith(
        users: state.users.where((u) => u.id != id).toList(growable: false),
        saving: false,
      );
      return true;
    } catch (err) {
      state = state.copyWith(saving: false, error: describeError(err));
      return false;
    }
  }
}

final usersProvider = NotifierProvider<UsersNotifier, UsersState>(
  UsersNotifier.new,
);
