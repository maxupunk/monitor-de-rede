import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:shared_preferences/shared_preferences.dart';

const String _themeModePrefKey = 'netmonitor_theme_mode';

/// Preferência de tema (Claro, Escuro ou Sistema), persistida localmente.
class ThemePreferenceNotifier extends Notifier<ThemeMode> {
  @override
  ThemeMode build() {
    Future<void>.microtask(_load);
    return ThemeMode.dark;
  }

  Future<void> _load() async {
    try {
      final prefs = await SharedPreferences.getInstance();
      final saved = prefs.getString(_themeModePrefKey);
      if (saved != null) {
        state = _decode(saved);
      }
    } catch (_) {
      // Mantém o padrão escuro se storage indisponível (ex.: ambiente de teste).
    }
  }

  Future<void> setThemeMode(ThemeMode mode) async {
    state = mode;
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setString(_themeModePrefKey, _encode(mode));
    } catch (_) {}
  }

  String _encode(ThemeMode mode) => switch (mode) {
    ThemeMode.light => 'light',
    ThemeMode.dark => 'dark',
    ThemeMode.system => 'system',
  };

  ThemeMode _decode(String value) => switch (value) {
    'light' => ThemeMode.light,
    'system' => ThemeMode.system,
    _ => ThemeMode.dark,
  };
}

final themePreferenceProvider =
    NotifierProvider<ThemePreferenceNotifier, ThemeMode>(
      ThemePreferenceNotifier.new,
    );
