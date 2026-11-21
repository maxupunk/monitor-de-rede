import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';

/// Área Servidores (ADR 014 / 017 / 021 / 022): Docker dos hosts, deploys contínuos e Registry.
///
/// Worker dono: Servers (`lib/areas/servers_area.dart` + `lib/features/servers/`).
final AreaContribution serversArea = AreaContribution(
  id: ProductArea.servers,
  routes: <RouteBase>[
    GoRoute(
      path: '/docker',
      name: 'docker-overview',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Docker',
        area: ProductArea.servers,
        icon: Icons.view_in_ar_outlined,
      ),
      routes: <RouteBase>[
        GoRoute(
          path: 'containers',
          name: 'docker-containers',
          builder: (context, state) => const FeaturePlaceholderPage(
            title: 'Docker · Containers',
            area: ProductArea.servers,
            icon: Icons.view_in_ar_outlined,
          ),
        ),
        GoRoute(
          path: 'volumes',
          name: 'docker-volumes',
          builder: (context, state) => const FeaturePlaceholderPage(
            title: 'Docker · Volumes',
            area: ProductArea.servers,
            icon: Icons.storage_outlined,
          ),
        ),
        GoRoute(
          path: 'networks',
          name: 'docker-networks',
          builder: (context, state) => const FeaturePlaceholderPage(
            title: 'Docker · Redes',
            area: ProductArea.servers,
            icon: Icons.hub_outlined,
          ),
        ),
        GoRoute(
          path: 'images',
          name: 'docker-images',
          builder: (context, state) => const FeaturePlaceholderPage(
            title: 'Docker · Imagens',
            area: ProductArea.servers,
            icon: Icons.layers_outlined,
          ),
        ),
        GoRoute(
          path: 'compose',
          name: 'docker-compose',
          builder: (context, state) => const FeaturePlaceholderPage(
            title: 'Docker · Compose',
            area: ProductArea.servers,
            icon: Icons.description_outlined,
          ),
        ),
        GoRoute(
          path: 'history',
          name: 'docker-history',
          builder: (context, state) => const FeaturePlaceholderPage(
            title: 'Docker · Histórico',
            area: ProductArea.servers,
            icon: Icons.history,
          ),
        ),
      ],
    ),
    GoRoute(
      path: '/deploys',
      name: 'deploys',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Deploys',
        area: ProductArea.servers,
        icon: Icons.rocket_launch_outlined,
      ),
    ),
    GoRoute(
      path: '/deploys/github',
      redirect: (_, state) {
        final query = state.uri.query;
        return query.isEmpty ? '/deploys' : '/deploys?$query';
      },
    ),
    GoRoute(
      path: '/registry',
      name: 'registry',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Registry',
        area: ProductArea.servers,
        icon: Icons.inventory_2_outlined,
      ),
    ),
  ],
  routeMeta: const <String, AreaRouteMeta>{
    '/docker': AreaRouteMeta(area: ProductArea.servers),
    '/deploys': AreaRouteMeta(area: ProductArea.servers),
    '/registry': AreaRouteMeta(area: ProductArea.servers),
  },
  nav: const <NavEntry>[
    NavEntry(
      area: ProductArea.servers,
      section: NavSectionId.servers,
      order: 10,
      item: NavItemSpec(title: 'Docker', icon: Icons.view_in_ar_outlined, to: '/docker'),
    ),
    NavEntry(
      area: ProductArea.servers,
      section: NavSectionId.servers,
      order: 15,
      item: NavItemSpec(title: 'Deploys', icon: Icons.rocket_launch_outlined, to: '/deploys'),
    ),
    NavEntry(
      area: ProductArea.servers,
      section: NavSectionId.servers,
      order: 16,
      item: NavItemSpec(
        title: 'Registry',
        icon: Icons.inventory_2_outlined,
        to: '/registry',
      ),
    ),
  ],
);
