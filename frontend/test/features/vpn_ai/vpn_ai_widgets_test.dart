import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/areas/composition_registries.dart';
import 'package:netmonitor_app/areas/vpn_area.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/pages/ai_chat_page.dart';
import 'package:netmonitor_app/features/ai/pages/decider_benchmark_page.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_settings_card.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_tool_card.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_tool_chart.dart';
import 'package:netmonitor_app/features/ai/widgets/decider_settings_card.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/pages/plugin_app_page.dart';
import 'package:netmonitor_app/features/plugins/pages/plugins_page.dart';
import 'package:netmonitor_app/features/plugins/providers/plugin_apps_provider.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/features/plugins/utils/plugin_settings_logic.dart';
import 'package:netmonitor_app/features/plugins/widgets/device_access_dialog.dart';
import 'package:netmonitor_app/features/plugins/widgets/device_plugins_tab.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_approval_dialog.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_review_dialog.dart';
import 'package:netmonitor_app/features/vpn/models/vpn_models.dart';
import 'package:netmonitor_app/features/vpn/pages/vpn_devices_page.dart';
import 'package:netmonitor_app/features/vpn/pages/vpn_server_page.dart';
import 'package:netmonitor_app/features/vpn/providers/vpn_provider.dart';
import 'package:netmonitor_app/features/vpn/widgets/vpn_script_viewer.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class _AdminAuthStub extends AuthNotifier {
  @override
  AuthState build() => const AuthState(
    token: 'test-token',
    initialized: true,
    user: AuthUser(
      id: 1,
      email: 'admin@netmonitor.local',
      fullName: 'Admin',
      role: UserRole.admin,
    ),
  );
}

class _SeededVpnNotifier extends VpnNotifier {
  @override
  VpnState build() => const VpnState(
    serverState: VpnServerStateModel(
      configured: true,
      server: VpnServerConfig(
        id: 1,
        networkId: 1,
        interfaceName: 'wg0',
        listenPort: 51820,
        publicEndpoint: 'vpn.exemplo.com.br',
        publicKey: 'PUBKEY1234567890',
        allowPeerToPeer: false,
        mtu: 1420,
        active: true,
      ),
      cidr: '10.8.0.0/24',
      serverAddress: '10.8.0.1',
      peersTotal: 1,
      peersConnected: 1,
      bytesRx: 1024 * 1024,
      bytesTx: 2048 * 1024,
      persistentKeepalive: 25,
    ),
    peers: <VpnPeer>[
      VpnPeer(
        id: 10,
        vpnServerId: 1,
        deviceId: 5,
        publicKey: 'PEERKEY123',
        deviceProfile: 'linux',
        persistentKeepalive: 25,
        bytesRx: 512000,
        bytesTx: 256000,
        enabled: true,
        connectionStatus: VpnConnectionStatus.connected,
        needsFirewallHint: true,
      ),
    ],
  );

  @override
  Future<void> fetchServer({bool includeAuxiliary = false}) async {}

  @override
  Future<void> fetchPeers() async {}
}

class _SeededAiSettingsNotifier extends AiSettingsNotifier {
  @override
  AiSettingsState build() => const AiSettingsState(
    settings: AiSettings(
      enabled: true,
      activeDriver: 'ollama',
      ollamaModel: 'netmonitor/chat:q8_0',
    ),
  );

  @override
  Future<void> load() async {}

  @override
  Future<void> loadOllamaModels({String? baseUrl}) async {}
}

class _SeededAiConversationsNotifier extends AiConversationsNotifier {
  @override
  AiConversationsState build() => const AiConversationsState();

  @override
  Future<void> fetchAll() async {}
}

class _SeededAiDeciderNotifier extends AiDeciderNotifier {
  @override
  AiDeciderState build() => const AiDeciderState(
    saved: AiDeciderSettings(
      enabled: false,
      provider: 'ollaya',
      model: 'netmonitor/decima-network:v15',
    ),
    models: DeciderModelsResponse(online: true),
  );

  @override
  Future<void> loadAll() async {}
}

class _SeededAiBenchmarkNotifier extends AiBenchmarkNotifier {
  @override
  AiBenchmarkState build() => const AiBenchmarkState();

  @override
  Future<void> loadChat({bool force = false}) async {}

  @override
  Future<void> loadDecider({bool force = false}) async {}
}

class _SeededPluginsNotifier extends PluginsNotifier {
  @override
  PluginsState build() => const PluginsState(
    library: <PluginSummary>[
      PluginSummary(
        id: 7,
        slug: 'wifi-fleet',
        name: 'Gestor Wi-Fi Corporativo',
        version: '1.2.0',
        status: 'active',
        description: 'Gerencia redes sem fio em frota de pontos de acesso.',
        transports: <String>['ssh'],
      ),
    ],
  );

  @override
  Future<void> fetchLibrary() async {}

  @override
  Future<PluginDetail> fetchDetail(int id) async => const PluginDetail(
    summary: PluginSummary(
      id: 7,
      slug: 'wifi-fleet',
      name: 'Gestor Wi-Fi Corporativo',
      version: '1.2.0',
      status: 'active',
    ),
    package: PluginPackage(
      manifest: <String, dynamic>{},
      script: '',
      usage: '# Uso',
    ),
  );
}

class _SeededPluginAppsNotifier extends PluginAppsNotifier {
  @override
  PluginAppsState build() => const PluginAppsState(
    fleets: <int, FleetView>{
      7: FleetView(
        plugin: PluginSummary(
          id: 7,
          slug: 'wifi-fleet',
          name: 'Gestor Wi-Fi Corporativo',
          version: '1.2.0',
          status: 'active',
          description:
              'Aplicativo de frota com descrição longa para testar quebra segura no mobile.',
          wifi: true,
          fleet: PluginFleetSpec(
            title: 'Redes Wi-Fi',
            statusAction: 'status',
            list: PluginItemListSpec(title: 'Redes'),
          ),
        ),
        members: <FleetMember>[
          FleetMember(
            deviceId: 1,
            deviceName: 'AP-Recepcao',
            ipAddress: '10.0.0.21',
          ),
        ],
      ),
    },
  );

  @override
  Future<void> loadFleet(int pluginId) async {}

  @override
  Future<int> runAction({
    required int pluginId,
    required String action,
    List<int>? deviceIds,
    Map<String, dynamic> params = const <String, dynamic>{},
    Map<int, Map<String, dynamic>>? deviceParams,
    bool confirmWrite = false,
  }) async => 1;
}

class _StubDeviceAccessController extends DeviceAccessController {
  _StubDeviceAccessController({
    this.platform = 'openwrt',
    this.rejectProvisionWith401 = false,
  });

  final String platform;
  final bool rejectProvisionWith401;
  int provisionAttempts = 0;

  @override
  Future<DeviceAccessViewModel> fetchAccess(int deviceId) async =>
      DeviceAccessViewModel(
        deviceId: deviceId,
        deviceName: platform == 'mikrotik' ? 'RB-Borda' : 'AP-Escritorio',
        dedicatedPlatform: platform,
        platformHint: platform,
      );

  @override
  Future<void> provisionDedicatedKey({
    required int deviceId,
    required String platform,
    required String username,
    required String password,
    int port = 22,
    int? viaProbeId,
  }) async {
    provisionAttempts += 1;
    if (rejectProvisionWith401) {
      throw const ApiException(
        statusCode: 401,
        message: 'Usuário ou senha recusados pelo equipamento.',
      );
    }
  }
}

class _OpenWrtFleetAppsNotifier extends PluginAppsNotifier {
  _OpenWrtFleetAppsNotifier({required this.candidates});

  final List<FleetCandidate> candidates;

  @override
  PluginAppsState build() => PluginAppsState(
    fleets: <int, FleetView>{
      9: FleetView(
        plugin: const PluginSummary(
          id: 9,
          slug: 'openwrt-wifi',
          name: 'Rede Wi-Fi OpenWrt',
          version: '1.4.0',
          status: 'active',
          wifi: true,
          actions: <PluginAction>[
            PluginAction(name: 'status', title: 'Estado atual'),
            PluginAction(
              name: 'add_ssid',
              title: 'Nova rede Wi-Fi',
              effect: 'write',
            ),
            PluginAction(
              name: 'plan_channels',
              title: 'Planejar canais',
              description: 'Analisa interferência entre rádios da frota.',
            ),
          ],
          fleet: PluginFleetSpec(
            title: 'Rede Wi-Fi OpenWrt',
            statusAction: 'status',
            actions: <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 'plan_channels',
                'action': 'plan_channels',
                'title': 'Planejar canais',
                'description': 'Analisa interferência entre rádios da frota.',
              },
            ],
            list: PluginItemListSpec(
              title: 'Redes',
              itemsKey: 'networks',
              itemKey: 'ssid',
              addAction: PluginItemAction(
                action: 'add_ssid',
                title: 'Adicionar rede',
              ),
              editAction: PluginItemAction(action: 'add_ssid', title: 'Editar'),
              wifiQr: PluginWifiQrSpec(
                ssid: 'ssid',
                encryption: 'encryption',
                revealAction: 'reveal_psk',
                secretField: 'password',
              ),
            ),
          ),
        ),
        members: const <FleetMember>[],
        candidates: candidates,
        batches: const <PluginBatchView>[
          PluginBatchView(
            id: 101,
            pluginId: 9,
            action: 'status',
            title: 'Atualizar',
            status: 'succeeded',
            finishedAt: '2026-11-22T12:00:00Z',
            devices: <BatchDevice>[
              BatchDevice(
                deviceId: 1,
                deviceName: 'AP-Recepcao',
                status: 'succeeded',
                output: <String, dynamic>{
                  'networks': <Map<String, dynamic>>[
                    <String, dynamic>{
                      'ssid': 'Corporativo-5G',
                      'encryption': 'psk2',
                      'channel': 36,
                    },
                  ],
                },
              ),
            ],
          ),
        ],
      ),
    },
  );

  @override
  Future<void> loadFleet(int pluginId) async {}

  @override
  Future<int> runAction({
    required int pluginId,
    required String action,
    required List<int> deviceIds,
    required bool confirmWrite,
    Map<String, dynamic> params = const <String, dynamic>{},
    Map<int, Map<String, dynamic>>? deviceParams,
  }) async => 102;

  @override
  Future<FleetView> identify(
    int pluginId, {
    List<int> deviceIds = const <int>[],
  }) async => state.fleets[pluginId]!;
}

class _SeededDevicePluginsNotifier extends PluginsNotifier {
  @override
  PluginsState build() => const PluginsState(
    deviceViews: <int, DevicePluginsView>{
      5: DevicePluginsView(
        deviceId: 5,
        platform: 'openwrt',
        firmware: 'OpenWrt 23.05.5',
        plugins: <DevicePluginItem>[
          DevicePluginItem(
            installed: true,
            compatibility: 'validated',
            plugin: PluginSummary(
              id: 9,
              slug: 'openwrt-wifi',
              name: 'Rede Wi-Fi OpenWrt',
              version: '1.4.0',
              status: 'active',
              actions: <PluginAction>[
                PluginAction(name: 'status', title: 'Ler rádios'),
                PluginAction(
                  name: 'apply_wifi',
                  title: 'Aplicar Wi-Fi',
                  effect: 'write',
                ),
              ],
            ),
          ),
        ],
        credentials: <CredentialView>[
          CredentialView(
            kind: 'ssh',
            username: 'root',
            storage: 'key',
            port: 22,
            dedicated: DedicatedAccess(
              username: 'root',
              platform: 'openwrt',
              fingerprint: 'SHA256:abc123',
            ),
          ),
        ],
      ),
    },
  );

  @override
  Future<void> loadDevice(int deviceId) async {}
}

void _setMobileViewport(WidgetTester tester) {
  tester.view.physicalSize = const Size(390, 844);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(() {
    tester.view.resetPhysicalSize();
    tester.view.resetDevicePixelRatio();
  });
}

Widget _wrap(Widget child, {List<Override> overrides = const <Override>[]}) {
  return ProviderScope(
    overrides: overrides,
    child: MaterialApp(
      theme: AppTheme.light(),
      darkTheme: AppTheme.dark(),
      home: Scaffold(body: child),
    ),
  );
}

void main() {
  group('ADR 017/019 & Plugin Color Token Contracts (vpnArea)', () {
    test(
      'preserves exact routeMeta areas for VPN, AI (core) and Plugins (network)',
      () {
        expect(vpnArea.routeMeta['/vpn']?.area, ProductArea.vpn);
        expect(vpnArea.routeMeta['/ai-chat']?.area, ProductArea.core);
        expect(vpnArea.routeMeta['/ai-chat']?.fullBleed, isTrue);
        expect(vpnArea.routeMeta['/ai-benchmark']?.area, ProductArea.core);
        expect(vpnArea.routeMeta['/plugins']?.area, ProductArea.network);
        expect(vpnArea.routeMeta['/plugins']?.requiresAdmin, isTrue);
        expect(vpnArea.routeMeta['/apps']?.area, ProductArea.network);
      },
    );

    test('AI settings cards stay visible even when VPN area is disabled', () {
      final onlyCore = <ProductArea>{ProductArea.core};
      final tabs = SettingsSectionRegistry.visibleTabs(onlyCore);
      final iaTab = tabs.where((t) => t.tab.value == 'ia').firstOrNull;
      expect(iaTab, isNotNull);
      expect(iaTab!.cards.length, 4);
    });

    test(
      'DevicePluginsTab uses SemanticTone.plugin (teal) and never purple/primary',
      () {
        final pluginsTab = vpnArea.deviceDetailTabs.firstWhere(
          (tab) => tab.id == 'plugins',
        );
        expect(pluginsTab.area, ProductArea.network);
        expect(pluginsTab.tone, SemanticTone.plugin);
        expect(AppSemanticColors.light.plugin, const Color(0xFF00796B));
        expect(AppSemanticColors.dark.plugin, const Color(0xFF1DE9B6));
      },
    );

    test(
      'registers extraSseBuilders and extraShellOverlays per ADR 017 area',
      () {
        expect(vpnArea.sseBuilder, isNotNull);
        expect(vpnArea.extraSseBuilders.containsKey(ProductArea.core), isTrue);
        expect(
          vpnArea.extraSseBuilders.containsKey(ProductArea.network),
          isTrue,
        );

        final overlaysCoreOnly = visibleShellOverlays(<ProductArea>{
          ProductArea.core,
        });
        expect(overlaysCoreOnly.length, greaterThanOrEqualTo(2));
      },
    );
  });

  group('Security & Interactive Widget Parity Tests', () {
    testWidgets(
      'AiToolCard shows DeviceAccess write hallucination warning & Approve/Reject',
      (tester) async {
        const tool = AiToolCallState(
          id: 'call-dev-1',
          name: 'device_ssh_exec',
          arguments: <String, dynamic>{'command': 'uci commit'},
          summary: 'ALTERA O EQUIPAMENTO: aplicar uci commit',
          status: AiToolStatus.awaiting,
          risk: AiToolRisk.high,
          reason: 'Modifica configuração ativa do roteador.',
        );

        await tester.pumpWidget(
          _wrap(
            const SingleChildScrollView(
              child: AiToolCard(messageId: 'm-1', tool: tool),
            ),
          ),
        );

        expect(
          find.textContaining('ALTERA a configuração do equipamento'),
          findsOneWidget,
        );
        expect(find.text('Risco alto'), findsOneWidget);
        expect(find.text('Confirmar'), findsOneWidget);
        expect(find.text('Cancelar'), findsOneWidget);
      },
    );

    testWidgets('AiToolChart renders fl_chart series and legend', (
      tester,
    ) async {
      const chart = AiChart(
        title: 'Latência do Gateway',
        subtitle: 'Última hora',
        unit: 'latency',
        avgValue: 12.5,
        series: <AiChartSeries>[
          AiChartSeries(
            id: 'rtt',
            label: 'RTT',
            points: <AiChartPoint>[
              AiChartPoint(time: '2026-11-20T10:00:00Z', value: 10),
              AiChartPoint(time: '2026-11-20T10:05:00Z', value: 15),
            ],
          ),
        ],
      );

      await tester.pumpWidget(
        _wrap(
          const Padding(
            padding: EdgeInsets.all(16),
            child: AiToolChart(chart: chart),
          ),
        ),
      );

      expect(find.text('Latência do Gateway'), findsOneWidget);
      expect(find.textContaining('Média:'), findsOneWidget);
    });

    testWidgets('PluginApprovalOverlay shows write access confirmation modal', (
      tester,
    ) async {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      container.read(pluginsProvider.notifier).applyApprovalRequired(
        const <String, dynamic>{
          'key': 'appr-write-1',
          'runId': 22,
          'deviceId': 4,
          'deviceName': 'SW-Core',
          'pluginName': 'VLAN Manager',
          'action': 'create_vlan',
          'request': <String, dynamic>{
            'transport': 'ssh',
            'effect': 'write',
            'summary': 'interface vlan 50',
          },
          'expiresAt': '2026-11-20T12:30:00Z',
        },
      );

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            theme: AppTheme.light(),
            home: const Scaffold(
              body: Stack(
                children: <Widget>[SizedBox.expand(), PluginApprovalOverlay()],
              ),
            ),
          ),
        ),
      );

      expect(find.text('Aprovar acesso ao equipamento'), findsOneWidget);
      expect(find.text('Altera o equipamento'), findsOneWidget);
      expect(find.text('interface vlan 50'), findsOneWidget);
      expect(find.text('Aprovar'), findsOneWidget);
      expect(find.text('Negar'), findsOneWidget);
    });

    testWidgets('PluginReviewDialog blocks installation on critical risk', (
      tester,
    ) async {
      const detail = PluginDetail(
        summary: PluginSummary(
          id: 99,
          slug: 'evil-plugin',
          name: 'Plugin Suspeito',
          version: '1.0.0',
          status: 'quarantine',
        ),
        package: PluginPackage(manifest: <String, dynamic>{}, script: ''),
        review: ReviewReport(
          risk: 'critical',
          staticFindings: <ReviewFinding>[
            ReviewFinding(
              severity: 'critical',
              message: 'Tentativa de exfiltração',
              line: 4,
            ),
          ],
        ),
      );

      await tester.pumpWidget(
        _wrap(
          Builder(
            builder: (context) => FilledButton(
              onPressed: () =>
                  showPluginReviewDialog(context: context, detail: detail),
              child: const Text('Abrir revisão'),
            ),
          ),
        ),
      );

      await tester.tap(find.text('Abrir revisão'));
      await tester.pumpAndSettle();

      expect(find.textContaining('BLOQUEADO'), findsOneWidget);
      final installBtn = tester.widget<FilledButton>(
        find.widgetWithText(FilledButton, 'Instalar como rascunho'),
      );
      expect(installBtn.onPressed, isNull);
    });

    testWidgets(
      'VpnScriptViewer shows one-time key warning and hides script when consumed',
      (tester) async {
        const consumed = VpnArtifact(
          peerId: 10,
          profile: 'android',
          label: 'Android',
          delivery: 'qr',
          fileName: 'wg0.conf',
          language: 'ini',
          content: '[Interface]\nPrivateKey = CHAVE-PRIVADA-INDISPONIVEL',
          supportsQrCode: true,
          summary: <VpnArtifactSummaryItem>[
            VpnArtifactSummaryItem(label: 'IP VPN', value: '10.8.0.2/32'),
          ],
        );

        await tester.pumpWidget(
          _wrap(
            Consumer(
              builder: (context, ref, _) => FilledButton(
                onPressed: () => showVpnScriptViewer(
                  context: context,
                  ref: ref,
                  artifact: consumed,
                ),
                child: const Text('Abrir cofre'),
              ),
            ),
          ),
        );

        await tester.tap(find.text('Abrir cofre'));
        await tester.pumpAndSettle();

        expect(find.text('Chave privada já entregue'), findsOneWidget);
        expect(find.text('Gerar novas chaves'), findsOneWidget);
        expect(find.text('Copiar tudo'), findsNothing);
      },
    );
  });

  group('Mobile 390x844 zero-overflow UX audit (VPN, AI, Plugins)', () {
    testWidgets(
      'VpnServerPage and VpnDevicesPage scroll cleanly at 390x844 with zero overflow',
      (tester) async {
        _setMobileViewport(tester);
        final overrides = <Override>[
          authProvider.overrideWith(_AdminAuthStub.new),
          vpnProvider.overrideWith(_SeededVpnNotifier.new),
        ];

        await tester.pumpWidget(
          _wrap(
            const Padding(padding: EdgeInsets.all(12), child: VpnServerPage()),
            overrides: overrides,
          ),
        );
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        expect(find.byType(CustomScrollView), findsOneWidget);
        expect(find.byType(ResponsiveFieldRow), findsOneWidget);

        await tester.pumpWidget(
          _wrap(
            const Padding(padding: EdgeInsets.all(12), child: VpnDevicesPage()),
            overrides: overrides,
          ),
        );
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        expect(find.byType(UnifiedFilterBar), findsOneWidget);
      },
    );

    testWidgets(
      'AiChatPage, DeciderBenchmarkPage, AiSettingsCard and DeciderSettingsCard render cleanly at 390x844',
      (tester) async {
        _setMobileViewport(tester);
        final overrides = <Override>[
          authProvider.overrideWith(_AdminAuthStub.new),
          aiSettingsProvider.overrideWith(_SeededAiSettingsNotifier.new),
          aiConversationsProvider.overrideWith(
            _SeededAiConversationsNotifier.new,
          ),
          aiDeciderProvider.overrideWith(_SeededAiDeciderNotifier.new),
          aiBenchmarkProvider.overrideWith(_SeededAiBenchmarkNotifier.new),
        ];

        await tester.pumpWidget(
          _wrap(const AiChatPage(), overrides: overrides),
        );
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        expect(find.byTooltip('Mais opções'), findsOneWidget);

        await tester.pumpWidget(
          _wrap(
            const Padding(
              padding: EdgeInsets.all(12),
              child: DeciderBenchmarkPage(),
            ),
            overrides: overrides,
          ),
        );
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);

        await tester.pumpWidget(
          _wrap(
            const SingleChildScrollView(
              padding: EdgeInsets.all(12),
              child: Column(
                children: <Widget>[
                  AiSettingsCard(),
                  SizedBox(height: 12),
                  DeciderSettingsCard(),
                ],
              ),
            ),
            overrides: overrides,
          ),
        );
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        expect(find.byType(ResponsiveFieldRow), findsWidgets);
      },
    );

    testWidgets(
      'PluginsPage and PluginAppPage preserve plugin teal tabs and render at 390x844 with zero overflow',
      (tester) async {
        _setMobileViewport(tester);
        final overrides = <Override>[
          authProvider.overrideWith(_AdminAuthStub.new),
          pluginsProvider.overrideWith(_SeededPluginsNotifier.new),
          pluginAppsProvider.overrideWith(_SeededPluginAppsNotifier.new),
        ];

        await tester.pumpWidget(
          _wrap(
            const Padding(padding: EdgeInsets.all(12), child: PluginsPage()),
            overrides: overrides,
          ),
        );
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        expect(find.byType(UnifiedFilterBar), findsOneWidget);

        await tester.pumpWidget(
          _wrap(
            const Padding(
              padding: EdgeInsets.all(12),
              child: PluginAppPage(pluginId: 7),
            ),
            overrides: overrides,
          ),
        );
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        expect(find.byType(NestedScrollView), findsOneWidget);
        final tabBar = tester.widget<TabBar>(find.byType(TabBar));
        expect(tabBar.isScrollable, isTrue);
        expect(tabBar.tabAlignment, TabAlignment.start);
        expect(tabBar.indicatorColor, AppSemanticColors.light.plugin);
      },
    );

    testWidgets(
      'DeviceAccessDialog switches cleanly between dedicated SSH key and 30-min session at 390x844 with zero overflow',
      (tester) async {
        _setMobileViewport(tester);
        await tester.pumpWidget(
          _wrap(
            Consumer(
              builder: (context, ref, _) => FilledButton(
                onPressed: () => showDeviceAccessDialog(
                  context: context,
                  ref: ref,
                  deviceId: 5,
                ),
                child: const Text('Abrir acesso'),
              ),
            ),
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthStub.new),
              deviceAccessProvider.overrideWith(
                () => _StubDeviceAccessController(platform: 'openwrt'),
              ),
            ],
          ),
        );

        await tester.tap(find.text('Abrir acesso'));
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(find.text('Chave SSH dedicada (openwrt)'), findsOneWidget);
        expect(find.text('Usuário e senha'), findsOneWidget);
        expect(find.text('Cofre'), findsNothing);
        expect(find.text('Provisionar chave SSH'), findsOneWidget);

        await tester.tap(find.text('Usuário e senha'));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        expect(find.text('Salvar credencial'), findsOneWidget);
      },
    );

    testWidgets(
      'DeciderSettingsCard stacks chip and switch cleanly without crushing title at 360x800',
      (tester) async {
        tester.view.physicalSize = const Size(360, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(() {
          tester.view.resetPhysicalSize();
          tester.view.resetDevicePixelRatio();
        });

        await tester.pumpWidget(
          _wrap(
            const SingleChildScrollView(
              padding: EdgeInsets.all(12),
              child: DeciderSettingsCard(),
            ),
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthStub.new),
              aiDeciderProvider.overrideWith(_SeededAiDeciderNotifier.new),
            ],
          ),
        );
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        final titleFinder = find.text('Decisor Rápido (Ollaya / Roteador)');
        expect(titleFinder, findsOneWidget);
        expect(find.text('Modelo não baixado'), findsOneWidget);
        expect(find.byType(Switch), findsWidgets);

        // Title must get full horizontal room (not vertically crushed beside chip+switch)
        final titleSize = tester.getSize(titleFinder);
        expect(titleSize.width, greaterThanOrEqualTo(200));

        // Stacked chip + switch sit cleanly on the same row below the title
        final titleRect = tester.getRect(titleFinder);
        final chipRect = tester.getRect(find.text('Modelo não baixado'));
        final headerSwitchRect = tester.getRect(find.byType(Switch).first);
        expect(chipRect.top, greaterThanOrEqualTo(titleRect.bottom));
        expect(
          (chipRect.center.dy - headerSwitchRect.center.dy).abs(),
          lessThan(12),
        );
      },
    );

    testWidgets(
      'PluginAppPage (Rede Wi-Fi OpenWrt) renders compact header and 50%/50% SymmetricalActionBar across Redes, Equipamentos and Avançado at 360x800 and 390x844',
      (tester) async {
        final candidates = <FleetCandidate>[
          FleetCandidate.fromJson(const <String, dynamic>{
            'deviceId': 11,
            'name': 'AP assistencia',
            'ip': null,
            'compat': 'possible',
            'reasons': <String>[
              "· o firmware ainda não foi lido — rode 'Detectar'",
            ],
          }),
          FleetCandidate.fromJson(const <String, dynamic>{
            'deviceId': 12,
            'name': 'Borda',
            'ip': '192.168.1.1',
            'compat': 'possible',
            'reasons': <String>[
              "o firmware ainda não foi lido — rode 'Detectar'",
            ],
          }),
        ];

        for (final viewport in const <Size>[Size(360, 800), Size(390, 844)]) {
          tester.view.physicalSize = viewport;
          tester.view.devicePixelRatio = 1.0;

          final overrides = <Override>[
            authProvider.overrideWith(_AdminAuthStub.new),
            pluginsProvider.overrideWith(_SeededPluginsNotifier.new),
            pluginAppsProvider.overrideWith(
              () => _OpenWrtFleetAppsNotifier(candidates: candidates),
            ),
          ];

          await tester.pumpWidget(
            _wrap(
              const Padding(
                padding: EdgeInsets.all(12),
                child: PluginAppPage(pluginId: 9),
              ),
              overrides: overrides,
            ),
          );
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);

          // Compact header holds title + equipment count badge without overflow
          expect(find.text('Rede Wi-Fi OpenWrt'), findsOneWidget);
          expect(find.text('0 equipamentos'), findsOneWidget);

          // Redes tab: QR Wi-Fi + Editar sit 50% / 50% side-by-side
          expect(find.text('Corporativo-5G'), findsOneWidget);
          final qrBtn = find.widgetWithText(OutlinedButton, 'QR Wi-Fi');
          final editBtn = find.ancestor(
            of: find.text('Editar'),
            matching: find.bySubtype<ButtonStyleButton>(),
          );
          expect(qrBtn, findsOneWidget);
          expect(editBtn, findsOneWidget);
          final qrRect = tester.getRect(qrBtn);
          final editRect = tester.getRect(editBtn);
          expect((qrRect.center.dy - editRect.center.dy).abs(), lessThan(2));
          expect((qrRect.width - editRect.width).abs(), lessThan(2));

          // Equipamentos tab: Adicionar (2) + Verificar (2) sit 50% / 50% on 1 symmetrical row
          await tester.tap(find.text('Equipamentos (0)'));
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);

          final addBtn = find.ancestor(
            of: find.text('Adicionar (2)'),
            matching: find.bySubtype<ButtonStyleButton>(),
          );
          final verifyBtn = find.ancestor(
            of: find.text('Verificar (2)'),
            matching: find.bySubtype<ButtonStyleButton>(),
          );
          expect(addBtn, findsOneWidget);
          expect(verifyBtn, findsOneWidget);

          final addRect = tester.getRect(addBtn);
          final verifyRect = tester.getRect(verifyBtn);
          expect((addRect.center.dy - verifyRect.center.dy).abs(), lessThan(2));
          expect((addRect.width - verifyRect.width).abs(), lessThan(2));

          // Avançado tab renders cleanly with SymmetricalActionBar
          final advancedTab = find.text('Avançado');
          await tester.ensureVisible(advancedTab);
          await tester.pumpAndSettle();
          await tester.tap(advancedTab);
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          expect(find.text('Planejar canais'), findsOneWidget);
          expect(find.text('Executar'), findsOneWidget);

          await tester.pumpWidget(const SizedBox.shrink());
        }

        tester.view.resetPhysicalSize();
        tester.view.resetDevicePixelRatio();
      },
    );

    testWidgets(
      'Adicionar equipamento à frota sheet renders bordered cards, zero leading "· " bug, symmetrical buttons and >5 search filter at 360x800',
      (tester) async {
        tester.view.physicalSize = const Size(360, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(() {
          tester.view.resetPhysicalSize();
          tester.view.resetDevicePixelRatio();
        });

        final sixCandidates = <FleetCandidate>[
          FleetCandidate.fromJson(const <String, dynamic>{
            'deviceId': 11,
            'name': 'AP assistencia',
            'ip': null,
            'compat': 'possible',
            'reasons': <String>[
              "· o firmware ainda não foi lido — rode 'Detectar'",
            ],
          }),
          FleetCandidate.fromJson(const <String, dynamic>{
            'deviceId': 12,
            'name': 'Borda',
            'ip': '10.0.0.1',
            'compat': 'likely',
            'reasons': <String>[],
          }),
          FleetCandidate.fromJson(const <String, dynamic>{
            'deviceId': 13,
            'name': 'Switch Core',
            'ip': '10.0.0.2',
            'compat': 'incompatible',
            'reasons': <String>['· Plataforma RouterOS incompatível'],
          }),
          for (var i = 4; i <= 6; i++)
            FleetCandidate(
              deviceId: 20 + i,
              deviceName: 'AP Andar $i',
              ipAddress: '10.0.0.${20 + i}',
              compatibility: 'likely',
            ),
        ];

        await tester.pumpWidget(
          _wrap(
            Builder(
              builder: (context) => FilledButton(
                onPressed: () =>
                    showPluginFleetAddSheet(context: context, pluginId: 9),
                child: const Text('Abrir candidatos'),
              ),
            ),
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthStub.new),
              pluginAppsProvider.overrideWith(
                () => _OpenWrtFleetAppsNotifier(candidates: sixCandidates),
              ),
            ],
          ),
        );

        await tester.tap(find.text('Abrir candidatos'));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);

        // Quick search filter appears when > 5 candidates
        expect(find.byType(TextField), findsOneWidget);
        expect(find.text('AP assistencia'), findsOneWidget);
        expect(find.text('Talvez compatível'), findsOneWidget);
        expect(
          find.text(
            'Firmware ainda não identificado — toque em Detectar sistema.',
          ),
          findsOneWidget,
        );

        // Zero orphan leading "· " in any rendered Text widget
        for (final textWidget in tester.widgetList<Text>(find.byType(Text))) {
          final data = textWidget.data ?? '';
          expect(
            data.trimLeft().startsWith('·'),
            isFalse,
            reason: 'Found orphan leading middle-dot in "$data"',
          );
        }

        // First candidate ('possible') has 50%/50% side-by-side Detectar sistema + Adicionar à frota
        final detectBtn = find
            .ancestor(
              of: find.text('Detectar sistema'),
              matching: find.bySubtype<ButtonStyleButton>(),
            )
            .first;
        final addFleetBtn = find
            .ancestor(
              of: find.text('Adicionar à frota'),
              matching: find.bySubtype<ButtonStyleButton>(),
            )
            .first;
        final detectRect = tester.getRect(detectBtn);
        final addFleetRect = tester.getRect(addFleetBtn);
        expect(
          (detectRect.center.dy - addFleetRect.center.dy).abs(),
          lessThan(2),
        );
        expect((detectRect.width - addFleetRect.width).abs(), lessThan(2));

        // Search filter narrows candidates cleanly
        await tester.enterText(find.byType(TextField), 'Core');
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        expect(find.text('AP assistencia'), findsNothing);
        expect(find.text('Switch Core'), findsOneWidget);
        expect(find.text('Incompatível'), findsOneWidget);
        expect(find.text('Plataforma RouterOS incompatível'), findsOneWidget);
        expect(find.text('Adicionar à frota'), findsNothing);
      },
    );

    testWidgets(
      'DeviceAccessDialog defaults OpenWrt username to root and shows AlertDialog on HTTP 401 without closing or logging out at 360x800',
      (tester) async {
        tester.view.physicalSize = const Size(360, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(() {
          tester.view.resetPhysicalSize();
          tester.view.resetDevicePixelRatio();
        });

        final accessCtrl = _StubDeviceAccessController(
          platform: 'openwrt',
          rejectProvisionWith401: true,
        );
        final container = ProviderContainer(
          overrides: <Override>[
            authProvider.overrideWith(_AdminAuthStub.new),
            deviceAccessProvider.overrideWith(() => accessCtrl),
          ],
        );
        addTearDown(container.dispose);

        await tester.pumpWidget(
          UncontrolledProviderScope(
            container: container,
            child: MaterialApp(
              theme: AppTheme.light(),
              home: Scaffold(
                body: Consumer(
                  builder: (context, ref, _) => FilledButton(
                    onPressed: () => showDeviceAccessDialog(
                      context: context,
                      ref: ref,
                      deviceId: 3,
                    ),
                    child: const Text('Configurar acesso SSH'),
                  ),
                ),
              ),
            ),
          ),
        );

        await tester.tap(find.text('Configurar acesso SSH'));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);

        // OpenWrt dedicated SSH defaults username to 'root' (never 'admin')
        final userField = tester.widget<TextField>(
          find.widgetWithText(TextField, 'Usuário admin atual'),
        );
        expect(userField.controller?.text, 'root');

        // Enter wrong password and submit -> triggers 401 from device endpoint
        await tester.enterText(
          find.widgetWithText(TextField, 'Senha administrativa (uso único)'),
          'senha-errada',
        );
        final provisionBtn = find.text('Provisionar chave SSH');
        await tester.ensureVisible(provisionBtn);
        await tester.tap(provisionBtn);
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(accessCtrl.provisionAttempts, 1);

        // Explicit AlertDialog explains the credential rejection + OpenWrt 'root' hint
        expect(find.byType(AlertDialog), findsOneWidget);
        expect(find.text('Usuário ou senha incorretos'), findsOneWidget);
        expect(
          find.descendant(
            of: find.byType(AlertDialog),
            matching: find.textContaining(
              "No OpenWrt o usuário padrão é 'root'.",
            ),
          ),
          findsOneWidget,
        );
        expect(find.text('Entendi'), findsOneWidget);

        // Dismiss AlertDialog -> DeviceAccessDialog stays open AND user remains logged in!
        await tester.tap(find.text('Entendi'));
        await tester.pumpAndSettle();

        expect(find.byType(AlertDialog), findsNothing);
        expect(find.text('Acesso ao equipamento'), findsOneWidget);
        expect(find.text('Provisionar chave SSH'), findsOneWidget);
        expect(container.read(authProvider).isAuthenticated, isTrue);
        expect(container.read(authProvider).token, 'test-token');

        // Close sheet and verify MikroTik platform defaults to 'admin' at 390x844
        await tester.tap(find.byTooltip('Fechar'));
        await tester.pumpAndSettle();

        tester.view.physicalSize = const Size(390, 844);
        await tester.pumpWidget(
          _wrap(
            Consumer(
              builder: (context, ref, _) => FilledButton(
                onPressed: () => showDeviceAccessDialog(
                  context: context,
                  ref: ref,
                  deviceId: 8,
                ),
                child: const Text('Abrir MikroTik'),
              ),
            ),
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthStub.new),
              deviceAccessProvider.overrideWith(
                () => _StubDeviceAccessController(platform: 'mikrotik'),
              ),
            ],
          ),
        );
        await tester.tap(find.text('Abrir MikroTik'));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        final mikrotikUserField = tester.widget<TextField>(
          find.widgetWithText(TextField, 'Usuário admin atual'),
        );
        expect(mikrotikUserField.controller?.text, 'admin');
      },
    );

    testWidgets(
      'DevicePluginsTab renders symmetrical action bars and credential cards cleanly at 360x800',
      (tester) async {
        tester.view.physicalSize = const Size(360, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(() {
          tester.view.resetPhysicalSize();
          tester.view.resetDevicePixelRatio();
        });

        await tester.pumpWidget(
          _wrap(
            const Padding(
              padding: EdgeInsets.all(12),
              child: DevicePluginsTab(deviceId: 5),
            ),
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthStub.new),
              pluginsProvider.overrideWith(_SeededDevicePluginsNotifier.new),
            ],
          ),
        );
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);

        // PopupMenuButton stays inline on the right of SectionCard header (not stacked below)
        final cardTitleRect = tester.getRect(
          find.descendant(
            of: find.byType(SectionCard),
            matching: find.text('Rede Wi-Fi OpenWrt'),
          ),
        );
        final menuBtnRect = tester.getRect(find.byTooltip('Opções do plugin'));
        expect(menuBtnRect.left, greaterThan(cardTitleRect.right - 8));
        expect(
          (menuBtnRect.center.dy - cardTitleRect.center.dy).abs(),
          lessThan(24),
        );

        // humanizePluginFieldKey translates technical & camelCase keys cleanly
        expect(humanizePluginFieldKey('band'), 'Banda');
        expect(humanizePluginFieldKey('encryption'), 'Segurança');
        expect(humanizePluginFieldKey('clients'), 'Clientes');
        expect(humanizePluginFieldKey('redesAtivas'), 'Redes ativas');

        // Installed plugin actions sit 50% / 50% side-by-side
        final readRect = tester.getRect(
          find.ancestor(
            of: find.text('Ler rádios'),
            matching: find.bySubtype<ButtonStyleButton>(),
          ),
        );
        final writeRect = tester.getRect(
          find.ancestor(
            of: find.text('Aplicar Wi-Fi'),
            matching: find.bySubtype<ButtonStyleButton>(),
          ),
        );
        expect((readRect.center.dy - writeRect.center.dy).abs(), lessThan(2));
        expect((readRect.width - writeRect.width).abs(), lessThan(2));

        // Credentials chip shows bordered card with Gerenciar + Remover side-by-side
        final credChip = find.text('Credenciais (1)');
        await tester.ensureVisible(credChip);
        await tester.pumpAndSettle();
        await tester.tap(credChip);
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        expect(find.text('root (porta 22)'), findsOneWidget);
        expect(find.text('Gerenciar'), findsOneWidget);
        expect(find.text('Remover'), findsOneWidget);
      },
    );
  });
}
