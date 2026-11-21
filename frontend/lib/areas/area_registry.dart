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
final List<AreaContribution> registeredAreas =
    List<AreaContribution>.unmodifiable(<AreaContribution>[
      coreArea,
      networkArea,
      serversArea,
      vpnArea,
    ]);

AreaContribution? areaContributionById(ProductArea id) {
  for (final area in registeredAreas) {
    if (area.id == id) return area;
  }
  return null;
}

/// Deduplica `GoRoute`s de topo por `path`, mantendo o **último** registro.
/// Permite que módulos posteriores (ex.: `networkArea`, `serversArea`, `vpnArea`)
/// substituam placeholders de módulos anteriores sem colisão de rotas/nomes no GoRouter.
List<RouteBase> deduplicateAreaRoutes(Iterable<AreaContribution> modules) {
  final result = <RouteBase>[];
  final indexByPath = <String, int>{};

  for (final area in modules) {
    for (final route in area.routes) {
      if (route is GoRoute) {
        final existingIndex = indexByPath[route.path];
        if (existingIndex != null) {
          result[existingIndex] = route;
        } else {
          indexByPath[route.path] = result.length;
          result.add(route);
        }
      } else {
        result.add(route);
      }
    }
  }
  return List<RouteBase>.unmodifiable(result);
}

List<RouteBase> get allAreaRoutes => deduplicateAreaRoutes(registeredAreas);

/// Deduplica `NavEntry`s estáticos com mesmo `item.to` (último vence), preservando
/// entradas dinâmicas (`builder`) e ações (`action`).
List<NavEntry> deduplicateNavEntries(Iterable<AreaContribution> modules) {
  final result = <NavEntry>[];
  final indexByPath = <String, int>{};

  for (final area in modules) {
    for (final entry in area.nav) {
      final path = entry.item?.to;
      if (path != null && path.isNotEmpty) {
        final existingIndex = indexByPath[path];
        if (existingIndex != null) {
          result[existingIndex] = entry;
        } else {
          indexByPath[path] = result.length;
          result.add(entry);
        }
      } else {
        result.add(entry);
      }
    }
  }
  return List<NavEntry>.unmodifiable(result);
}

List<NavEntry> get allNavEntries => deduplicateNavEntries(registeredAreas);

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
  final paths =
      registeredAreas
          .where((area) => visibleAreas.contains(area.id))
          .expand((area) => area.serverConnectPaths)
          .toList()
        ..sort((a, b) => a.order.compareTo(b.order));
  return paths;
}

/// Monta as abas de Configurações visíveis, avaliando visibilidade por aba e por
/// cartão (`item.area ?? module.id` — ADR 017/019). Abas sem cartão visível somem.
List<SettingsTabView> buildVisibleSettingsTabsFrom(
  Iterable<AreaContribution> modules,
  Set<ProductArea> visibleAreas,
) {
  final tabsByValue = <String, SettingsTabContribution>{};
  final visibleCards = <SettingsCardContribution>[];

  for (final module in modules) {
    for (final tab in module.settingsTabs) {
      final effectiveArea = tab.area ?? module.id;
      if (visibleAreas.contains(effectiveArea)) {
        tabsByValue[tab.value] = tab;
      }
    }
    for (final card in module.settingsCards) {
      final effectiveArea = card.area ?? module.id;
      if (visibleAreas.contains(effectiveArea)) {
        visibleCards.add(card);
      }
    }
  }

  final sortedTabs = tabsByValue.values.toList()
    ..sort((a, b) => a.order.compareTo(b.order));

  final views = <SettingsTabView>[];
  for (final tab in sortedTabs) {
    final cards = visibleCards.where((c) => c.tab == tab.value).toList()
      ..sort((a, b) => a.order.compareTo(b.order));
    if (cards.isNotEmpty) {
      views.add(SettingsTabView(tab: tab, cards: cards));
    }
  }
  return views;
}

List<SettingsTabView> buildVisibleSettingsTabs(Set<ProductArea> visibleAreas) =>
    buildVisibleSettingsTabsFrom(registeredAreas, visibleAreas);

/// Resolve os metadados da rota pelo caminho mais específico (em empate, último módulo vence).
AreaRouteMeta? resolveRouteMetaFrom(
  Iterable<AreaContribution> modules,
  String location,
) {
  final uri = Uri.tryParse(location);
  final path = uri?.path ?? location;

  AreaRouteMeta? best;
  var bestLength = -1;

  for (final area in modules) {
    area.routeMeta.forEach((prefix, meta) {
      final matches = prefix == '/'
          ? path == '/'
          : (path == prefix || path.startsWith('$prefix/'));
      if (matches && prefix.length >= bestLength) {
        best = meta;
        bestLength = prefix.length;
      }
    });
  }
  return best;
}

AreaRouteMeta? resolveRouteMeta(String location) =>
    resolveRouteMetaFrom(registeredAreas, location);

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
/// `visibleAreasProvider` (inclui `sseBuilder` principal e `extraSseBuilders` por área ADR 017).
final areaRealtimeCoordinatorProvider = Provider<void>((ref) {
  final installed = <String, VoidCallback>{};

  void syncKey(
    String key,
    ProductArea gatedArea,
    AreaSseRoutesBuilder builder,
    Set<ProductArea> visible,
  ) {
    final sse = ref.read(sseServiceProvider.notifier);
    final on = visible.contains(gatedArea);
    final current = installed[key];
    if (on && current == null) {
      installed[key] = sse.registerRoutes(builder(ref));
    } else if (!on && current != null) {
      current();
      installed.remove(key);
    }
  }

  void sync(Set<ProductArea> visible) {
    for (final area in registeredAreas) {
      if (area.sseBuilder != null) {
        syncKey('${area.id.id}:primary', area.id, area.sseBuilder!, visible);
      }
      area.extraSseBuilders.forEach((targetArea, builder) {
        syncKey(
          '${area.id.id}:extra:${targetArea.id}',
          targetArea,
          builder,
          visible,
        );
      });
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
