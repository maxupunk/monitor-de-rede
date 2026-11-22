import 'dart:math' as math;

/// Conversores defensivos canônicos de JSON (DRY & DIP — AGENTS.md §10–11).
/// Centralizados em `lib/core/utils/` para eliminar duplicação entre áreas e
/// impedir acoplamento cruzado entre módulos de domínio (ADR 017).

int? parseIntOrNull(Object? value) {
  if (value == null) return null;
  if (value is int) return value;
  if (value is num) return value.isFinite ? value.toInt() : null;
  if (value is String) {
    final trimmed = value.trim();
    if (trimmed.isEmpty) return null;
    final asInt = int.tryParse(trimmed);
    if (asInt != null) return asInt;
    final asDouble = double.tryParse(trimmed);
    return (asDouble != null && asDouble.isFinite) ? asDouble.toInt() : null;
  }
  return null;
}

double? parseDoubleOrNull(Object? value) {
  if (value == null) return null;
  if (value is double) return value.isFinite ? value : null;
  if (value is num) {
    final d = value.toDouble();
    return d.isFinite ? d : null;
  }
  if (value is String) {
    final trimmed = value.trim();
    if (trimmed.isEmpty) return null;
    final parsed = double.tryParse(trimmed);
    return (parsed != null && parsed.isFinite) ? parsed : null;
  }
  return null;
}

bool parseBool(Object? value, {bool fallback = false}) {
  if (value == null) return fallback;
  if (value is bool) return value;
  if (value is num) return value != 0;
  if (value is String) {
    final lower = value.trim().toLowerCase();
    if (lower == 'true' || lower == '1' || lower == 'yes') return true;
    if (lower == 'false' || lower == '0' || lower == 'no') return false;
  }
  return fallback;
}

Map<String, dynamic> asJsonMap(Object? value) {
  if (value is Map<String, dynamic>) return value;
  if (value is Map) return Map<String, dynamic>.from(value);
  return const <String, dynamic>{};
}

List<Map<String, dynamic>> asJsonMapList(Object? value) {
  if (value is! List) return const <Map<String, dynamic>>[];
  return value.whereType<Map>().map(asJsonMap).toList(growable: false);
}

List<T> asJsonList<T>(
  Object? value,
  T Function(Map<String, dynamic> json) fromJson,
) {
  if (value is! List) return <T>[];
  return value
      .whereType<Map>()
      .map((raw) => fromJson(asJsonMap(raw)))
      .toList(growable: false);
}

List<String> asStringList(Object? value) {
  if (value is! List) return const <String>[];
  return value
      .where((item) => item != null)
      .map((item) => item.toString())
      .toList(growable: false);
}

List<int> asIntList(Object? value) {
  if (value is! List) return const <int>[];
  return value.map(parseIntOrNull).whereType<int>().toList(growable: false);
}

Map<String, String> asStringMap(Object? value) {
  if (value is! Map) return const <String, String>{};
  final out = <String, String>{};
  value.forEach((key, val) {
    if (key != null && val != null) {
      out[key.toString()] = val.toString();
    }
  });
  return Map<String, String>.unmodifiable(out);
}

/// Abrevia um hash Git SHA para exibição concisa (padrão 7 caracteres).
String shortGitSha(String? sha, {int length = 7, String fallback = '—'}) {
  if (sha == null) return fallback;
  final trimmed = sha.trim();
  if (trimmed.isEmpty) return fallback;
  final safeLen = math.max(1, math.min(length, trimmed.length));
  return trimmed.substring(0, safeLen);
}
