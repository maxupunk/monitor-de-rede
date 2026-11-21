import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/areas/area_registry.dart';
import 'package:netmonitor_app/areas/composition_registries.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';

void main() {
  group('area_registry cross-area composition & deduplication', () {
    test('deduplicateAreaRoutes keeps last GoRoute per path without duplicates', () {
      final coreStub = AreaContribution(
        id: ProductArea.core,
        routes: <RouteBase>[
          GoRoute(
            path: '/ai-chat',
            name: 'ai-chat-placeholder',
            builder: (_, _) => const SizedBox.shrink(),
          ),
          GoRoute(
            path: '/',
            name: 'dashboard',
            builder: (_, _) => const SizedBox.shrink(),
          ),
        ],
      );
      final vpnReal = AreaContribution(
        id: ProductArea.vpn,
        routes: <RouteBase>[
          GoRoute(
            path: '/ai-chat',
            name: 'ai-chat',
            builder: (_, _) => const Text('real-ai-chat'),
          ),
        ],
      );

      final routes = deduplicateAreaRoutes(<AreaContribution>[coreStub, vpnReal]);
      expect(routes.length, 2);
      final aiRoute = routes.whereType<GoRoute>().firstWhere((r) => r.path == '/ai-chat');
      expect(aiRoute.name, 'ai-chat');
    });

    test('SettingsCardContribution.area keeps core AI cards visible when vpn area is off', () {
      final coreMod = AreaContribution(
        id: ProductArea.core,
        settingsTabs: const <SettingsTabContribution>[
          SettingsTabContribution(
            value: 'ia',
            title: 'Inteligência artificial',
            icon: Icons.smart_toy_outlined,
            order: 50,
          ),
        ],
      );
      final vpnMod = AreaContribution(
        id: ProductArea.vpn,
        settingsCards: <SettingsCardContribution>[
          SettingsCardContribution(
            tab: 'ia',
            order: 10,
            area: ProductArea.core,
            builder: (_) => const SizedBox.shrink(),
          ),
          SettingsCardContribution(
            tab: 'ia',
            order: 20,
            // Sem `area` explícito -> herda `ProductArea.vpn`
            builder: (_) => const SizedBox.shrink(),
          ),
        ],
      );

      final tabsWhenVpnOff = buildVisibleSettingsTabsFrom(
        <AreaContribution>[coreMod, vpnMod],
        const <ProductArea>{ProductArea.core},
      );
      expect(tabsWhenVpnOff.length, 1);
      expect(tabsWhenVpnOff.first.tab.value, 'ia');
      expect(tabsWhenVpnOff.first.cards.length, 1);
      expect(tabsWhenVpnOff.first.cards.first.order, 10);
    });

    test('extraShellOverlays respects per-area ADR 017 visibility independently of module id', () {
      Widget coreOverlay(BuildContext _) => const SizedBox.shrink();
      Widget networkOverlay(BuildContext _) => const SizedBox.shrink();
      Widget vpnOverlay(BuildContext _) => const SizedBox.shrink();

      final vpnMod = AreaContribution(
        id: ProductArea.vpn,
        shellOverlays: <WidgetBuilder>[vpnOverlay],
        extraShellOverlays: <ProductArea, List<WidgetBuilder>>{
          ProductArea.core: <WidgetBuilder>[coreOverlay],
          ProductArea.network: <WidgetBuilder>[networkOverlay],
        },
      );

      final onlyCore = visibleShellOverlaysFrom(
        <AreaContribution>[vpnMod],
        const <ProductArea>{ProductArea.core},
      );
      expect(onlyCore, <WidgetBuilder>[coreOverlay]);

      final coreAndNetwork = visibleShellOverlaysFrom(
        <AreaContribution>[vpnMod],
        const <ProductArea>{ProductArea.core, ProductArea.network},
      );
      expect(coreAndNetwork, <WidgetBuilder>[coreOverlay, networkOverlay]);
    });
  });
}
