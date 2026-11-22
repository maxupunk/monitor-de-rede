import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/models/pagination.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/features/core/models/admin_models.dart';

@immutable
class AuditState {
  const AuditState({
    this.filters = const AuditFilters(),
    this.entries = const <AuditLogEntry>[],
    this.meta,
    this.loading = false,
    this.error,
  });

  final AuditFilters filters;
  final List<AuditLogEntry> entries;
  final PaginationMeta? meta;
  final bool loading;
  final String? error;

  AuditState copyWith({
    AuditFilters? filters,
    List<AuditLogEntry>? entries,
    PaginationMeta? meta,
    bool clearMeta = false,
    bool? loading,
    String? error,
    bool clearError = false,
  }) {
    return AuditState(
      filters: filters ?? this.filters,
      entries: entries ?? this.entries,
      meta: clearMeta ? null : (meta ?? this.meta),
      loading: loading ?? this.loading,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class AuditNotifier extends Notifier<AuditState> {
  @override
  AuditState build() => const AuditState();

  Future<void> fetchPage({int page = 1, int limit = 20}) async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final f = state.filters;
      final params = <String, dynamic>{
        'page': page,
        'limit': limit,
        if (f.userId != null) 'userId': f.userId,
        if (f.resourceType != null && f.resourceType!.isNotEmpty)
          'resourceType': f.resourceType,
        if (f.action != null && f.action!.isNotEmpty) 'action': f.action,
        if (f.from != null && f.from!.isNotEmpty) 'from': f.from,
        if (f.to != null && f.to!.isNotEmpty) 'to': f.to,
        if (f.search.trim().isNotEmpty) 'q': f.search.trim(),
      };
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/audit-logs', queryParameters: params);
      final paginated = PaginatedResult<AuditLogEntry>.fromDynamic(
        raw,
        AuditLogEntry.fromJson,
        fallbackPerPage: limit,
      );
      state = state.copyWith(
        entries: paginated.data,
        meta: paginated.meta,
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(loading: false, error: describeError(err));
    }
  }

  Future<void> applyFilters(AuditFilters next) async {
    state = state.copyWith(filters: next, clearMeta: true);
    await fetchPage(page: 1);
  }

  Future<void> clearFilters() => applyFilters(const AuditFilters());
}

final auditProvider = NotifierProvider<AuditNotifier, AuditState>(
  AuditNotifier.new,
);
