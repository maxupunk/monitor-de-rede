import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
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
      List<HistoricalEventRecord> list;
      PaginationMeta? meta;
      if (raw is Map) {
        final data = raw['data'] is List ? raw['data'] as List : const [];
        list = data
            .whereType<Map>()
            .map(
              (e) =>
                  HistoricalEventRecord.fromJson(Map<String, dynamic>.from(e)),
            )
            .toList(growable: false);
        if (raw['meta'] is Map) {
          meta = PaginationMeta.fromJson(
            Map<String, dynamic>.from(raw['meta'] as Map),
          );
        }
      } else if (raw is List) {
        list = raw
            .whereType<Map>()
            .map(
              (e) =>
                  HistoricalEventRecord.fromJson(Map<String, dynamic>.from(e)),
            )
            .toList(growable: false);
      } else {
        list = const <HistoricalEventRecord>[];
      }
      state = state.copyWith(
        items: append ? <HistoricalEventRecord>[...state.items, ...list] : list,
        meta: meta,
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(loading: false, error: err.toString());
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
          : (raw is Map && raw['buckets'] is List
                ? raw['buckets'] as List
                : (raw is Map && raw['data'] is List
                      ? raw['data'] as List
                      : const []));
      return list
          .whereType<Map>()
          .map((e) => HourlyEventBucket.fromJson(Map<String, dynamic>.from(e)))
          .toList(growable: false);
    } catch (_) {
      return const <HourlyEventBucket>[];
    }
  }
}

final eventsHistoryProvider =
    NotifierProvider<EventsHistoryNotifier, EventsHistoryState>(
      EventsHistoryNotifier.new,
    );
