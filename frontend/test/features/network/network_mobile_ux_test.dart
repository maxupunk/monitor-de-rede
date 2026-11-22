import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/core/models/log_models.dart';
import 'package:netmonitor_app/features/core/providers/logs_provider.dart';
import 'package:netmonitor_app/features/core/providers/settings_provider.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/models/link_test_models.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/models/wifi_models.dart';
import 'package:netmonitor_app/features/network/pages/device_detail_page.dart';
import 'package:netmonitor_app/features/network/pages/devices_page.dart';
import 'package:netmonitor_app/features/network/pages/link_tests_page.dart';
import 'package:netmonitor_app/features/network/pages/networks_page.dart';
import 'package:netmonitor_app/features/network/pages/probes_page.dart';
import 'package:netmonitor_app/features/network/pages/sites_page.dart';
import 'package:netmonitor_app/features/network/pages/topology_page.dart';
import 'package:netmonitor_app/features/network/pages/wifi_clients_page.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/diagnostics_provider.dart';
import 'package:netmonitor_app/features/network/providers/link_tests_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/features/network/providers/topology_provider.dart';
import 'package:netmonitor_app/features/network/providers/wifi_provider.dart';
import 'package:netmonitor_app/features/network/widgets/device_access_host.dart';
import 'package:netmonitor_app/features/network/widgets/device_form_sheet.dart';
import 'package:netmonitor_app/features/network/widgets/diagnostics_dialogs.dart';
import 'package:netmonitor_app/features/network/widgets/dns_widgets.dart';
import 'package:netmonitor_app/features/network/widgets/network_settings_cards.dart';
import 'package:netmonitor_app/features/network/widgets/wifi_qr_sheet.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class _StubAuthNotifier extends AuthNotifier {
  @override
  AuthState build() => const AuthState(
    token: 'test-token',
    user: AuthUser(
      id: 1,
      fullName: 'Admin',
      email: 'admin@local',
      role: UserRole.admin,
    ),
  );
}

class _StubAreasNotifier extends AreasNotifier {
  @override
  AreasState build() => const AreasState();
}

class _StubSitesNotifier extends SitesNotifier {
  @override
  SitesState build() => SitesState(
    items: <SiteModel>[
      SiteModel.fromJson(const <String, dynamic>{
        'id': 1,
        'name': 'Matriz São Paulo',
        'location': 'Av. Paulista, 1000',
        'devicesCount': 12,
        'networksCount': 3,
      }),
      SiteModel.fromJson(const <String, dynamic>{
        'id': 2,
        'name': 'Filial Campinas',
        'location': 'Centro · Campinas - SP',
        'devicesCount': 5,
        'networksCount': 1,
      }),
    ],
  );

  @override
  Future<void> fetchSites() async {}
}

class _StubNetworksNotifier extends NetworksNotifier {
  @override
  NetworksState build() => NetworksState(
    items: <NetworkModel>[
      NetworkModel.fromJson(const <String, dynamic>{
        'id': 1,
        'name': 'LAN Corporativa Principal',
        'cidr': '10.10.0.0/24',
        'gateway': '10.10.0.1',
        'vlanId': 10,
        'usableHosts': 254,
        'scannable': true,
        'scanEnabled': true,
        'siteId': 1,
      }),
    ],
  );

  @override
  Future<void> fetchNetworks() async {}
}

class _StubProbesNotifier extends ProbesNotifier {
  @override
  ProbesState build() => ProbesState(
    items: <ProbeModel>[
      ProbeModel.fromJson(const <String, dynamic>{
        'id': 1,
        'name': 'vpn-probe',
        'status': 'online',
        'ipAddress': '10.8.0.2',
        'probeType': 'vpn-probe',
        'version': '2.1.0',
      }),
    ],
  );

  @override
  Future<void> fetchProbes() async {}
}

class _StubDevicesNotifier extends DevicesNotifier {
  @override
  DevicesState build() => DevicesState(
    items: <DeviceModel>[
      DeviceModel.fromJson(const <String, dynamic>{
        'id': 1,
        'name': 'MikroTik CCR2004 Principal',
        'type': 'router',
        'status': 'online',
        'ipAddress': '10.10.0.1',
        'macAddress': 'DC:2C:6E:00:00:01',
        'vendor': 'MikroTik',
        'model': 'CCR2004-16G-2S+',
        'snmpEnabled': true,
        'siteId': 1,
        'networkId': 1,
      }),
      DeviceModel.fromJson(const <String, dynamic>{
        'id': 2,
        'name': 'AP OpenWrt Recepção',
        'type': 'access_point',
        'status': 'warning',
        'ipAddress': '10.10.0.25',
        'vendor': 'OpenWrt',
        'snmpEnabled': true,
        'siteId': 1,
        'networkId': 1,
      }),
    ],
  );

  @override
  Future<void> fetchDevices({int? siteId, int? networkId}) async {}
}

class _StubDeviceDetailNotifier extends DeviceDetailNotifier {
  @override
  DeviceDetailState build() => DeviceDetailState(
    device: DeviceModel.fromJson(const <String, dynamic>{
      'id': 2,
      'name': 'AP OpenWrt Recepção',
      'type': 'access_point',
      'status': 'online',
      'ipAddress': '10.10.0.25',
      'macAddress': 'AA:BB:CC:DD:EE:02',
      'vendor': 'OpenWrt',
      'snmpEnabled': true,
    }),
    capabilities: const DeviceCapabilitiesModel(deviceId: 2, wifi: true),
    interfaces: <DeviceInterfaceModel>[
      DeviceInterfaceModel.fromJson(const <String, dynamic>{
        'id': 10,
        'ifIndex': 1,
        'name': 'eth0',
        'operStatus': 'up',
        'speed': 1000000000,
        'inBps': 24000000,
        'outBps': 12000000,
        'isMonitored': true,
      }),
    ],
    monitors: <DeviceMonitorSummary>[
      DeviceMonitorSummary.fromJson(const <String, dynamic>{
        'id': 5,
        'name': 'ICMP Ping',
        'type': 'ping',
        'status': 'online',
        'latencyMs': 2.4,
      }),
    ],
    events: <DeviceEventModel>[
      DeviceEventModel.fromJson(const <String, dynamic>{
        'id': 1,
        'eventType': 'status_change',
        'severity': 'info',
        'message': 'Equipamento respondeu normalmente.',
        'createdAt': '2026-11-21T12:00:00Z',
      }),
    ],
  );

  @override
  Future<void> loadDeviceDetails(int id) async {}
}

class _StubLinkTestsNotifier extends LinkTestsNotifier {
  @override
  LinkTestsState build() {
    const active = LinkTestModel(
      id: 77,
      kind: 'link',
      status: 'running',
      origin: 'manual',
      spec: LinkTestSpecModel(durationS: 10, direction: 'both', streams: 4),
      legs: <LinkTestLegModel>[
        LinkTestLegModel(
          seq: 0,
          kind: 'link',
          status: 'running',
          durationS: 10,
          a: LinkTestEndpointModel(kind: 'device', name: 'Core-CCR'),
          b: LinkTestEndpointModel(kind: 'device', name: 'SW-Distribuicao'),
          forwardBps: 940000000,
          reverseBps: 910000000,
        ),
      ],
      summary: LinkTestSummaryModel(
        minForwardBps: 940000000,
        minReverseBps: 910000000,
      ),
      createdAt: '2026-11-21T14:00:00Z',
    );
    return LinkTestsState(
      focusedId: 77,
      tests: const <int, LinkTestModel>{77: active},
      pageItems: const <LinkTestModel>[active],
      total: 1,
      schedules: <LinkTestScheduleModel>[
        LinkTestScheduleModel.fromJson(const <String, dynamic>{
          'id': 1,
          'name': 'Teste Noturno Backbone Principal',
          'kind': 'link',
          'intervalMinutes': 60,
          'windowStart': '02:00',
          'windowEnd': '05:00',
          'enabled': true,
          'spec': <String, dynamic>{
            'durationS': 10,
            'direction': 'both',
            'streams': 4,
          },
        }),
      ],
    );
  }

  @override
  Future<void> fetchPage({
    int page = 1,
    int perPage = linkTestHistoryPerPage,
    int? deviceId,
    int? linkId,
    String? status,
    String? kind,
  }) async {}

  @override
  Future<void> fetchSchedules() async {}
}

class _StubWifiClientsNotifier extends WifiClientsNotifier {
  @override
  WifiClientsState build() => WifiClientsState(
    snapshots: const <int, WifiDeviceSnapshotModel>{
      2: WifiDeviceSnapshotModel(
        deviceId: 2,
        deviceName: 'AP OpenWrt Recepção',
        status: 'ok',
        canPing: true,
        stations: <WifiStationModel>[
          WifiStationModel(
            mac: 'aa:bb:cc:dd:ee:10',
            ip: '10.10.0.105',
            hostname: 'Notebook-Diretoria',
            signalDbm: -54,
            signalGrade: 'excellent',
            health: 'good',
            linkScore: 0.92,
            ssid: 'CorpWiFi-5G',
            band: '5GHz',
            txRateBps: 866700000,
          ),
          WifiStationModel(
            mac: 'aa:bb:cc:dd:ee:22',
            vendor: 'Tuya Smart',
            signalDbm: -68,
            signalGrade: 'good',
            health: 'good',
            linkScore: 0.74,
            ssid: 'CorpWiFi-IoT',
            band: '2.4GHz',
          ),
          WifiStationModel(
            mac: 'aa:bb:cc:dd:ee:33',
            vendor: 'Espressif',
            ip: '10.10.0.142',
            signalDbm: -69,
            signalGrade: 'good',
            health: 'good',
            linkScore: 0.71,
            ssid: 'CorpWiFi-IoT',
            band: '2.4GHz',
          ),
        ],
      ),
    },
    watched: <WifiWatchedClientModel>[
      WifiWatchedClientModel.fromJson(const <String, dynamic>{
        'id': 1,
        'mac': 'aa:bb:cc:dd:ee:10',
        'name': 'Notebook Diretoria',
        'state': 'online',
        'deviceId': 2,
        'deviceName': 'AP OpenWrt Recepção',
      }),
    ],
  );
}

class _StubTopologyNotifier extends TopologyNotifier {
  @override
  TopologyState build() => TopologyState(
    nodes: const <TopologyNodeModel>[
      TopologyNodeModel(
        id: 1,
        name: 'Core-CCR',
        type: 'router',
        status: 'online',
        ipAddress: '10.10.0.1',
      ),
      TopologyNodeModel(
        id: 2,
        name: 'AP OpenWrt Recepção',
        type: 'access_point',
        status: 'online',
        ipAddress: '10.10.0.25',
      ),
    ],
    edges: const <TopologyEdgeModel>[
      TopologyEdgeModel(
        id: 10,
        sourceDeviceId: 1,
        targetDeviceId: 2,
        linkType: 'ethernet',
        discoveryMethod: 'lldp',
        status: 'up',
        speedBps: 1000000000,
      ),
    ],
    positions: const <int, TopologyPosition>{
      1: TopologyPosition(120, 120),
      2: TopologyPosition(420, 120),
    },
  );

  @override
  Future<void> fetchTopology({
    int? siteId,
    bool updateSiteFilter = false,
    bool showLoading = true,
    bool preserveUnsavedPositions = true,
  }) async {}
}

class _StubDnsServersNotifier extends DnsServersNotifier {
  @override
  DnsServersState build() => DnsServersState(
    items: <DnsServerModel>[
      DnsServerModel.fromJson(const <String, dynamic>{
        'id': 1,
        'name': 'Cloudflare',
        'address': '1.1.1.1',
        'protocol': 'udp',
        'isDefault': true,
      }),
    ],
  );

  @override
  Future<void> fetchServers() async {}
}

class _StubVendorsNotifier extends VendorsNotifier {
  @override
  VendorsState build() => const VendorsState(
    status: VendorRegistryStatus(
      entries: 35000,
      builtinEntries: 120,
      stale: false,
      autoUpdate: true,
      updating: false,
      sources: <String>['IEEE OUI'],
    ),
  );

  @override
  Future<void> fetchStatus() async {}
}

class _StubWifiSettingsNotifier extends WifiSettingsNotifier {
  @override
  WifiSettingsState build() => const WifiSettingsState(
    settings: WifiSettingsModel(
      liveIntervalS: 5,
      backgroundIntervalS: 60,
      environment: 'office',
      roamingRetentionDays: 30,
      pingPongCount: 4,
      pingPongWindowMin: 10,
    ),
  );

  @override
  Future<void> fetch() async {}
}

class _StubCoreSettingsNotifier extends CoreSettingsNotifier {
  @override
  CoreSettingsState build() => const CoreSettingsState();

  @override
  Future<void> fetchServerAddresses({bool force = false}) async {}
}

class _StubOperatingSystemsNotifier extends OperatingSystemsNotifier {
  @override
  List<OperatingSystemOption> build() => const <OperatingSystemOption>[
    OperatingSystemOption(
      id: 'openwrt',
      label: 'OpenWrt',
      supportsSyslog: true,
    ),
    OperatingSystemOption(
      id: 'routeros',
      label: 'MikroTik RouterOS',
      supportsSyslog: true,
    ),
  ];

  @override
  Future<void> fetchAll({bool force = false}) async {}

  @override
  Future<IdentifyResultModel?> identify({
    String? name,
    required String? ipAddress,
    String? snmpVersion,
    String? snmpCommunity,
    String? vendor,
    String? model,
  }) async => null;
}

class _StubSnmpProfilesNotifier extends SnmpProfilesNotifier {
  @override
  List<SnmpProfileModel> build() => const <SnmpProfileModel>[];

  @override
  Future<void> fetchProfiles() async {}
}

class _StubLogsNotifier extends LogsNotifier {
  @override
  LogsState build() => const LogsState();

  @override
  Future<SetupGuide?> fetchSetupGuide({String? address}) async {
    const guide = SetupGuide(
      serverAddress: '10.10.0.10',
      port: 514,
      snippets: <SetupSnippet>[
        SetupSnippet(
          id: 'openwrt',
          label: 'OpenWrt (logread)',
          commands: "uci set system.@system[0].log_ip='10.10.0.10'",
        ),
      ],
    );
    state = state.copyWith(setupGuide: guide);
    return guide;
  }

  @override
  Future<ProvisionHintsResponse?> fetchProvisionHints(int deviceId) async =>
      null;
}

class _StubDeviceAccessController extends DeviceAccessController {
  _StubDeviceAccessController({
    this.dedicatedPlatform = 'openwrt',
    this.provisionError,
  });

  final String? dedicatedPlatform;
  final Object? provisionError;
  Map<String, dynamic>? lastSavedCredentialPayload;
  Map<String, dynamic>? lastProvisionPayload;

  @override
  DeviceAccessRequest? build() => null;

  @override
  Future<DeviceAccessViewModel> fetchAccess(int deviceId) async {
    return DeviceAccessViewModel(
      deviceId: deviceId,
      deviceName: 'AP OpenWrt Recepção',
      dedicatedPlatform: dedicatedPlatform,
      agents: const <AgentRouteOption>[
        AgentRouteOption(id: 3, name: 'Agente Matriz', online: true),
      ],
    );
  }

  @override
  Future<void> saveCredential(
    int deviceId,
    Map<String, dynamic> payload,
  ) async {
    lastSavedCredentialPayload = Map<String, dynamic>.from(payload);
  }

  @override
  Future<DeviceCredentialView> provisionDedicatedKey(
    int deviceId,
    Map<String, dynamic> payload,
  ) async {
    if (provisionError != null) throw provisionError!;
    lastProvisionPayload = Map<String, dynamic>.from(payload);
    return const DeviceCredentialView(
      kind: 'ssh',
      username: 'netmonitor',
      storage: 'key',
      dedicated: true,
    );
  }
}

void _setViewport(WidgetTester tester, Size size) {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
}

List<Override> _baseOverrides() => <Override>[
  authProvider.overrideWith(_StubAuthNotifier.new),
  areasProvider.overrideWith(_StubAreasNotifier.new),
  sitesProvider.overrideWith(_StubSitesNotifier.new),
  networksProvider.overrideWith(_StubNetworksNotifier.new),
  probesProvider.overrideWith(_StubProbesNotifier.new),
  devicesProvider.overrideWith(_StubDevicesNotifier.new),
  deviceDetailProvider.overrideWith(_StubDeviceDetailNotifier.new),
  linkTestsProvider.overrideWith(_StubLinkTestsNotifier.new),
  wifiClientsProvider.overrideWith(_StubWifiClientsNotifier.new),
  topologyProvider.overrideWith(_StubTopologyNotifier.new),
  dnsServersProvider.overrideWith(_StubDnsServersNotifier.new),
  vendorsProvider.overrideWith(_StubVendorsNotifier.new),
  wifiSettingsProvider.overrideWith(_StubWifiSettingsNotifier.new),
  coreSettingsProvider.overrideWith(_StubCoreSettingsNotifier.new),
  operatingSystemsProvider.overrideWith(_StubOperatingSystemsNotifier.new),
  snmpProfilesProvider.overrideWith(_StubSnmpProfilesNotifier.new),
  logsProvider.overrideWith(_StubLogsNotifier.new),
];

Widget _wrapMobile(
  Widget child, {
  Key? scopeKey,
  List<Override> extraOverrides = const [],
}) {
  return ProviderScope(
    key: scopeKey,
    overrides: <Override>[..._baseOverrides(), ...extraOverrides],
    child: MaterialApp(
      theme: AppTheme.dark(),
      home: Scaffold(
        body: SafeArea(
          child: Padding(padding: const EdgeInsets.all(12), child: child),
        ),
      ),
    ),
  );
}

void main() {
  testWidgets(
    'DevicesPage renders UnifiedFilterBar and device cards at 390x844 with zero overflow',
    (tester) async {
      _setViewport(tester, const Size(390, 844));
      await tester.pumpWidget(_wrapMobile(const DevicesPage()));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.byType(UnifiedFilterBar), findsOneWidget);
      expect(find.text('Dispositivos'), findsOneWidget);
      expect(find.text('MikroTik CCR2004 Principal'), findsOneWidget);
      expect(find.byType(Checkbox), findsNothing);
      expect(find.text('Filtros'), findsOneWidget);

      await tester.tap(find.byTooltip('Mais ações'));
      await tester.pumpAndSettle();
      expect(find.text('Definir pai em lote'), findsOneWidget);
      await tester.tapAt(const Offset(8, 8));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Filtros'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('Filtrar Dispositivos'), findsOneWidget);
    },
  );

  testWidgets(
    'DeviceDetailPage renders pinned scrollable TabBar with teal plugin tab at 390x844',
    (tester) async {
      _setViewport(tester, const Size(390, 844));
      await tester.pumpWidget(_wrapMobile(const DeviceDetailPage(deviceId: 2)));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      final tabBar = tester.widget<TabBar>(find.byType(TabBar));
      expect(tabBar.isScrollable, isTrue);
      expect(tabBar.tabAlignment, TabAlignment.start);
      expect(find.text('Clientes Wi-Fi'), findsOneWidget);
      final wifiTabText = tester.widget<Text>(find.text('Clientes Wi-Fi'));
      expect(wifiTabText.style?.color, isNotNull);

      await tester.ensureVisible(find.text('Clientes Wi-Fi'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Clientes Wi-Fi'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('Notebook-Diretoria'), findsOneWidget);
    },
  );

  testWidgets(
    'SitesPage, NetworksPage, and ProbesPage render symmetrically at 390x844 with zero overflow',
    (tester) async {
      _setViewport(tester, const Size(390, 844));

      await tester.pumpWidget(_wrapMobile(const SitesPage()));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('Matriz São Paulo'), findsOneWidget);
      expect(find.byType(UnifiedFilterBar), findsOneWidget);

      await tester.pumpWidget(_wrapMobile(const NetworksPage()));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('LAN Corporativa Principal'), findsOneWidget);
      expect(find.byType(UnifiedFilterBar), findsOneWidget);

      // Abre sheet de Nova Rede para validar ResponsiveFieldRow no mobile
      await tester.tap(find.text('Nova'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('Nova Rede IP'), findsOneWidget);
      await tester.tap(find.text('Cancelar'));
      await tester.pumpAndSettle();

      await tester.pumpWidget(_wrapMobile(const ProbesPage()));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('vpn-probe'), findsWidgets);
      expect(find.byType(UnifiedFilterBar), findsOneWidget);
    },
  );

  testWidgets(
    'LinkTestsPage uses NestedScrollView + UnifiedFilterBar and breathable 360px wizard',
    (tester) async {
      _setViewport(tester, const Size(390, 844));
      await tester.pumpWidget(_wrapMobile(const LinkTestsPage()));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.byType(NestedScrollView), findsOneWidget);
      expect(find.byType(UnifiedFilterBar), findsOneWidget);
      expect(find.text('Testes de Enlace'), findsOneWidget);

      // Abre o wizard em largura estreita (360px) para validar Sentido + Duração/Streams e SegmentedButton
      _setViewport(tester, const Size(360, 844));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Novo'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('Novo Teste de Velocidade / Enlace'), findsOneWidget);
      expect(find.text('Sentido'), findsOneWidget);
      expect(find.text('Duração'), findsOneWidget);
      expect(find.text('Streams'), findsOneWidget);

      final seg = tester.widget<SegmentedButton<String>>(
        find.byType(SegmentedButton<String>),
      );
      expect(seg.showSelectedIcon, isFalse);

      // Sentido fica acima de Duração e Streams (não espremido em 3 colunas)
      final sentidoTop = tester.getTopLeft(find.text('Sentido')).dy;
      final duracaoTop = tester.getTopLeft(find.text('Duração')).dy;
      final streamsTop = tester.getTopLeft(find.text('Streams')).dy;
      expect(duracaoTop, greaterThan(sentidoTop));
      expect(streamsTop, closeTo(duracaoTop, 2.0));
    },
  );

  testWidgets(
    'WifiClientsPage renders NestedScrollView + UnifiedFilterBar at 390x844 with zero overflow',
    (tester) async {
      _setViewport(tester, const Size(390, 844));
      await tester.pumpWidget(_wrapMobile(const WifiClientsPage()));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.byType(NestedScrollView), findsOneWidget);
      expect(find.byType(UnifiedFilterBar), findsOneWidget);
      expect(find.text('Notebook Diretoria'), findsOneWidget);

      await tester.tap(find.text('Mapa de Sinal'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'TopologyPage renders compact 1-row floating mobile toolbar at 390x844 with zero overflow',
    (tester) async {
      _setViewport(tester, const Size(390, 844));
      await tester.pumpWidget(_wrapMobile(const TopologyPage()));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.text('Core-CCR'), findsOneWidget);
      expect(find.byTooltip('Mais ações da topologia'), findsOneWidget);
    },
  );

  testWidgets(
    'DeviceAccessPanel switches 1 mode at a time at 360px and sends exact Rust DTOs',
    (tester) async {
      _setViewport(tester, const Size(360, 844));
      final stubCtrl = _StubDeviceAccessController(
        dedicatedPlatform: 'openwrt',
      );

      await tester.pumpWidget(
        _wrapMobile(
          const SingleChildScrollView(child: DeviceAccessPanel(deviceId: 2)),
          extraOverrides: <Override>[
            deviceAccessProvider.overrideWith(() => stubCtrl),
          ],
        ),
      );
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      // Modo padrão quando suportado: Chave SSH dedicada (somente 1 formulário visível!)
      expect(find.text('Chave SSH dedicada (openwrt)'), findsOneWidget);
      expect(find.text('Provisionar chave SSH'), findsOneWidget);
      expect(find.text('Salvar credencial'), findsNothing);

      // Submete provisionamento e verifica contrato Rust `ProvisionInput`
      await tester.enterText(
        find.widgetWithText(TextField, 'Usuário admin atual'),
        'root',
      );
      await tester.enterText(
        find.widgetWithText(TextField, 'Senha administrativa (uso único)'),
        'secret123',
      );
      await tester.tap(find.text('Provisionar chave SSH'));
      await tester.pumpAndSettle();

      expect(stubCtrl.lastProvisionPayload, isNotNull);
      expect(stubCtrl.lastProvisionPayload!['username'], 'root');
      expect(stubCtrl.lastProvisionPayload!['password'], 'secret123');
      expect(stubCtrl.lastProvisionPayload!['platform'], 'openwrt');
      expect(
        stubCtrl.lastProvisionPayload!.containsKey('adminUsername'),
        isFalse,
      );

      // Alterna para "Usuário e senha": formulário dedicado some, só transporte aparece
      await tester.tap(find.text('Usuário e senha'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('Provisionar chave SSH'), findsNothing);
      expect(find.text('Salvar credencial'), findsOneWidget);

      await tester.enterText(
        find.widgetWithText(TextField, 'Usuário'),
        'admin',
      );
      await tester.enterText(
        find.widgetWithText(TextField, 'Senha de sessão (30 min)'),
        'pass456',
      );
      await tester.tap(find.text('Salvar credencial'));
      await tester.pumpAndSettle();

      expect(stubCtrl.lastSavedCredentialPayload, isNotNull);
      expect(stubCtrl.lastSavedCredentialPayload!['kind'], 'ssh');
      expect(stubCtrl.lastSavedCredentialPayload!['username'], 'admin');
      expect(stubCtrl.lastSavedCredentialPayload!['secret'], 'pass456');
      expect(stubCtrl.lastSavedCredentialPayload!['storage'], 'ask');
      expect(
        stubCtrl.lastSavedCredentialPayload!.containsKey('password'),
        isFalse,
      );
    },
  );

  test(
    'formatDeviceAccessError turns bare Bad Request and field errors into readable PT-BR',
    () {
      const bare400 = ApiException(message: 'Bad Request', statusCode: 400);
      expect(
        formatDeviceAccessError(bare400),
        contains('Dados de acesso inválidos'),
      );

      const detailed = ApiException(
        message: 'Bad Request',
        statusCode: 422,
        payload: ApiErrorPayload(
          errors: <ApiFieldError>[
            ApiFieldError(field: 'username', message: 'Informe o usuário.'),
          ],
        ),
      );
      expect(
        formatDeviceAccessError(detailed),
        contains('username: Informe o usuário.'),
      );
    },
  );

  testWidgets(
    'Network settings cards and diagnostic sheets render at 360px with zero overflow',
    (tester) async {
      _setViewport(tester, const Size(360, 844));
      await tester.pumpWidget(
        _wrapMobile(
          ListView(
            children: const <Widget>[
              DnsServersCard(),
              SizedBox(height: 12),
              VendorRegistryCard(),
              SizedBox(height: 12),
              WifiSettingsCard(),
            ],
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      // Abre Port Scan e Wi-Fi QR a partir do contexto a 360px
      final ctx = tester.element(find.byType(DnsServersCard));
      showPortScanDialog(ctx, initialHost: '10.10.0.1');
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('Varredura de Portas'), findsOneWidget);
      Navigator.of(ctx).pop();
      await tester.pumpAndSettle();

      showWifiQrSheet(ctx, initialSsid: 'CorpWiFi');
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('QR Code Wi-Fi'), findsOneWidget);
    },
  );

  testWidgets(
    'WifiClientsPage station sheet uses SymmetricalActionBar for 2-button (50/50) and 3-button (50/50 + 100%) layouts at 360px',
    (tester) async {
      _setViewport(tester, const Size(360, 800));
      await tester.pumpWidget(_wrapMobile(const WifiClientsPage()));
      await tester.pumpAndSettle();

      // 1) Cliente com 3 ações (Ping pelo AP + Vigiado + QR Wi-Fi): 50%/50% na linha 1 + 100% na linha 2!
      await tester.tap(find.text('Notebook Diretoria').last);
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      final pingBtn = find.widgetWithText(
        FilledButton,
        'Ping pelo AP',
        skipOffstage: false,
      );
      final watchBtn3 = find.widgetWithText(
        OutlinedButton,
        'Vigiado (Notebook Diretoria)',
        skipOffstage: false,
      );
      final qrBtn3 = find.widgetWithText(
        OutlinedButton,
        'QR Wi-Fi',
        skipOffstage: false,
      );

      final pingRect = tester.getRect(pingBtn);
      final watchRect3 = tester.getRect(watchBtn3);
      final qrRect3 = tester.getRect(qrBtn3);

      // Linha 1: Ping + Vigiado lado a lado com mesma largura; Linha 2: QR Wi-Fi largura total (sem botão órfão!)
      expect(pingRect.top, closeTo(watchRect3.top, 1.0));
      expect(pingRect.width, closeTo(watchRect3.width, 1.0));
      expect(qrRect3.top, greaterThan(pingRect.bottom));
      expect(qrRect3.left, closeTo(pingRect.left, 1.0));
      expect(qrRect3.right, closeTo(watchRect3.right, 1.0));

      await tester.tap(find.byTooltip('Fechar'));
      await tester.pumpAndSettle();

      // 2) Cliente com 2 ações (Tuya Smart sem IP -> Vigiar aparelho + QR Wi-Fi): 50%/50% em linha única!
      await tester.tap(find.text('Tuya Smart'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      final watchBtn2 = find.widgetWithText(OutlinedButton, 'Vigiar aparelho');
      final qrBtn2 = find.widgetWithText(OutlinedButton, 'QR Wi-Fi');
      final watchRect2 = tester.getRect(watchBtn2);
      final qrRect2 = tester.getRect(qrBtn2);

      expect(watchRect2.top, closeTo(qrRect2.top, 1.0));
      expect(watchRect2.width, closeTo(qrRect2.width, 1.0));
    },
  );

  testWidgets(
    'WifiClientsPage "Mapa de Sinal" auto-fits at 390x844 with anti-overlap nodes and pans freely on drag',
    (tester) async {
      _setViewport(tester, const Size(390, 844));
      await tester.pumpWidget(_wrapMobile(const WifiClientsPage()));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Mapa de Sinal'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      final viewerFinder = find.byKey(
        const ValueKey<String>('wifi-signal-interactive-viewer'),
      );
      expect(viewerFinder, findsOneWidget);

      final viewer = tester.widget<InteractiveViewer>(viewerFinder);
      final ctrl = viewer.transformationController!;
      final initialTx = ctrl.value.storage[12];
      final initialTy = ctrl.value.storage[13];
      final initialScale = ctrl.value.storage[0];

      // Auto-fit enquadra o radar inteiro (< 1.0x em 390px) sem cortar a borda direita!
      expect(initialScale, lessThan(1.0));
      expect(initialScale, greaterThanOrEqualTo(0.25));

      // Nós com RSSI próximo (-68 e -69 dBm) não se sobrepõem graças ao relaxamento anti-colisão
      final tuyaRect = tester.getRect(
        find.byKey(
          const ValueKey<String>('wifi-map-station-2-aa:bb:cc:dd:ee:22'),
        ),
      );
      final espRect = tester.getRect(
        find.byKey(
          const ValueKey<String>('wifi-map-station-2-aa:bb:cc:dd:ee:33'),
        ),
      );
      expect(tuyaRect.overlaps(espRect), isFalse);

      // Arrastar o mapa move genuinamente a câmera em X e Y (TabBarView/NestedScrollView não roubam o gesto!)
      await tester.drag(viewerFinder, const Offset(-85, -55));
      await tester.pumpAndSettle();

      final draggedTx = ctrl.value.storage[12];
      final draggedTy = ctrl.value.storage[13];
      expect((draggedTx - initialTx).abs(), greaterThan(20.0));
      expect((draggedTy - initialTy).abs(), greaterThan(15.0));

      // Botão flutuante "Ajustar mapa à tela" recentraliza o radar
      await tester.tap(find.byTooltip('Ajustar mapa à tela'));
      await tester.pumpAndSettle();
      expect(ctrl.value.storage[12], closeTo(initialTx, 0.5));
      expect(ctrl.value.storage[13], closeTo(initialTy, 0.5));
    },
  );

  testWidgets(
    'OpenWrt defaults to "root", MikroTik to "admin", and HTTP 401 shows "Usuário ou senha incorretos" AlertDialog without logging out',
    (tester) async {
      _setViewport(tester, const Size(360, 800));

      // 1) OpenWrt preenche "root" automaticamente e exibe AlertDialog em HTTP 401 mantendo sessão ativa
      final openwrtCtrl = _StubDeviceAccessController(
        dedicatedPlatform: 'openwrt',
        provisionError: const ApiException(
          message: 'Usuário ou senha recusados pelo equipamento.',
          statusCode: 401,
        ),
      );

      late WidgetRef capturedRef;
      await tester.pumpWidget(
        _wrapMobile(
          Consumer(
            builder: (context, ref, _) {
              capturedRef = ref;
              return SingleChildScrollView(
                child: Column(
                  children: <Widget>[
                    const DeviceAccessPanel(deviceId: 2),
                    OutlinedButton(
                      onPressed: () => showDeviceFormSheet(
                        context: context,
                        prefill: const <String, dynamic>{
                          'name': 'AP OpenWrt Sala',
                          'ipAddress': '10.10.0.50',
                          'vendor': 'OpenWrt',
                          'operatingSystem': 'openwrt',
                        },
                      ),
                      child: const Text('Abrir cadastro OpenWrt'),
                    ),
                  ],
                ),
              );
            },
          ),
          extraOverrides: <Override>[
            deviceAccessProvider.overrideWith(() => openwrtCtrl),
          ],
        ),
      );
      await tester.pumpAndSettle();

      final adminUserField = tester.widget<TextField>(
        find.widgetWithText(TextField, 'Usuário admin atual'),
      );
      expect(adminUserField.controller?.text, 'root');

      // Submete senha incorreta -> retorna 401 -> exibe AlertDialog sem encerrar a sessão do operador!
      await tester.enterText(
        find.widgetWithText(TextField, 'Senha administrativa (uso único)'),
        'senha-errada',
      );
      await tester.tap(find.text('Provisionar chave SSH'));
      await tester.pumpAndSettle();

      expect(find.byType(AlertDialog), findsOneWidget);
      expect(find.text('Usuário ou senha incorretos'), findsOneWidget);
      expect(
        find.descendant(
          of: find.byType(AlertDialog),
          matching: find.textContaining("usuário padrão é 'root'"),
        ),
        findsOneWidget,
      );
      expect(capturedRef.read(authProvider).isAuthenticated, isTrue);

      await tester.tap(find.text('Entendi'));
      await tester.pumpAndSettle();
      expect(find.byType(AlertDialog), findsNothing);
      expect(find.byType(DeviceAccessPanel), findsOneWidget);

      // 2) DeviceFormSheet com OpenWrt também inicia o Syslog automático com "root" (nunca "admin")!
      await tester.ensureVisible(find.text('Abrir cadastro OpenWrt'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Abrir cadastro OpenWrt'));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Receber logs (Syslog)'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Receber logs (Syslog)'));
      await tester.pumpAndSettle();

      final syslogUserField = tester.widget<TextFormField>(
        find.widgetWithText(
          TextFormField,
          'Usuário admin atual',
          skipOffstage: false,
        ),
      );
      expect(syslogUserField.controller?.text, 'root');
      await tester.tap(find.byTooltip('Fechar'));
      await tester.pumpAndSettle();

      // 3) MikroTik preenche "admin" automaticamente
      final mikrotikCtrl = _StubDeviceAccessController(
        dedicatedPlatform: 'mikrotik',
      );
      await tester.pumpWidget(
        _wrapMobile(
          const SingleChildScrollView(child: DeviceAccessPanel(deviceId: 1)),
          scopeKey: UniqueKey(),
          extraOverrides: <Override>[
            deviceAccessProvider.overrideWith(() => mikrotikCtrl),
          ],
        ),
      );
      await tester.pumpAndSettle();
      final mkUserField = tester.widget<TextField>(
        find.widgetWithText(TextField, 'Usuário admin atual'),
      );
      expect(mkUserField.controller?.text, 'admin');
    },
  );

  testWidgets(
    'Visual polish (#111): LinkTestsPage PT-BR labels without arrow boxes, inline SectionCard header actions, localized RSSI grade, and single-row VendorRegistryCard',
    (tester) async {
      _setViewport(tester, const Size(390, 844));

      // 1) LinkTestsPage: zero setas Unicode quebráveis, chips com ícones e status/subtítulo em PT-BR
      await tester.pumpWidget(_wrapMobile(const LinkTestsPage()));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.textContaining('Core-CCR -> SW-Distribuicao'), findsWidgets);
      expect(
        find.text('Enlace · Ida e volta · 10s (4 fluxos)'),
        findsOneWidget,
      );
      expect(find.text('Em execução'), findsWidgets);
      expect(find.byIcon(Icons.arrow_downward_rounded), findsOneWidget);
      expect(find.byIcon(Icons.arrow_upward_rounded), findsOneWidget);

      // 2) SitesPage: botões editar/excluir permanecem inline no cabeçalho (mesma linha do título!) + rodapé 50%/50%
      await tester.pumpWidget(
        _wrapMobile(const SitesPage(), scopeKey: UniqueKey()),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      final siteTitleRect = tester.getRect(find.text('Matriz São Paulo'));
      final siteEditRect = tester.getRect(find.byTooltip('Editar site').first);
      expect(
        (siteEditRect.center.dy - siteTitleRect.center.dy).abs(),
        lessThan(28.0),
      );
      final topoBtnRect = tester.getRect(
        find.widgetWithText(OutlinedButton, 'Topologia').first,
      );
      final netBtnRect = tester.getRect(
        find.widgetWithText(OutlinedButton, 'Redes IP').first,
      );
      expect(topoBtnRect.top, closeTo(netBtnRect.top, 1.0));
      expect(topoBtnRect.width, closeTo(netBtnRect.width, 1.0));

      // 3) ProbesPage: rodapé simétrico 50%/50% entre Testar e Revogar
      await tester.pumpWidget(
        _wrapMobile(const ProbesPage(), scopeKey: UniqueKey()),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      final testProbeRect = tester.getRect(
        find.widgetWithText(OutlinedButton, 'Testar'),
      );
      final revokeProbeRect = tester.getRect(
        find.widgetWithText(OutlinedButton, 'Revogar'),
      );
      expect(testProbeRect.top, closeTo(revokeProbeRect.top, 1.0));
      expect(testProbeRect.width, closeTo(revokeProbeRect.width, 1.0));

      // 4) VendorRegistryCard: campo MAC + botão Consultar na mesma linha em 390px
      await tester.pumpWidget(
        _wrapMobile(
          const SingleChildScrollView(child: VendorRegistryCard()),
          scopeKey: UniqueKey(),
        ),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      final macFieldRect = tester.getRect(
        find.widgetWithText(TextField, 'Testar consulta de MAC'),
      );
      final consultarRect = tester.getRect(
        find.widgetWithText(FilledButton, 'Consultar'),
      );
      expect(
        (macFieldRect.center.dy - consultarRect.center.dy).abs(),
        lessThan(16.0),
      );

      // 5) WifiClientsPage: qualidade de sinal traduzida ("excelente" / "bom")
      await tester.pumpWidget(
        _wrapMobile(const WifiClientsPage(), scopeKey: UniqueKey()),
      );
      await tester.pumpAndSettle();
      await tester.tap(find.text('Notebook Diretoria').last);
      await tester.pumpAndSettle();
      expect(find.text('-54 dBm (excelente)'), findsOneWidget);
    },
  );
}
