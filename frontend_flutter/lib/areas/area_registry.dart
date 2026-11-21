import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/areas/core_area.dart';
import 'package:netmonitor_app/areas/network_area.dart';
import 'package:netmonitor_app/areas/servers_area.dart';
import 'package:netmonitor_app/areas/vpn_area.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';

/// Registro único das 4 áreas do produto (ADR 017).
final List<AreaContribution> registeredAreas = List<AreaContribution>.unmodifiable(
  <AreaContribution>[
    coreArea,
    networkArea,
    serversArea,
    vpnArea,
  ],
);

AreaContribution? areaContributionById(ProductArea id) {
  for (final area in registeredAreas) {
    if (area.id == id) return area;
  }
  return null;
}

List<RouteBase> get allAreaRoutes =>
    registeredAreas.expand((area) => area.routes).toList(growable: false);

List<NavEntry> get allNavEntries =>
    registeredAreas.expand((area) => area.nav).toList(growable: false);

Map<String, DashboardWidgetContribution> get allDashboardWidgets {
  final map = <String, DashboardWidgetContribution>{};
  for (final area in registeredAreas) {
    map.addAll(area.widgets);
  }
  return map;
}

List<ServerConnectPathContribution> visibleServerConnectPaths(
  Set<ProductArea> visibleAreas,
) {
  final paths = registeredAreas
      .where((area) => visibleAreas.contains(area.id))
      .expand((area) => area.serverConnectPaths)
      .toList()
    ..sort((a, b) => a.order.compareTo(b.order));
  return paths;
}

/// Monta as abas de Configurações visíveis, mantendo apenas abas que tenham cartões.
List<SettingsTabView> buildVisibleSettingsTabs(Set<ProductArea> visibleAreas) {
  final visibleModules = registeredAreas
      .where((area) => visibleAreas.contains(area.id))
      .toList(growable: false);

  final allCards = visibleModules
      .expand((area) => area.settingsCards)
      .toList(growable: false);

  final allTabs = visibleModules
      .expand((area) => area.settingsTabs)
      .toList()
    ..sort((a, b) => a.order.compareTo(b.order));

  final views = <SettingsTabView>[];
  for (final tab in allTabs) {
    final cards = allCards.where((c) => c.tab == tab.value).toList()
      ..sort((a, b) => a.order.compareTo(b.order));
    if (cards.isNotEmpty) {
      views.add(SettingsTabView(tab: tab, cards: cards));
    }
  }
  return views;
}

/// Resolve os metadados da rota pelo caminho mais específico declarado nas áreas.
AreaRouteMeta? resolveRouteMeta(String location) {
  final uri = Uri.tryParse(location);
  final path = uri?.path ?? location;

  AreaRouteMeta? best;
  var bestLength = -1;

  for (final area in registeredAreas) {
    area.routeMeta.forEach((prefix, meta) {
      final matches = prefix == '/'
          ? path == '/'
          : (path == prefix || path.startsWith('$prefix/'));
      if (matches && prefix.length > bestLength) {
        best = meta;
        bestLength = prefix.length;
      }
    });
  }
  return best;
}

/// Despacha uma `NavAction` declarada em alguma área visível.
bool dispatchAreaNavAction(
  NavAction action,
  BuildContext context,
  WidgetRef ref,
) {
  for (final area in registeredAreas) {
    final handler = area.navActions[action];
    if (handler != null) {
      handler(context, ref);
      return true;
    }
  }
  return false;
}

/// Gerencia dinamicamente o registro/remoção das rotas SSE por área conforme
/// `visibleAreasProvider` (paridade com `installAreaRealtime` de `realtime.ts`).
final areaRealtimeCoordinatorProvider = Provider<void>((ref) {
  final installed = <ProductArea, VoidCallback>{};

  void sync(Set<ProductArea> visible) {
    final sse = ref.read(sseServiceProvider.notifier);
    for (final area in registeredAreas) {
      final on = visible.contains(area.id);
      final current = installed[area.id];
      if (on && current == null && area.sseBuilder != null) {
        installed[area.id] = sse.registerRoutes(area.sseBuilder!(ref));
      } else if (!on && current != null) {
        current();
        installed.remove(area.id);
      }
    }
  }

  sync(ref.read(visibleAreasProvider));
  ref.listen<Set<ProductArea>>(visibleAreasProvider, (_, next) => sync(next));

  ref.onDispose(() {
    for (final remove in installed.values) {
      remove();
    }
    installed.clear();
  });
});
