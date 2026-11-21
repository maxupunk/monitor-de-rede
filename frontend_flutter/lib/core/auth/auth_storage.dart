import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:shared_preferences/shared_preferences.dart';

const String _tokenKey = 'netmonitor_auth_token';
const String _userKey = 'netmonitor_auth_user';
const String _rememberKey = 'netmonitor_auth_remember';

/// Persistência segura de sessão:
/// - Android: `FlutterSecureStorage` para o token + `SharedPreferences` para metadados.
/// - Web: `SharedPreferences` (localStorage), mantendo compatibilidade em PWA.
class AuthStorage {
  AuthStorage({FlutterSecureStorage? secureStorage})
      : _secureStorage = secureStorage ?? const FlutterSecureStorage();

  final FlutterSecureStorage _secureStorage;

  Future<({String? token, Map<String, dynamic>? user, bool rememberMe})> load() async {
    try {
      final prefs = await SharedPreferences.getInstance();
      final remember = prefs.getBool(_rememberKey) ?? true;

      String? token;
      if (kIsWeb) {
        token = prefs.getString(_tokenKey);
      } else {
        try {
          token = await _secureStorage.read(key: _tokenKey);
        } catch (_) {
          token = prefs.getString(_tokenKey);
        }
      }

      Map<String, dynamic>? user;
      final rawUser = prefs.getString(_userKey);
      if (rawUser != null && rawUser.isNotEmpty) {
        try {
          final decoded = jsonDecode(rawUser);
          if (decoded is Map) {
            user = Map<String, dynamic>.from(decoded);
          }
        } catch (_) {}
      }

      return (token: token, user: user, rememberMe: remember);
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
      await prefs.setString(_userKey, jsonEncode(user));
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
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.remove(_tokenKey);
      await prefs.remove(_userKey);
      await prefs.remove(_rememberKey);
      if (!kIsWeb) {
        try {
          await _secureStorage.delete(key: _tokenKey);
        } catch (_) {}
      }
    } catch (_) {}
  }
}
