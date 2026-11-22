import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_storage.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

/// Usuário autenticado na sessão atual.
@immutable
class AuthUser {
  const AuthUser({
    required this.id,
    required this.email,
    this.fullName,
    this.role,
    this.areaRoles = const <ProductArea, UserRole>{},
    this.menuPrefs = MenuPrefs.empty,
  });

  final int id;
  final String email;
  final String? fullName;
  final UserRole? role;
  final AreaRoles areaRoles;
  final MenuPrefs menuPrefs;

  String get displayName => (fullName != null && fullName!.trim().isNotEmpty)
      ? fullName!.trim()
      : email;

  AuthUser copyWith({
    int? id,
    String? email,
    String? fullName,
    UserRole? role,
    AreaRoles? areaRoles,
    MenuPrefs? menuPrefs,
  }) {
    return AuthUser(
      id: id ?? this.id,
      email: email ?? this.email,
      fullName: fullName ?? this.fullName,
      role: role ?? this.role,
      areaRoles: areaRoles ?? this.areaRoles,
      menuPrefs: menuPrefs ?? this.menuPrefs,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
    'id': id,
    'email': email,
    if (fullName != null) 'fullName': fullName,
    if (role != null) 'role': role!.id,
    'areaRoles': <String, String>{
      for (final entry in areaRoles.entries) entry.key.id: entry.value.id,
    },
    'menuPrefs': menuPrefs.toJson(),
  };

  static AuthUser? tryFromJson(Object? raw) {
    if (raw is! Map) return null;
    final map = asJsonMap(raw);
    final inner = map['user'] is Map ? asJsonMap(map['user']) : map;
    final id = parseIntOrNull(inner['id']);
    final email = inner['email'];
    if (id == null || email is! String) return null;

    return AuthUser(
      id: id,
      email: email,
      fullName: (inner['fullName'] ?? inner['name']) as String?,
      role: UserRole.tryParse(inner['role']),
      areaRoles: parseAreaRoles(inner['areaRoles']),
      menuPrefs: MenuPrefs.tryParse(inner['menuPrefs']) ?? MenuPrefs.empty,
    );
  }
}

/// Estado global de autenticação e controle de acesso por área (ADR 019).
@immutable
class AuthState {
  const AuthState({
    this.token,
    this.user,
    this.rememberMe = true,
    this.loading = false,
    this.savingMenu = false,
    this.error,
    this.needsSetup,
    this.currentArea = ProductArea.core,
    this.initialized = false,
  });

  final String? token;
  final AuthUser? user;
  final bool rememberMe;
  final bool loading;
  final bool savingMenu;
  final String? error;
  final bool? needsSetup;
  final ProductArea currentArea;
  final bool initialized;

  bool get isAuthenticated => token != null && token!.isNotEmpty;
  bool get isAdmin => user?.role == UserRole.admin;
  bool get canManagePlugins => isAdmin;
  String get currentRoleLabel => roleLabel(user?.role);

  UserRole? roleIn(ProductArea area) =>
      roleInArea(user?.role, user?.areaRoles, area);

  /// Sem usuário carregado ainda, nada some da tela (`fetchMe` traz em seguida).
  bool hasAccess(ProductArea area) => user == null || roleIn(area) != null;

  /// Alias explícito para leitura da área (ADR 019).
  bool canRead(ProductArea area) => hasAccess(area);

  /// Pode alterar dados numa área específica?
  bool canWriteIn(ProductArea area) => roleCanWrite(roleIn(area));

  /// Pode alterar dados na área da tela atual?
  bool get canWrite => canWriteIn(currentArea);

  MenuPrefs get menuPrefs => user?.menuPrefs ?? MenuPrefs.empty;

  AuthState copyWith({
    String? token,
    bool clearToken = false,
    AuthUser? user,
    bool clearUser = false,
    bool? rememberMe,
    bool? loading,
    bool? savingMenu,
    String? error,
    bool clearError = false,
    bool? needsSetup,
    bool clearNeedsSetup = false,
    ProductArea? currentArea,
    bool? initialized,
  }) {
    return AuthState(
      token: clearToken ? null : (token ?? this.token),
      user: clearUser ? null : (user ?? this.user),
      rememberMe: rememberMe ?? this.rememberMe,
      loading: loading ?? this.loading,
      savingMenu: savingMenu ?? this.savingMenu,
      error: clearError ? null : (error ?? this.error),
      needsSetup: clearNeedsSetup ? null : (needsSetup ?? this.needsSetup),
      currentArea: currentArea ?? this.currentArea,
      initialized: initialized ?? this.initialized,
    );
  }
}

class AuthNotifier extends Notifier<AuthState> {
  final AuthStorage _storage = AuthStorage();

  @override
  AuthState build() {
    ref.listen<int>(sessionExpiredSignalProvider, (prev, next) {
      if (prev != null && next != prev && state.isAuthenticated) {
        _clearLocalSession();
      }
    });
    return const AuthState();
  }

  /// Restaura sessão armazenada localmente.
  Future<void> initialize() async {
    final loaded = await _storage.load();
    final parsedUser = AuthUser.tryFromJson(loaded.user);
    if (loaded.token != null && loaded.token!.isNotEmpty) {
      ref.read(sessionTokenProvider.notifier).state = loaded.token;
    }
    state = state.copyWith(
      token: loaded.token,
      user: parsedUser,
      rememberMe: loaded.rememberMe,
      initialized: true,
    );
  }

  void clearError() {
    state = state.copyWith(clearError: true);
  }

  void setCurrentArea(ProductArea area) {
    if (state.currentArea == area) return;
    state = state.copyWith(currentArea: area);
  }

  Future<bool> refreshSetupStatus() async {
    try {
      final response = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>('/auth/setup');
      final needs = response['needsSetup'] == true;
      state = state.copyWith(needsSetup: needs);
      return needs;
    } catch (_) {
      state = state.copyWith(needsSetup: false);
      return false;
    }
  }

  Future<bool> ensureSetupStatus() async {
    if (state.needsSetup != null) return state.needsSetup!;
    return refreshSetupStatus();
  }

  Future<bool> login(
    String email,
    String password, {
    bool rememberMe = true,
  }) async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final response = await ref
          .read(apiClientProvider)
          .post<Map<String, dynamic>>(
            '/auth/login',
            data: <String, dynamic>{
              'email': email.trim(),
              'password': password,
            },
          );
      final token = response['token'] as String?;
      final user = AuthUser.tryFromJson(response['user']);
      if (token == null || token.isEmpty || user == null) {
        state = state.copyWith(
          loading: false,
          error: 'Resposta inesperada do servidor de autenticação.',
        );
        return false;
      }
      await _persistSession(token: token, user: user, rememberMe: rememberMe);
      state = state.copyWith(
        token: token,
        user: user,
        rememberMe: rememberMe,
        needsSetup: false,
        loading: false,
        clearError: true,
      );
      return true;
    } catch (err) {
      state = state.copyWith(
        loading: false,
        error: _describeError(err, 'Falha na autenticação.'),
      );
      return false;
    }
  }

  Future<bool> completeSetup({
    required String name,
    required String email,
    required String password,
    required String token,
  }) async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final response = await ref
          .read(apiClientProvider)
          .post<Map<String, dynamic>>(
            '/auth/setup',
            data: <String, dynamic>{
              'name': name.trim(),
              'email': email.trim(),
              'password': password,
              'token': token.trim(),
            },
          );
      final sessionToken = response['token'] as String?;
      final user = AuthUser.tryFromJson(response['user']);
      if (sessionToken == null || sessionToken.isEmpty || user == null) {
        state = state.copyWith(
          loading: false,
          error: 'Resposta inesperada ao concluir a instalação.',
        );
        return false;
      }
      await _persistSession(token: sessionToken, user: user, rememberMe: true);
      state = state.copyWith(
        token: sessionToken,
        user: user,
        rememberMe: true,
        needsSetup: false,
        loading: false,
        clearError: true,
      );
      return true;
    } catch (err) {
      state = state.copyWith(
        loading: false,
        error: _describeError(err, 'Falha ao concluir a instalação.'),
      );
      return false;
    }
  }

  Future<void> fetchMe() async {
    final token = state.token;
    if (token == null || token.isEmpty) return;
    try {
      final response = await ref
          .read(apiClientProvider)
          .get<dynamic>('/auth/me');
      final user = AuthUser.tryFromJson(response);
      if (user != null) {
        state = state.copyWith(user: user);
        await _persistSession(
          token: token,
          user: user,
          rememberMe: state.rememberMe,
        );
      }
    } catch (err) {
      if (err is ApiException && err.isUnauthorized) {
        await logout();
      }
    }
  }

  /// Atualiza o acesso do usuário em tempo real via SSE (`user:access_updated`).
  void applyAccess(Map<String, dynamic> payload) {
    final current = state.user;
    if (current == null) return;
    final userId = payload['userId'];
    if (userId is! num || userId.toInt() != current.id) return;

    final nextRole = UserRole.tryParse(payload['role']) ?? current.role;
    final nextAreaRoles = parseAreaRoles(payload['areaRoles']);
    final updated = current.copyWith(role: nextRole, areaRoles: nextAreaRoles);
    state = state.copyWith(user: updated);
    if (state.token != null) {
      _storage.save(
        token: state.token!,
        user: updated.toJson(),
        rememberMe: state.rememberMe,
      );
    }
  }

  /// Atualiza o menu pessoal via SSE (`user:menu_updated`).
  void applyMenuPrefs(Map<String, dynamic> payload) {
    final current = state.user;
    if (current == null) return;
    final userId = payload['userId'];
    if (userId is! num || userId.toInt() != current.id) return;
    final parsed = MenuPrefs.tryParse(payload);
    if (parsed == null) return;
    _updateMenuPrefs(parsed);
  }

  /// Salva preferências pessoais do menu (`PUT /api/me/menu`).
  Future<void> saveMenuPrefs(MenuPrefs prefs) async {
    state = state.copyWith(savingMenu: true);
    try {
      final response = await ref
          .read(apiClientProvider)
          .put<Map<String, dynamic>>('/me/menu', data: prefs.toJson());
      final saved = MenuPrefs.tryParse(response) ?? prefs;
      _updateMenuPrefs(saved);
    } finally {
      state = state.copyWith(savingMenu: false);
    }
  }

  void _updateMenuPrefs(MenuPrefs prefs) {
    final current = state.user;
    if (current == null) return;
    final updated = current.copyWith(menuPrefs: prefs);
    state = state.copyWith(user: updated);
    if (state.token != null) {
      _storage.save(
        token: state.token!,
        user: updated.toJson(),
        rememberMe: state.rememberMe,
      );
    }
  }

  Future<void> logout() async {
    final token = state.token;
    try {
      if (token != null && token.isNotEmpty) {
        await ref.read(apiClientProvider).post<dynamic>('/auth/logout');
      }
    } catch (_) {
      // Limpeza local continua obrigatória mesmo se o token já expirou.
    } finally {
      await _clearLocalSession();
    }
  }

  Future<void> _persistSession({
    required String token,
    required AuthUser user,
    required bool rememberMe,
  }) async {
    ref.read(sessionTokenProvider.notifier).state = token;
    await _storage.save(
      token: token,
      user: user.toJson(),
      rememberMe: rememberMe,
    );
  }

  Future<void> _clearLocalSession() async {
    ref.read(sessionTokenProvider.notifier).state = null;
    await _storage.clear();
    state = state.copyWith(
      clearToken: true,
      clearUser: true,
      clearError: true,
      currentArea: ProductArea.core,
    );
  }

  String _describeError(Object err, String fallback) =>
      describeError(err, fallback: fallback);
}

final authProvider = NotifierProvider<AuthNotifier, AuthState>(
  AuthNotifier.new,
);
