import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
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
    bool preserveUnsavedPositions = false,
  }) async {
    if (updateSiteFilter) {
      state = state.copyWith(siteId: siteId, clearSiteId: siteId == null);
    }
  }

  @override
  Future<bool> saveLayout({
    List<int>? onlyNodeIds,
    Map<int, TopologyPosition>? overridePositions,
  }) async {
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

  final List<int> deletedLinkIds = <int>[];

  @override
  Future<bool> deleteLink(int id) async {
    deletedLinkIds.add(id);
    state = state.copyWith(
      edges: state.edges.where((e) => e.id != id).toList(growable: false),
    );
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
      topologyNodeInterfacesProvider.overrideWith((ref, deviceId) async {
        return <DeviceInterfaceModel>[
          const DeviceInterfaceModel(
            id: 1,
            deviceId: 1,
            ifName: 'sfp-sfpplus1',
            alias: 'Uplink Fibra',
            operStatus: 'up',
            speed: 10000000000,
            inBps: 250000000,
            outBps: 120000000,
          ),
          const DeviceInterfaceModel(
            id: 2,
            deviceId: 1,
            ifName: 'ether1',
            alias: 'LAN Matriz',
            operStatus: 'up',
            speed: 1000000000,
          ),
          for (var i = 3; i <= 8; i++)
            DeviceInterfaceModel(
              id: i,
              deviceId: 1,
              ifName: 'ether$i',
              operStatus: 'down',
              speed: 1000000000,
            ),
        ];
      }),
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
      expect(find.text('Informações do equipamento'), findsOneWidget);
      expect(find.text('Interfaces (8)'), findsOneWidget);
      expect(find.text('2 UP'), findsOneWidget);
      expect(find.text('sfp-sfpplus1'), findsWidgets);
      expect(find.text('ether1'), findsOneWidget);
      expect(find.text('10 Gbps'), findsWidgets);
      expect(find.text('Ver todas as 8 interfaces'), findsOneWidget);
      expect(find.text('Ping rápido'), findsOneWidget);
      expect(find.text('Traçar caminho'), findsOneWidget);
      expect(find.text('Até a raiz'), findsOneWidget);
      expect(find.text('Listar leases DHCP'), findsOneWidget);
      expect(find.text('Conexões e Vizinhos (1)'), findsOneWidget);

      // Executa Ping rápido inline antes de expandir a lista de interfaces
      await tester.ensureVisible(find.text('Ping rápido'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Ping rápido'));
      await tester.pumpAndSettle();
      expect(
        find.text('Latência estável sem perda de pacotes.'),
        findsOneWidget,
      );

      await tester.ensureVisible(find.text('Ver todas as 8 interfaces'));
      await tester.tap(find.text('Ver todas as 8 interfaces'));
      await tester.pumpAndSettle();
      expect(find.text('ether8'), findsOneWidget);
      expect(find.text('Mostrar menos'), findsOneWidget);

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
    'Repeated node drags A -> B -> C -> D, OFF/ON edit toggle, SSE non-clobbering & Undo/Discard/Save pill',
    (tester) async {
      _setViewport(tester, const Size(1280, 800));
      final notifier = _TestTopologyNotifier();
      await tester.pumpWidget(_buildApp(topoNotifier: notifier));
      await tester.pumpAndSettle();

      final posA = notifier.state.positions[1]!;
      final node1Finder = find.byKey(
        const ValueKey<String>('topology-node-detail-1'),
      );

      // Liga o modo de edição -> exibe pílula auxiliar de edição
      await tester.tap(find.byTooltip('Editar posições'));
      await tester.pumpAndSettle();
      expect(find.text('Arraste os nós para mover'), findsOneWidget);

      // Arraste 1: A -> B
      await tester.drag(node1Finder, const Offset(80, 40));
      await tester.pumpAndSettle();
      final posB = notifier.state.positions[1]!;
      expect(posB.x, isNot(closeTo(posA.x, 1.0)));
      expect(find.text('Layout editado'), findsOneWidget);
      expect(find.text('Desfazer'), findsOneWidget);
      expect(find.text('Descartar'), findsOneWidget);
      expect(find.text('Salvar'), findsOneWidget);

      // Arraste 2: MESMO nó B -> C (regressão direta do bug reportado)
      await tester.drag(node1Finder, const Offset(60, 50));
      await tester.pumpAndSettle();
      final posC = notifier.state.positions[1]!;
      expect(posC.x, isNot(closeTo(posB.x, 1.0)));
      expect(posC.y, isNot(closeTo(posB.y, 1.0)));

      // Arraste 3: MESMO nó C -> D
      await tester.drag(node1Finder, const Offset(-90, 40));
      await tester.pumpAndSettle();
      final posD = notifier.state.positions[1]!;
      expect(posD.x, isNot(closeTo(posC.x, 1.0)));

      // Eventos SSE em tempo real (status, health, traffic) NÃO sobrescrevem a posição D
      notifier.applyRealtimeStatus(<String, dynamic>{
        'id': 1,
        'status': 'warning',
      });
      notifier.applyRealtimeHealth(<String, dynamic>{
        'deviceId': 1,
        'metrics': <Map<String, dynamic>>[
          <String, dynamic>{'name': 'cpu_usage', 'value': 82.0},
        ],
      });
      notifier.applyRealtimeTraffic(<String, dynamic>{
        'deviceId': 1,
        'interfaces': <Map<String, dynamic>>[
          <String, dynamic>{
            'name': 'sfp-sfpplus1',
            'inBps': 500000000.0,
            'outBps': 300000000.0,
          },
        ],
      });
      // Após `applyRealtimeTraffic`, a animação contínua de fluxo fica ativa;
      // usamos `pump` com duração fixa em vez de `pumpAndSettle`.
      await tester.pump(const Duration(milliseconds: 250));
      expect(notifier.state.positions[1]!.x, closeTo(posD.x, 0.01));
      expect(notifier.state.positions[1]!.y, closeTo(posD.y, 0.01));

      // Desliga "Editando posições" -> arraste NÃO move o nó
      await tester.tap(find.byTooltip('Bloquear posições'));
      await tester.pump(const Duration(milliseconds: 250));
      await tester.drag(node1Finder, const Offset(70, 30));
      await tester.pump(const Duration(milliseconds: 250));
      expect(notifier.state.positions[1]!.x, closeTo(posD.x, 0.01));
      expect(notifier.state.positions[1]!.y, closeTo(posD.y, 0.01));

      // Liga "Editando posições" novamente -> arraste volta a mover (D -> E)
      await tester.tap(find.byTooltip('Editar posições'));
      await tester.pump(const Duration(milliseconds: 250));
      await tester.drag(node1Finder, const Offset(75, -35));
      await tester.pump(const Duration(milliseconds: 250));
      final posE = notifier.state.positions[1]!;
      expect(posE.x, isNot(closeTo(posD.x, 1.0)));

      // Desfazer 1x: E -> D
      await tester.tap(find.text('Desfazer'));
      await tester.pump(const Duration(milliseconds: 250));
      expect(notifier.state.positions[1]!.x, closeTo(posD.x, 0.01));
      expect(notifier.state.positions[1]!.y, closeTo(posD.y, 0.01));
      expect(find.text('Layout editado'), findsOneWidget);

      // Desfazer 2x: D -> C
      await tester.tap(find.text('Desfazer'));
      await tester.pump(const Duration(milliseconds: 250));
      expect(notifier.state.positions[1]!.x, closeTo(posC.x, 0.01));
      expect(notifier.state.positions[1]!.y, closeTo(posC.y, 0.01));

      // Descartar restaura completamente para A e oculta a pílula de não salvos
      await tester.tap(find.text('Descartar'));
      await tester.pump(const Duration(milliseconds: 250));
      expect(notifier.state.positions[1]!.x, closeTo(posA.x, 0.01));
      expect(notifier.state.positions[1]!.y, closeTo(posA.y, 0.01));
      expect(find.text('Layout editado'), findsNothing);

      // Move novamente e salva com sucesso
      await tester.drag(node1Finder, const Offset(65, 35));
      await tester.pump(const Duration(milliseconds: 250));
      expect(find.text('Layout editado'), findsOneWidget);
      await tester.tap(find.text('Salvar'));
      await tester.pump(const Duration(milliseconds: 250));
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

  testWidgets(
    'Zoom controls use unmistakable zoom_in/zoom_out icons with live percentage pill and Delete key confirms link deletion',
    (tester) async {
      _setViewport(tester, const Size(1280, 800));
      final notifier = _TestTopologyNotifier();
      await tester.pumpWidget(_buildApp(topoNotifier: notifier));
      await tester.pumpAndSettle();

      // Controles de zoom usam lupa (zoom_in/zoom_out) e nunca "+" genérico de adicionar.
      expect(find.byIcon(Icons.zoom_in), findsOneWidget);
      expect(find.byIcon(Icons.zoom_out), findsOneWidget);
      expect(
        find.descendant(
          of: find.byType(TopologyHud),
          matching: find.byIcon(Icons.add),
        ),
        findsNothing,
      );
      expect(find.textContaining('%'), findsWidgets);

      // Seleciona o enlace 10 via Inspetor de Nó -> chip do enlace no vizinho.
      await tester.tap(
        find.byKey(const ValueKey<String>('topology-node-detail-1')),
      );
      await tester.pump(const Duration(milliseconds: 350));
      await tester.pumpAndSettle();
      final edgeBadge = find.descendant(
        of: find.byType(TopologyNodeInspectorCard),
        matching: find.text('10 Gbps'),
      );
      await tester.ensureVisible(edgeBadge.last);
      await tester.pumpAndSettle();
      await tester.tap(edgeBadge.last);
      await tester.pumpAndSettle();
      expect(find.byType(TopologyEdgeInspectorCard), findsOneWidget);

      // Pressiona tecla Delete -> abre diálogo de confirmação; Cancelar NÃO remove.
      await tester.sendKeyEvent(LogicalKeyboardKey.delete);
      await tester.pumpAndSettle();
      expect(find.text('Remover enlace'), findsOneWidget);
      expect(
        find.text(
          'Remover o enlace entre Borda-CCR e Switch-Core da topologia?',
        ),
        findsOneWidget,
      );
      await tester.tap(find.text('Cancelar'));
      await tester.pumpAndSettle();
      expect(notifier.deletedLinkIds, isEmpty);
      expect(find.byType(TopologyEdgeInspectorCard), findsOneWidget);

      // Pressiona Delete novamente e confirma -> remove o enlace 10 e fecha o inspetor.
      await tester.sendKeyEvent(LogicalKeyboardKey.delete);
      await tester.pumpAndSettle();
      await tester.tap(find.text('Remover'));
      await tester.pumpAndSettle();
      expect(notifier.deletedLinkIds, equals(<int>[10]));
      expect(find.byType(TopologyEdgeInspectorCard), findsNothing);
    },
  );
}
