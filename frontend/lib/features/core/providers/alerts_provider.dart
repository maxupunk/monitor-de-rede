import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/models/pagination.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/core/models/alert_models.dart';

@immutable
class AlertsState {
  const AlertsState({
    this.alertEvents = const <AlertEvent>[],
    this.alertRules = const <AlertRule>[],
    this.ruleTemplates = const <AlertRuleTemplate>[],
    this.ruleCategories = const <String, String>{},
    this.historyItems = const <AlertEvent>[],
    this.historyMeta,
    this.rcaSummary,
    this.loading = false,
    this.catalogLoading = false,
    this.rcaLoading = false,
    this.historyLoading = false,
    this.error,
  });

  final List<AlertEvent> alertEvents;
  final List<AlertRule> alertRules;
  final List<AlertRuleTemplate> ruleTemplates;
  final Map<String, String> ruleCategories;
  final List<AlertEvent> historyItems;
  final PaginationMeta? historyMeta;
  final RootCauseAnalysisSummary? rcaSummary;
  final bool loading;
  final bool catalogLoading;
  final bool rcaLoading;
  final bool historyLoading;
  final String? error;

  List<AlertEvent> get activeAlerts =>
      alertEvents.where((a) => !a.isResolved).toList(growable: false);

  List<AlertEvent> get resolvedAlerts =>
      alertEvents.where((a) => a.isResolved).toList(growable: false);

  int get criticalCount => activeAlerts
      .where((a) => a.severity == 'critical' || a.severity == 'error')
      .length;

  AlertsState copyWith({
    List<AlertEvent>? alertEvents,
    List<AlertRule>? alertRules,
    List<AlertRuleTemplate>? ruleTemplates,
    Map<String, String>? ruleCategories,
    List<AlertEvent>? historyItems,
    PaginationMeta? historyMeta,
    RootCauseAnalysisSummary? rcaSummary,
    bool? loading,
    bool? catalogLoading,
    bool? rcaLoading,
    bool? historyLoading,
    String? error,
    bool clearError = false,
  }) {
    return AlertsState(
      alertEvents: alertEvents ?? this.alertEvents,
      alertRules: alertRules ?? this.alertRules,
      ruleTemplates: ruleTemplates ?? this.ruleTemplates,
      ruleCategories: ruleCategories ?? this.ruleCategories,
      historyItems: historyItems ?? this.historyItems,
      historyMeta: historyMeta ?? this.historyMeta,
      rcaSummary: rcaSummary ?? this.rcaSummary,
      loading: loading ?? this.loading,
      catalogLoading: catalogLoading ?? this.catalogLoading,
      rcaLoading: rcaLoading ?? this.rcaLoading,
      historyLoading: historyLoading ?? this.historyLoading,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class AlertsNotifier extends Notifier<AlertsState> {
  @override
  AlertsState build() => const AlertsState();

  Future<void> fetchActiveAlerts() async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final raw = await ref.read(apiClientProvider).get<dynamic>('/alerts');
      final paginated = PaginatedResult<AlertEvent>.fromDynamic(
        raw,
        AlertEvent.fromJson,
      );
      state = state.copyWith(alertEvents: paginated.data, loading: false);
    } catch (err) {
      state = state.copyWith(loading: false, error: describeError(err));
    }
  }

  Future<void> fetchHistory({int page = 1, bool append = false}) async {
    state = state.copyWith(historyLoading: true, clearError: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>(
            '/alerts',
            queryParameters: <String, dynamic>{'page': page, 'limit': 30},
          );
      final paginated = PaginatedResult<AlertEvent>.fromDynamic(
        raw,
        AlertEvent.fromJson,
        fallbackPerPage: 30,
      );
      state = state.copyWith(
        historyItems: append
            ? <AlertEvent>[...state.historyItems, ...paginated.data]
            : paginated.data,
        historyMeta: raw is Map && raw['meta'] is Map ? paginated.meta : null,
        historyLoading: false,
      );
    } catch (err) {
      state = state.copyWith(historyLoading: false, error: describeError(err));
    }
  }

  Future<void> fetchRootCauseAnalysis() async {
    state = state.copyWith(rcaLoading: true);
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>('/alerts/root-cause-analysis');
      state = state.copyWith(
        rcaSummary: RootCauseAnalysisSummary.fromJson(raw),
        rcaLoading: false,
      );
    } catch (_) {
      state = state.copyWith(rcaLoading: false);
    }
  }

  Future<AlertCorrelation?> fetchCorrelation(int alertId) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>('/alerts/$alertId/correlation');
      return AlertCorrelation.fromJson(raw);
    } catch (_) {
      return null;
    }
  }

  Future<List<ScopeInstability>> fetchInstability({
    String? scopeKey,
    int hours = 24,
    int? limit,
  }) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>(
            '/alerts/instability',
            queryParameters: <String, dynamic>{
              'scopeKey': ?scopeKey,
              'hours': hours,
              'limit': ?limit,
            },
          );
      return asJsonList<ScopeInstability>(raw, ScopeInstability.fromJson);
    } catch (_) {
      return const <ScopeInstability>[];
    }
  }

  Future<String?> acknowledgeAlert(int alertId) async {
    try {
      final res = await ref
          .read(apiClientProvider)
          .post<Map<String, dynamic>>('/alerts/$alertId/acknowledge');
      if (res['event'] is Map) {
        upsertAlertEvent(AlertEvent.fromJson(asJsonMap(res['event'])));
      } else {
        patchAlertEvent(
          alertId,
          status: res['resolved'] == true ? 'resolved' : 'acknowledged',
        );
      }
      return res['message'] as String?;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return null;
    }
  }

  /// Repete a checagem do alvo. `checkable` falso: o alerta é de evento (sem
  /// monitor) e não havia o que checar — a tela oferece encerrar.
  Future<({bool ok, bool resolved, bool checkable, String? message})>
  verifyAlert(int alertId) async {
    try {
      final res = await ref
          .read(apiClientProvider)
          .post<Map<String, dynamic>>('/alerts/$alertId/verify');
      if (res['event'] is Map) {
        upsertAlertEvent(AlertEvent.fromJson(asJsonMap(res['event'])));
      }
      return (
        ok: true,
        resolved: res['resolved'] == true,
        checkable: res['checkable'] != false,
        message: res['message'] as String?,
      );
    } catch (err) {
      return (
        ok: false,
        resolved: false,
        checkable: true,
        message: describeError(err),
      );
    }
  }

  /// Encerra o alerta à mão (a condição já passou e a volta não chegou).
  Future<({bool ok, String? message})> closeAlert(int alertId) async {
    try {
      final res = await ref
          .read(apiClientProvider)
          .post<Map<String, dynamic>>('/alerts/$alertId/close');
      if (res['event'] is Map) {
        upsertAlertEvent(AlertEvent.fromJson(asJsonMap(res['event'])));
      }
      return (ok: true, message: res['message'] as String?);
    } catch (err) {
      return (ok: false, message: describeError(err));
    }
  }

  Future<({bool ok, int resolvedCount, String? message})>
  verifyAllAlerts() async {
    state = state.copyWith(loading: true);
    try {
      final res = await ref
          .read(apiClientProvider)
          .post<Map<String, dynamic>>('/alerts/verify-all');
      await fetchActiveAlerts();
      return (
        ok: true,
        resolvedCount: parseIntOrNull(res['resolvedCount']) ?? 0,
        message: res['message'] as String?,
      );
    } catch (err) {
      final msg = describeError(err);
      state = state.copyWith(loading: false, error: msg);
      return (ok: false, resolvedCount: 0, message: msg);
    }
  }

  Future<bool> silenceAlert(int alertId, int durationMinutes) async {
    try {
      await ref
          .read(apiClientProvider)
          .post<dynamic>(
            '/alerts/$alertId/silence',
            data: <String, dynamic>{
              'minutes': durationMinutes,
              'durationMinutes': durationMinutes,
            },
          );
      patchAlertEvent(alertId, status: 'silenced');
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  Future<List<AlertRule>> fetchAlertRules({
    int? siteId,
    int? deviceId,
    int? monitorId,
    bool includeGlobal = false,
  }) async {
    state = state.copyWith(loading: true, clearError: true);
    try {
      final params = <String, dynamic>{
        'siteId': ?siteId,
        'deviceId': ?deviceId,
        'monitorId': ?monitorId,
        if (includeGlobal) 'includeGlobal': 'true',
      };
      final raw = await ref
          .read(apiClientProvider)
          .get<dynamic>(
            '/alert-rules',
            queryParameters: params.isEmpty ? null : params,
          );
      final rules = asJsonList<AlertRule>(raw, AlertRule.fromJson);
      state = state.copyWith(alertRules: rules, loading: false);
      return rules;
    } catch (err) {
      state = state.copyWith(loading: false, error: describeError(err));
      return const <AlertRule>[];
    }
  }

  Future<void> fetchRuleCatalog({
    int? siteId,
    int? deviceId,
    int? monitorId,
  }) async {
    state = state.copyWith(catalogLoading: true, clearError: true);
    try {
      final params = <String, dynamic>{
        'siteId': ?siteId,
        'deviceId': ?deviceId,
        'monitorId': ?monitorId,
      };
      final raw = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>(
            '/alert-rules/catalog',
            queryParameters: params.isEmpty ? null : params,
          );
      state = state.copyWith(
        ruleTemplates: asJsonList<AlertRuleTemplate>(
          raw['templates'],
          AlertRuleTemplate.fromJson,
        ),
        ruleCategories: asStringMap(raw['categories']),
        catalogLoading: false,
      );
    } catch (err) {
      state = state.copyWith(catalogLoading: false, error: describeError(err));
    }
  }

  Future<int> applyCatalogRules(
    List<String> keys, {
    int? siteId,
    int? deviceId,
    int? monitorId,
  }) async {
    state = state.copyWith(catalogLoading: true, clearError: true);
    try {
      final res = await ref
          .read(apiClientProvider)
          .post<Map<String, dynamic>>(
            '/alert-rules/catalog',
            data: <String, dynamic>{
              'keys': keys,
              'siteId': siteId,
              'deviceId': deviceId,
              'monitorId': monitorId,
            },
          );
      final created = asJsonList<AlertRule>(res['created'], AlertRule.fromJson);
      for (final rule in created) {
        upsertAlertRule(rule);
      }
      await fetchRuleCatalog(
        siteId: siteId,
        deviceId: deviceId,
        monitorId: monitorId,
      );
      return created.length;
    } catch (err) {
      state = state.copyWith(catalogLoading: false, error: describeError(err));
      return 0;
    }
  }

  Future<bool> createAlertRule(Map<String, dynamic> payload) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .post<Map<String, dynamic>>('/alert-rules', data: payload);
      upsertAlertRule(AlertRule.fromJson(raw));
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  Future<bool> updateAlertRule(int id, Map<String, dynamic> payload) async {
    try {
      final raw = await ref
          .read(apiClientProvider)
          .put<Map<String, dynamic>>('/alert-rules/$id', data: payload);
      upsertAlertRule(AlertRule.fromJson(raw));
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  Future<bool> deleteAlertRule(int id) async {
    try {
      await ref.read(apiClientProvider).delete<dynamic>('/alert-rules/$id');
      removeAlertRule(id);
      return true;
    } catch (err) {
      state = state.copyWith(error: describeError(err));
      return false;
    }
  }

  void upsertAlertEvent(AlertEvent event) {
    final index = state.alertEvents.indexWhere((a) => a.id == event.id);
    if (index == -1) {
      state = state.copyWith(
        alertEvents: <AlertEvent>[event, ...state.alertEvents],
      );
    } else {
      final copy = List<AlertEvent>.from(state.alertEvents);
      copy[index] = event;
      state = state.copyWith(alertEvents: copy);
    }
  }

  void patchAlertEvent(
    int id, {
    String? status,
    String? resolvedAt,
    String? silencedUntil,
  }) {
    state = state.copyWith(
      alertEvents: state.alertEvents
          .map(
            (a) => a.id == id
                ? a.copyWith(
                    status: status ?? a.status,
                    resolvedAt: resolvedAt ?? a.resolvedAt,
                    silencedUntil: silencedUntil ?? a.silencedUntil,
                  )
                : a,
          )
          .toList(growable: false),
    );
  }

  void applyAiSummary(int id, Map<String, dynamic> summary) {
    state = state.copyWith(
      alertEvents: state.alertEvents
          .map(
            (a) => a.id == id
                ? a.copyWith(
                    data: <String, dynamic>{...?a.data, 'aiSummary': summary},
                  )
                : a,
          )
          .toList(growable: false),
    );
  }

  void applyDeciderTriage(int id, Map<String, dynamic> triage) {
    state = state.copyWith(
      alertEvents: state.alertEvents
          .map(
            (a) => a.id == id
                ? a.copyWith(
                    data: <String, dynamic>{
                      ...?a.data,
                      'deciderTriage': triage,
                    },
                  )
                : a,
          )
          .toList(growable: false),
    );
  }

  void upsertAlertRule(AlertRule rule) {
    final index = state.alertRules.indexWhere((r) => r.id == rule.id);
    if (index == -1) {
      state = state.copyWith(
        alertRules: <AlertRule>[...state.alertRules, rule],
      );
    } else {
      final copy = List<AlertRule>.from(state.alertRules);
      copy[index] = rule;
      state = state.copyWith(alertRules: copy);
    }
  }

  void removeAlertRule(int id) {
    state = state.copyWith(
      alertRules: state.alertRules
          .where((r) => r.id != id)
          .toList(growable: false),
    );
  }
}

final alertsProvider = NotifierProvider<AlertsNotifier, AlertsState>(
  AlertsNotifier.new,
);
