import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/pages/topology_page.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/diagnostics_provider.dart';
import 'package:netmonitor_app/features/network/providers/link_tests_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/features/network/providers/topology_provider.dart';
import 'package:netmonitor_app/features/network/providers/wifi_provider.dart';
import 'package:netmonitor_app/features/network/topology/canvas/topology_canvas.dart';
import 'package:netmonitor_app/features/network/topology/widgets/topology_widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:shared_preferences/shared_preferences.dart';

class _TestAuthNotifier extends AuthNotifier {
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

class _TestSitesNotifier extends SitesNotifier {
  @override
  SitesState build() => const SitesState(
    items: <SiteModel>[
      SiteModel(id: 1, name: 'Matriz São Paulo'),
      SiteModel(id: 2, name: 'Filial Campinas'),
    ],
  );

  @override
  Future<void> fetchSites() async {}
}

class _TestDevicesNotifier extends DevicesNotifier {
  @override
  DevicesState build() => const DevicesState(
    items: <DeviceModel>[
      DeviceModel(
        id: 1,
        name: 'Borda-CCR',
        type: 'router',
        status: 'online',
        ipAddress: '10.10.0.1',
        macAddress: 'AA:BB:CC:00:00:01',
        vendor: 'MikroTik',
        siteId: 1,
      ),
      DeviceModel(
        id: 2,
        name: 'Switch-Core',
        type: 'switch',
        status: 'warning',
        ipAddress: '10.10.0.2',
        macAddress: 'AA:BB:CC:00:00:02',
        vendor: 'Ubiquiti',
        siteId: 1,
      ),
      DeviceModel(
        id: 3,
        name: 'Servidor-ERP',
        type: 'server',
        status: 'offline',
        ipAddress: '10.10.0.30',
        macAddress: 'AA:BB:CC:00:00:03',
        vendor: 'Dell',
        siteId: 2,
      ),
    ],
  );

  @override
  Future<void> fetchDevices() async {}
}

class _TestLinkTestsNotifier extends LinkTestsNotifier {
  @override
  LinkTestsState build() => const LinkTestsState();
}

class _TestWifiClientsNotifier extends WifiClientsNotifier {
  @override
  WifiClientsState build() => const WifiClientsState();
}

class _TestAiSettingsNotifier extends AiSettingsNotifier {
  @override
  AiSettingsState build() => const AiSettingsState();
}

class _FakeDiagnosticsService extends DiagnosticsService {
  _FakeDiagnosticsService()
    : super(
        ApiClient(
          baseUrlProvider: () => 'http://127.0.0.1',
          tokenProvider: () => null,
          onUnauthorized: () {},
        ),
      );

  @override
  Future<QuickPingReport> quickPing(int deviceId) async {
    return const QuickPingReport(
      target: '10.10.0.1',
      sent: 4,
      samples: 4,
      lossPct: 0,
      small: QuickPingRtt(minMs: 1.2, avgMs: 2.1, maxMs: 3.4),
      jitterMs: 0.4,
      capacityNote: 'Link saudável',
      verdict: 'Latência estável sem perda de pacotes.',
    );
  }
}

class _TestTopologyNotifier extends TopologyNotifier {
  int saveLayoutCalls = 0;
  List<int>? lastSavedNodeIds;
  int addLinkCalls = 0;
  int createUnmanagedSwitchCalls = 0;

  @override
  TopologyState build() => const TopologyState(
    loaded: true,
    nodes: <TopologyNodeModel>[
      TopologyNodeModel(
        id: 1,
        name: 'Borda-CCR',
        type: 'router',
        status: 'online',
        siteId: 1,
        siteName: 'Matriz São Paulo',
        ipAddress: '10.10.0.1',
        vendor: 'MikroTik',
        model: 'CCR2004',
        interfaceCount: 16,
        snmpEnabled: true,
        cpuPercent: 24,
        memoryPercent: 41,
      ),
      TopologyNodeModel(
        id: 2,
        name: 'Switch-Core',
        type: 'switch',
        status: 'warning',
        siteId: 1,
        siteName: 'Matriz São Paulo',
        ipAddress: '10.10.0.2',
        vendor: 'Ubiquiti',
        interfaceCount: 24,
        monitorsDown: 1,
        cpuPercent: 78,
        memoryPercent: 62,
      ),
      TopologyNodeModel(
        id: 3,
        name: 'Servidor-ERP',
        type: 'server',
        status: 'offline',
        siteId: 2,
        siteName: 'Filial Campinas',
        ipAddress: '10.10.0.30',
        vendor: 'Dell',
        interfaceCount: 4,
        monitorsDown: 2,
      ),
    ],
    edges: <TopologyEdgeModel>[
      TopologyEdgeModel(
        id: 10,
        sourceDeviceId: 1,
        targetDeviceId: 2,
        sourceDeviceName: 'Borda-CCR',
        targetDeviceName: 'Switch-Core',
        sourceInterfaceName: 'sfp-sfpplus1',
        targetInterfaceName: 'eth24',
        linkType: 'fiber',
        discoveryMethod: 'lldp',
        status: 'up',
        speedBps: 10000000000,
        confirmed: true,
      ),
      TopologyEdgeModel(
        id: 11,
        sourceDeviceId: 2,
        targetDeviceId: 3,
        sourceDeviceName: 'Switch-Core',
        targetDeviceName: 'Servidor-ERP',
        sourceInterfaceName: 'eth1',
        targetInterfaceName: 'eno1',
        linkType: 'ethernet',
        discoveryMethod: 'manual',
        status: 'down',
        speedBps: 1000000000,
      ),
    ],
    positions: <int, TopologyPosition>{
      1: TopologyPosition(100, 120),
      2: TopologyPosition(380, 120),
      3: TopologyPosition(660, 120),
    },
    pluginActions: <int, List<TopologyPluginActionModel>>{
      1: <TopologyPluginActionModel>[
        TopologyPluginActionModel(
          pluginId: 1,
          pluginName: 'MikroTik',
          action: 'dhcp_leases',
          label: 'Listar leases DHCP',
        ),
      ],
    },
  );

  @override
  Future<void> fetchTopology({
    int? siteId,
    bool updateSiteFilter = false,
    bool showLoading = true,
  }) async {
    if (updateSiteFilter) {
      state = state.copyWith(siteId: siteId, clearSiteId: siteId == null);
    }
  }

  @override
  Future<bool> saveLayout({List<int>? onlyNodeIds}) async {
    saveLayoutCalls += 1;
    lastSavedNodeIds = onlyNodeIds;
    return true;
  }

  @override
  Future<bool> addLink(Map<String, dynamic> payload) async {
    addLinkCalls += 1;
    return true;
  }

  @override
  Future<bool> createUnmanagedSwitch(Map<String, dynamic> payload) async {
    createUnmanagedSwitchCalls += 1;
    return true;
  }

  @override
  Future<TopologyPathModel?> fetchPath(int fromId, int toId) async => null;
}

void _setViewport(WidgetTester tester, Size size) {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1.0;
  addTearDown(tester.view.resetPhysicalSize);
  addTearDown(tester.view.resetDevicePixelRatio);
}

Widget _buildApp({
  required _TestTopologyNotifier topoNotifier,
  ThemeData? theme,
}) {
  return ProviderScope(
    overrides: <Override>[
      authProvider.overrideWith(_TestAuthNotifier.new),
      sitesProvider.overrideWith(_TestSitesNotifier.new),
      devicesProvider.overrideWith(_TestDevicesNotifier.new),
      linkTestsProvider.overrideWith(_TestLinkTestsNotifier.new),
      wifiClientsProvider.overrideWith(_TestWifiClientsNotifier.new),
      aiSettingsProvider.overrideWith(_TestAiSettingsNotifier.new),
      diagnosticsServiceProvider.overrideWithValue(_FakeDiagnosticsService()),
      topologyProvider.overrideWith(() => topoNotifier),
    ],
    child: MaterialApp(
      theme: theme ?? AppTheme.dark(),
      home: const Scaffold(body: TopologyPage()),
    ),
  );
}

void main() {
  setUp(() {
    SharedPreferences.setMockInitialValues(<String, Object>{});
  });

  testWidgets(
    'Mobile (390x844 & 360x844) and Desktop (1280x800) render with zero overflow in Dark & Light themes',
    (tester) async {
      for (final size in const <Size>[
        Size(390, 844),
        Size(360, 844),
        Size(1280, 800),
      ]) {
        for (final theme in <ThemeData>[AppTheme.dark(), AppTheme.light()]) {
          _setViewport(tester, size);
          final notifier = _TestTopologyNotifier();
          await tester.pumpWidget(
            _buildApp(topoNotifier: notifier, theme: theme),
          );
          await tester.pumpAndSettle();

          expect(tester.takeException(), isNull);
          expect(find.byType(TopologyCanvasView), findsOneWidget);
          expect(find.byType(TopologyHud), findsOneWidget);
          expect(find.text('1 online'), findsOneWidget);
          expect(find.text('1 alerta'), findsOneWidget);
          expect(find.text('1 off'), findsOneWidget);
          expect(find.byTooltip('Mais ações da topologia'), findsOneWidget);
        }
      }
    },
  );

  testWidgets(
    'Node tap opens adaptive Node Inspector with quick ping, quick actions & neighbor navigation',
    (tester) async {
      _setViewport(tester, const Size(1280, 800));
      final notifier = _TestTopologyNotifier();
      await tester.pumpWidget(_buildApp(topoNotifier: notifier));
      await tester.pumpAndSettle();

      // Toca em Borda-CCR -> abre painel lateral fixo no desktop
      await tester.tap(
        find.byKey(const ValueKey<String>('topology-node-detail-1')),
      );
      await tester.pump(const Duration(milliseconds: 350));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.byType(TopologyNodeInspectorCard), findsOneWidget);
      expect(find.text('Ping rápido'), findsOneWidget);
      expect(find.text('Traçar caminho'), findsOneWidget);
      expect(find.text('Até a raiz'), findsOneWidget);
      expect(find.text('Listar leases DHCP'), findsOneWidget);
      expect(find.text('Conexões e Vizinhos (1)'), findsOneWidget);

      // Executa Ping rápido inline
      await tester.tap(find.text('Ping rápido'));
      await tester.pumpAndSettle();
      expect(
        find.text('Latência estável sem perda de pacotes.'),
        findsOneWidget,
      );

      // Toca no vizinho Switch-Core -> navega e inspeciona o vizinho
      final neighborFinder = find.descendant(
        of: find.byType(TopologyNodeInspectorCard),
        matching: find.text('Switch-Core'),
      );
      await tester.ensureVisible(neighborFinder);
      await tester.pumpAndSettle();
      await tester.tap(neighborFinder);
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('Conexões e Vizinhos (2)'), findsOneWidget);
    },
  );

  testWidgets(
    'Search & Jump sheet finds node by vendor/IP and focuses/inspects it on mobile',
    (tester) async {
      _setViewport(tester, const Size(390, 844));
      final notifier = _TestTopologyNotifier();
      await tester.pumpWidget(_buildApp(topoNotifier: notifier));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Buscar nó...'));
      await tester.pumpAndSettle();
      expect(find.text('Localizar Equipamento'), findsOneWidget);

      await tester.enterText(find.byType(TextField).first, 'Dell');
      await tester.pumpAndSettle();
      expect(find.text('Servidor-ERP'), findsWidgets);

      await tester.tap(find.text('Servidor-ERP').last);
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      // No mobile (< 900px) abre o bottom sheet do inspetor para o nó selecionado
      expect(find.byType(TopologyNodeInspectorBody), findsOneWidget);
    },
  );

  testWidgets(
    'Filters & Layout sheet switches layout mode via ChoiceCards and toggles visual options',
    (tester) async {
      _setViewport(tester, const Size(390, 844));
      final notifier = _TestTopologyNotifier();
      await tester.pumpWidget(_buildApp(topoNotifier: notifier));
      await tester.pumpAndSettle();

      await tester.tap(find.byTooltip('Filtros e visualização'));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.text('Filtros, Layout e Visualização'), findsOneWidget);
      expect(find.text('Hierárquica'), findsOneWidget);
      expect(find.text('Radial'), findsOneWidget);
      expect(find.text('Força'), findsOneWidget);
      expect(find.text('Por Site'), findsOneWidget);

      await tester.tap(find.text('Radial'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);

      await tester.tap(find.text('Por Site'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'Edit-layout lock prevents accidental drag until unlocked and shows Save/Discard pill after move',
    (tester) async {
      _setViewport(tester, const Size(1280, 800));
      final notifier = _TestTopologyNotifier();
      await tester.pumpWidget(_buildApp(topoNotifier: notifier));
      await tester.pumpAndSettle();

      final initialPos = notifier.state.positions[1]!;

      // Bloqueado por padrão: arrastar nó NÃO move e NÃO mostra pílula de salvar
      await tester.drag(find.text('Borda-CCR').first, const Offset(80, 40));
      await tester.pumpAndSettle();
      expect(notifier.state.positions[1]!.x, closeTo(initialPos.x, 0.01));
      expect(find.text('Layout editado'), findsNothing);

      // Destrava o modo de edição de posições
      await tester.tap(find.byTooltip('Editar posições'));
      await tester.pumpAndSettle();

      // Agora arrastar o nó move e exibe a pílula "Layout editado — Salvar / Descartar"
      await tester.drag(find.text('Borda-CCR').first, const Offset(90, 50));
      await tester.pumpAndSettle();
      expect(notifier.state.positions[1]!.x, isNot(closeTo(initialPos.x, 1.0)));
      expect(find.text('Layout editado'), findsOneWidget);
      expect(find.text('Salvar'), findsOneWidget);
      expect(find.text('Descartar'), findsOneWidget);

      // Descartar restaura a posição original e oculta a pílula
      await tester.tap(find.text('Descartar'));
      await tester.pumpAndSettle();
      expect(notifier.state.positions[1]!.x, closeTo(initialPos.x, 0.01));
      expect(find.text('Layout editado'), findsNothing);

      // Arrasta novamente e salva
      await tester.drag(find.text('Borda-CCR').first, const Offset(70, 30));
      await tester.pumpAndSettle();
      expect(find.text('Layout editado'), findsOneWidget);

      await tester.tap(find.text('Salvar'));
      await tester.pumpAndSettle();
      expect(notifier.saveLayoutCalls, 1);
      expect(notifier.lastSavedNodeIds, contains(1));
      expect(find.text('Layout editado'), findsNothing);
    },
  );

  testWidgets(
    'Path trace highlights shortest path and displays Path Trace Banner with hop count',
    (tester) async {
      _setViewport(tester, const Size(1280, 800));
      final notifier = _TestTopologyNotifier();
      await tester.pumpWidget(_buildApp(topoNotifier: notifier));
      await tester.pumpAndSettle();

      // Seleciona Borda-CCR e aciona "Traçar caminho"
      await tester.tap(
        find.byKey(const ValueKey<String>('topology-node-detail-1')),
      );
      await tester.pump(const Duration(milliseconds: 350));
      await tester.pumpAndSettle();
      await tester.ensureVisible(find.text('Traçar caminho'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Traçar caminho'));
      await tester.pumpAndSettle();

      // Escolhe Servidor-ERP como destino
      await tester.tap(find.text('Servidor-ERP').last);
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(
        find.text('Caminho: Borda-CCR → Switch-Core → Servidor-ERP (2 saltos)'),
        findsOneWidget,
      );

      // Fecha o banner de caminho
      await tester.tap(find.text('Fechar'));
      await tester.pumpAndSettle();
      expect(
        find.text('Caminho: Borda-CCR → Switch-Core → Servidor-ERP (2 saltos)'),
        findsNothing,
      );
    },
  );

  testWidgets(
    'Add Link, Unmanaged Switch, and Legend sheets render cleanly at 360px with zero overflow',
    (tester) async {
      _setViewport(tester, const Size(360, 844));
      final notifier = _TestTopologyNotifier();
      await tester.pumpWidget(_buildApp(topoNotifier: notifier));
      await tester.pumpAndSettle();

      // Abre menu ⋮ -> Novo link
      await tester.tap(find.byTooltip('Mais ações da topologia'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Novo link'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('Adicionar Enlace Manual'), findsOneWidget);
      await tester.tap(find.text('Criar enlace'));
      await tester.pumpAndSettle();
      expect(notifier.addLinkCalls, 1);

      // Abre menu ⋮ -> Switch não gerenciado
      await tester.tap(find.byTooltip('Mais ações da topologia'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Switch não gerenciado'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('Novo Switch Não Gerenciável'), findsOneWidget);
      await tester.enterText(
        find.widgetWithText(TextField, 'Nome do switch *'),
        'SW-Sala-01',
      );
      await tester.tap(find.text('Adicionar switch'));
      await tester.pumpAndSettle();
      expect(notifier.createUnmanagedSwitchCalls, 1);

      // Abre menu ⋮ -> Legenda
      await tester.tap(find.byTooltip('Mais ações da topologia'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Legenda'));
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
      expect(find.text('Legenda da Topologia'), findsOneWidget);
    },
  );
}
