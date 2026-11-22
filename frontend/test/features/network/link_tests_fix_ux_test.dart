import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/link_test_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/pages/link_tests_page.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/link_tests_provider.dart';
import 'package:netmonitor_app/features/network/providers/topology_provider.dart';
import 'package:netmonitor_app/features/network/widgets/device_access_host.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/features/plugins/widgets/device_plugins_tab.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class _AdminAuthNotifier extends AuthNotifier {
  @override
  AuthState build() => const AuthState(
    token: 'test-jwt',
    user: AuthUser(
      id: 1,
      fullName: 'Admin NOC',
      email: 'admin@netmonitor.local',
      role: UserRole.admin,
    ),
  );
}

class _StubAreasNotifier extends AreasNotifier {
  @override
  AreasState build() => const AreasState();
}

class _ScenarioDevicesNotifier extends DevicesNotifier {
  _ScenarioDevicesNotifier(this._devices);

  final List<DeviceModel> _devices;
  int fetchCount = 0;

  @override
  DevicesState build() => DevicesState(items: _devices);

  @override
  Future<void> fetchDevices({int? siteId, int? networkId}) async {
    fetchCount++;
  }
}

class _ScenarioTopologyNotifier extends TopologyNotifier {
  _ScenarioTopologyNotifier({this.initialEdges = const <TopologyEdgeModel>[]});

  final List<TopologyEdgeModel> initialEdges;

  @override
  TopologyState build() => TopologyState(edges: initialEdges);

  @override
  Future<void> fetchTopology({
    int? siteId,
    bool updateSiteFilter = false,
    bool showLoading = true,
    bool preserveUnsavedPositions = true,
  }) async {}
}

class _ScenarioLinkTestsNotifier extends LinkTestsNotifier {
  _ScenarioLinkTestsNotifier({
    required this.onPlan,
    this.onInstall,
    this.onWaitForRun,
  });

  final Future<LinkTestPlanModel> Function(Map<String, dynamic> req) onPlan;
  final Future<void> Function(int deviceId, int pluginId)? onInstall;
  final Future<LinkTestPluginRunResult> Function(int runId)? onWaitForRun;

  final List<Map<String, dynamic>> planRequests = <Map<String, dynamic>>[];
  final List<Map<String, dynamic>> startRequests = <Map<String, dynamic>>[];
  final List<(int, int)> installedCalls = <(int, int)>[];
  final List<Map<String, dynamic>> runActionCalls = <Map<String, dynamic>>[];

  @override
  LinkTestsState build() => const LinkTestsState();

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

  @override
  Future<void> installDevicePlugin(int deviceId, int pluginId) async {
    installedCalls.add((deviceId, pluginId));
    if (onInstall != null) {
      await onInstall!(deviceId, pluginId);
    }
  }

  @override
  Future<int> runDevicePluginAction({
    required int deviceId,
    required int pluginId,
    required String action,
    Map<String, dynamic> params = const <String, dynamic>{},
    bool confirmWrite = false,
  }) async {
    runActionCalls.add(<String, dynamic>{
      'deviceId': deviceId,
      'pluginId': pluginId,
      'action': action,
      'params': Map<String, dynamic>.from(params),
      'confirmWrite': confirmWrite,
    });
    return 501;
  }

  @override
  Future<LinkTestPluginRunResult> waitForPluginRun(int runId) async {
    if (onWaitForRun != null) {
      return onWaitForRun!(runId);
    }
    return LinkTestPluginRunResult(id: runId, status: 'succeeded');
  }

  @override
  Future<LinkTestPlanModel> plan(Map<String, dynamic> request) async {
    planRequests.add(Map<String, dynamic>.from(request));
    return onPlan(request);
  }

  @override
  Future<int> start(Map<String, dynamic> request) async {
    startRequests.add(Map<String, dynamic>.from(request));
    const id = 99;
    final started = LinkTestModel(
      id: id,
      kind: (request['kind'] as String?) ?? 'server',
      status: 'running',
      origin: 'user',
      spec: LinkTestSpecModel.fromJson(asJsonMap(request['spec'])),
      legs: const <LinkTestLegModel>[
        LinkTestLegModel(
          seq: 1,
          kind: 'end_to_end',
          status: 'running',
          durationS: 10,
          engine: 'iperf3',
          a: LinkTestEndpointModel(
            kind: 'central',
            name: 'Servidor NetMonitor',
          ),
          b: LinkTestEndpointModel(kind: 'device', deviceId: 10, name: 'Borda'),
        ),
      ],
      createdAt: '2026-11-22T16:00:00Z',
    );
    applyUpdated(<String, dynamic>{
      'id': started.id,
      'kind': started.kind,
      'status': started.status,
      'origin': started.origin,
      'spec': started.spec.toJson(),
      'legs': const <Map<String, dynamic>>[
        <String, dynamic>{
          'seq': 1,
          'kind': 'end_to_end',
          'status': 'running',
          'durationS': 10,
          'engine': 'iperf3',
          'a': <String, dynamic>{
            'kind': 'central',
            'name': 'Servidor NetMonitor',
          },
          'b': <String, dynamic>{
            'kind': 'device',
            'deviceId': 10,
            'name': 'Borda',
          },
        },
      ],
      'createdAt': started.createdAt,
    });
    focus(id);
    return id;
  }
}

class _ScenarioPluginsNotifier extends PluginsNotifier {
  _ScenarioPluginsNotifier({
    this.onInstall,
    this.initialDeviceViews = const <int, DevicePluginsView>{},
  });

  final Future<void> Function(int deviceId, int pluginId)? onInstall;
  final Map<int, DevicePluginsView> initialDeviceViews;

  final List<(int, int)> installedCalls = <(int, int)>[];

  @override
  PluginsState build() => PluginsState(deviceViews: initialDeviceViews);

  @override
  Future<void> loadDevice(int deviceId) async {}

  @override
  Future<void> installPlugin(int deviceId, int pluginId) async {
    installedCalls.add((deviceId, pluginId));
    if (onInstall != null) {
      await onInstall!(deviceId, pluginId);
    }
  }
}

class _ScenarioDeviceAccessController extends DeviceAccessController {
  _ScenarioDeviceAccessController({this.dedicatedPlatform = 'openwrt'});

  final String dedicatedPlatform;
  Object? nextProvisionError;
  Map<String, dynamic>? lastProvisionPayload;
  final List<DeviceCredentialView> _credentials = <DeviceCredentialView>[];

  @override
  Future<DeviceAccessViewModel> fetchAccess(int deviceId) async {
    return DeviceAccessViewModel(
      deviceId: deviceId,
      deviceName: 'Borda',
      dedicatedPlatform: dedicatedPlatform,
      credentials: List<DeviceCredentialView>.unmodifiable(_credentials),
    );
  }

  @override
  Future<void> provisionDedicatedKey({
    required int deviceId,
    required String platform,
    required String username,
    required String password,
    int port = 22,
    int? viaProbeId,
  }) async {
    if (nextProvisionError != null) {
      final err = nextProvisionError!;
      nextProvisionError = null;
      throw err;
    }
    lastProvisionPayload = <String, dynamic>{
      'platform': platform,
      'username': username,
      'password': password,
      'port': port,
      'viaProbeId': ?viaProbeId,
    };
    const view = DeviceCredentialView(
      kind: 'ssh',
      username: 'netmonitor',
      port: 22,
      storage: 'key',
      dedicatedFlag: true,
    );
    _credentials
      ..removeWhere((c) => c.kind == 'ssh')
      ..add(view);
  }
}

void _setViewport(WidgetTester tester, Size size) {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1.0;
  addTearDown(() {
    tester.view.resetPhysicalSize();
    tester.view.resetDevicePixelRatio();
  });
}

Widget _buildApp({required Widget child, required List<Override> overrides}) {
  return ProviderScope(
    overrides: <Override>[
      authProvider.overrideWith(_AdminAuthNotifier.new),
      areasProvider.overrideWith(_StubAreasNotifier.new),
      ...overrides,
    ],
    child: MaterialApp(
      theme: AppTheme.dark(),
      home: Scaffold(body: child),
    ),
  );
}

void main() {
  group('LinkTest domain & UX helpers', () {
    test(
      'filterLinkTestSourceDevices and filterLinkTestTargetDevices exclude Server in server mode, unmanaged switches, IP-less devices, and duplicate endpoints',
      () {
        final server = DeviceModel.fromJson(const <String, dynamic>{
          'id': 1,
          'name': 'Servidor NetMonitor',
          'type': 'server',
          'status': 'online',
          'isSystem': true,
          'ipAddress': '10.0.0.254',
        });
        final unmanaged = DeviceModel.fromJson(const <String, dynamic>{
          'id': 2,
          'name': 'Switch Mesa',
          'type': 'unmanaged_switch',
          'status': 'online',
        });
        final noIpAp = DeviceModel.fromJson(const <String, dynamic>{
          'id': 3,
          'name': 'AP Sem IP',
          'type': 'access_point',
          'status': 'unknown',
        });
        final borda = DeviceModel.fromJson(const <String, dynamic>{
          'id': 10,
          'name': 'Borda',
          'type': 'router',
          'status': 'online',
          'ipAddress': '10.0.0.1',
        });
        final apAssistencia = DeviceModel.fromJson(const <String, dynamic>{
          'id': 20,
          'name': 'AP assistencia',
          'type': 'access_point',
          'status': 'online',
          'ipAddress': '10.0.0.2',
        });

        final all = <DeviceModel>[
          server,
          unmanaged,
          noIpAp,
          borda,
          apAssistencia,
        ];

        final serverTargets = filterLinkTestTargetDevices(all, kind: 'server');
        expect(serverTargets.map((d) => d.id), <int>[10, 20]);

        final e2eSources = filterLinkTestSourceDevices(all);
        expect(e2eSources.map((d) => d.id), <int>[1, 10, 20]);

        final e2eTargets = filterLinkTestTargetDevices(
          all,
          kind: 'end_to_end',
          excludeSourceId: 10,
        );
        expect(e2eTargets.map((d) => d.id), <int>[1, 20]);
      },
    );

    test(
      'humanizeLinkTestText and LinkTestFixModel.actionButtonLabel replace jargon and arrows cleanly',
      () {
        expect(
          humanizeLinkTestText('Perna 1 (Borda ↔ AP): 2 pernas → pulada'),
          'Trecho 1 (Borda <-> AP): 2 trechos -> pulada',
        );

        const fixPlugin = LinkTestFixModel(
          kind: 'install_plugin',
          deviceId: 10,
          deviceName: 'Borda',
          pluginId: 5,
          message: 'Borda precisa do plugin «OpenWrt – teste de enlace»',
        );
        const fixCred = LinkTestFixModel(
          kind: 'add_credential',
          deviceId: 10,
          deviceName: 'Borda',
          transport: 'ssh',
          message: 'Borda está sem acesso SSH',
        );
        const fixPkg = LinkTestFixModel(
          kind: 'install_package',
          deviceId: 10,
          deviceName: 'Borda',
          pluginId: 5,
          action: 'tp_prepare',
          engine: 'iperf3',
          role: 'serve',
          packages: <String>['iperf3'],
          message: 'Borda não tem o iperf3',
        );

        expect(fixPlugin.actionButtonLabel, 'Ativar plugin');
        expect(fixCred.actionButtonLabel, 'Configurar acesso');
        expect(fixPkg.actionButtonLabel, 'Instalar iperf3');
      },
    );
  });

  group('LinkTestsPage end-to-end wizard & 3 LinkTestFix variants', () {
    testWidgets(
      'Auto-selects single valid target, hides ~0s/perna jargon, resolves install_plugin -> add_credential (with 401 dialog) -> install_package (error + success), and starts test',
      (tester) async {
        _setViewport(tester, const Size(390, 844));

        final devices = <DeviceModel>[
          DeviceModel.fromJson(const <String, dynamic>{
            'id': 1,
            'name': 'Servidor NetMonitor',
            'type': 'server',
            'status': 'online',
            'isSystem': true,
            'ipAddress': '10.0.0.254',
          }),
          DeviceModel.fromJson(const <String, dynamic>{
            'id': 2,
            'name': 'Switch Nao Gerenciavel',
            'type': 'unmanaged_switch',
            'status': 'online',
          }),
          DeviceModel.fromJson(const <String, dynamic>{
            'id': 10,
            'name': 'Borda',
            'type': 'router',
            'status': 'online',
            'ipAddress': '10.0.0.1',
            'operatingSystem': 'openwrt',
          }),
        ];

        var pluginInstalled = false;
        var sshConfigured = false;
        var iperfInstalled = false;
        var failNextPackageInstall = true;
        final installCompleter = Completer<void>();

        final accessCtrl = _ScenarioDeviceAccessController(
          dedicatedPlatform: 'openwrt',
        );

        final linkTestsNotifier = _ScenarioLinkTestsNotifier(
          onInstall: (deviceId, pluginId) async {
            await installCompleter.future;
            pluginInstalled = true;
          },
          onWaitForRun: (runId) async {
            if (failNextPackageInstall) {
              failNextPackageInstall = false;
              return LinkTestPluginRunResult(
                id: runId,
                status: 'failed',
                error: 'Falha no opkg update (repositório inacessível).',
              );
            }
            iperfInstalled = true;
            return LinkTestPluginRunResult(id: runId, status: 'succeeded');
          },
          onPlan: (req) async {
            if (!pluginInstalled) {
              return LinkTestPlanModel.fromJson(const <String, dynamic>{
                'kind': 'server',
                'estimatedSeconds': 0,
                'legs': <Map<String, dynamic>>[
                  <String, dynamic>{
                    'seq': 1,
                    'kind': 'end_to_end',
                    'durationS': 10,
                    'runnable': false,
                    'a': <String, dynamic>{
                      'kind': 'central',
                      'name': 'Servidor NetMonitor',
                    },
                    'b': <String, dynamic>{
                      'kind': 'device',
                      'deviceId': 10,
                      'name': 'Borda',
                    },
                  },
                ],
                'fixes': <Map<String, dynamic>>[
                  <String, dynamic>{
                    'kind': 'install_plugin',
                    'deviceId': 10,
                    'deviceName': 'Borda',
                    'pluginId': 5,
                    'pluginSlug': 'openwrt-link-test',
                    'pluginName': 'OpenWrt – teste de enlace',
                    'message':
                        'Borda precisa do plugin «OpenWrt – teste de enlace»',
                  },
                ],
              });
            }
            if (!sshConfigured) {
              return LinkTestPlanModel.fromJson(const <String, dynamic>{
                'kind': 'server',
                'estimatedSeconds': 0,
                'legs': <Map<String, dynamic>>[
                  <String, dynamic>{
                    'seq': 1,
                    'kind': 'end_to_end',
                    'durationS': 10,
                    'runnable': false,
                    'a': <String, dynamic>{
                      'kind': 'central',
                      'name': 'Servidor NetMonitor',
                    },
                    'b': <String, dynamic>{
                      'kind': 'device',
                      'deviceId': 10,
                      'name': 'Borda',
                    },
                  },
                ],
                'fixes': <Map<String, dynamic>>[
                  <String, dynamic>{
                    'kind': 'add_credential',
                    'deviceId': 10,
                    'deviceName': 'Borda',
                    'transport': 'ssh',
                    'message': 'Borda está sem acesso SSH',
                  },
                ],
              });
            }
            if (!iperfInstalled) {
              return LinkTestPlanModel.fromJson(const <String, dynamic>{
                'kind': 'server',
                'estimatedSeconds': 0,
                'legs': <Map<String, dynamic>>[
                  <String, dynamic>{
                    'seq': 1,
                    'kind': 'end_to_end',
                    'durationS': 10,
                    'runnable': false,
                    'engine': 'iperf3',
                    'a': <String, dynamic>{
                      'kind': 'central',
                      'name': 'Servidor NetMonitor',
                    },
                    'b': <String, dynamic>{
                      'kind': 'device',
                      'deviceId': 10,
                      'name': 'Borda',
                    },
                  },
                ],
                'fixes': <Map<String, dynamic>>[
                  <String, dynamic>{
                    'kind': 'install_package',
                    'deviceId': 10,
                    'deviceName': 'Borda',
                    'pluginId': 5,
                    'pluginSlug': 'openwrt-link-test',
                    'action': 'tp_prepare',
                    'engine': 'iperf3',
                    'role': 'serve',
                    'packages': <String>['iperf3'],
                    'message': 'Borda não tem o iperf3',
                  },
                ],
              });
            }
            return LinkTestPlanModel.fromJson(const <String, dynamic>{
              'kind': 'server',
              'estimatedSeconds': 18,
              'legs': <Map<String, dynamic>>[
                <String, dynamic>{
                  'seq': 1,
                  'kind': 'end_to_end',
                  'durationS': 10,
                  'runnable': true,
                  'engine': 'iperf3',
                  'a': <String, dynamic>{
                    'kind': 'central',
                    'name': 'Servidor NetMonitor',
                  },
                  'b': <String, dynamic>{
                    'kind': 'device',
                    'deviceId': 10,
                    'name': 'Borda',
                  },
                },
              ],
            });
          },
        );

        final devicesNotifier = _ScenarioDevicesNotifier(devices);

        await tester.pumpWidget(
          _buildApp(
            child: const LinkTestsPage(),
            overrides: <Override>[
              devicesProvider.overrideWith(() => devicesNotifier),
              topologyProvider.overrideWith(_ScenarioTopologyNotifier.new),
              linkTestsProvider.overrideWith(() => linkTestsNotifier),
              deviceAccessProvider.overrideWith(() => accessCtrl),
            ],
          ),
        );
        await tester.pumpAndSettle();

        // Abre "Novo"
        await tester.tap(find.text('Novo'));
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);

        // 1) Auto-selecionou Borda (único destino válido) e disparou plan() automaticamente!
        expect(linkTestsNotifier.planRequests.length, 1);
        expect(linkTestsNotifier.planRequests.single['targetDeviceId'], 10);
        expect(find.text('Planejar (Dry-Run)'), findsNothing);
        expect(find.textContaining('~0s'), findsNothing);
        expect(find.textContaining('perna'), findsNothing);
        expect(
          find.text('Preparação necessária antes do teste'),
          findsOneWidget,
        );
        expect(find.text('1 pendente(s)'), findsOneWidget);
        expect(find.text('Ativar plugin'), findsOneWidget);
        expect(find.text('Iniciar Teste'), findsNothing);

        // 2) Clica em "Ativar plugin": exibe estado de carregamento "Resolvendo..." e chama installPlugin(10, 5)
        await tester.ensureVisible(find.text('Ativar plugin'));
        await tester.tap(find.text('Ativar plugin'));
        await tester.pump();
        expect(find.text('Resolvendo...'), findsOneWidget);

        installCompleter.complete();
        await tester.pumpAndSettle();

        expect(linkTestsNotifier.installedCalls, <(int, int)>[(10, 5)]);
        expect(
          find.textContaining(
            'Plugin ativado com sucesso em Borda! Próximo passo: Borda está sem acesso SSH',
          ),
          findsOneWidget,
        );
        expect(find.text('Configurar acesso'), findsOneWidget);

        // 3) Clica em "Configurar acesso": abre DeviceAccessPanel por cima do modal com 'root'
        await tester.ensureVisible(find.text('Configurar acesso'));
        await tester.tap(find.text('Configurar acesso'));
        await tester.pumpAndSettle();

        expect(find.byType(DeviceAccessPanel), findsOneWidget);
        final userField = tester.widget<TextField>(
          find.widgetWithText(TextField, 'Usuário admin atual'),
        );
        expect(userField.controller?.text, 'root');

        // Testa erro 401 SSH -> exibe AlertDialog sem fechar modal nem deslogar!
        accessCtrl.nextProvisionError = const ApiException(
          message: 'Usuário ou senha recusados pelo equipamento.',
          statusCode: 401,
        );
        await tester.enterText(
          find.widgetWithText(TextField, 'Senha administrativa (uso único)'),
          'errada',
        );
        await tester.tap(find.text('Provisionar chave SSH'));
        await tester.pumpAndSettle();

        expect(find.text('Usuário ou senha incorretos'), findsOneWidget);
        await tester.tap(find.text('Entendi'));
        await tester.pumpAndSettle();

        // Agora provisiona com sucesso
        sshConfigured = true;
        await tester.enterText(
          find.widgetWithText(TextField, 'Senha administrativa (uso único)'),
          'correta123',
        );
        await tester.ensureVisible(find.text('Provisionar chave SSH'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Provisionar chave SSH'));
        await tester.pumpAndSettle();

        expect(find.byType(DeviceAccessPanel), findsNothing);
        expect(
          find.textContaining(
            'Acesso configurado em Borda! Próximo passo: Borda não tem o iperf3',
          ),
          findsOneWidget,
        );
        expect(find.text('Instalar iperf3'), findsOneWidget);

        // 4a) Clica em "Instalar iperf3" -> 1ª tentativa falha e exibe erro inline claro!
        await tester.ensureVisible(find.text('Instalar iperf3'));
        await tester.tap(find.text('Instalar iperf3'));
        await tester.pumpAndSettle();

        expect(linkTestsNotifier.runActionCalls.length, 1);
        expect(linkTestsNotifier.runActionCalls.single['action'], 'tp_prepare');
        expect(linkTestsNotifier.runActionCalls.single['confirmWrite'], isTrue);
        expect(
          linkTestsNotifier.runActionCalls.single['params'],
          <String, dynamic>{'engine': 'iperf3', 'role': 'serve'},
        );
        expect(
          find.textContaining(
            'Falha no opkg update (repositório inacessível).',
          ),
          findsOneWidget,
        );

        // 4b) 2ª tentativa sucede -> plano fica pronto (~18s) e libera "Iniciar Teste"!
        await tester.ensureVisible(find.text('Instalar iperf3'));
        await tester.tap(find.text('Instalar iperf3'));
        await tester.pumpAndSettle();

        expect(
          find.textContaining(
            'Pacote iperf3 instalado com sucesso em Borda! Pré-requisitos concluídos! O teste já pode ser iniciado.',
          ),
          findsOneWidget,
        );
        expect(find.text('Pronto para iniciar medição'), findsOneWidget);
        expect(find.text('~18s'), findsOneWidget);
        expect(
          find.text('Servidor NetMonitor -> Borda · iperf3 · 10s'),
          findsOneWidget,
        );
        expect(find.text('Iniciar Teste'), findsOneWidget);

        // 5) Clica em "Iniciar Teste": dispara POST /link-tests (confirm: true), fecha o modal e foca no teste #99!
        await tester.ensureVisible(find.text('Iniciar Teste'));
        await tester.tap(find.text('Iniciar Teste'));
        await tester.pumpAndSettle();

        expect(linkTestsNotifier.startRequests.length, 1);
        expect(linkTestsNotifier.startRequests.single['targetDeviceId'], 10);
        expect(
          find.text('Teste #99 · Servidor NetMonitor -> Borda'),
          findsOneWidget,
        );
      },
    );

    testWidgets(
      'Batch "Resolver tudo (2)" prepares multiple devices with progress and Blocking panel sanitizes jargon',
      (tester) async {
        _setViewport(tester, const Size(390, 844));

        final devices = <DeviceModel>[
          DeviceModel.fromJson(const <String, dynamic>{
            'id': 10,
            'name': 'Borda',
            'type': 'router',
            'status': 'online',
            'ipAddress': '10.0.0.1',
            'operatingSystem': 'openwrt',
          }),
          DeviceModel.fromJson(const <String, dynamic>{
            'id': 20,
            'name': 'AP assistencia',
            'type': 'access_point',
            'status': 'online',
            'ipAddress': '10.0.0.2',
            'operatingSystem': 'openwrt',
          }),
        ];

        final installedIds = <int>{};
        var showBlocking = false;
        final secondStepGate = Completer<void>();

        final linkTestsNotifier = _ScenarioLinkTestsNotifier(
          onInstall: (deviceId, pluginId) async {
            if (deviceId == 20 && !secondStepGate.isCompleted) {
              await secondStepGate.future;
            }
            installedIds.add(deviceId);
          },
          onPlan: (req) async {
            if (showBlocking) {
              return LinkTestPlanModel.fromJson(const <String, dynamic>{
                'kind': 'path',
                'estimatedSeconds': 0,
                'legs': <Map<String, dynamic>>[],
                'blocking': <String>[
                  'Perna 1 (Borda ↔ AP assistencia): sem motor em comum',
                ],
              });
            }
            final fixes = <Map<String, dynamic>>[
              if (!installedIds.contains(10))
                <String, dynamic>{
                  'kind': 'install_plugin',
                  'deviceId': 10,
                  'deviceName': 'Borda',
                  'pluginId': 5,
                  'message':
                      'Borda precisa do plugin «OpenWrt – teste de enlace»',
                },
              if (!installedIds.contains(20))
                <String, dynamic>{
                  'kind': 'install_plugin',
                  'deviceId': 20,
                  'deviceName': 'AP assistencia',
                  'pluginId': 5,
                  'message':
                      'AP assistencia precisa do plugin «OpenWrt – teste de enlace»',
                },
            ];
            return LinkTestPlanModel.fromJson(<String, dynamic>{
              'kind': 'end_to_end',
              'estimatedSeconds': fixes.isEmpty ? 24 : 0,
              'legs': <Map<String, dynamic>>[
                <String, dynamic>{
                  'seq': 1,
                  'kind': 'end_to_end',
                  'durationS': 10,
                  'runnable': fixes.isEmpty,
                  'engine': 'iperf3',
                  'a': <String, dynamic>{
                    'kind': 'device',
                    'deviceId': 10,
                    'name': 'Borda',
                  },
                  'b': <String, dynamic>{
                    'kind': 'device',
                    'deviceId': 20,
                    'name': 'AP assistencia',
                  },
                },
              ],
              'fixes': fixes,
            });
          },
        );

        await tester.pumpWidget(
          _buildApp(
            child: const LinkTestsPage(),
            overrides: <Override>[
              devicesProvider.overrideWith(
                () => _ScenarioDevicesNotifier(devices),
              ),
              topologyProvider.overrideWith(_ScenarioTopologyNotifier.new),
              linkTestsProvider.overrideWith(() => linkTestsNotifier),
              pendingLinkTestDraftProvider.overrideWith(
                (ref) => const LinkTestRequestDraft(
                  kind: 'end_to_end',
                  sourceDeviceId: 10,
                  targetDeviceId: 20,
                ),
              ),
            ],
          ),
        );
        await tester.pumpAndSettle();

        // Abriu automaticamente via pendingLinkTestDraftProvider e planejou A -> B!
        expect(find.text('2 pendente(s)'), findsOneWidget);
        expect(find.text('Resolver tudo (2)'), findsOneWidget);

        await tester.ensureVisible(find.text('Resolver tudo (2)'));
        await tester.tap(find.text('Resolver tudo (2)'));
        await tester.pump();
        await tester.pump();

        // Exibe progresso na 2ª etapa enquanto aguarda conclusão
        expect(find.text('Preparando 2 de 2: AP assistencia...'), findsWidgets);

        secondStepGate.complete();
        await tester.pumpAndSettle();

        expect(linkTestsNotifier.installedCalls, <(int, int)>[
          (10, 5),
          (20, 5),
        ]);
        expect(find.text('Pronto para iniciar medição'), findsOneWidget);
        expect(find.text('~24s'), findsOneWidget);
        expect(
          find.text('Borda -> AP assistencia · iperf3 · 10s'),
          findsOneWidget,
        );

        // Muda para aba Caminho com impedimento bloqueante -> mostra "Não é possível medir este caminho" com "Trecho 1 (Borda <-> AP assistencia)"
        showBlocking = true;
        await tester.ensureVisible(find.text('Caminho'));
        await tester.tap(find.text('Caminho'));
        await tester.pumpAndSettle();

        expect(find.text('Não é possível medir este caminho'), findsOneWidget);
        expect(
          find.text('Trecho 1 (Borda <-> AP assistencia): sem motor em comum'),
          findsOneWidget,
        );
        expect(find.textContaining('~0s'), findsNothing);
      },
    );
  });

  group('Global audit: DevicePluginsTab action error feedback', () {
    testWidgets(
      'DevicePluginsTab surfaces clear error feedback instead of uncaught exception when plugin install fails',
      (tester) async {
        _setViewport(tester, const Size(390, 844));

        final pluginsNotifier = _ScenarioPluginsNotifier(
          initialDeviceViews: <int, DevicePluginsView>{
            10: const DevicePluginsView(
              deviceId: 10,
              platform: 'openwrt',
              plugins: <DevicePluginItem>[
                DevicePluginItem(
                  plugin: PluginSummary(
                    id: 5,
                    slug: 'openwrt-link-test',
                    name: 'OpenWrt – teste de enlace',
                    version: '1.0.0',
                    status: 'active',
                  ),
                  installed: false,
                  compatibility: 'validated',
                ),
              ],
            ),
          },
          onInstall: (_, _) async {
            throw const ApiException(
              message: 'Equipamento ocupado por outra operação.',
              statusCode: 409,
            );
          },
        );

        await tester.pumpWidget(
          _buildApp(
            child: const DevicePluginsTab(deviceId: 10),
            overrides: <Override>[
              pluginsProvider.overrideWith(() => pluginsNotifier),
            ],
          ),
        );
        await tester.pumpAndSettle();

        await tester.tap(find.text('Instalar'));
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(
          find.text('Equipamento ocupado por outra operação.'),
          findsWidgets,
        );
      },
    );
  });
}
