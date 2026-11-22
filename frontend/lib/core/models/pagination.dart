import 'package:flutter/foundation.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';

/// Metadados padronizados de paginação retornados pelo backend (Loco.rs / SeaORM).
@immutable
class PaginationMeta {
  const PaginationMeta({
    required this.total,
    required this.perPage,
    required this.currentPage,
    required this.lastPage,
    this.firstPage = 1,
  });

  final int total;
  final int perPage;
  final int currentPage;
  final int lastPage;
  final int firstPage;

  bool get hasMore => currentPage < lastPage;
  bool get hasNext => hasMore;

  factory PaginationMeta.fromJson(Map<String, dynamic> json) {
    return PaginationMeta(
      total: parseIntOrNull(json['total']) ?? 0,
      perPage: parseIntOrNull(json['perPage'] ?? json['per_page']) ?? 25,
      currentPage:
          parseIntOrNull(json['currentPage'] ?? json['current_page']) ?? 1,
      lastPage: parseIntOrNull(json['lastPage'] ?? json['last_page']) ?? 1,
      firstPage: parseIntOrNull(json['firstPage'] ?? json['first_page']) ?? 1,
    );
  }
}

/// Envelope paginado genérico aceitando tanto `{ data: [...], meta: {...} }`
/// quanto uma `List` direta.
@immutable
class PaginatedResult<T> {
  const PaginatedResult({required this.data, required this.meta});

  final List<T> data;
  final PaginationMeta meta;

  factory PaginatedResult.fromDynamic(
    Object? raw,
    T Function(Map<String, dynamic> json) fromJson, {
    int fallbackPerPage = 25,
  }) {
    if (raw is List) {
      final items = asJsonList<T>(raw, fromJson);
      return PaginatedResult<T>(
        data: items,
        meta: PaginationMeta(
          total: items.length,
          perPage: items.isEmpty ? fallbackPerPage : items.length,
          currentPage: 1,
          lastPage: 1,
        ),
      );
    }

    final map = asJsonMap(raw);
    final rawItems = map['data'] ?? map['items'] ?? map['rows'];
    final items = asJsonList<T>(rawItems, fromJson);
    final rawMeta = map['meta'] ?? map['pagination'];
    final meta = rawMeta is Map
        ? PaginationMeta.fromJson(asJsonMap(rawMeta))
        : PaginationMeta(
            total: parseIntOrNull(map['total']) ?? items.length,
            perPage:
                parseIntOrNull(map['perPage'] ?? map['limit']) ??
                (items.isEmpty ? fallbackPerPage : items.length),
            currentPage: parseIntOrNull(map['currentPage'] ?? map['page']) ?? 1,
            lastPage: parseIntOrNull(map['lastPage'] ?? map['totalPages']) ?? 1,
          );
    return PaginatedResult<T>(data: items, meta: meta);
  }
}
