import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';

NavItemSpec? _appsGroupBuilder(NavContext ctx) {
  if (ctx.apps.isEmpty) return null;
  return NavItemSpec(
    title: 'Aplicativos',
    icon: Icons.apps,
    children: ctx.apps
        .map(
          (app) => NavLeafSpec(
            title: app.title,
            icon: app.icon,
            to: '/apps/${app.id}',
          ),
        )
        .toList(growable: false),
  );
}

/// Área Rede (ADR 017): topologia, descoberta, redes IP, sites, enlaces, Wi-Fi, sondas e plugins.
///
/// Worker dono: Network (`lib/areas/network_area.dart` + `lib/features/network/`).
final AreaContribution networkArea = AreaContribution(
  id: ProductArea.network,
  routes: <RouteBase>[
    GoRoute(
      path: '/topology',
      name: 'topology',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Topologia',
        area: ProductArea.network,
        icon: Icons.account_tree_outlined,
      ),
    ),
    GoRoute(
      path: '/discovery',
      name: 'discovery',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Descoberta',
        area: ProductArea.network,
        icon: Icons.radar,
      ),
    ),
    GoRoute(
      path: '/networks',
      name: 'networks',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Redes IP',
        area: ProductArea.network,
        icon: Icons.lan_outlined,
      ),
    ),
    GoRoute(
      path: '/sites',
      name: 'sites',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Sites',
        area: ProductArea.network,
        icon: Icons.domain_outlined,
      ),
    ),
    GoRoute(
      path: '/link-tests',
      name: 'link-tests',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Testes de velocidade',
        area: ProductArea.network,
        icon: Icons.speed,
      ),
    ),
    GoRoute(
      path: '/wifi',
      name: 'wifi-clients',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Sinais Wi-Fi',
        area: ProductArea.network,
        icon: Icons.wifi,
      ),
    ),
    GoRoute(
      path: '/probes',
      name: 'probes',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Sondas de rede',
        area: ProductArea.network,
        icon: Icons.sensors,
      ),
    ),
    GoRoute(
      path: '/apps/:id',
      name: 'plugin-app',
      builder: (context, state) => FeaturePlaceholderPage(
        title: 'Aplicativo #${state.pathParameters['id'] ?? ''}',
        area: ProductArea.network,
        icon: Icons.extension_outlined,
      ),
    ),
    GoRoute(
      path: '/plugins',
      name: 'plugins',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Plugins de dispositivo',
        area: ProductArea.network,
        icon: Icons.extension_outlined,
      ),
    ),
  ],
  routeMeta: const <String, AreaRouteMeta>{
    '/topology': AreaRouteMeta(area: ProductArea.network, fullBleed: true),
    '/discovery': AreaRouteMeta(area: ProductArea.network),
    '/networks': AreaRouteMeta(area: ProductArea.network),
    '/sites': AreaRouteMeta(area: ProductArea.network),
    '/link-tests': AreaRouteMeta(area: ProductArea.network),
    '/wifi': AreaRouteMeta(area: ProductArea.network),
    '/probes': AreaRouteMeta(area: ProductArea.network),
    '/apps': AreaRouteMeta(area: ProductArea.network),
    '/plugins': AreaRouteMeta(area: ProductArea.network, requiresAdmin: true),
  },
  nav: <NavEntry>[
    const NavEntry(
      area: ProductArea.network,
      section: NavSectionId.monitoring,
      order: 20,
      item: NavItemSpec(
        title: 'Topologia',
        icon: Icons.account_tree_outlined,
        to: '/topology',
      ),
    ),
    const NavEntry(
      area: ProductArea.network,
      section: NavSectionId.network,
      order: 10,
      item: NavItemSpec(title: 'Descoberta', icon: Icons.radar, to: '/discovery'),
    ),
    const NavEntry(
      area: ProductArea.network,
      section: NavSectionId.network,
      order: 20,
      item: NavItemSpec(title: 'Redes IP', icon: Icons.lan_outlined, to: '/networks'),
    ),
    const NavEntry(
      area: ProductArea.network,
      section: NavSectionId.network,
      order: 30,
      item: NavItemSpec(title: 'Sites', icon: Icons.domain_outlined, to: '/sites'),
    ),
    const NavEntry(
      area: ProductArea.network,
      section: NavSectionId.network,
      order: 40,
      item: NavItemSpec(
        title: 'Testes de velocidade',
        icon: Icons.speed,
        to: '/link-tests',
      ),
    ),
    const NavEntry(
      area: ProductArea.network,
      section: NavSectionId.network,
      order: 50,
      item: NavItemSpec(title: 'Sinais Wi-Fi', icon: Icons.wifi, to: '/wifi'),
    ),
    const NavEntry(
      area: ProductArea.network,
      section: NavSectionId.network,
      order: 60,
      item: NavItemSpec(title: 'Sondas de rede', icon: Icons.sensors, to: '/probes'),
    ),
    const NavEntry(
      area: ProductArea.network,
      section: NavSectionId.network,
      order: 70,
      item: NavItemSpec(
        title: 'Diagnóstico',
        icon: Icons.medical_services_outlined,
        children: <NavLeafSpec>[
          NavLeafSpec(
            title: 'Playbooks',
            icon: Icons.playlist_play,
            action: NavAction.playbook,
          ),
          NavLeafSpec(
            title: 'Traceroute ICMP',
            icon: Icons.alt_route,
            action: NavAction.traceroute,
          ),
          NavLeafSpec(
            title: 'Velocidade do navegador',
            icon: Icons.network_check,
            action: NavAction.speedTest,
          ),
        ],
      ),
    ),
    NavEntry(
      area: ProductArea.network,
      section: NavSectionId.network,
      order: 80,
      builder: _appsGroupBuilder,
    ),
    const NavEntry(
      area: ProductArea.network,
      section: NavSectionId.admin,
      order: 30,
      item: NavItemSpec(
        title: 'Plugins de dispositivo',
        icon: Icons.extension_outlined,
        to: '/plugins',
        admin: true,
      ),
    ),
  ],
  settingsTabs: const <SettingsTabContribution>[
    SettingsTabContribution(
      value: 'rede',
      title: 'Rede',
      icon: Icons.lan_outlined,
      order: 30,
    ),
    SettingsTabContribution(
      value: 'wifi',
      title: 'Wi-Fi',
      icon: Icons.wifi,
      order: 35,
    ),
  ],
);
