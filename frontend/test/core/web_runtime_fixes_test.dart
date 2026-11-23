import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/core_area.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/clipboard_utils.dart';
import 'package:netmonitor_app/features/core/models/alert_models.dart';
import 'package:netmonitor_app/features/core/models/monitor_models.dart';
import 'package:netmonitor_app/features/core/presentation/alerts/alerts_page.dart';
import 'package:netmonitor_app/features/core/presentation/backup/backup_page.dart';
import 'package:netmonitor_app/features/core/presentation/monitors/monitors_page.dart';
import 'package:netmonitor_app/features/core/providers/alerts_provider.dart';
import 'package:netmonitor_app/features/core/providers/backup_provider.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/features/network/models/link_test_models.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/pages/discovery_page.dart';
import 'package:netmonitor_app/features/network/pages/link_tests_page.dart';
import 'package:netmonitor_app/features/network/pages/wifi_clients_page.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/link_tests_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/features/network/providers/topology_provider.dart';
import 'package:netmonitor_app/features/network/providers/wifi_provider.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/models/storage_models.dart';
import 'package:netmonitor_app/features/servers/pages/deploys_page.dart';
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/providers/storages_provider.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class _AdminAuthNotifier extends AuthNotifier {
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

class _FakeMonitorsNotifier extends MonitorsNotifier {
  @override
  MonitorsState build() => MonitorsState(
    monitors: <MonitorModel>[
      MonitorModel.fromJson(const <String, dynamic>{
        'id': 1,
        'name': 'Gateway Online',
        'type': 'icmp',
        'target': '10.0.0.1',
        'status': 'up',
        'isEnabled': true,
      }),
      MonitorModel.fromJson(const <String, dynamic>{
        'id': 2,
        'name': 'Switch Offline',
        'type': 'icmp',
        'target': '10.0.0.2',
        'status': 'down',
        'isEnabled': true,
      }),
    ],
  );

  @override
  Future<List<MonitorModel>> fetchMonitors({
    int? deviceId,
    String? status,
    String? type,
    bool? enabled,
  }) async => state.monitors;

  @override
  Future<void> fetchReferenceCatalogs() async {}
}

class _FakeAlertsNotifier extends AlertsNotifier {
  final List<int?> fetchedRuleDeviceIds = <int?>[];
  bool fetchedCatalog = false;

  @override
  AlertsState build() => const AlertsState();

  @override
  Future<void> fetchActiveAlerts() async {}

  @override
  Future<void> fetchRootCauseAnalysis() async {}

  @override
  Future<List<AlertRule>> fetchAlertRules({
    int? deviceId,
    int? siteId,
    int? monitorId,
    bool includeGlobal = true,
  }) async {
    fetchedRuleDeviceIds.add(deviceId);
    return const <AlertRule>[];
  }

  @override
  Future<void> fetchRuleCatalog({
    int? deviceId,
    int? siteId,
    int? monitorId,
  }) async {
    fetchedCatalog = true;
  }

  @override
  Future<void> fetchHistory({int page = 1, bool append = false}) async {}
}

class _FakeBackupNotifier extends BackupNotifier {
  @override
  BackupState build() => const BackupState();

  @override
  Future<void> fetchPlan() async {}
}

class _FakeDatabasesNotifier extends DatabasesNotifier {
  @override
  DatabasesState build() => const DatabasesState();

  @override
  Future<bool> fetchConnections() async => true;
}

class _FakeStoragesNotifier extends StoragesNotifier {
  @override
  StoragesState build() => const StoragesState();

  @override
  Future<bool> fetchStorages() async => true;

  @override
  Future<StoragesMetaResponse?> fetchMeta() async => null;
}

class _FakeSseService extends SseService {
  final List<String> acquiredInterests = <String>[];

  @override
  RealtimeState build() => const RealtimeState();

  @override
  VoidCallback acquireInterest(String name) {
    acquiredInterests.add(name);
    return () {};
  }
}

class _FakeWifiClientsNotifier extends WifiClientsNotifier {
  @override
  WifiClientsState build() => const WifiClientsState();
}

class _FakeDevicesNotifier extends DevicesNotifier {
  @override
  DevicesState build() => const DevicesState();

  @override
  Future<void> fetchDevices({int? siteId, int? networkId}) async {}
}

class _FakeTopologyNotifier extends TopologyNotifier {
  @override
  TopologyState build() => const TopologyState();

  @override
  Future<void> fetchTopology({
    int? siteId,
    bool showLoading = true,
    bool updateSiteFilter = true,
    bool preserveUnsavedPositions = false,
  }) async {}
}

class _FakeLinkTestsNotifier extends LinkTestsNotifier {
  final List<int?> fetchedDeviceIds = <int?>[];
  final List<int> fetchedDetailIds = <int>[];
  final List<int?> focusedIds = <int?>[];

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
  }) async {
    fetchedDeviceIds.add(deviceId);
  }

  @override
  Future<void> fetchSchedules() async {}

  @override
  Future<LinkTestModel?> fetchDetail(int id) async {
    fetchedDetailIds.add(id);
    return null;
  }

  @override
  void focus(int? id) {
    focusedIds.add(id);
    state = state.copyWith(focusedId: id, clearFocusedId: id == null);
  }
}

class _FakeNetworksNotifier extends NetworksNotifier {
  @override
  NetworksState build() => const NetworksState(
    items: <NetworkModel>[
      NetworkModel(id: 1, name: 'LAN Escritório', cidr: '192.168.1.0/24'),
      NetworkModel(id: 2, name: 'VLAN Servidores', cidr: '10.10.0.0/24'),
    ],
  );

  @override
  Future<void> fetchNetworks() async {}
}

class _FakeDiscoveryNotifier extends DiscoveryNotifier {
  final List<int> startedNetworkScans = <int>[];

  @override
  DiscoveryState build() => const DiscoveryState();

  @override
  Future<void> fetchDiscoveryRuns() async {}

  @override
  Future<void> fetchEnvironment() async {}

  @override
  Future<void> fetchConflicts() async {}

  @override
  Future<int?> startScan(int networkId) async {
    startedNetworkScans.add(networkId);
    return 100 + networkId;
  }
}

class _FakeDeploysNotifier extends DeploysNotifier {
  final List<int> openedRunIds = <int>[];
  final List<int> openedBuildIds = <int>[];

  @override
  DeploysState build() => DeploysState(
    overview: DeployOverview.fromJson(const <String, dynamic>{}),
  );

  @override
  Future<bool> fetchOverview() async => true;

  @override
  Future<void> fetchRecentBuilds() async {}

  @override
  Future<void> openRunConsole(int runId) async {
    openedRunIds.add(runId);
  }

  @override
  void closeRunConsole(int runId) {}

  @override
  Future<DeployChangesView> runChanges(int runId) async =>
      const DeployChangesView(commits: <DeployCommitView>[]);

  @override
  Future<List<DeployBackupView>> runBackups(int runId) async =>
      const <DeployBackupView>[];

  @override
  Future<void> openBuildConsole(int buildId) async {
    openedBuildIds.add(buildId);
  }

  @override
  void closeBuildConsole(int buildId) {}
}

Widget _wrapWithProviders(
  Widget child, {
  List<Override> overrides = const <Override>[],
}) {
  return ProviderScope(
    overrides: <Override>[
      authProvider.overrideWith(_AdminAuthNotifier.new),
      ...overrides,
    ],
    child: MaterialApp(
      theme: AppTheme.light(),
      home: Scaffold(body: child),
    ),
  );
}

File _resolveRepoFile(String relativeFromRepoRoot) {
  final fromFrontend = File('../$relativeFromRepoRoot');
  if (fromFrontend.existsSync()) return fromFrontend;
  final fromRoot = File(relativeFromRepoRoot);
  if (fromRoot.existsSync()) return fromRoot;
  throw StateError('Arquivo não encontrado: $relativeFromRepoRoot');
}

void main() {
  group('P0 #2 — HTTP LAN/VPN Clipboard fallback', () {
    tearDown(() {
      TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(SystemChannels.platform, null);
    });

    test(
      'uses native Clipboard.setData when available without fallback',
      () async {
        String? copiedText;
        var fallbackCalled = false;
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(SystemChannels.platform, (call) async {
              if (call.method == 'Clipboard.setData') {
                copiedText =
                    (call.arguments as Map<dynamic, dynamic>)['text']
                        as String?;
                return null;
              }
              return null;
            });

        final ok = await copyToClipboard(
          'wg-key-123',
          fallback: (_) {
            fallbackCalled = true;
            return true;
          },
        );

        expect(ok, isTrue);
        expect(copiedText, 'wg-key-123');
        expect(fallbackCalled, isFalse);
      },
    );

    testWidgets(
      'falls back cleanly when Clipboard.setData throws PlatformException (HTTP LAN/VPN)',
      (tester) async {
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
            .setMockMethodCallHandler(SystemChannels.platform, (call) async {
              if (call.method == 'Clipboard.setData') {
                throw PlatformException(
                  code: 'copy_fail',
                  message: 'navigator.clipboard unavailable on HTTP',
                );
              }
              return null;
            });

        String? fallbackValue;
        bool? successResult;
        bool? failureResult;

        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: Builder(
                builder: (context) => Column(
                  children: <Widget>[
                    TextButton(
                      onPressed: () async {
                        successResult = await copyToClipboardWithFeedback(
                          context,
                          'http://192.168.1.10:3333',
                          fallback: (value) {
                            fallbackValue = value;
                            return true;
                          },
                        );
                      },
                      child: const Text('Copy OK'),
                    ),
                    TextButton(
                      onPressed: () async {
                        failureResult = await copyToClipboardWithFeedback(
                          context,
                          'cannot-copy',
                          fallback: (_) => false,
                        );
                      },
                      child: const Text('Copy Fail'),
                    ),
                  ],
                ),
              ),
            ),
          ),
        );

        await tester.tap(find.text('Copy OK'));
        await tester.pumpAndSettle();

        expect(successResult, isTrue);
        expect(fallbackValue, 'http://192.168.1.10:3333');
        expect(
          find.text('Copiado para a área de transferência.'),
          findsOneWidget,
        );

        await tester.tap(find.text('Copy Fail'));
        await tester.pumpAndSettle();

        expect(failureResult, isFalse);
        expect(
          find.text('Não foi possível copiar para a área de transferência.'),
          findsOneWidget,
        );
      },
    );
  });

  group('P1 #4 — /databases redirect & didUpdateWidget deep links', () {
    testWidgets('/databases redirects to /backup?tab=databases', (
      tester,
    ) async {
      final databasesRoute = coreArea.routes.whereType<GoRoute>().firstWhere(
        (r) => r.path == '/databases',
      );
      String? resolvedLocation;
      final router = GoRouter(
        initialLocation: '/databases',
        routes: <RouteBase>[
          GoRoute(path: '/databases', redirect: databasesRoute.redirect),
          GoRoute(
            path: '/backup',
            builder: (context, state) {
              resolvedLocation = state.uri.toString();
              return const SizedBox.shrink();
            },
          ),
        ],
      );
      addTearDown(router.dispose);

      await tester.pumpWidget(MaterialApp.router(routerConfig: router));
      await tester.pumpAndSettle();

      expect(resolvedLocation, '/backup?tab=databases');
    });

    testWidgets('MonitorsPage updates status filter on didUpdateWidget', (
      tester,
    ) async {
      final statusNotifier = ValueNotifier<String?>(null);
      addTearDown(statusNotifier.dispose);

      await tester.pumpWidget(
        _wrapWithProviders(
          ValueListenableBuilder<String?>(
            valueListenable: statusNotifier,
            builder: (_, status, _) => MonitorsPage(initialStatus: status),
          ),
          overrides: <Override>[
            monitorsProvider.overrideWith(_FakeMonitorsNotifier.new),
          ],
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('Gateway Online'), findsOneWidget);
      expect(find.text('Switch Offline'), findsOneWidget);

      statusNotifier.value = 'down';
      await tester.pumpAndSettle();

      expect(find.text('Gateway Online'), findsNothing);
      expect(find.text('Switch Offline'), findsOneWidget);

      statusNotifier.value = null;
      await tester.pumpAndSettle();

      expect(find.text('Gateway Online'), findsOneWidget);
      expect(find.text('Switch Offline'), findsOneWidget);
    });

    testWidgets('BackupPage switches active tab on didUpdateWidget', (
      tester,
    ) async {
      final tabNotifier = ValueNotifier<String?>(null);
      addTearDown(tabNotifier.dispose);

      await tester.pumpWidget(
        _wrapWithProviders(
          ValueListenableBuilder<String?>(
            valueListenable: tabNotifier,
            builder: (_, tab, _) => BackupPage(initialTab: tab),
          ),
          overrides: <Override>[
            backupProvider.overrideWith(_FakeBackupNotifier.new),
            databasesProvider.overrideWith(_FakeDatabasesNotifier.new),
            storagesProvider.overrideWith(_FakeStoragesNotifier.new),
          ],
        ),
      );
      await tester.pumpAndSettle();

      TabBar tabBar() => tester.widget<TabBar>(find.byType(TabBar));
      expect(tabBar().controller!.index, 0);

      tabNotifier.value = 'databases';
      await tester.pumpAndSettle();
      expect(tabBar().controller!.index, 1);

      tabNotifier.value = 'destinos';
      await tester.pumpAndSettle();
      expect(tabBar().controller!.index, 2);

      tabNotifier.value = null;
      await tester.pumpAndSettle();
      expect(tabBar().controller!.index, 0);
    });

    testWidgets(
      'AlertsPage switches tab and opens catalog on didUpdateWidget',
      (tester) async {
        final fakeAlerts = _FakeAlertsNotifier();
        final queryNotifier = ValueNotifier<({String? tab, String? catalog})>((
          tab: 'active',
          catalog: null,
        ));
        addTearDown(queryNotifier.dispose);

        await tester.pumpWidget(
          _wrapWithProviders(
            ValueListenableBuilder<({String? tab, String? catalog})>(
              valueListenable: queryNotifier,
              builder: (_, q, _) =>
                  AlertsPage(initialTab: q.tab, catalogKey: q.catalog),
            ),
            overrides: <Override>[
              alertsProvider.overrideWith(() => fakeAlerts),
            ],
          ),
        );
        await tester.pumpAndSettle();

        TabBar tabBar() => tester.widget<TabBar>(find.byType(TabBar));
        expect(tabBar().controller!.index, 0);

        queryNotifier.value = (tab: 'rules', catalog: null);
        await tester.pumpAndSettle();
        expect(tabBar().controller!.index, 2);

        queryNotifier.value = (tab: 'rules', catalog: 'host_offline');
        await tester.pumpAndSettle();
        expect(fakeAlerts.fetchedCatalog, isTrue);
      },
    );

    testWidgets('WifiClientsPage switches to map tab on didUpdateWidget', (
      tester,
    ) async {
      final fakeSse = _FakeSseService();
      final tabNotifier = ValueNotifier<String?>(null);
      addTearDown(tabNotifier.dispose);

      await tester.pumpWidget(
        _wrapWithProviders(
          ValueListenableBuilder<String?>(
            valueListenable: tabNotifier,
            builder: (_, tab, _) => WifiClientsPage(initialTab: tab),
          ),
          overrides: <Override>[
            sseServiceProvider.overrideWith(() => fakeSse),
            wifiClientsProvider.overrideWith(_FakeWifiClientsNotifier.new),
          ],
        ),
      );
      await tester.pumpAndSettle();

      TabBar tabBar() => tester.widget<TabBar>(find.byType(TabBar));
      expect(tabBar().controller!.index, 0);

      tabNotifier.value = 'map';
      await tester.pumpAndSettle();
      expect(tabBar().controller!.index, 1);
      expect(fakeSse.acquiredInterests, contains('wifi'));
    });

    testWidgets(
      'LinkTestsPage updates tab, device filter and focused test on didUpdateWidget',
      (tester) async {
        final fakeLinkTests = _FakeLinkTestsNotifier();
        final paramsNotifier =
            ValueNotifier<({String? tab, int? testId, int? deviceId})>((
              tab: null,
              testId: null,
              deviceId: null,
            ));
        addTearDown(paramsNotifier.dispose);

        await tester.pumpWidget(
          _wrapWithProviders(
            ValueListenableBuilder<({String? tab, int? testId, int? deviceId})>(
              valueListenable: paramsNotifier,
              builder: (_, p, _) => LinkTestsPage(
                initialTab: p.tab,
                initialTestId: p.testId,
                initialDeviceId: p.deviceId,
              ),
            ),
            overrides: <Override>[
              devicesProvider.overrideWith(_FakeDevicesNotifier.new),
              topologyProvider.overrideWith(_FakeTopologyNotifier.new),
              linkTestsProvider.overrideWith(() => fakeLinkTests),
            ],
          ),
        );
        await tester.pumpAndSettle();

        TabBar tabBar() => tester.widget<TabBar>(find.byType(TabBar));
        expect(tabBar().controller!.index, 0);

        paramsNotifier.value = (tab: 'schedules', testId: 42, deviceId: 7);
        await tester.pumpAndSettle();

        expect(tabBar().controller!.index, 1);
        expect(fakeLinkTests.fetchedDeviceIds, contains(7));
        expect(fakeLinkTests.fetchedDetailIds, contains(42));
        expect(fakeLinkTests.focusedIds, contains(42));
      },
    );

    testWidgets(
      'DiscoveryPage updates network and triggers autoStartScan on didUpdateWidget',
      (tester) async {
        final fakeDiscovery = _FakeDiscoveryNotifier();
        final paramsNotifier =
            ValueNotifier<({int? networkId, bool autoStart})>((
              networkId: 1,
              autoStart: false,
            ));
        addTearDown(paramsNotifier.dispose);

        await tester.pumpWidget(
          _wrapWithProviders(
            ValueListenableBuilder<({int? networkId, bool autoStart})>(
              valueListenable: paramsNotifier,
              builder: (_, p, _) => DiscoveryPage(
                initialNetworkId: p.networkId,
                autoStartScan: p.autoStart,
              ),
            ),
            overrides: <Override>[
              networksProvider.overrideWith(_FakeNetworksNotifier.new),
              devicesProvider.overrideWith(_FakeDevicesNotifier.new),
              discoveryProvider.overrideWith(() => fakeDiscovery),
            ],
          ),
        );
        await tester.pumpAndSettle();
        expect(fakeDiscovery.startedNetworkScans, isEmpty);

        paramsNotifier.value = (networkId: 2, autoStart: true);
        await tester.pumpAndSettle();

        expect(fakeDiscovery.startedNetworkScans, <int>[2]);
      },
    );

    testWidgets(
      'DeploysPage consumes new run/build queryParams on didUpdateWidget',
      (tester) async {
        final fakeDeploys = _FakeDeploysNotifier();
        final queryNotifier = ValueNotifier<Map<String, String>>(
          const <String, String>{},
        );
        addTearDown(queryNotifier.dispose);

        await tester.pumpWidget(
          _wrapWithProviders(
            ValueListenableBuilder<Map<String, String>>(
              valueListenable: queryNotifier,
              builder: (_, q, _) => DeploysPage(queryParams: q),
            ),
            overrides: <Override>[
              deploysProvider.overrideWith(() => fakeDeploys),
            ],
          ),
        );
        await tester.pumpAndSettle();
        expect(fakeDeploys.openedRunIds, isEmpty);

        queryNotifier.value = const <String, String>{'run': '77'};
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 200));

        expect(fakeDeploys.openedRunIds, <int>[77]);
        expect(find.text('Deploy #77'), findsOneWidget);
      },
    );
  });

  group('Web runtime & root config guards (P0 #3, P1 #5, P2 #6)', () {
    test('clipboard_fallback_web uses package:web without dart:html', () {
      final src = _resolveRepoFile(
        'frontend/lib/core/utils/clipboard_fallback_web.dart',
      ).readAsStringSync();
      expect(src, contains("import 'package:web/web.dart' as web;"));
      expect(src, isNot(contains('dart:html')));
      expect(src, contains("execCommand('copy')"));
    });

    test('deploys_page opens GitHub installUrl in _self on Web', () {
      final src = _resolveRepoFile(
        'frontend/lib/features/servers/pages/deploys_page.dart',
      ).readAsStringSync();
      expect(src, contains("webOnlyWindowName: '_self'"));
    });

    test('file_download_web defers URL.revokeObjectURL via Timer', () {
      final src = _resolveRepoFile(
        'frontend/lib/core/utils/file_download_web.dart',
      ).readAsStringSync();
      expect(src, contains('Timer('));
      expect(src, contains('web.URL.revokeObjectURL(url)'));
      expect(src, isNot(contains('Timer.periodic(')));
    });

    test(
      'root .env.example and .dockerignore have no leftover Vite/dist refs',
      () {
        final envExample = _resolveRepoFile('.env.example').readAsStringSync();
        final dockerignore = _resolveRepoFile(
          '.dockerignore',
        ).readAsStringSync();
        expect(envExample, isNot(contains('Vite')));
        expect(envExample, isNot(contains(':5173')));
        expect(dockerignore, isNot(contains('frontend/dist/')));
      },
    );
  });
}
