import 'dart:convert';
import 'dart:math' as math;

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/features/core/models/dashboard_models.dart';
import 'package:shared_preferences/shared_preferences.dart';

const String _layoutStorageKey = 'netmonitor_dashboard_layout_v2';
const String _syncModeStorageKey = 'netmonitor_dashboard_sync_mode';
const String _promptDismissedKey = 'netmonitor_dashboard_prompt_dismissed';

@immutable
class DashboardState {
  const DashboardState({
    this.catalog,
    this.widgets = const <DashboardWidgetConfig>[],
    this.serverLayout,
    this.ready = false,
    this.loadingServer = false,
    this.savingGlobal = false,
    this.isEditMode = false,
    this.syncMode = DashboardSyncMode.server,
    this.promptDismissed = false,
    this.showServerPrompt = false,
    this.clientIdSession = '',
    this.dismissedAreaSuggestions = const <ProductArea>{},
    this.error,
  });

  final DashboardCatalog? catalog;
  final List<DashboardWidgetConfig> widgets;
  final List<DashboardWidgetConfig>? serverLayout;
  final bool ready;
  final bool loadingServer;
  final bool savingGlobal;
  final bool isEditMode;
  final DashboardSyncMode syncMode;
  final bool promptDismissed;
  final bool showServerPrompt;
  final String clientIdSession;
  final Set<ProductArea> dismissedAreaSuggestions;
  final String? error;

  ProductArea areaOfWidget(DashboardWidgetConfig widget) {
    return catalog?.findByType(widget.type)?.area ?? ProductArea.core;
  }

  bool isDefaultWidget(String id) {
    return catalog?.defaultLayout.any((item) => item.type == id) ?? false;
  }

  List<DashboardWidgetConfig> sortedWidgets() {
    final copy = List<DashboardWidgetConfig>.from(widgets)
      ..sort((a, b) => a.order.compareTo(b.order));
    return copy;
  }

  List<DashboardWidgetConfig> availableWidgets(Set<ProductArea> visibleAreas) {
    return sortedWidgets()
        .where((w) => visibleAreas.contains(areaOfWidget(w)))
        .toList(growable: false);
  }

  List<DashboardWidgetConfig> visibleWidgets(Set<ProductArea> visibleAreas) {
    return availableWidgets(
      visibleAreas,
    ).where((w) => w.visible).toList(growable: false);
  }

  List<DashboardWidgetConfig> hiddenWidgets(Set<ProductArea> visibleAreas) {
    return availableWidgets(
      visibleAreas,
    ).where((w) => !w.visible).toList(growable: false);
  }

  DashboardState copyWith({
    DashboardCatalog? catalog,
    List<DashboardWidgetConfig>? widgets,
    List<DashboardWidgetConfig>? serverLayout,
    bool clearServerLayout = false,
    bool? ready,
    bool? loadingServer,
    bool? savingGlobal,
    bool? isEditMode,
    DashboardSyncMode? syncMode,
    bool? promptDismissed,
    bool? showServerPrompt,
    String? clientIdSession,
    Set<ProductArea>? dismissedAreaSuggestions,
    String? error,
    bool clearError = false,
  }) {
    return DashboardState(
      catalog: catalog ?? this.catalog,
      widgets: widgets ?? this.widgets,
      serverLayout: clearServerLayout
          ? null
          : (serverLayout ?? this.serverLayout),
      ready: ready ?? this.ready,
      loadingServer: loadingServer ?? this.loadingServer,
      savingGlobal: savingGlobal ?? this.savingGlobal,
      isEditMode: isEditMode ?? this.isEditMode,
      syncMode: syncMode ?? this.syncMode,
      promptDismissed: promptDismissed ?? this.promptDismissed,
      showServerPrompt: showServerPrompt ?? this.showServerPrompt,
      clientIdSession: clientIdSession ?? this.clientIdSession,
      dismissedAreaSuggestions:
          dismissedAreaSuggestions ?? this.dismissedAreaSuggestions,
      error: clearError ? null : (error ?? this.error),
    );
  }
}

class DashboardNotifier extends Notifier<DashboardState> {
  Future<void>? _catalogFuture;

  @override
  DashboardState build() {
    final rng = math.Random();
    final id = 'client-${rng.nextInt(1 << 30).toRadixString(36)}';
    return DashboardState(clientIdSession: id);
  }

  List<DashboardWidgetConfig> _defaultWidgetsOf(DashboardCatalog? catalog) {
    if (catalog == null) return const <DashboardWidgetConfig>[];
    final list = <DashboardWidgetConfig>[];
    for (var i = 0; i < catalog.defaultLayout.length; i++) {
      final spec = catalog.defaultLayout[i];
      final entry = catalog.findByType(spec.type);
      if (entry == null) continue;
      list.add(
        DashboardWidgetConfig(
          id: spec.type,
          type: entry.type,
          title: entry.title,
          category: entry.category,
          cols: 12,
          sm: 12,
          md: spec.size.mdCols,
          lg: spec.size.lgCols,
          visible: spec.visible,
          order: i,
          description: entry.description,
          icon: entry.icon,
        ),
      );
    }
    return list;
  }

  List<DashboardWidgetConfig> _mergeLayout(
    List<dynamic> rawSaved,
    DashboardCatalog? catalog,
  ) {
    final defaults = _defaultWidgetsOf(catalog);
    final merged = <DashboardWidgetConfig>[];
    final seen = <String>{};
    final visibleAreas = ref.read(visibleAreasProvider);

    for (var i = 0; i < rawSaved.length; i++) {
      final parsed = DashboardWidgetConfig.tryFromJson(
        rawSaved[i],
        fallbackOrder: i,
      );
      if (parsed == null) continue;
      final def = defaults.cast<DashboardWidgetConfig?>().firstWhere(
        (d) => d?.id == parsed.id,
        orElse: () => null,
      );
      if (def != null) {
        merged.add(
          def.copyWith(
            title: parsed.title.isNotEmpty ? parsed.title : def.title,
            cols: parsed.cols,
            sm: parsed.sm,
            md: parsed.md,
            lg: parsed.lg,
            visible: parsed.visible,
            order: parsed.order,
            config: parsed.config.isNotEmpty ? parsed.config : def.config,
          ),
        );
        seen.add(def.id);
      } else {
        final entry = catalog?.findByType(parsed.type);
        merged.add(
          parsed.copyWith(
            title: parsed.title.isNotEmpty
                ? parsed.title
                : (entry?.title ?? 'Widget'),
            category: entry?.category ?? parsed.category,
            description: parsed.description.isNotEmpty
                ? parsed.description
                : (entry?.description ?? ''),
            icon: parsed.icon.isNotEmpty ? parsed.icon : (entry?.icon ?? ''),
          ),
        );
        seen.add(parsed.id);
      }
    }

    for (final def in defaults) {
      final area = catalog?.findByType(def.type)?.area ?? ProductArea.core;
      if (!seen.contains(def.id) && visibleAreas.contains(area)) {
        merged.add(def.copyWith(order: merged.length));
      }
    }
    merged.sort((a, b) => a.order.compareTo(b.order));
    return merged;
  }

  Future<void> initialize() {
    _catalogFuture ??= _loadCatalogAndLayout();
    return _catalogFuture!;
  }

  Future<void> reload() {
    _catalogFuture = _loadCatalogAndLayout();
    return _catalogFuture!;
  }

  Future<void> _loadCatalogAndLayout() async {
    DashboardSyncMode savedSync = DashboardSyncMode.server;
    bool dismissed = false;
    List<dynamic>? localSaved;
    try {
      final prefs = await SharedPreferences.getInstance();
      savedSync = DashboardSyncMode.parse(prefs.getString(_syncModeStorageKey));
      dismissed = prefs.getBool(_promptDismissedKey) ?? false;
      final rawJson = prefs.getString(_layoutStorageKey);
      if (rawJson != null && rawJson.isNotEmpty) {
        final decoded = jsonDecode(rawJson);
        if (decoded is List) localSaved = decoded;
      }
    } catch (_) {}

    DashboardCatalog? catalog;
    String? err;
    try {
      final res = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>('/dashboard/catalog');
      catalog = DashboardCatalog.fromJson(res);
    } catch (e) {
      err = describeError(e);
    }

    final initialWidgets = localSaved != null
        ? _mergeLayout(localSaved, catalog)
        : _defaultWidgetsOf(catalog);

    state = state.copyWith(
      catalog: catalog,
      widgets: initialWidgets,
      syncMode: savedSync,
      promptDismissed: dismissed,
      ready: true,
      error: err,
      clearError: err == null,
    );

    await checkServerPrompt();
  }

  Future<List<DashboardWidgetConfig>?> fetchServerLayout() async {
    state = state.copyWith(loadingServer: true);
    try {
      final res = await ref
          .read(apiClientProvider)
          .get<Map<String, dynamic>>('/dashboard/layout');
      final raw = res['layout'];
      if (raw is List && raw.isNotEmpty) {
        final parsed = _mergeLayout(raw, state.catalog);
        if (state.syncMode == DashboardSyncMode.server) {
          state = state.copyWith(
            serverLayout: parsed,
            widgets: parsed,
            loadingServer: false,
          );
          await _saveLocalCache(parsed);
        } else {
          state = state.copyWith(serverLayout: parsed, loadingServer: false);
        }
        return parsed;
      }
      state = state.copyWith(clearServerLayout: true, loadingServer: false);
      return null;
    } catch (_) {
      state = state.copyWith(loadingServer: false);
      return null;
    }
  }

  Future<void> checkServerPrompt() async {
    final remote = await fetchServerLayout();
    if (!state.promptDismissed && remote != null && remote.isNotEmpty) {
      state = state.copyWith(showServerPrompt: true);
    }
  }

  Future<void> chooseInitialSyncMode(DashboardSyncMode mode) async {
    await setSyncMode(mode);
    state = state.copyWith(showServerPrompt: false, promptDismissed: true);
  }

  Future<void> setSyncMode(DashboardSyncMode mode) async {
    state = state.copyWith(syncMode: mode, promptDismissed: true);
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setString(_syncModeStorageKey, mode.id);
      await prefs.setBool(_promptDismissedKey, true);
    } catch (_) {}

    if (mode == DashboardSyncMode.server && state.serverLayout != null) {
      state = state.copyWith(widgets: state.serverLayout);
      await _saveLocalCache(state.serverLayout!);
    }
  }

  Future<bool> saveLayoutGlobally() async {
    state = state.copyWith(savingGlobal: true);
    try {
      final payload = state.widgets
          .map((w) => w.toJson())
          .toList(growable: false);
      await ref
          .read(apiClientProvider)
          .post<dynamic>(
            '/dashboard/layout',
            data: <String, dynamic>{
              'layout': payload,
              'clientId': state.clientIdSession,
            },
          );
      state = state.copyWith(
        serverLayout: List<DashboardWidgetConfig>.from(state.widgets),
        savingGlobal: false,
      );
      await setSyncMode(DashboardSyncMode.server);
      return true;
    } catch (_) {
      state = state.copyWith(savingGlobal: false);
      return false;
    }
  }

  /// Aplica layout recebido pelo SSE (`dashboard:layout_updated`).
  void applyRemoteLayout(Map<String, dynamic> data) {
    final rawLayout = data['layout'];
    if (rawLayout is! List) return;
    final remoteClientId = data['clientId']?.toString();
    final parsed = _mergeLayout(rawLayout, state.catalog);
    state = state.copyWith(serverLayout: parsed);

    if (remoteClientId != null && remoteClientId == state.clientIdSession) {
      if (state.syncMode == DashboardSyncMode.server) {
        state = state.copyWith(widgets: parsed);
        _saveLocalCache(parsed);
      }
      return;
    }

    if (state.promptDismissed) {
      if (state.syncMode == DashboardSyncMode.server) {
        state = state.copyWith(widgets: parsed);
        _saveLocalCache(parsed);
      }
      return;
    }

    state = state.copyWith(showServerPrompt: true);
  }

  void toggleEditMode([bool? enabled]) {
    state = state.copyWith(isEditMode: enabled ?? !state.isEditMode);
  }

  void dismissAreaSuggestion(ProductArea area) {
    state = state.copyWith(
      dismissedAreaSuggestions: <ProductArea>{
        ...state.dismissedAreaSuggestions,
        area,
      },
    );
  }

  Future<void> _commitWidgets(List<DashboardWidgetConfig> next) async {
    state = state.copyWith(widgets: next);
    await _saveLocalCache(next);
    if (state.syncMode == DashboardSyncMode.server) {
      await saveLayoutGlobally();
    }
  }

  Future<void> _saveLocalCache(List<DashboardWidgetConfig> list) async {
    try {
      final prefs = await SharedPreferences.getInstance();
      await prefs.setString(
        _layoutStorageKey,
        jsonEncode(list.map((w) => w.toJson()).toList(growable: false)),
      );
    } catch (_) {}
  }

  Future<void> toggleWidgetVisibility(String id, [bool? visible]) async {
    final next = state.widgets
        .map((w) => w.id == id ? w.copyWith(visible: visible ?? !w.visible) : w)
        .toList(growable: false);
    await _commitWidgets(next);
  }

  Future<void> removeWidget(String id) async {
    final next = state.isDefaultWidget(id)
        ? state.widgets
              .map((w) => w.id == id ? w.copyWith(visible: false) : w)
              .toList(growable: false)
        : state.widgets.where((w) => w.id != id).toList(growable: false);
    await _commitWidgets(next);
  }

  Future<void> addCustomWidget(
    String type, {
    String? title,
    Map<String, dynamic> config = const <String, dynamic>{},
  }) async {
    final entry = state.catalog?.findByType(type);
    final size = entry?.defaultSize ?? WidgetSize.half;
    final id = '${type}_${DateTime.now().millisecondsSinceEpoch}';
    final created = DashboardWidgetConfig(
      id: id,
      type: type,
      title: (title != null && title.trim().isNotEmpty)
          ? title.trim()
          : (entry?.title ?? 'Widget personalizado'),
      category: entry?.category ?? 'charts',
      cols: 12,
      sm: 12,
      md: size.mdCols,
      lg: size.lgCols,
      visible: true,
      order: state.widgets.length,
      description: entry?.description ?? '',
      icon: entry?.icon ?? '',
      config: config,
    );
    await _commitWidgets(<DashboardWidgetConfig>[...state.widgets, created]);
  }

  Future<void> updateWidgetConfig(
    String id, {
    String? title,
    Map<String, dynamic>? config,
  }) async {
    final next = state.widgets
        .map(
          (w) => w.id == id
              ? w.copyWith(
                  title: title != null && title.trim().isNotEmpty
                      ? title.trim()
                      : w.title,
                  config: config != null
                      ? <String, dynamic>{...w.config, ...config}
                      : w.config,
                )
              : w,
        )
        .toList(growable: false);
    await _commitWidgets(next);
  }

  Future<void> resizeWidget(String id, WidgetSize size) async {
    final next = state.widgets
        .map(
          (w) => w.id == id ? w.copyWith(md: size.mdCols, lg: size.lgCols) : w,
        )
        .toList(growable: false);
    await _commitWidgets(next);
  }

  Future<void> moveWidget(String id, {required bool up}) async {
    final visibleAreas = ref.read(visibleAreasProvider);
    final visible = state.visibleWidgets(visibleAreas);
    final index = visible.indexWhere((w) => w.id == id);
    if (index == -1) return;
    final targetIndex = up ? index - 1 : index + 1;
    if (targetIndex < 0 || targetIndex >= visible.length) return;

    final current = visible[index];
    final target = visible[targetIndex];
    final next = state.widgets.map((w) {
      if (w.id == current.id) return w.copyWith(order: target.order);
      if (w.id == target.id) return w.copyWith(order: current.order);
      return w;
    }).toList()..sort((a, b) => a.order.compareTo(b.order));

    final normalized = <DashboardWidgetConfig>[
      for (var i = 0; i < next.length; i++) next[i].copyWith(order: i),
    ];
    await _commitWidgets(normalized);
  }

  Future<void> resetToDefaultLayout() async {
    await _commitWidgets(_defaultWidgetsOf(state.catalog));
  }
}

final dashboardProvider = NotifierProvider<DashboardNotifier, DashboardState>(
  DashboardNotifier.new,
);
