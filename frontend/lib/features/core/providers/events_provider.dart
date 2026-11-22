import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/core/models/dashboard_models.dart';
import 'package:netmonitor_app/features/core/models/event_models.dart';

@immutable
class EventsHistoryState {
  const EventsHistoryState({
    this.items = const <HistoricalEventRecord>[],
    this.meta,
    this.loading = false,
    this.error,
  });

  final List<HistoricalEventRecord> items;
  final PaginationMeta? meta;
  final bool loading;
  final String? error;

  EventsHistoryState copyWith({
    List<HistoricalEventRecord>? items,
    PaginationMeta? meta,
    bool? loading,
    String? error,
    bool clearError = false,
  }) {
    return EventsHistoryState(
      items: items ?? this.items,
      meta: meta ?? this.meta,
      loading: loading ?? this.loading,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class EventsHistoryNotifier extends Notifier<EventsHistoryState> {
  @override
  EventsHistoryState build() => const EventsHistoryState();

  Future<void> fetchPage({
    int page = 1,
    int limit = 50,
    bool append = false,
  }) async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>(
            '/events',
            queryParameters: <String, dynamic>{'page': page, 'limit': limit},
          );
      final paginated = PaginatedResult<HistoricalEventRecord>.fromDynamic(
        raw,
        HistoricalEventRecord.fromJson,
        fallbackPerPage: limit,
      );
      state = state.copyWith(
        items: append
            ? <HistoricalEventRecord>[...state.items, ...paginated.data]
            : paginated.data,
        meta: raw is Map && raw['meta'] is Map ? paginated.meta : null,
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(loading: false, error: describeError(err));
    }
  }

  Future<List<HourlyEventBucket>> fetchHourlyDistribution({
    int hours = 24,
  }) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>(
            '/events/hourly-distribution',
            queryParameters: <String, dynamic>{'hours': hours},
          );
      final list = raw is List
          ? raw
          : (raw is Map ? (raw['buckets'] ?? raw['data']) : null);
      return asJsonList<HourlyEventBucket>(list, HourlyEventBucket.fromJson);
    } catch (_) {
      return const <HourlyEventBucket>[];
    }
  }
}

final eventsHistoryProvider =
    NotifierProvider<EventsHistoryNotifier, EventsHistoryState>(
      EventsHistoryNotifier.new,
    );
