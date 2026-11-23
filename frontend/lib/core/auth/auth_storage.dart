import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:netmonitor_app/core/auth/session_scoped_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

const String _tokenKey = 'netmonitor_auth_token';
const String _userKey = 'netmonitor_auth_user';
const String _rememberKey = 'netmonitor_auth_remember';

/// Persistência segura de sessão:
/// - "Permanecer logado" marcado: Android usa `FlutterSecureStorage` para o
///   token + `SharedPreferences` para metadados; Web usa `SharedPreferences`
///   (localStorage), mantendo compatibilidade em PWA.
/// - Desmarcado: token e usuário vivem só na sessão (`sessionStorage` na Web,
///   memória do processo no Android) e nada fica gravado no disco.
class AuthStorage {
  AuthStorage({FlutterSecureStorage? secureStorage})
    : _secureStorage = secureStorage ?? const FlutterSecureStorage();

  final FlutterSecureStorage _secureStorage;

  Future<({String? token, Map<String, dynamic>? user, bool rememberMe})>
  load() async {
    try {
      final prefs = await SharedPreferences.getInstance();
      final remember = prefs.getBool(_rememberKey) ?? true;

      final String? token;
      final String? rawUser;
      if (remember) {
        token = await _readPersistentToken(prefs);
        rawUser = prefs.getString(_userKey);
      } else {
        token = SessionScopedStore.read(_tokenKey);
        rawUser = SessionScopedStore.read(_userKey);
      }

      return (token: token, user: _decodeUser(rawUser), rememberMe: remember);
    } catch (_) {
      return (token: null, user: null, rememberMe: true);
    }
  }

  Future<void> save({
    required String token,
    required Map<String, dynamic> user,
    required bool rememberMe,
  }) async {
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setBool(_rememberKey, rememberMe);
      final encodedUser = jsonEncode(user);
      if (!rememberMe) {
        SessionScopedStore.write(_tokenKey, token);
        SessionScopedStore.write(_userKey, encodedUser);
        await _clearPersistent(prefs);
        return;
      }
      SessionScopedStore.remove(_tokenKey);
      SessionScopedStore.remove(_userKey);
      await prefs.setString(_userKey, encodedUser);
      if (kIsWeb) {
        await prefs.setString(_tokenKey, token);
      } else {
        try {
          await _secureStorage.write(key: _tokenKey, value: token);
        } catch (_) {
          await prefs.setString(_tokenKey, token);
        }
      }
    } catch (_) {}
  }

  Future<void> clear() async {
    SessionScopedStore.remove(_tokenKey);
    SessionScopedStore.remove(_userKey);
    try {
      final prefs = await SharedPreferences.getInstance();
      await _clearPersistent(prefs);
      await prefs.remove(_rememberKey);
    } catch (_) {}
  }

  Future<String?> _readPersistentToken(SharedPreferences prefs) async {
    if (kIsWeb) return prefs.getString(_tokenKey);
    try {
      return await _secureStorage.read(key: _tokenKey) ??
          prefs.getString(_tokenKey);
    } catch (_) {
      return prefs.getString(_tokenKey);
    }
  }

  Future<void> _clearPersistent(SharedPreferences prefs) async {
    await prefs.remove(_tokenKey);
    await prefs.remove(_userKey);
    if (!kIsWeb) {
      try {
        await _secureStorage.delete(key: _tokenKey);
      } catch (_) {}
    }
  }

  Map<String, dynamic>? _decodeUser(String? rawUser) {
    if (rawUser == null || rawUser.isEmpty) return null;
    try {
      final decoded = jsonDecode(rawUser);
      return decoded is Map ? Map<String, dynamic>.from(decoded) : null;
    } catch (_) {
      return null;
    }
  }
}
