import 'dart:convert';

import 'package:netmonitor_app/features/network/topology/engine/topology_analysis.dart'
    show TopologyFilterSpec;
import 'package:shared_preferences/shared_preferences.dart';

/// Persistência resiliente das preferências visuais e filtros da topologia.
class TopologyPrefsStorage {
  const TopologyPrefsStorage._();

  static const String storageKey = 'netmonitor.topology.prefs.v3';

  /// Carrega as preferências salvas; devolve o padrão seguro em caso de erro.
  static Future<TopologyFilterSpec> load({SharedPreferences? prefs}) async {
    try {
      final store = prefs ?? await SharedPreferences.getInstance();
      final raw = store.getString(storageKey);
      if (raw == null || raw.trim().isEmpty) {
        return const TopologyFilterSpec();
      }
      final decoded = jsonDecode(raw);
      if (decoded is! Map) return const TopologyFilterSpec();
      return TopologyFilterSpec.fromJson(Map<String, dynamic>.from(decoded));
    } catch (_) {
      return const TopologyFilterSpec();
    }
  }

  /// Salva as preferências no armazenamento local sem lançar exceções de I/O.
  static Future<void> save(
    TopologyFilterSpec spec, {
    SharedPreferences? prefs,
  }) async {
    try {
      final store = prefs ?? await SharedPreferences.getInstance();
      await store.setString(storageKey, jsonEncode(spec.toJson()));
    } catch (_) {
      // Armazenamento indisponível (ex.: aba privada): segue em memória.
    }
  }

  /// Remove as preferências persistidas.
  static Future<void> clear({SharedPreferences? prefs}) async {
    try {
      final store = prefs ?? await SharedPreferences.getInstance();
      await store.remove(storageKey);
    } catch (_) {}
  }
}
