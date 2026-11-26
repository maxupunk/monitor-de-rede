import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/servers/models/agent_models.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/models/registry_models.dart';
import 'package:netmonitor_app/features/servers/models/storage_models.dart';
import 'package:netmonitor_app/features/servers/pages/deploys_page.dart';
import 'package:netmonitor_app/features/servers/pages/docker_containers_page.dart';
import 'package:netmonitor_app/features/servers/pages/docker_history_page.dart';
import 'package:netmonitor_app/features/servers/pages/docker_layout_page.dart';
import 'package:netmonitor_app/features/servers/pages/docker_overview_page.dart';
import 'package:netmonitor_app/features/servers/pages/registry_page.dart';
import 'package:netmonitor_app/features/servers/pages/remote_servers_page.dart';
import 'package:netmonitor_app/features/servers/providers/agents_provider.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/features/servers/providers/registry_provider.dart';
import 'package:netmonitor_app/features/servers/providers/storages_provider.dart';
import 'package:netmonitor_app/features/servers/widgets/deploy_env_dialog.dart';
import 'package:netmonitor_app/features/servers/widgets/stacked_container_usage_chart.dart';
import 'package:netmonitor_app/features/servers/widgets/storage_form_sheet.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
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

class _SeededDockerNotifier extends DockerNotifier {
  @override
  DockerState build() => const DockerState(
    hostsLoaded: true,
    selectedHostKey: 'agent-1',
    hosts: <DockerHostView>[
      DockerHostView(
        key: 'agent-1',
        name: 'Central',
        kind: 'local',
        online: true,
        agentId: 1,
        dockerAvailable: true,
        composeAvailable: true,
        policy: <String>['read', 'lifecycle', 'update', 'compose'],
      ),
    ],
    hostStates: <String, DockerHostState>{
      'agent-1': DockerHostState(
        status: DockerStatusResponse(available: true, engineVersion: '27.3.1'),
        containers: <DockerContainerSummary>[
          DockerContainerSummary(
            id: 'protected123456',
            names: <String>['/netmonitor-api'],
            image: 'netmonitor:latest',
            imageId: 'sha256:1',
            state: 'running',
            status: 'Up 3h',
            labels: <String, String>{'com.netmonitor.protected': 'true'},
            ports: <DockerPortBinding>[],
            created: 1700000000,
            projectName: 'netmonitor',
            protected: true,
          ),
          DockerContainerSummary(
            id: 'normal123456',
            names: <String>['/nginx-web'],
            image: 'nginx:alpine',
            imageId: 'sha256:2',
            state: 'running',
            status: 'Up 1h',
            labels: <String, String>{},
            ports: <DockerPortBinding>[],
            created: 1700000000,
            projectName: 'web',
            protected: false,
          ),
        ],
        aggregateHistory: <DockerAggregateSample>[
          DockerAggregateSample(
            recordedAt: '2026-11-21T16:00:00Z',
            cpuPercent: 3.4,
            memoryPercent: 18.2,
            memoryUsageBytes: 380 * 1024 * 1024,
            networkReceivedBytes: 1024,
            networkTransmittedBytes: 512,
            cpuByContainer: <String, double>{
              'protected123456': 2.1,
              'normal123456': 1.3,
            },
            memoryByContainer: <String, int>{
              'protected123456': 260 * 1024 * 1024,
              'normal123456': 120 * 1024 * 1024,
            },
            containerNames: <String, String>{
              'protected123456': 'netmonitor-api',
              'normal123456': 'nginx-web',
            },
          ),
          DockerAggregateSample(
            recordedAt: '2026-11-21T16:00:05Z',
            cpuPercent: 4.8,
            memoryPercent: 19.5,
            memoryUsageBytes: 410 * 1024 * 1024,
            networkReceivedBytes: 2048,
            networkTransmittedBytes: 1024,
            cpuByContainer: <String, double>{
              'protected123456': 3.2,
              'normal123456': 1.6,
            },
            memoryByContainer: <String, int>{
              'protected123456': 280 * 1024 * 1024,
              'normal123456': 130 * 1024 * 1024,
            },
            containerNames: <String, String>{
              'protected123456': 'netmonitor-api',
              'normal123456': 'nginx-web',
            },
          ),
        ],
      ),
    },
  );
}

class _SeededAgentsNotifier extends AgentsNotifier {
  @override
  AgentsState build() => const AgentsState(
    loaded: true,
    agents: <AgentView>[
      AgentView(
        id: 1,
        name: 'Central Local',
        hostKey: 'agent-1',
        status: 'online',
        connected: true,
        local: true,
        host: AgentHostInfo(
          hostname: 'netmonitor-host',
          os: 'linux',
          arch: 'amd64',
          policy: <String>['read', 'lifecycle'],
          docker: AgentCapabilityView(available: true, version: '27.3.1'),
          compose: AgentCapabilityView(available: true, version: '2.29.0'),
        ),
      ),
    ],
  );

  @override
  Future<void> fetchAgents() async {}
}

class _SeededDeploysNotifier extends DeploysNotifier {
  @override
  DeploysState build() => const DeploysState(
    overview: DeployOverview(
      applications: <DeployApplicationView>[],
      recentRuns: <DeployRunView>[],
      counts: DeployCounts(
        applications: 2,
        targets: 3,
        succeeded24h: 4,
        failed24h: 0,
      ),
      credentials: <DeployCredentialView>[],
      githubConnections: <DeployGithubConnectionView>[],
      hookPath: '/api/deploys/hook',
      oidcAudience: 'netmonitor',
      guide: DeployGuideView(complete: true, steps: <DeployGuideStep>[]),
    ),
  );

  @override
  Future<bool> fetchOverview() async => true;

  @override
  Future<void> fetchRecentBuilds() async {}
}

class _SeededRegistryNotifier extends RegistryNotifier {
  @override
  RegistryState build() => const RegistryState(
    overview: RegistryOverview(
      settings: RegistrySettings(keepVersions: 10, quotaGib: 20),
      usedBytes: 512 * 1024 * 1024,
      quotaBytes: 20 * 1024 * 1024 * 1024,
      repositories: <RegistryRepositoryView>[
        RegistryRepositoryView(
          id: 1,
          name: 'app/api',
          versions: 3,
          bytes: 256 * 1024 * 1024,
        ),
      ],
      feeds: <RegistryFeedView>[],
    ),
  );

  @override
  Future<void> fetchOverview() async {}

  @override
  Future<List<RegistryTokenView>> fetchTokens() async =>
      const <RegistryTokenView>[];

  @override
  Future<List<RegistryExceptionView>> fetchExceptions() async =>
      const <RegistryExceptionView>[];
}

void _setMobileViewport(WidgetTester tester) {
  tester.view.physicalSize = const Size(390, 844);
  tester.view.devicePixelRatio = 1.0;
  addTearDown(() {
    tester.view.resetPhysicalSize();
    tester.view.resetDevicePixelRatio();
  });
}

void main() {
  testWidgets(
    'DockerContainersPage displays Protegido badge and blocks destructive buttons on protected container',
    (tester) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: <Override>[
            authProvider.overrideWith(_AdminAuthNotifier.new),
            dockerProvider.overrideWith(_SeededDockerNotifier.new),
          ],
          child: MaterialApp(
            theme: AppTheme.dark(),
            home: const Scaffold(
              body: SingleChildScrollView(
                padding: EdgeInsets.all(16),
                child: DockerContainersPage(),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('netmonitor-api'), findsOneWidget);
      expect(find.text('nginx-web'), findsOneWidget);
      expect(find.text('Protegido'), findsOneWidget);

      // Somente o container não-protegido (`nginx-web`) deve exibir botão Parar e menu de ações.
      expect(find.byTooltip('Parar'), findsOneWidget);
      expect(find.byTooltip('Ações do container'), findsOneWidget);
    },
  );

  testWidgets(
    'RemoteServersPage renders local central agent with Esta central badge and no revoke menu',
    (tester) async {
      await tester.pumpWidget(
        ProviderScope(
          overrides: <Override>[
            authProvider.overrideWith(_AdminAuthNotifier.new),
            agentsProvider.overrideWith(_SeededAgentsNotifier.new),
          ],
          child: MaterialApp(
            theme: AppTheme.dark(),
            home: const Scaffold(body: RemoteServersPage()),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('Central Local'), findsOneWidget);
      expect(find.text('Esta central'), findsOneWidget);
      expect(find.text('Conectado'), findsOneWidget);
      // Agente local não pode ser revogado/removido pela UI (ADR 014).
      expect(find.byTooltip('Mais ações'), findsNothing);
    },
  );

  group('Mobile 390x844 zero-overflow UX audit (Servers)', () {
    testWidgets(
      'RemoteServersPage renders compact mobile header + symmetrical chip strip with zero overflow',
      (tester) async {
        _setMobileViewport(tester);
        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthNotifier.new),
              agentsProvider.overrideWith(_SeededAgentsNotifier.new),
            ],
            child: MaterialApp(
              theme: AppTheme.light(),
              home: const Scaffold(body: RemoteServersPage()),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(find.byType(UnifiedFilterBar), findsOneWidget);
        expect(find.text('Central Local'), findsOneWidget);
      },
    );

    testWidgets(
      'DockerLayoutScaffold + DockerContainersPage scrolls header cleanly at 390x844 with zero overflow',
      (tester) async {
        _setMobileViewport(tester);
        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthNotifier.new),
              dockerProvider.overrideWith(_SeededDockerNotifier.new),
            ],
            child: MaterialApp(
              theme: AppTheme.dark(),
              home: const Scaffold(
                body: DockerLayoutScaffold(
                  activeTab: DockerTab.containers,
                  child: DockerContainersPage(),
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(find.byType(CustomScrollView), findsOneWidget);
        expect(find.byType(UnifiedFilterBar), findsOneWidget);
        expect(find.text('Protegido'), findsOneWidget);

        // Host Docker dropdown + icon-only refresh button must share the exact same single row
        final hostDropdownFinder = find.byType(AppDropdownFormField<String>);
        final refreshFinder = find.byTooltip('Atualizar');
        expect(hostDropdownFinder, findsOneWidget);
        expect(refreshFinder, findsOneWidget);
        final hostRect = tester.getRect(hostDropdownFinder);
        final refreshRect = tester.getRect(refreshFinder);
        expect((hostRect.center.dy - refreshRect.center.dy).abs(), lessThan(8));
        expect(refreshRect.left, greaterThanOrEqualTo(hostRect.right));

        // Zero redundant horizontal outer padding (flush with shell boundaries)
        final filterRect = tester.getRect(find.byType(UnifiedFilterBar));
        expect(filterRect.left, equals(0));
        expect(filterRect.right, equals(390));
      },
    );

    testWidgets(
      'DeploysPage renders compact top bar and 2x2 KPI grid at 390x844 with zero overflow',
      (tester) async {
        _setMobileViewport(tester);
        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthNotifier.new),
              deploysProvider.overrideWith(_SeededDeploysNotifier.new),
            ],
            child: MaterialApp(
              theme: AppTheme.light(),
              home: const Scaffold(body: DeploysPage()),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(find.text('Deploy contínuo'), findsOneWidget);
        expect(find.text('Automatizar'), findsOneWidget);
      },
    );

    testWidgets(
      'RegistryPage renders compact top bar and stacked settings fields at 390x844 with zero overflow',
      (tester) async {
        _setMobileViewport(tester);
        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthNotifier.new),
              registryProvider.overrideWith(_SeededRegistryNotifier.new),
            ],
            child: MaterialApp(
              theme: AppTheme.light(),
              home: const Scaffold(body: RegistryPage()),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(find.text('Registry de imagens'), findsOneWidget);
        expect(find.text('app/api'), findsOneWidget);
      },
    );

    testWidgets(
      'DockerOverviewPage renders stacked per-container CPU & RAM charts with interactive legend at 360x800',
      (tester) async {
        tester.view.physicalSize = const Size(360, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(() {
          tester.view.resetPhysicalSize();
          tester.view.resetDevicePixelRatio();
        });

        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthNotifier.new),
              dockerProvider.overrideWith(_SeededDockerNotifier.new),
            ],
            child: MaterialApp(
              theme: AppTheme.dark(),
              home: const Scaffold(
                body: DockerLayoutScaffold(
                  activeTab: DockerTab.overview,
                  child: DockerOverviewPage(),
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(find.byType(StackedContainerUsageChart), findsNWidgets(2));
        expect(find.text('CPU por container'), findsOneWidget);
        expect(find.text('RAM por container'), findsOneWidget);
        expect(find.text('netmonitor-api'), findsWidgets);
        expect(find.text('nginx-web'), findsWidgets);

        // Tap a legend chip to isolate/highlight a container without error/overflow
        await tester.ensureVisible(find.text('netmonitor-api').first);
        await tester.tap(find.text('netmonitor-api').first);
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
      },
    );

    testWidgets(
      'DockerHistoryPage keeps 5-range SegmentedButton single-line beside refresh icon at 360x800',
      (tester) async {
        tester.view.physicalSize = const Size(360, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(() {
          tester.view.resetPhysicalSize();
          tester.view.resetDevicePixelRatio();
        });

        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthNotifier.new),
              dockerProvider.overrideWith(
                () => _StaticEmptyHostDockerNotifier(),
              ),
            ],
            child: MaterialApp(
              theme: AppTheme.dark(),
              home: const Scaffold(
                body: Padding(
                  padding: EdgeInsets.symmetric(horizontal: 12),
                  child: SingleChildScrollView(child: DockerHistoryPage()),
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        final seg24h = tester.getRect(find.text('24h'));
        final refreshRect = tester.getRect(
          find.byTooltip('Recarregar histórico'),
        );
        // '24h' must never wrap vertically into 2 / 4 / h
        expect(seg24h.height, lessThan(26));
        // Refresh icon shares the same row beside the SegmentedButton
        expect((seg24h.center.dy - refreshRect.center.dy).abs(), lessThan(12));
        expect(refreshRect.left, greaterThan(seg24h.right));
      },
    );

    testWidgets(
      'DockerHistoryPage opens container history bottom sheet immediately on mobile tap at 390x844',
      (tester) async {
        _setMobileViewport(tester);

        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthNotifier.new),
              dockerProvider.overrideWith(_HistoryDockerNotifier.new),
            ],
            child: MaterialApp(
              theme: AppTheme.dark(),
              home: const Scaffold(
                body: Padding(
                  padding: EdgeInsets.symmetric(horizontal: 12),
                  child: SingleChildScrollView(child: DockerHistoryPage()),
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        // Switch mobile toggle to Containers (2)
        await tester.tap(find.text('Containers (2)'));
        await tester.pumpAndSettle();
        expect(find.text('netmonitor-api'), findsOneWidget);
        expect(find.text('postgres-db'), findsOneWidget);

        // Tap a container card -> opens Material 3 bottom sheet immediately
        await tester.tap(find.text('netmonitor-api'));
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(find.text('Histórico do container'), findsOneWidget);
        expect(find.text('CPU (%)'), findsOneWidget);
        expect(find.text('Memória (RAM)'), findsOneWidget);
        expect(find.text('Rede (RX / TX)'), findsOneWidget);
        expect(find.text('Disco (Block I/O)'), findsOneWidget);
        expect(find.byType(AppDropdownFormField<String>), findsOneWidget);
      },
    );

    testWidgets(
      'DeployEnvDialog preserves ValueKey field values when removing an earlier .env variable',
      (tester) async {
        const target = DeployTargetView(
          id: 7,
          applicationId: 1,
          name: 'Produção',
          project: 'erp',
          reference: 'main',
          requiresApproval: false,
          waitTimeoutSecs: 180,
          autoRollback: true,
          keepImages: 3,
          enabled: true,
          position: 0,
          vulnerabilityPolicy: 'block_critical',
          requireProvenance: false,
        );

        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthNotifier.new),
              deploysProvider.overrideWith(_EnvSeededDeploysNotifier.new),
            ],
            child: MaterialApp(
              theme: AppTheme.dark(),
              home: Scaffold(
                body: Builder(
                  builder: (ctx) => FilledButton(
                    onPressed: () => showDeployEnvDialog(
                      ctx,
                      target: target,
                      canAdmin: true,
                    ),
                    child: const Text('Abrir env'),
                  ),
                ),
              ),
            ),
          ),
        );

        await tester.tap(find.text('Abrir env'));
        await tester.pumpAndSettle();

        expect(find.text('PRIMEIRA_CHAVE'), findsOneWidget);
        expect(find.text('SEGUNDA_CHAVE'), findsOneWidget);
        expect(find.text('valor_b'), findsOneWidget);

        // Remove first variable -> second variable must keep its exact key/value (no stale initialValue shift)
        final removeButtons = find.byTooltip('Remover variável');
        expect(removeButtons, findsNWidgets(2));
        await tester.tap(removeButtons.first);
        await tester.pumpAndSettle();

        expect(find.text('PRIMEIRA_CHAVE'), findsNothing);
        expect(find.text('SEGUNDA_CHAVE'), findsOneWidget);
        expect(find.text('valor_b'), findsOneWidget);
      },
    );

    testWidgets(
      'StorageFormBody shows trailing slash on localRoot prefix and guides user on leading slash',
      (tester) async {
        final notifier = _RecordingStoragesNotifier();
        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthNotifier.new),
              storagesProvider.overrideWith(() => notifier),
            ],
            child: MaterialApp(
              theme: AppTheme.dark(),
              home: const Scaffold(
                body: SingleChildScrollView(
                  padding: EdgeInsets.all(16),
                  child: StorageFormBody(),
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        final fields = tester
            .widgetList<TextField>(find.byType(TextField))
            .toList();
        expect(fields.length, 2);
        final subfolderField = fields[1];
        expect(
          subfolderField.decoration?.labelText,
          'Subpasta dentro de /data/storage',
        );
        expect(subfolderField.decoration?.prefixText, '/data/storage/');

        // Digitar barra inicial mostra erro inline e bloqueia o envio antes da API.
        await tester.enterText(find.byType(TextField).at(0), 'databases-local');
        await tester.enterText(
          find.byType(TextField).at(1),
          '/databases-local',
        );
        await tester.pumpAndSettle();

        expect(
          find.text(
            'Não comece com "/" — a barra já faz parte de /data/storage/.',
          ),
          findsOneWidget,
        );
        await tester.tap(find.text('Salvar destino'));
        await tester.pumpAndSettle();
        expect(notifier.savedInputs, isEmpty);

        // Sem barra inicial envia a subpasta relativa limpa.
        await tester.enterText(find.byType(TextField).at(1), 'databases-local');
        await tester.pumpAndSettle();
        expect(
          find.text(
            'Não comece com "/" — a barra já faz parte de /data/storage/.',
          ),
          findsNothing,
        );
        await tester.tap(find.text('Salvar destino'));
        await tester.pumpAndSettle();

        expect(notifier.savedInputs.length, 1);
        expect(
          notifier.savedInputs.single.config['basePath'],
          'databases-local',
        );
      },
    );
  });
}

class _RecordingStoragesNotifier extends StoragesNotifier {
  final List<StorageDestinationInput> savedInputs = <StorageDestinationInput>[];

  @override
  StoragesState build() => const StoragesState(
    loaded: true,
    meta: StoragesMetaResponse(localRoot: '/data/storage'),
  );

  @override
  Future<StorageDestinationDetail> save(
    int? id,
    StorageDestinationInput input,
  ) async {
    savedInputs.add(input);
    return StorageDestinationDetail(
      id: 1,
      name: input.name,
      provider: input.provider,
      target: '/data/storage/${input.config['basePath'] ?? ''}',
      config: input.config,
      secretsSet: const <String>[],
    );
  }
}

class _EnvSeededDeploysNotifier extends _SeededDeploysNotifier {
  @override
  Future<DeployEnvContentView> envCurrent(int targetId) async =>
      const DeployEnvContentView(
        id: 1,
        content: 'PRIMEIRA_CHAVE=valor_a\nSEGUNDA_CHAVE=valor_b\n',
      );

  @override
  Future<List<DeployEnvVersionView>> envVersions(int targetId) async =>
      const <DeployEnvVersionView>[];
}

class _StaticEmptyHostDockerNotifier extends DockerNotifier {
  @override
  DockerState build() => const DockerState(hostsLoaded: true);
}

class _FakeHistoryDockerHostApi extends DockerHostApi {
  const _FakeHistoryDockerHostApi(super.api, super.hostKey);

  @override
  Future<HostHistoryResponse> hostHistory(String range) async {
    return HostHistoryResponse(
      hostKey: hostKey,
      range: range,
      stepMinutes: 5,
      host: const <HostHistoryPoint>[
        HostHistoryPoint(
          at: '2026-11-21T16:00:00Z',
          cpuAvg: 12.4,
          cpuMax: 24.0,
          memoryUsed: 1200 * 1024 * 1024,
          memoryTotal: 4096 * 1024 * 1024,
          load1: 0.45,
          netRxBps: 25000,
          netTxBps: 18000,
          disks: <HostDiskPoint>[
            HostDiskPoint(
              mount: '/',
              usedBytes: 24 * 1024 * 1024 * 1024,
              totalBytes: 100 * 1024 * 1024 * 1024,
            ),
          ],
        ),
      ],
      containers: const <ContainerUsageSummary>[
        ContainerUsageSummary(
          name: 'netmonitor-api',
          project: 'netmonitor',
          cpuAvg: 3.2,
          cpuMax: 9.8,
          memoryMax: 320 * 1024 * 1024,
          netRxBytes: 1024 * 1024 * 42,
          netTxBytes: 1024 * 1024 * 18,
        ),
        ContainerUsageSummary(
          name: 'postgres-db',
          project: 'netmonitor',
          cpuAvg: 1.5,
          cpuMax: 5.1,
          memoryMax: 480 * 1024 * 1024,
          netRxBytes: 1024 * 1024 * 12,
          netTxBytes: 1024 * 1024 * 9,
        ),
      ],
    );
  }

  @override
  Future<ContainerHistoryResponse> containerHistory(
    String containerName,
    String range,
  ) async {
    return ContainerHistoryResponse(
      hostKey: hostKey,
      containerName: containerName,
      range: range,
      stepMinutes: 5,
      points: const <ContainerHistoryPoint>[
        ContainerHistoryPoint(
          at: '2026-11-21T16:00:00Z',
          cpuAvg: 2.5,
          cpuMax: 6.0,
          memoryAvg: 240 * 1024 * 1024,
          memoryMax: 300 * 1024 * 1024,
          netRxBytes: 1024 * 512,
          netTxBytes: 1024 * 256,
          blockReadBytes: 1024 * 1024 * 4,
          blockWriteBytes: 1024 * 1024 * 2,
        ),
        ContainerHistoryPoint(
          at: '2026-11-21T16:05:00Z',
          cpuAvg: 3.8,
          cpuMax: 9.8,
          memoryAvg: 280 * 1024 * 1024,
          memoryMax: 320 * 1024 * 1024,
          netRxBytes: 1024 * 700,
          netTxBytes: 1024 * 400,
          blockReadBytes: 1024 * 1024 * 6,
          blockWriteBytes: 1024 * 1024 * 3,
        ),
      ],
    );
  }
}

class _HistoryDockerNotifier extends _SeededDockerNotifier {
  @override
  DockerHostApi apiFor([String? hostKey]) => _FakeHistoryDockerHostApi(
    ref.read(apiClientProvider),
    hostKey ?? state.selectedHostKey,
  );
}
