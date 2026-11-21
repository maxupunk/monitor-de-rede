import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';

/// Área VPN WireGuard (ADR 017): servidor VPN e dispositivos conectados.
///
/// Worker dono: VPN (`lib/areas/vpn_area.dart` + `lib/features/vpn/`).
final AreaContribution vpnArea = AreaContribution(
  id: ProductArea.vpn,
  routes: <RouteBase>[
    GoRoute(
      path: '/vpn',
      name: 'vpn-server',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'VPN WireGuard',
        area: ProductArea.vpn,
        icon: Icons.vpn_lock_outlined,
      ),
      routes: <RouteBase>[
        GoRoute(
          path: 'devices',
          name: 'vpn-devices',
          builder: (context, state) => const FeaturePlaceholderPage(
            title: 'VPN · Dispositivos',
            area: ProductArea.vpn,
            icon: Icons.devices_other_outlined,
          ),
        ),
      ],
    ),
  ],
  routeMeta: const <String, AreaRouteMeta>{
    '/vpn': AreaRouteMeta(area: ProductArea.vpn),
  },
  nav: const <NavEntry>[
    NavEntry(
      area: ProductArea.vpn,
      section: NavSectionId.vpn,
      order: 10,
      item: NavItemSpec(
        title: 'VPN WireGuard',
        icon: Icons.vpn_lock_outlined,
        to: '/vpn',
      ),
    ),
  ],
  serverConnectPaths: const <ServerConnectPathContribution>[
    ServerConnectPathContribution(
      id: 'vpn',
      order: 20,
      title: 'Pela VPN',
      description:
          'Servidor em outra rede. Um script só sobe o túnel WireGuard até o IP público desta central e instala o agente, que passa a falar com ela apenas por dentro do túnel.',
      icon: Icons.vpn_lock_outlined,
      recommended: true,
      moveExistingLabel: 'Mover para a VPN',
      moveExistingIcon: Icons.vpn_lock_outlined,
    ),
  ],
);
