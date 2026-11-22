import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/wifi_models.dart';

const int _recentRoamsCap = 50;

@immutable
class WifiPingState {
  const WifiPingState({this.loading = false, this.result, this.error});

  final bool loading;
  final WifiPingResultModel? result;
  final String? error;
}

@immutable
class WifiRoamHistoryState {
  const WifiRoamHistoryState({
    this.loading = false,
    this.events = const <WifiRoamEventModel>[],
    this.error,
  });

  final bool loading;
  final List<WifiRoamEventModel> events;
  final String? error;
}

@immutable
class WifiClientsState {
  const WifiClientsState({
    this.snapshots = const <int, WifiDeviceSnapshotModel>{},
    this.watched = const <WifiWatchedClientModel>[],
    this.roams = const <WifiRoamEventModel>[],
    this.roamHistory = const <String, WifiRoamHistoryState>{},
    this.pings = const <String, WifiPingState>{},
    this.requesting = false,
    this.error,
  });

  final Map<int, WifiDeviceSnapshotModel> snapshots;
  final List<WifiWatchedClientModel> watched;
  final List<WifiRoamEventModel> roams;
  final Map<String, WifiRoamHistoryState> roamHistory;
  final Map<String, WifiPingState> pings;
  final bool requesting;
  final String? error;

  List<WifiDeviceSnapshotModel> get devices {
    final list = snapshots.values.toList()
      ..sort((a, b) => a.deviceName.compareTo(b.deviceName));
    return list;
  }

  List<WifiStationRow> get rows {
    final out = <WifiStationRow>[];
    for (final dev in devices) {
      for (final station in dev.stations) {
        out.add(
          WifiStationRow(
            deviceId: dev.deviceId,
            deviceName: dev.deviceName,
            canPing: dev.canPing,
            station: station,
          ),
        );
      }
    }
    return out;
  }

  bool get isReading => devices.any((d) => d.status == 'reading');

  Map<String, WifiWatchedClientModel> get watchedByMac =>
      <String, WifiWatchedClientModel>{
        for (final client in watched) client.mac.toLowerCase(): client,
      };

  WifiDeviceSnapshotModel? snapshotFor(int deviceId) => snapshots[deviceId];

  WifiPingState? pingFor(int deviceId, String mac) => pings['$deviceId:$mac'];

  WifiClientsState copyWith({
    Map<int, WifiDeviceSnapshotModel>? snapshots,
    List<WifiWatchedClientModel>? watched,
    List<WifiRoamEventModel>? roams,
    Map<String, WifiRoamHistoryState>? roamHistory,
    Map<String, WifiPingState>? pings,
    bool? requesting,
    String? error,
    bool clearError = false,
  }) {
    return WifiClientsState(
      snapshots: snapshots ?? this.snapshots,
      watched: watched ?? this.watched,
      roams: roams ?? this.roams,
      roamHistory: roamHistory ?? this.roamHistory,
      pings: pings ?? this.pings,
      requesting: requesting ?? this.requesting,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class WifiClientsNotifier extends Notifier<WifiClientsState> {
  @override
  WifiClientsState build() => const WifiClientsState();

  /// SSE `wifi:snapshot`
  void applySnapshot(Map<String, dynamic> data) {
    final parsed = WifiDeviceSnapshotModel.tryFromJson(data);
    if (parsed == null) return;
    state = state.copyWith(
      snapshots: <int, WifiDeviceSnapshotModel>{
        ...state.snapshots,
        parsed.deviceId: parsed,
      },
    );
  }

  /// SSE `wifi:watched`
  void applyWatched(Map<String, dynamic> data) {
    if (data['clients'] is! List) return;
    final parsed = asJsonMapList(
      data['clients'],
    ).map(WifiWatchedClientModel.fromJson).toList(growable: false);
    state = state.copyWith(watched: parsed);
  }

  /// SSE `wifi:roamed`
  void applyRoamed(Map<String, dynamic> data) {
    final event = WifiRoamEventModel.tryFromJson(data);
    if (event == null) return;
    if (state.roams.any((r) => r.id == event.id)) return;
    final nextRoams = <WifiRoamEventModel>[event, ...state.roams];
    if (nextRoams.length > _recentRoamsCap) nextRoams.length = _recentRoamsCap;

    var nextHistory = state.roamHistory;
    final existingHistory = state.roamHistory[event.mac];
    if (existingHistory != null &&
        !existingHistory.events.any((e) => e.id == event.id)) {
      nextHistory = <String, WifiRoamHistoryState>{
        ...state.roamHistory,
        event.mac: WifiRoamHistoryState(
          loading: existingHistory.loading,
          events: <WifiRoamEventModel>[event, ...existingHistory.events],
          error: existingHistory.error,
        ),
      };
    }

    state = state.copyWith(roams: nextRoams, roamHistory: nextHistory);
  }

  Future<bool> refresh({int? deviceId}) async {
    state = state.copyWith(requesting: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>(
            '/wifi/refresh',
            data: <String, dynamic>{'deviceId': ?deviceId},
          );
      final map = asJsonMap(raw);
      if (deviceId == null && map['deviceIds'] is List) {
        final keep = asIntList(map['deviceIds']).toSet();
        final filtered = <int, WifiDeviceSnapshotModel>{
          for (final entry in state.snapshots.entries)
            if (keep.contains(entry.key)) entry.key: entry.value,
        };
        state = state.copyWith(snapshots: filtered);
      }
      state = state.copyWith(requesting: false);
      return true;
    } catch (err) {
      state = state.copyWith(requesting: false, error: describeError(err));
      return false;
    }
  }

  Future<void> pingStation(WifiStationRow row) async {
    final ip = row.station.ip;
    if (ip == null || ip.isEmpty) return;
    final key = '${row.deviceId}:${row.station.mac}';
    state = state.copyWith(
      pings: <String, WifiPingState>{
        ...state.pings,
        key: const WifiPingState(loading: true),
      },
    );
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>(
            '/wifi/ping',
            timeout: const Duration(seconds: 30),
            data: <String, dynamic>{'deviceId': row.deviceId, 'target': ip},
          );
      final result = WifiPingResultModel.fromJson(asJsonMap(raw));
      state = state.copyWith(
        pings: <String, WifiPingState>{
          ...state.pings,
          key: WifiPingState(result: result),
        },
      );
    } catch (err) {
      state = state.copyWith(
        pings: <String, WifiPingState>{
          ...state.pings,
          key: WifiPingState(error: describeError(err)),
        },
      );
    }
  }

  Future<void> fetchRoaming(String mac) async {
    final prev = state.roamHistory[mac];
    state = state.copyWith(
      roamHistory: <String, WifiRoamHistoryState>{
        ...state.roamHistory,
        mac: WifiRoamHistoryState(
          loading: true,
          events: prev?.events ?? const <WifiRoamEventModel>[],
        ),
      },
    );
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>(
            '/wifi/roaming',
            queryParameters: <String, dynamic>{'mac': mac},
          );
      final events = asJsonMapList(raw)
          .map(WifiRoamEventModel.tryFromJson)
          .whereType<WifiRoamEventModel>()
          .toList(growable: false);
      state = state.copyWith(
        roamHistory: <String, WifiRoamHistoryState>{
          ...state.roamHistory,
          mac: WifiRoamHistoryState(events: events),
        },
      );
    } catch (err) {
      state = state.copyWith(
        roamHistory: <String, WifiRoamHistoryState>{
          ...state.roamHistory,
          mac: WifiRoamHistoryState(
            events: prev?.events ?? const <WifiRoamEventModel>[],
            error: describeError(err),
          ),
        },
      );
    }
  }

  Future<bool> watch(String mac, String name) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>(
            '/wifi/watched',
            data: <String, dynamic>{'mac': mac, 'name': name.trim()},
          );
      final saved = WifiWatchedClientModel.fromJson(asJsonMap(raw));
      final others = state.watched.where((w) => w.id != saved.id).toList();
      final next = <WifiWatchedClientModel>[...others, saved]
        ..sort((a, b) => a.name.compareTo(b.name));
      state = state.copyWith(watched: next);
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  Future<bool> renameWatched(int id, String name) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .patch<dynamic>(
            '/wifi/watched/$id',
            data: <String, dynamic>{'name': name.trim()},
          );
      final saved = WifiWatchedClientModel.fromJson(asJsonMap(raw));
      final next = state.watched.map((w) => w.id == id ? saved : w).toList()
        ..sort((a, b) => a.name.compareTo(b.name));
      state = state.copyWith(watched: next);
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  Future<bool> unwatch(int id) async {
    try {
      await ref.read(apiClientProvider).delete<dynamic>('/wifi/watched/$id');
      state = state.copyWith(
        watched: state.watched.where((w) => w.id != id).toList(growable: false),
      );
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  /// Revelação efêmera (`POST /api/devices/:id/plugins/:pluginId/reveal/:action`) — somente admin, nunca salva (ADR 012).
  Future<String?> revealWifiPassword({
    required int deviceId,
    required int pluginId,
    required String action,
    required String ssid,
  }) async {
    final raw = await ref
        .read(apiClientProvider)
        .post<dynamic>(
          '/devices/$deviceId/plugins/$pluginId/reveal/$action',
          timeout: const Duration(seconds: 30),
          data: <String, dynamic>{
            'params': <String, dynamic>{'ssid': ssid.trim()},
          },
        );
    final out = asJsonMap(asJsonMap(raw)['output']);
    return (out['password'] ?? out['psk'] ?? out['passphrase'])?.toString();
  }
}

final wifiClientsProvider =
    NotifierProvider<WifiClientsNotifier, WifiClientsState>(
      WifiClientsNotifier.new,
    );

// ── Wi-Fi Settings ────────────────────────────────────────────────────────────

@immutable
class WifiSettingsState {
  const WifiSettingsState({
    this.settings,
    this.loading = false,
    this.saving = false,
    this.error,
  });

  final WifiSettingsModel? settings;
  final bool loading;
  final bool saving;
  final String? error;

  WifiSettingsState copyWith({
    WifiSettingsModel? settings,
    bool? loading,
    bool? saving,
    String? error,
    bool clearError = false,
  }) {
    return WifiSettingsState(
      settings: settings ?? this.settings,
      loading: loading ?? this.loading,
      saving: saving ?? this.saving,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class WifiSettingsNotifier extends Notifier<WifiSettingsState> {
  @override
  WifiSettingsState build() => const WifiSettingsState();

  Future<void> fetch() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/wifi/settings');
      state = state.copyWith(
        settings: WifiSettingsModel.fromJson(asJsonMap(raw)),
        loading: false,
      );
    } catch (err) {
      state = state.copyWith(loading: false, error: describeError(err));
    }
  }

  Future<bool> save(WifiSettingsModel input) async {
    state = state.copyWith(saving: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .put<dynamic>('/wifi/settings', data: input.toJson());
      state = state.copyWith(
        settings: WifiSettingsModel.fromJson(asJsonMap(raw)),
        saving: false,
      );
      return true;
    } catch (err) {
      state = state.copyWith(saving: false, error: describeError(err));
      return false;
    }
  }
}

final wifiSettingsProvider =
    NotifierProvider<WifiSettingsNotifier, WifiSettingsState>(
      WifiSettingsNotifier.new,
    );
