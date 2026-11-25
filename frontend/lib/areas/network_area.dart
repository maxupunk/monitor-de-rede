import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/features/network/pages/device_detail_page.dart';
import 'package:netmonitor_app/features/network/pages/devices_page.dart';
import 'package:netmonitor_app/features/network/pages/discovery_page.dart';
import 'package:netmonitor_app/features/network/pages/link_tests_page.dart';
import 'package:netmonitor_app/features/network/pages/networks_page.dart';
import 'package:netmonitor_app/features/network/pages/probes_page.dart';
import 'package:netmonitor_app/features/network/pages/sites_page.dart';
import 'package:netmonitor_app/features/network/pages/topology_page.dart';
import 'package:netmonitor_app/features/network/pages/wifi_clients_page.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/link_tests_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/features/network/providers/topology_provider.dart';
import 'package:netmonitor_app/features/network/providers/wifi_provider.dart';
import 'package:netmonitor_app/features/network/widgets/device_access_host.dart';
import 'package:netmonitor_app/features/network/widgets/diagnostics_dialogs.dart';
import 'package:netmonitor_app/features/network/widgets/dns_widgets.dart';
import 'package:netmonitor_app/features/network/widgets/network_dashboard_widgets.dart';
import 'package:netmonitor_app/features/network/widgets/network_settings_cards.dart';
import 'package:netmonitor_app/features/network/widgets/snmp_credentials_card.dart';

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

void _scheduleDiscoveryRefresh(dynamic ref, SseBus bus) {
  bus.scheduleRefresh('discovery', () {
    ref.read(discoveryProvider.notifier).fetchDiscoveryRuns();
    ref.read(discoveryProvider.notifier).fetchConflicts();
  });
}

const AiScreenContribution _aiDevices = AiScreenContribution(
  id: 'devices',
  title: 'Dispositivos',
);
const AiScreenContribution _aiDeviceDetail = AiScreenContribution(
  id: 'device-detail',
  title: 'Detalhe do Dispositivo',
);
const AiScreenContribution _aiTopology = AiScreenContribution(
  id: 'topology',
  title: 'Topologia',
);
const AiScreenContribution _aiDiscovery = AiScreenContribution(
  id: 'discovery',
  title: 'Descoberta',
);
const AiScreenContribution _aiNetworks = AiScreenContribution(
  id: 'networks',
  title: 'Redes IP',
);
const AiScreenContribution _aiSites = AiScreenContribution(
  id: 'sites',
  title: 'Sites',
);
const AiScreenContribution _aiLinkTests = AiScreenContribution(
  id: 'link-tests',
  title: 'Testes de velocidade',
);
const AiScreenContribution _aiWifi = AiScreenContribution(
  id: 'wifi',
  title: 'Clientes Wi-Fi',
);

/// Área Rede + Dispositivos (Bucket 2 — ADR 017/019).
///
/// - `/devices` e `/devices/:id` declaram `area: ProductArea.core` em `routeMeta`
///   e seus handlers SSE/overlay vivem em `extraSseBuilders`/`extraShellOverlays`
///   para continuarem ativos mesmo se a área opcional `network` for desligada.
/// - As rotas `/plugins` e `/apps/:id` são registradas em `vpn_area.dart` (Bucket 4)
///   com `routeMeta: ProductArea.network`.
final AreaContribution networkArea = AreaContribution(
  id: ProductArea.network,
  routes: <RouteBase>[
    GoRoute(
      path: '/devices',
      name: 'devices',
      builder: (context, state) => DevicesPage(
        initialSiteId: int.tryParse(state.uri.queryParameters['siteId'] ?? ''),
      ),
    ),
    GoRoute(
      path: '/devices/:id',
      name: 'device-detail',
      builder: (context, state) => DeviceDetailPage(
        deviceId: int.tryParse(state.pathParameters['id'] ?? '') ?? 0,
        initialTab: state.uri.queryParameters['tab'],
        initialSection: state.uri.queryParameters['section'],
      ),
    ),
    GoRoute(
      path: '/topology',
      name: 'topology',
      builder: (context, state) => const TopologyPage(),
    ),
    GoRoute(
      path: '/discovery',
      name: 'discovery',
      builder: (context, state) => DiscoveryPage(
        initialNetworkId: int.tryParse(
          state.uri.queryParameters['networkId'] ?? '',
        ),
        autoStartScan: state.uri.queryParameters['scan'] == '1',
      ),
    ),
    GoRoute(
      path: '/networks',
      name: 'networks',
      builder: (context, state) => const NetworksPage(),
    ),
    GoRoute(
      path: '/sites',
      name: 'sites',
      builder: (context, state) => const SitesPage(),
    ),
    GoRoute(
      path: '/link-tests',
      name: 'link-tests',
      builder: (context, state) => LinkTestsPage(
        initialTab: state.uri.queryParameters['tab'],
        initialTestId: int.tryParse(state.uri.queryParameters['test'] ?? ''),
        initialDeviceId: int.tryParse(
          state.uri.queryParameters['deviceId'] ?? '',
        ),
      ),
    ),
    GoRoute(
      path: '/wifi',
      name: 'wifi-clients',
      builder: (context, state) =>
          WifiClientsPage(initialTab: state.uri.queryParameters['tab']),
    ),
    GoRoute(
      path: '/probes',
      name: 'probes',
      builder: (context, state) => const ProbesPage(),
    ),
  ],
  routeMeta: const <String, AreaRouteMeta>{
    '/devices': AreaRouteMeta(area: ProductArea.core, aiScreen: _aiDevices),
    '/devices/:id': AreaRouteMeta(
      area: ProductArea.core,
      aiScreen: _aiDeviceDetail,
    ),
    '/topology': AreaRouteMeta(
      area: ProductArea.network,
      fullBleed: true,
      aiScreen: _aiTopology,
    ),
    '/discovery': AreaRouteMeta(
      area: ProductArea.network,
      aiScreen: _aiDiscovery,
    ),
    '/networks': AreaRouteMeta(
      area: ProductArea.network,
      aiScreen: _aiNetworks,
    ),
    '/sites': AreaRouteMeta(area: ProductArea.network, aiScreen: _aiSites),
    '/link-tests': AreaRouteMeta(
      area: ProductArea.network,
      aiScreen: _aiLinkTests,
    ),
    '/wifi': AreaRouteMeta(area: ProductArea.network, aiScreen: _aiWifi),
    '/probes': AreaRouteMeta(area: ProductArea.network),
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
      item: NavItemSpec(
        title: 'Descoberta',
        icon: Icons.radar,
        to: '/discovery',
      ),
    ),
    const NavEntry(
      area: ProductArea.network,
      section: NavSectionId.network,
      order: 20,
      item: NavItemSpec(
        title: 'Redes IP',
        icon: Icons.lan_outlined,
        to: '/networks',
      ),
    ),
    const NavEntry(
      area: ProductArea.network,
      section: NavSectionId.network,
      order: 30,
      item: NavItemSpec(
        title: 'Sites',
        icon: Icons.domain_outlined,
        to: '/sites',
      ),
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
      item: NavItemSpec(
        title: 'Sondas de rede',
        icon: Icons.sensors,
        to: '/probes',
      ),
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
  navActions: <NavAction, NavActionHandler>{
    NavAction.playbook: (context, _) => showDiagnosticPlaybookDialog(context),
    NavAction.traceroute: (context, _) => showTracerouteDialog(context),
    NavAction.speedTest: (context, _) => showSpeedTestDialog(context),
  },
  widgets: <String, DashboardWidgetContribution>{
    'dns_latency': DashboardWidgetContribution(
      type: 'dns_latency',
      builder: (_, config) => DnsLatencyCard(config: config),
    ),
    'saas_services': DashboardWidgetContribution(
      type: 'saas_services',
      builder: (_, config) => SaasServicesWidget(config: config),
    ),
    'saas_heatmap': DashboardWidgetContribution(
      type: 'saas_heatmap',
      builder: (_, config) => SaasLatencyHeatmapWidget(config: config),
    ),
    'ether_bandwidth': DashboardWidgetContribution(
      type: 'ether_bandwidth',
      builder: (_, config) => EtherBandwidthWidget(config: config),
    ),
    'bandwidth_vs_latency': DashboardWidgetContribution(
      type: 'bandwidth_vs_latency',
      builder: (_, config) => BandwidthVsLatencyWidget(config: config),
    ),
  },
  settingsTabs: const <SettingsTabContribution>[
    SettingsTabContribution(
      value: 'rede',
      title: 'Rede',
      icon: Icons.lan_outlined,
      order: 30,
      area: ProductArea.network,
    ),
    SettingsTabContribution(
      value: 'wifi',
      title: 'Wi-Fi',
      icon: Icons.wifi,
      order: 35,
      area: ProductArea.network,
    ),
  ],
  settingsCards: <SettingsCardContribution>[
    SettingsCardContribution(
      tab: 'conectividade',
      order: 20,
      area: ProductArea.core,
      builder: (_) => const DnsServersCard(),
    ),
    SettingsCardContribution(
      tab: 'rede',
      order: 5,
      area: ProductArea.network,
      builder: (_) => const SnmpCredentialsCard(),
    ),
    SettingsCardContribution(
      tab: 'rede',
      order: 10,
      area: ProductArea.network,
      builder: (_) => const VendorRegistryCard(),
    ),
    SettingsCardContribution(
      tab: 'wifi',
      order: 10,
      area: ProductArea.network,
      builder: (_) => const WifiSettingsCard(),
    ),
  ],
  aiScreens: const <String, AiScreenContribution>{
    'devices': _aiDevices,
    'device-detail': _aiDeviceDetail,
    'sites': _aiSites,
    'networks': _aiNetworks,
    'discovery': _aiDiscovery,
    'link-tests': _aiLinkTests,
    'topology': _aiTopology,
    'wifi': _aiWifi,
  },
  extraShellOverlays: <ProductArea, List<WidgetBuilder>>{
    ProductArea.core: <WidgetBuilder>[(_) => const DeviceAccessHost()],
  },
  extraSseBuilders: <ProductArea, AreaSseRoutesBuilder>{
    // Eventos de Dispositivos (área Core — sempre ativos mesmo com Rede desligada).
    ProductArea.core: (ref) => <String, SseRoute>{
      'device:status': SseRoute(
        handle: (data, _, _) {
          ref.read(devicesProvider.notifier).applyRealtimeStatus(data);
          ref.read(deviceDetailProvider.notifier).applyDeviceStatus(data);
        },
      ),
      'device:updated': SseRoute(
        handle: (data, _, _) {
          ref.read(devicesProvider.notifier).applyRealtimeStatus(data);
          ref.read(deviceDetailProvider.notifier).applyDeviceStatus(data);
        },
      ),
      'monitor:result': SseRoute(
        handle: (data, _, _) {
          ref.read(deviceDetailProvider.notifier).applyMonitorResult(data);
          ref.read(deviceDetailProvider.notifier).applyRecordedMetrics(data);
        },
      ),
      'metric:recorded': SseRoute(
        handle: (data, _, _) =>
            ref.read(deviceDetailProvider.notifier).applyRecordedMetrics(data),
      ),
      'interface:traffic': SseRoute(
        ephemeral: true,
        handle: (data, _, _) =>
            ref.read(deviceDetailProvider.notifier).applyInterfaceTraffic(data),
      ),
      'interface:status_change': SseRoute(
        handle: (data, event, _) => ref
            .read(deviceDetailProvider.notifier)
            .applyInterfaceChange(event.type, data),
      ),
      'interface:speed_change': SseRoute(
        handle: (data, event, _) => ref
            .read(deviceDetailProvider.notifier)
            .applyInterfaceChange(event.type, data),
      ),
      'interface:speed_downgrade': SseRoute(
        handle: (data, event, _) => ref
            .read(deviceDetailProvider.notifier)
            .applyInterfaceChange(event.type, data),
      ),
    },
  },
  sseBuilder: (ref) => <String, SseRoute>{
    // Topologia em tempo real
    'monitor:result': SseRoute(
      handle: (data, _, _) =>
          ref.read(topologyProvider.notifier).applyRealtimeHealth(data),
    ),
    'device:status': SseRoute(
      handle: (data, _, _) =>
          ref.read(topologyProvider.notifier).applyRealtimeStatus(data),
    ),
    'device:updated': SseRoute(
      handle: (data, _, _) =>
          ref.read(topologyProvider.notifier).applyRealtimeStatus(data),
    ),
    'interface:traffic': SseRoute(
      ephemeral: true,
      handle: (data, _, _) =>
          ref.read(topologyProvider.notifier).applyRealtimeTraffic(data),
    ),
    'interface:status_change': SseRoute(
      handle: (data, event, _) => ref
          .read(topologyProvider.notifier)
          .applyInterfaceChange(event.type, data),
    ),
    'interface:speed_change': SseRoute(
      handle: (data, event, _) => ref
          .read(topologyProvider.notifier)
          .applyInterfaceChange(event.type, data),
    ),
    'interface:speed_downgrade': SseRoute(
      handle: (data, event, _) => ref
          .read(topologyProvider.notifier)
          .applyInterfaceChange(event.type, data),
    ),
    'topology:updated': SseRoute(
      handle: (_, _, bus) => bus.scheduleRefresh(
        'topology',
        () => ref.read(topologyProvider.notifier).refreshIfLoaded(),
      ),
    ),
    'topology:layout_updated': SseRoute(
      handle: (data, _, _) =>
          ref.read(topologyProvider.notifier).applyLayoutUpdated(data),
    ),
    // Sondas
    'probe:status': SseRoute(
      handle: (data, _, _) =>
          ref.read(probesProvider.notifier).applyRealtimeStatus(data),
    ),
    // Descoberta de rede
    'discovery:scan': SseRoute(
      ephemeral: true,
      handle: (data, _, _) =>
          ref.read(discoveryProvider.notifier).applyScanSnapshot(data),
    ),
    'discovery:started': SseRoute(
      handle: (_, _, bus) => _scheduleDiscoveryRefresh(ref, bus),
    ),
    'discovery:completed': SseRoute(
      handle: (_, _, bus) => _scheduleDiscoveryRefresh(ref, bus),
    ),
    'discovery:cancelled': SseRoute(
      handle: (_, _, bus) => _scheduleDiscoveryRefresh(ref, bus),
    ),
    'discovery:failed': SseRoute(
      handle: (_, _, bus) => _scheduleDiscoveryRefresh(ref, bus),
    ),
    'discovery:conflicts_detected': SseRoute(
      handle: (data, _, _) =>
          ref.read(discoveryProvider.notifier).applyConflictsDetected(data),
    ),
    // Testes de enlace
    'link_test:updated': SseRoute(
      ephemeral: true,
      handle: (data, _, _) =>
          ref.read(linkTestsProvider.notifier).applyUpdated(data),
    ),
    'link_test:sample': SseRoute(
      ephemeral: true,
      handle: (data, _, _) =>
          ref.read(linkTestsProvider.notifier).applySample(data),
    ),
    'link_test:health': SseRoute(
      ephemeral: true,
      handle: (data, _, _) =>
          ref.read(linkTestsProvider.notifier).applyHealth(data),
    ),
    'link_test_schedule:updated': SseRoute(
      handle: (data, _, _) =>
          ref.read(linkTestsProvider.notifier).applyScheduleUpdated(data),
    ),
    // Wi-Fi em tempo real
    'wifi:snapshot': SseRoute(
      ephemeral: true,
      handle: (data, _, _) =>
          ref.read(wifiClientsProvider.notifier).applySnapshot(data),
    ),
    'wifi:watched': SseRoute(
      ephemeral: true,
      handle: (data, _, _) =>
          ref.read(wifiClientsProvider.notifier).applyWatched(data),
    ),
    'wifi:roamed': SseRoute(
      ephemeral: true,
      handle: (data, _, _) =>
          ref.read(wifiClientsProvider.notifier).applyRoamed(data),
    ),
  },
);
