import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/link_test_models.dart';

const int linkTestSampleLimit = 120;
const int linkTestHealthLimit = 480;
const int linkTestHistoryPerPage = 20;

String linkTestSampleKey(int testId, int legSeq, String flow) =>
    '$testId:$legSeq:$flow';
String linkTestHealthKey(int testId, int legSeq) => '$testId:$legSeq';

@immutable
class LinkTestsState {
  const LinkTestsState({
    this.tests = const <int, LinkTestModel>{},
    this.samples = const <String, List<LinkTestSampleModel>>{},
    this.health = const <String, List<LinkTestHealthSampleModel>>{},
    this.pageItems = const <LinkTestModel>[],
    this.page = 1,
    this.total = 0,
    this.pageLoading = false,
    this.pageError,
    this.schedules = const <LinkTestScheduleModel>[],
    this.schedulesLoaded = false,
    this.schedulesLoading = false,
    this.schedulesError,
    this.focusedId,
  });

  final Map<int, LinkTestModel> tests;
  final Map<String, List<LinkTestSampleModel>> samples;
  final Map<String, List<LinkTestHealthSampleModel>> health;
  final List<LinkTestModel> pageItems;
  final int page;
  final int total;
  final bool pageLoading;
  final String? pageError;
  final List<LinkTestScheduleModel> schedules;
  final bool schedulesLoaded;
  final bool schedulesLoading;
  final String? schedulesError;
  final int? focusedId;

  List<LinkTestModel> get allSorted {
    final list = tests.values.toList()
      ..sort((a, b) {
        final cmp = b.createdAt.compareTo(a.createdAt);
        return cmp != 0 ? cmp : b.id.compareTo(a.id);
      });
    return list;
  }

  List<LinkTestModel> get activeTests =>
      allSorted.where((t) => t.isActive).toList(growable: false);

  LinkTestModel? get focused => focusedId != null ? tests[focusedId!] : null;

  List<LinkTestSampleModel> samplesFor(int testId, int legSeq, String flow) =>
      samples[linkTestSampleKey(testId, legSeq, flow)] ??
      const <LinkTestSampleModel>[];

  List<LinkTestHealthSampleModel> healthFor(int testId, int legSeq) =>
      health[linkTestHealthKey(testId, legSeq)] ??
      const <LinkTestHealthSampleModel>[];

  LinkTestsState copyWith({
    Map<int, LinkTestModel>? tests,
    Map<String, List<LinkTestSampleModel>>? samples,
    Map<String, List<LinkTestHealthSampleModel>>? health,
    List<LinkTestModel>? pageItems,
    int? page,
    int? total,
    bool? pageLoading,
    String? pageError,
    bool clearPageError = false,
    List<LinkTestScheduleModel>? schedules,
    bool? schedulesLoaded,
    bool? schedulesLoading,
    String? schedulesError,
    bool clearSchedulesError = false,
    int? focusedId,
    bool clearFocusedId = false,
  }) {
    return LinkTestsState(
      tests: tests ?? this.tests,
      samples: samples ?? this.samples,
      health: health ?? this.health,
      pageItems: pageItems ?? this.pageItems,
      page: page ?? this.page,
      total: total ?? this.total,
      pageLoading: pageLoading ?? this.pageLoading,
      pageError: clearPageError ? null : (pageError ?? this.pageError),
      schedules: schedules ?? this.schedules,
      schedulesLoaded: schedulesLoaded ?? this.schedulesLoaded,
      schedulesLoading: schedulesLoading ?? this.schedulesLoading,
      schedulesError: clearSchedulesError
          ? null
          : (schedulesError ?? this.schedulesError),
      focusedId: clearFocusedId ? null : (focusedId ?? this.focusedId),
    );
  }
}

class LinkTestsNotifier extends Notifier<LinkTestsState> {
  @override
  LinkTestsState build() => const LinkTestsState();

  void focus(int? id) {
    state = id == null
        ? state.copyWith(clearFocusedId: true)
        : state.copyWith(focusedId: id);
  }

  Future<LinkTestPlanModel> plan(Map<String, dynamic> request) async {
    final body = Map<String, dynamic>.from(request)..remove('confirm');
    final raw = await ref
        .read(apiClientProvider)
        .post<dynamic>(
          '/link-tests/plan',
          data: body,
          timeout: const Duration(seconds: 30),
        );
    return LinkTestPlanModel.fromJson(asJsonMap(raw));
  }

  Future<int> start(Map<String, dynamic> request) async {
    final body = <String, dynamic>{...request, 'confirm': true};
    final raw = await ref
        .read(apiClientProvider)
        .post<dynamic>(
          '/link-tests',
          data: body,
          timeout: const Duration(seconds: 30),
        );
    final map = asJsonMap(raw);
    final parsed = LinkTestModel.tryFromJson(map);
    if (parsed != null) {
      _upsert(parsed);
      focus(parsed.id);
      return parsed.id;
    }
    final id = parseIntOrNull(map['id']) ?? 0;
    if (id > 0) focus(id);
    return id;
  }

  Future<void> cancel(int id) async {
    try {
      await ref
          .read(apiClientProvider)
          .post<dynamic>(
            '/link-tests/$id/cancel',
            data: const <String, dynamic>{},
          );
    } on ApiException catch (err) {
      if (err.statusCode != 404 && err.statusCode != 409) rethrow;
    }
  }

  Future<void> fetchPage({
    int page = 1,
    int perPage = linkTestHistoryPerPage,
    int? deviceId,
    int? linkId,
    String? status,
    String? kind,
  }) async {
    state = state.copyWith(pageLoading: true, clearPageError: true);
    try {
      final query = <String, dynamic>{
        'page': page,
        'perPage': perPage,
        'deviceId': ?deviceId,
        'linkId': ?linkId,
        if (status != null && status.isNotEmpty) 'status': status,
        if (kind != null && kind.isNotEmpty) 'kind': kind,
      };
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/link-tests', queryParameters: query);
      final map = asJsonMap(raw);
      final items = asJsonMapList(map['items'])
          .map(LinkTestModel.tryFromJson)
          .whereType<LinkTestModel>()
          .toList(growable: false);
      final merged = Map<int, LinkTestModel>.from(state.tests);
      for (final item in items) {
        merged[item.id] = item;
      }
      state = state.copyWith(
        tests: merged,
        pageItems: items,
        page: parseIntOrNull(map['page']) ?? page,
        total: parseIntOrNull(map['total']) ?? items.length,
        pageLoading: false,
      );
    } catch (err) {
      state = state.copyWith(pageLoading: false, pageError: err.toString());
    }
  }

  Future<LinkTestModel?> fetchDetail(int id) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/link-tests/$id');
      final parsed = LinkTestModel.tryFromJson(raw);
      if (parsed == null) return null;
      _seedSamples(parsed);
      _upsert(parsed);
      return parsed;
    } catch (_) {
      return null;
    }
  }

  Future<void> fetchSchedules() async {
    state = state.copyWith(schedulesLoading: true, clearSchedulesError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>('/link-test-schedules');
      final list = asJsonMapList(
        raw,
      ).map(LinkTestScheduleModel.fromJson).toList(growable: false);
      state = state.copyWith(
        schedules: list,
        schedulesLoaded: true,
        schedulesLoading: false,
      );
    } catch (err) {
      state = state.copyWith(
        schedulesLoading: false,
        schedulesError: err.toString(),
      );
    }
  }

  Future<bool> createSchedule(Map<String, dynamic> input) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>('/link-test-schedules', data: input);
      final saved = LinkTestScheduleModel.fromJson(asJsonMap(raw));
      state = state.copyWith(
        schedules: <LinkTestScheduleModel>[...state.schedules, saved],
      );
      return true;
    } catch (err) {
      state = state.copyWith(schedulesError: err.toString());
      return false;
    }
  }

  Future<bool> updateSchedule(int id, Map<String, dynamic> input) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .put<dynamic>('/link-test-schedules/$id', data: input);
      final saved = LinkTestScheduleModel.fromJson(asJsonMap(raw));
      state = state.copyWith(
        schedules: state.schedules
            .map((s) => s.id == id ? saved : s)
            .toList(growable: false),
      );
      return true;
    } catch (err) {
      state = state.copyWith(schedulesError: err.toString());
      return false;
    }
  }

  Future<bool> deleteSchedule(int id) async {
    try {
      await ref
          .read(apiClientProvider)
          .delete<dynamic>('/link-test-schedules/$id');
      state = state.copyWith(
        schedules: state.schedules
            .where((s) => s.id != id)
            .toList(growable: false),
      );
      return true;
    } catch (err) {
      state = state.copyWith(schedulesError: err.toString());
      return false;
    }
  }

  Future<int?> runScheduleNow(LinkTestScheduleModel schedule) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<dynamic>(
            '/link-test-schedules/${schedule.id}/run',
            data: const <String, dynamic>{},
          );
      final id = parseIntOrNull(asJsonMap(raw)['id']);
      if (id != null) focus(id);
      return id;
    } on ApiException catch (err) {
      if (err.statusCode == 404 || err.statusCode == 405) {
        return start(<String, dynamic>{
          'kind': schedule.kind,
          if (schedule.linkId != null) 'linkId': schedule.linkId,
          if (schedule.sourceDeviceId != null)
            'sourceDeviceId': schedule.sourceDeviceId,
          if (schedule.targetDeviceId != null)
            'targetDeviceId': schedule.targetDeviceId,
          'spec': schedule.spec.toJson(),
        });
      }
      rethrow;
    }
  }

  void _upsert(LinkTestModel test) {
    final prev = state.tests[test.id];
    if (prev != null && !prev.isActive && test.isActive) return;
    final nextTests = Map<int, LinkTestModel>.from(state.tests)
      ..[test.id] = test;
    final nextPage = List<LinkTestModel>.from(state.pageItems);
    final idx = nextPage.indexWhere((t) => t.id == test.id);
    if (idx >= 0) {
      nextPage[idx] = test;
    } else if (state.page == 1) {
      nextPage.insert(0, test);
      if (nextPage.length > linkTestHistoryPerPage) {
        nextPage.length = linkTestHistoryPerPage;
      }
    }
    state = state.copyWith(
      tests: nextTests,
      pageItems: nextPage,
      total: idx >= 0 ? state.total : state.total + 1,
    );
  }

  void _seedSamples(LinkTestModel test) {
    final nextSamples = Map<String, List<LinkTestSampleModel>>.from(
      state.samples,
    );
    final nextHealth = Map<String, List<LinkTestHealthSampleModel>>.from(
      state.health,
    );
    for (final leg in test.legs) {
      for (final flow in const <String>['forward', 'reverse']) {
        final list = leg.samples.where((s) => s.flow == flow).toList()
          ..sort((a, b) => a.tMs.compareTo(b.tMs));
        if (list.isNotEmpty) {
          nextSamples[linkTestSampleKey(
            test.id,
            leg.seq,
            flow,
          )] = list.length > linkTestSampleLimit
              ? list.sublist(list.length - linkTestSampleLimit)
              : list;
        }
      }
      if (leg.health.isNotEmpty) {
        final sorted = List<LinkTestHealthSampleModel>.from(leg.health)
          ..sort((a, b) => a.atMs.compareTo(b.atMs));
        nextHealth[linkTestHealthKey(
          test.id,
          leg.seq,
        )] = sorted.length > linkTestHealthLimit
            ? sorted.sublist(sorted.length - linkTestHealthLimit)
            : sorted;
      }
    }
    state = state.copyWith(samples: nextSamples, health: nextHealth);
  }

  /// SSE `link_test:updated`
  void applyUpdated(Map<String, dynamic> data) {
    final parsed = LinkTestModel.tryFromJson(data);
    if (parsed == null) return;
    _upsert(parsed);
  }

  /// SSE `link_test:sample` (anel de 120 amostras por perna/sentido)
  void applySample(Map<String, dynamic> data) {
    final testId = parseIntOrNull(data['testId']);
    final legSeq = parseIntOrNull(data['legSeq']);
    final flow = data['flow'] as String?;
    final bps = parseDoubleOrNull(data['bps']);
    final tMs = parseIntOrNull(data['tMs']);
    if (testId == null ||
        legSeq == null ||
        (flow != 'forward' && flow != 'reverse') ||
        bps == null ||
        tMs == null) {
      return;
    }
    final key = linkTestSampleKey(testId, legSeq, flow!);
    final existing = state.samples[key] ?? const <LinkTestSampleModel>[];
    if (existing.isNotEmpty && tMs <= existing.last.tMs) return;
    final updated = <LinkTestSampleModel>[
      ...existing,
      LinkTestSampleModel(
        flow: flow,
        bps: bps,
        tMs: tMs,
        atMs: parseIntOrNull(data['atMs']),
      ),
    ];
    final capped = updated.length > linkTestSampleLimit
        ? updated.sublist(updated.length - linkTestSampleLimit)
        : updated;
    state = state.copyWith(
      samples: <String, List<LinkTestSampleModel>>{
        ...state.samples,
        key: capped,
      },
    );
  }

  /// SSE `link_test:health` (anel de 480 amostras por perna)
  void applyHealth(Map<String, dynamic> data) {
    final testId = parseIntOrNull(data['testId']);
    final legSeq = parseIntOrNull(data['legSeq']);
    final deviceId = parseIntOrNull(data['deviceId']);
    final atMs = parseIntOrNull(data['atMs']);
    if (testId == null || legSeq == null || deviceId == null || atMs == null) {
      return;
    }
    final key = linkTestHealthKey(testId, legSeq);
    final existing = state.health[key] ?? const <LinkTestHealthSampleModel>[];
    if (existing.any((e) => e.deviceId == deviceId && e.atMs >= atMs)) return;
    final updated = <LinkTestHealthSampleModel>[
      ...existing,
      LinkTestHealthSampleModel(
        deviceId: deviceId,
        atMs: atMs,
        cpu: parseDoubleOrNull(data['cpu']),
        memory: parseDoubleOrNull(data['memory']),
        phase: (data['phase'] as String?) ?? 'during',
      ),
    ];
    final capped = updated.length > linkTestHealthLimit
        ? updated.sublist(updated.length - linkTestHealthLimit)
        : updated;
    state = state.copyWith(
      health: <String, List<LinkTestHealthSampleModel>>{
        ...state.health,
        key: capped,
      },
    );
  }

  /// SSE `link_test_schedule:updated`
  void applyScheduleUpdated(Map<String, dynamic> _) {
    if (!state.schedulesLoaded) return;
    fetchSchedules();
  }
}

final linkTestsProvider = NotifierProvider<LinkTestsNotifier, LinkTestsState>(
  LinkTestsNotifier.new,
);

/// Handoff efêmero em memória de Topology / DeviceDetail para o wizard de LinkTests.
final pendingLinkTestDraftProvider = StateProvider<LinkTestRequestDraft?>(
  (ref) => null,
);
