import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
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
import 'package:netmonitor_app/features/network/topology/engine/topology_engine.dart';
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
  SitesState build() =>
      const SitesState(items: <SiteModel>[SiteModel(id: 1, name: 'Matriz')]);

  @override
  Future<void> fetchSites() async {}
}

class _TestDevicesNotifier extends DevicesNotifier {
  @override
  DevicesState build() => const DevicesState(
    items: <DeviceModel>[
      DeviceModel(
        id: 1,
        name: 'MikroTik CCR2004',
        type: 'router',
        status: 'online',
        ipAddress: '10.10.0.1',
        vendor: 'MikroTik',
        siteId: 1,
      ),
      DeviceModel(
        id: 2,
        name: 'AP OpenWrt Recepção',
        type: 'access_point',
        status: 'online',
        ipAddress: '10.10.0.25',
        vendor: 'OpenWrt',
        siteId: 1,
      ),
      DeviceModel(
        id: 3,
        name: 'Switch Campinas',
        type: 'switch',
        status: 'offline',
        ipAddress: '10.20.0.1',
        vendor: 'Ubiquiti',
        siteId: 1,
      ),
    ],
  );

  @override
  Future<void> fetchDevices({int? siteId, int? networkId}) async {}
}

class _TestLinkTestsNotifier extends LinkTestsNotifier {
  @override
  LinkTestsState build() => const LinkTestsState();
}

class _TestWifiClientsNotifier extends WifiClientsNotifier {
  @override
  WifiClientsState build() => const WifiClientsState();
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
      small: QuickPingRtt(minMs: 1.0, avgMs: 1.8, maxMs: 2.5),
      capacityNote: 'Link saudável',
      verdict: 'OK',
    );
  }
}

class _LiveTopologyNotifier extends TopologyNotifier {
  _LiveTopologyNotifier(this._initial);

  final TopologyState _initial;

  @override
  TopologyState build() => _initial;

  @override
  Future<void> fetchTopology({
    int? siteId,
    bool updateSiteFilter = false,
    bool showLoading = true,
    bool preserveUnsavedPositions = false,
  }) async {}
}

TopologyState _seededLiveTopologyState() {
  return const TopologyState(
    loaded: true,
    nodes: <TopologyNodeModel>[
      TopologyNodeModel(
        id: 1,
        name: 'MikroTik CCR2004',
        type: 'router',
        status: 'online',
        ipAddress: '10.10.0.1',
        vendor: 'MikroTik',
        snmpEnabled: true,
        interfaceCount: 2,
      ),
      TopologyNodeModel(
        id: 2,
        name: 'AP OpenWrt Recepção',
        type: 'access_point',
        status: 'online',
        ipAddress: '10.10.0.25',
        vendor: 'OpenWrt',
        snmpEnabled: true,
        interfaceCount: 1,
      ),
      TopologyNodeModel(
        id: 3,
        name: 'Switch Campinas',
        type: 'switch',
        status: 'offline',
        ipAddress: '10.20.0.1',
        vendor: 'Ubiquiti',
      ),
      TopologyNodeModel(
        id: 99,
        name: 'Servidor NetMonitor',
        type: 'server',
        status: 'online',
        isSystem: true,
      ),
    ],
    edges: <TopologyEdgeModel>[
      TopologyEdgeModel(
        id: 10,
        sourceDeviceId: 1,
        targetDeviceId: 2,
        sourceDeviceName: 'MikroTik CCR2004',
        targetDeviceName: 'AP OpenWrt Recepção',
        sourceInterfaceId: 101,
        targetInterfaceId: 201,
        sourceInterfaceName: 'ether2',
        targetInterfaceName: 'eth0',
        linkType: 'ethernet',
        discoveryMethod: 'lldp',
        status: 'up',
        speedBps: 1000000000,
        inBps: 84500000,
        outBps: 31200000,
      ),
      TopologyEdgeModel(
        id: 11,
        sourceDeviceId: 1,
        targetDeviceId: 3,
        sourceDeviceName: 'MikroTik CCR2004',
        targetDeviceName: 'Switch Campinas',
        sourceInterfaceId: 102,
        targetInterfaceId: 301,
        sourceInterfaceName: 'sfp1',
        targetInterfaceName: 'sfp1',
        linkType: 'fiber',
        discoveryMethod: 'manual',
        status: 'down',
        speedBps: 10000000000,
      ),
      TopologyEdgeModel(
        id: 12,
        sourceDeviceId: 3,
        targetDeviceId: 2,
        sourceDeviceName: 'Switch Campinas',
        targetDeviceName: 'AP OpenWrt Recepção',
        sourceInterfaceId: null,
        targetInterfaceId: 202,
        targetInterfaceName: 'eth1',
        linkType: 'ethernet',
        discoveryMethod: 'lldp',
        status: 'up',
        speedBps: 1000000000,
      ),
      TopologyEdgeModel(
        id: 13,
        sourceDeviceId: 99,
        targetDeviceId: 1,
        sourceDeviceName: 'Servidor NetMonitor',
        targetDeviceName: 'MikroTik CCR2004',
        linkType: 'ethernet',
        discoveryMethod: 'manual',
        status: 'up',
        speedBps: 1000000000,
      ),
    ],
    positions: <int, TopologyPosition>{
      1: TopologyPosition(180, 90),
      2: TopologyPosition(80, 260),
      3: TopologyPosition(280, 260),
      99: TopologyPosition(180, -70),
    },
  );
}

void _expectNoRawUnicodeArrows(String? value) {
  if (value == null) return;
  for (final forbidden in const <String>['↓', '↑', '→', '↔']) {
    expect(
      value.contains(forbidden),
      isFalse,
      reason: 'String "$value" must not contain raw Unicode arrow "$forbidden"',
    );
  }
}

void main() {
  setUp(() {
    SharedPreferences.setMockInitialValues(<String, Object>{});
  });

  group('TopologyEdgeModel canonical live traffic getters', () {
    test(
      'effectiveTrafficBps, hasLiveTraffic and formattedTraffic work when constructed with only inBps/outBps',
      () {
        const edge = TopologyEdgeModel(
          id: 10,
          sourceDeviceId: 1,
          targetDeviceId: 2,
          linkType: 'ethernet',
          discoveryMethod: 'lldp',
          status: 'up',
          speedBps: 1000000000,
          inBps: 84500000,
          outBps: 31200000,
        );

        expect(edge.trafficBps, isNull);
        expect(edge.hasLiveTraffic, isTrue);
        expect(edge.effectiveTrafficBps, closeTo(115700000, 0.1));
        expect(edge.formattedTraffic, '115.7 Mbps');

        const idleEdge = TopologyEdgeModel(
          id: 11,
          sourceDeviceId: 1,
          targetDeviceId: 3,
          linkType: 'fiber',
          discoveryMethod: 'manual',
          status: 'up',
          speedBps: 10000000000,
        );
        expect(idleEdge.hasLiveTraffic, isFalse);
        expect(idleEdge.effectiveTrafficBps, isNull);
        expect(idleEdge.formattedTraffic, '');
      },
    );
  });

  group('TopologyNotifier SSE realtime traffic & health updates (zero polling)', () {
    test(
      'applyRealtimeTraffic updates sourceDeviceId and targetDeviceId repeatedly without freezing after 1st event',
      () {
        final notifier = _LiveTopologyNotifier(_seededLiveTopologyState());
        final container = ProviderContainer(
          overrides: <Override>[topologyProvider.overrideWith(() => notifier)],
        );
        addTearDown(container.dispose);

        // 1) Pulso 1 no targetDeviceId (deviceId: 2, porta 202) onde sourceInterfaceId == null (edge 12).
        container.read(topologyProvider.notifier).applyRealtimeTraffic(
          <String, dynamic>{
            'deviceId': 2,
            'interfaces': <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 202,
                'name': 'eth1',
                'inBps': 50000000.0,
                'outBps': 15000000.0,
                'operStatus': 'up',
                'speed': 1000000000,
              },
            ],
          },
        );
        var edge12 = container
            .read(topologyProvider)
            .edges
            .firstWhere((e) => e.id == 12);
        expect(edge12.inBps, 50000000.0);
        expect(edge12.outBps, 15000000.0);

        // 2) Pulso 2 e Pulso 3 no MESMO targetDeviceId -> NUNCA congela na 1ª leitura!
        container.read(topologyProvider.notifier).applyRealtimeTraffic(
          <String, dynamic>{
            'deviceId': 2,
            'interfaces': <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 202,
                'name': 'eth1',
                'inBps': 185000000.0,
                'outBps': 64000000.0,
                'operStatus': 'up',
                'speed': 1000000000,
              },
            ],
          },
        );
        edge12 = container
            .read(topologyProvider)
            .edges
            .firstWhere((e) => e.id == 12);
        expect(edge12.inBps, 185000000.0);
        expect(edge12.outBps, 64000000.0);
        expect(edge12.effectiveTrafficBps, 249000000.0);
        expect(edge12.utilization, closeTo(0.185, 0.001));

        // 3) Também atualiza pelo targetDeviceId em enlace com sourceInterfaceId != null
        //    enquanto o source ainda não enviou telemetria SSE (edge 10), e depois cede ao source.
        container.read(topologyProvider.notifier).applyRealtimeTraffic(
          <String, dynamic>{
            'deviceId': 2,
            'interfaces': <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 201,
                'name': 'eth0',
                'inBps': 120000000.0,
                'outBps': 40000000.0,
                'operStatus': 'up',
              },
            ],
          },
        );
        var edge10 = container
            .read(topologyProvider)
            .edges
            .firstWhere((e) => e.id == 10);
        expect(edge10.inBps, 120000000.0);
        expect(edge10.outBps, 40000000.0);

        // Source (deviceId: 1, porta 101) envia SSE -> assume propriedade autoritativa
        container.read(topologyProvider.notifier).applyRealtimeTraffic(
          <String, dynamic>{
            'deviceId': 1,
            'interfaces': <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 101,
                'name': 'ether2',
                'inBps': 310000000.0,
                'outBps': 95000000.0,
                'operStatus': 'up',
                'speed': 1000000000,
              },
            ],
          },
        );
        edge10 = container
            .read(topologyProvider)
            .edges
            .firstWhere((e) => e.id == 10);
        expect(edge10.inBps, 310000000.0);
        expect(edge10.outBps, 95000000.0);
        expect(edge10.trafficFromSource, isTrue);

        // Evento subsequente do target atualiza status/speed do target sem sobrescrever RX/TX do source
        container.read(topologyProvider.notifier).applyRealtimeTraffic(
          <String, dynamic>{
            'deviceId': 2,
            'interfaces': <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 201,
                'name': 'eth0',
                'inBps': 10000000.0,
                'outBps': 5000000.0,
                'operStatus': 'up',
                'speed': 1000000000,
              },
            ],
          },
        );
        edge10 = container
            .read(topologyProvider)
            .edges
            .firstWhere((e) => e.id == 10);
        expect(edge10.inBps, 310000000.0);
        expect(edge10.outBps, 95000000.0);
      },
    );

    test(
      'applyRealtimeHealth (monitor:result) updates CPU/RAM, interfaceId traffic, and system host traffic',
      () {
        final notifier = _LiveTopologyNotifier(_seededLiveTopologyState());
        final container = ProviderContainer(
          overrides: <Override>[topologyProvider.overrideWith(() => notifier)],
        );
        addTearDown(container.dispose);

        // 1) monitor:result com cpu_usage, memory_usage e inBps/outBps por interfaceId
        container.read(topologyProvider.notifier).applyRealtimeHealth(
          <String, dynamic>{
            'deviceId': 1,
            'metrics': <Map<String, dynamic>>[
              <String, dynamic>{'name': 'cpu_usage', 'value': 44.0},
              <String, dynamic>{'name': 'memory_usage', 'value': 63.0},
              <String, dynamic>{
                'name': 'inBps',
                'value': 420000000.0,
                'interfaceId': 101,
              },
              <String, dynamic>{
                'name': 'outBps',
                'value': 180000000.0,
                'interfaceId': 101,
              },
            ],
          },
        );

        final node1 = container
            .read(topologyProvider)
            .nodes
            .firstWhere((n) => n.id == 1);
        final edge10 = container
            .read(topologyProvider)
            .edges
            .firstWhere((e) => e.id == 10);
        expect(node1.cpuPercent, 44.0);
        expect(node1.memoryPercent, 63.0);
        expect(edge10.inBps, 420000000.0);
        expect(edge10.outBps, 180000000.0);

        // 2) monitor:result do host sistema (isSystem == true, sem interfaceId)
        container.read(topologyProvider.notifier).applyRealtimeHealth(
          <String, dynamic>{
            'deviceId': 99,
            'metrics': <Map<String, dynamic>>[
              <String, dynamic>{'name': 'cpu_usage', 'value': 19.0},
              <String, dynamic>{'name': 'inBps', 'value': 72000000.0},
              <String, dynamic>{'name': 'outBps', 'value': 28000000.0},
            ],
          },
        );
        final edge13 = container
            .read(topologyProvider)
            .edges
            .firstWhere((e) => e.id == 13);
        expect(edge13.inBps, 72000000.0);
        expect(edge13.outBps, 28000000.0);
      },
    );
  });

  group('TopologyEdgeBadgeSpec, formatters & flow speed factor', () {
    test(
      'resolveEdgeBadgeSpec produces rich directional NOC spec, down label, latency spec and zero Unicode arrows',
      () {
        final state = _seededLiveTopologyState();
        final activeEdge = state.edges.firstWhere((e) => e.id == 10);
        final downEdge = state.edges.firstWhere((e) => e.id == 11);
        final idleEdge = state.edges.firstWhere((e) => e.id == 12);

        expect(kTopologyEdgeBadgeMinScale, 0.55);

        // Enlace ativo com inBps: 84.5 Mbps, outBps: 31.2 Mbps, speedBps: 1 Gbps (mesmo sem trafficBps explícito!)
        for (final mode in const <TopologyOverlayMode>[
          TopologyOverlayMode.status,
          TopologyOverlayMode.traffic,
          TopologyOverlayMode.health,
        ]) {
          final spec = resolveEdgeBadgeSpec(activeEdge, overlay: mode);
          expect(spec.hasDirectionalTraffic, isTrue);
          expect(spec.rxLabel, '84.5 Mbps');
          expect(spec.txLabel, '31.2 Mbps');
          expect(spec.secondaryLabel, '1 Gbps · 8,5%');
          expect(spec.utilization, closeTo(0.0845, 0.001));
          expect(spec.isDown, isFalse);
          _expectNoRawUnicodeArrows(spec.primaryLabel);
          _expectNoRawUnicodeArrows(spec.rxLabel);
          _expectNoRawUnicodeArrows(spec.txLabel);
          _expectNoRawUnicodeArrows(spec.secondaryLabel);
        }

        // Enlace down com speedBps: 10 Gbps -> "Offline · 10 Gbps" e SemanticTone.error
        final downSpec = resolveEdgeBadgeSpec(downEdge);
        expect(downSpec.isDown, isTrue);
        expect(downSpec.tone, SemanticTone.error);
        expect(downSpec.primaryLabel, 'Offline · 10 Gbps');
        expect(downSpec.hasDirectionalTraffic, isFalse);
        expect(formatEdgeBadgeLabel(downEdge), 'Offline · 10 Gbps');

        // Modo latência com telemetria secundária
        final latSpec = resolveEdgeBadgeSpec(
          activeEdge,
          overlay: TopologyOverlayMode.latency,
          latencyMs: 6.8,
        );
        expect(latSpec.primaryLabel, '6.8 ms');
        expect(latSpec.secondaryLabel, '115.7 Mbps');
        expect(latSpec.tone, SemanticTone.success);

        // Enlace sem telemetria SNMP ainda -> velocidade nominal
        final idleSpec = resolveEdgeBadgeSpec(idleEdge);
        expect(idleSpec.primaryLabel, '1 Gbps');
        expect(idleSpec.hasDirectionalTraffic, isFalse);

        // formatEdgeTrafficLabel sem setas Unicode cruas
        final trafficText = formatEdgeTrafficLabel(activeEdge);
        expect(trafficText, 'RX 84.5 Mbps · TX 31.2 Mbps');
        _expectNoRawUnicodeArrows(trafficText);

        // Velocidade das partículas ("formigas") proporcional ao tráfego/utilização
        expect(computeTrafficFlowSpeedFactor(downEdge), 0.0);
        final lowFlow = computeTrafficFlowSpeedFactor(activeEdge);
        final highFlow = computeTrafficFlowSpeedFactor(
          activeEdge.withTrafficUpdate(inBps: 900000000, outBps: 400000000),
        );
        expect(lowFlow, greaterThan(0.45));
        expect(highFlow, greaterThan(lowFlow));
      },
    );
  });

  group('Canvas NOC pill & Node Inspector live traffic widgets', () {
    testWidgets(
      'Canvas renders live traffic NOC pills at 0.58x and 1.0x zoom and Node Inspector updates live on SSE',
      (tester) async {
        tester.view.physicalSize = const Size(1280, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(() {
          tester.view.resetPhysicalSize();
          tester.view.resetDevicePixelRatio();
        });

        final notifier = _LiveTopologyNotifier(_seededLiveTopologyState());
        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              authProvider.overrideWith(_TestAuthNotifier.new),
              sitesProvider.overrideWith(_TestSitesNotifier.new),
              devicesProvider.overrideWith(_TestDevicesNotifier.new),
              linkTestsProvider.overrideWith(_TestLinkTestsNotifier.new),
              wifiClientsProvider.overrideWith(_TestWifiClientsNotifier.new),
              diagnosticsServiceProvider.overrideWithValue(
                _FakeDiagnosticsService(),
              ),
              topologyNodeInterfacesProvider.overrideWith(
                (ref, deviceId) async => const <DeviceInterfaceModel>[],
              ),
              topologyProvider.overrideWith(() => notifier),
            ],
            child: MaterialApp(
              theme: AppTheme.dark(),
              home: const Scaffold(body: TopologyPage()),
            ),
          ),
        );
        await tester.pump(const Duration(milliseconds: 350));

        expect(tester.takeException(), isNull);
        expect(find.byType(TopologyCanvasView), findsOneWidget);

        // Abre Inspetor de Nó clicando em MikroTik CCR2004
        await tester.tap(
          find.byKey(const ValueKey<String>('topology-node-detail-1')),
        );
        await tester.pump(const Duration(milliseconds: 350));

        expect(find.byType(TopologyNodeInspectorCard), findsOneWidget);
        expect(find.text('RX 84.5 Mbps'), findsOneWidget);
        expect(find.text('TX 31.2 Mbps'), findsOneWidget);
        expect(find.text('Offline · 10 Gbps'), findsOneWidget);
        expect(find.byIcon(Icons.arrow_downward_rounded), findsWidgets);
        expect(find.byIcon(Icons.arrow_upward_rounded), findsWidgets);

        // Emite pulso SSE interface:traffic -> Inspetor e Canvas atualizam em tempo real!
        notifier.applyRealtimeTraffic(<String, dynamic>{
          'deviceId': 1,
          'interfaces': <Map<String, dynamic>>[
            <String, dynamic>{
              'id': 101,
              'name': 'ether2',
              'inBps': 512000000.0,
              'outBps': 128000000.0,
              'operStatus': 'up',
              'speed': 1000000000,
            },
          ],
        });
        await tester.pump(const Duration(milliseconds: 200));

        expect(tester.takeException(), isNull);
        expect(find.text('RX 512 Mbps'), findsOneWidget);
        expect(find.text('TX 128 Mbps'), findsOneWidget);
      },
    );

    testWidgets(
      'Mobile 360x844 auto-fit renders NOC pills cleanly in Light & Dark themes without overflow',
      (tester) async {
        tester.view.physicalSize = const Size(360, 844);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(() {
          tester.view.resetPhysicalSize();
          tester.view.resetDevicePixelRatio();
        });

        for (final theme in <ThemeData>[AppTheme.dark(), AppTheme.light()]) {
          final controller = TopologyCanvasController();
          addTearDown(controller.dispose);
          final state = _seededLiveTopologyState();

          await tester.pumpWidget(
            MaterialApp(
              theme: theme,
              home: Scaffold(
                body: TopologyCanvasView(
                  nodes: state.nodes,
                  edges: state.edges,
                  positions: <int, Offset>{
                    for (final e in state.positions.entries)
                      e.key: Offset(e.value.x, e.value.y),
                  },
                  controller: controller,
                  showTrafficFlow: true,
                ),
              ),
            ),
          );
          await tester.pump(const Duration(milliseconds: 150));
          controller.setScale(0.58, animate: false);
          await tester.pump(const Duration(milliseconds: 100));
          expect(tester.takeException(), isNull);
        }
      },
    );
  });
}
