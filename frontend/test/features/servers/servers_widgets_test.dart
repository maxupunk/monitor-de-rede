import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/servers/models/agent_models.dart';
import 'package:netmonitor_app/features/servers/models/database_models.dart';
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
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/features/servers/providers/registry_provider.dart';
import 'package:netmonitor_app/features/servers/providers/storages_provider.dart';
import 'package:netmonitor_app/features/servers/widgets/databases_backup_tab.dart';
import 'package:netmonitor_app/features/servers/widgets/deploy/deploy_setup_wizard_dialog.dart';
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

    testWidgets(
      'DatabasesBackupTab clears stale failure warning immediately when starting a new backup',
      (tester) async {
        final dbNotifier = _InteractiveDatabasesNotifier();
        final storagesNotifier = _RecordingStoragesNotifier();
        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthNotifier.new),
              databasesProvider.overrideWith(() => dbNotifier),
              storagesProvider.overrideWith(() => storagesNotifier),
            ],
            child: MaterialApp(
              theme: AppTheme.dark(),
              home: const Scaffold(body: DatabasesBackupTab()),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(find.text('Último falhou'), findsOneWidget);
        expect(
          find.textContaining('password authentication failed'),
          findsOneWidget,
        );

        await tester.ensureVisible(find.text('Backup agora'));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Backup agora'));
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 100));

        // Assim que o novo backup inicia, o erro antigo e "Último falhou" somem.
        expect(find.text('Último falhou'), findsNothing);
        expect(
          find.textContaining('password authentication failed'),
          findsNothing,
        );
        expect(find.text('Em andamento'), findsWidgets);
      },
    );

    testWidgets(
      'DatabaseConnectionFormBody preselects first storage destination and enables scheduled backup by default',
      (tester) async {
        final dbNotifier = _InteractiveDatabasesNotifier();
        final storagesNotifier = _RecordingStoragesNotifier();
        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthNotifier.new),
              databasesProvider.overrideWith(() => dbNotifier),
              storagesProvider.overrideWith(() => storagesNotifier),
            ],
            child: MaterialApp(
              theme: AppTheme.dark(),
              home: const Scaffold(
                body: SingleChildScrollView(
                  padding: EdgeInsets.all(16),
                  child: DatabaseConnectionFormBody(),
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(find.text('backups-database (local)'), findsOneWidget);

        final scheduledSwitch = tester.widget<SwitchListTile>(
          find.widgetWithText(SwitchListTile, 'Backup automático agendado'),
        );
        expect(scheduledSwitch.value, isTrue);
        expect(scheduledSwitch.onChanged, isNotNull);

        await tester.enterText(
          find.widgetWithText(TextField, 'Nome da conexão'),
          'erp-prod',
        );
        await tester.enterText(
          find.widgetWithText(TextField, 'Host / IP'),
          '10.0.0.20',
        );
        final saveBtn = find.text('Salvar conexão');
        await tester.ensureVisible(saveBtn);
        await tester.tap(saveBtn);
        await tester.pumpAndSettle();

        expect(dbNotifier.savedInputs.length, 1);
        expect(dbNotifier.savedInputs.single.storageDestinationId, 10);
        expect(dbNotifier.savedInputs.single.backupEnabled, isTrue);
      },
    );

    testWidgets(
      'DeploySetupDialog lists GitHub App repositories, filters as user types, and auto-fills on tap',
      (tester) async {
        final deploysNotifier = _RepoDiscoveryDeploysNotifier();
        final dbNotifier = _InteractiveDatabasesNotifier();
        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              authProvider.overrideWith(_AdminAuthNotifier.new),
              deploysProvider.overrideWith(() => deploysNotifier),
              databasesProvider.overrideWith(() => dbNotifier),
            ],
            child: MaterialApp(
              theme: AppTheme.dark(),
              home: Scaffold(
                body: Builder(
                  builder: (ctx) => FilledButton(
                    onPressed: () => showDeploySetupDialog(ctx),
                    child: const Text('Abrir wizard'),
                  ),
                ),
              ),
            ),
          ),
        );

        await tester.tap(find.text('Abrir wizard'));
        await tester.pumpAndSettle();

        // Repositórios do GitHub App conectado aparecem automaticamente.
        expect(find.text('maxupunk/sgc-v4'), findsOneWidget);
        expect(find.text('maxupunk/site-institucional'), findsOneWidget);

        // Digitar no campo filtra a lista em tempo real.
        await tester.enterText(
          find.widgetWithText(TextField, 'Repositório (dono/repo)'),
          'sgc',
        );
        await tester.pumpAndSettle();
        expect(find.text('maxupunk/sgc-v4'), findsOneWidget);
        expect(find.text('maxupunk/site-institucional'), findsNothing);

        // Selecionar o repositório preenche nome, compose e recolhe a lista.
        final repoItem = find.text('maxupunk/sgc-v4');
        await tester.ensureVisible(repoItem);
        await tester.pumpAndSettle();
        await tester.tap(repoItem);
        await tester.pumpAndSettle();

        expect(find.text('Trocar'), findsOneWidget);
        expect(find.text('sgc-v4'), findsOneWidget);
        expect(find.text('docker-compose.prod.yml'), findsOneWidget);
        expect(
          find.textContaining('Detectado: docker-compose.prod.yml'),
          findsOneWidget,
        );
        expect(find.textContaining('.env.example'), findsOneWidget);

        // No passo 2, a branch padrão e os chips de branch já vêm prontos.
        final nextBtn = find.text('Próximo');
        await tester.ensureVisible(nextBtn);
        await tester.pumpAndSettle();
        await tester.tap(nextBtn);
        await tester.pumpAndSettle();
        expect(find.text('production'), findsWidgets);
        expect(find.text('staging'), findsOneWidget);

        // No passo 3, o .env.example vem pré-carregado no Modo lista e permite trocar de modelo.
        await tester.ensureVisible(nextBtn);
        await tester.pumpAndSettle();
        await tester.tap(nextBtn);
        await tester.pumpAndSettle();

        expect(find.text('.env.example'), findsOneWidget);
        expect(find.text('.env.production.example'), findsOneWidget);
        expect(find.text('DATABASE_URL'), findsOneWidget);
        expect(find.text('JWT_SECRET'), findsOneWidget);
        expect(find.text('1 em branco'), findsOneWidget);

        // Trocar para o segundo modelo atualiza a lista de variáveis.
        await tester.tap(find.text('.env.production.example'));
        await tester.pumpAndSettle();
        expect(find.text('NODE_ENV'), findsOneWidget);
        expect(find.text('production'), findsOneWidget);

        // Alternar para Texto .env exibe o conteúdo serializado.
        await tester.tap(find.text('Texto .env'));
        await tester.pumpAndSettle();
        expect(find.textContaining('NODE_ENV=production'), findsOneWidget);
      },
    );
  });
}

class _RepoDiscoveryDeploysNotifier extends _SeededDeploysNotifier {
  @override
  DeploysState build() => const DeploysState(
    overview: DeployOverview(
      applications: <DeployApplicationView>[],
      recentRuns: <DeployRunView>[],
      counts: DeployCounts(),
      credentials: <DeployCredentialView>[],
      githubConnections: <DeployGithubConnectionView>[
        DeployGithubConnectionView(
          id: 5,
          account: 'maxupunk',
          appSlug: 'netmonitor-prod',
          installed: true,
          canWrite: true,
          installUrl:
              'https://github.com/apps/netmonitor-prod/installations/new',
          settingsUrl: 'https://github.com/settings/installations/1',
          createdAt: '2026-11-20T10:00:00Z',
        ),
      ],
      hookPath: '/api/deploys/hook',
      oidcAudience: 'netmonitor',
      guide: DeployGuideView(complete: true, steps: <DeployGuideStep>[]),
    ),
    agents: <DeployAgentView>[
      DeployAgentView(
        id: 1,
        name: 'Este servidor',
        connected: true,
        allowsDeploy: true,
        allowsBuild: true,
      ),
    ],
  );

  @override
  Future<void> fetchAgents() async {}

  @override
  Future<List<DeployRepositoryView>> githubRepositories({
    int? connectionId,
    int? credentialId,
  }) async => const <DeployRepositoryView>[
    DeployRepositoryView(
      fullName: 'maxupunk/sgc-v4',
      isPrivate: true,
      defaultBranch: 'production',
      description: 'ERP principal',
    ),
    DeployRepositoryView(
      fullName: 'maxupunk/site-institucional',
      isPrivate: false,
      defaultBranch: 'main',
    ),
  ];

  @override
  Future<DeployComposeDiscovery> githubCompose({
    required String repository,
    String? reference,
    int? connectionId,
    int? credentialId,
  }) async => const DeployComposeDiscovery(
    files: <String>['docker-compose.prod.yml'],
    suggestedFiles: <String>['docker-compose.prod.yml'],
    services: <String>['api', 'web'],
    images: <DeployBuildImage>[],
    imageTagVariable: 'IMAGE_TAG',
    notes: <String>[],
    envTemplates: <DeployEnvTemplateView>[
      DeployEnvTemplateView(
        name: '.env.example',
        content: 'DATABASE_URL=postgres://localhost:5432/app\nJWT_SECRET=\n',
      ),
      DeployEnvTemplateView(
        name: '.env.production.example',
        content: 'NODE_ENV=production\n',
      ),
    ],
  );

  @override
  Future<DeployReferencesView> githubReferences({
    required String repository,
    int? connectionId,
    int? credentialId,
  }) async => const DeployReferencesView(
    defaultBranch: 'production',
    branches: <String>['production', 'staging', 'main'],
    tags: <String>['v1.0.0'],
  );

  @override
  Future<DeployReadinessView> readinessFor(int probeId, String project) async =>
      const DeployReadinessView(ready: true, checks: <DeployReadinessCheck>[]);
}

class _InteractiveDatabasesNotifier extends DatabasesNotifier {
  final List<DatabaseConnectionInput> savedInputs = <DatabaseConnectionInput>[];

  @override
  DatabasesState build() => const DatabasesState(
    loaded: true,
    connections: <DatabaseConnectionResponse>[
      DatabaseConnectionResponse(
        id: 1,
        name: 'sgc-v4-prod-postgres',
        engine: 'postgres',
        host: '172.19.0.6',
        port: 5432,
        username: 'sgc4user',
        passwordSet: true,
        sslMode: 'prefer',
        databases: <String>['app'],
        storageDestinationId: 10,
        storageDestinationName: 'backups-database',
        backupEnabled: true,
        backupIntervalHours: 24,
        backupRetention: 7,
        lastBackupAt: '2026-11-26T12:00:00Z',
        lastBackupStatus: 'failed',
        lastBackupError:
            'app: Não foi possível conectar: password authentication failed for user "sgc4user"',
      ),
    ],
  );

  @override
  Future<bool> fetchConnections() async => true;

  @override
  Future<void> fetchAgents() async {}

  @override
  Future<void> fetchContainers() async {}

  @override
  Future<DatabaseConnectionResponse> save(
    int? id,
    DatabaseConnectionInput input,
  ) async {
    savedInputs.add(input);
    return DatabaseConnectionResponse(
      id: id ?? 2,
      name: input.name,
      engine: input.engine,
      host: input.host,
      port: input.port,
      username: input.username,
      passwordSet: input.password != null,
      sslMode: input.sslMode,
      databases: input.databases,
      storageDestinationId: input.storageDestinationId,
      backupEnabled: input.backupEnabled,
      backupIntervalHours: input.backupIntervalHours,
      backupRetention: input.backupRetention,
    );
  }

  @override
  Future<DatabaseJobSnapshot?> runBackup(int id) async {
    final snap = DatabaseJobSnapshot(
      id: 'job-running-1',
      kind: 'backup',
      connectionId: id,
      database: 'app',
      stage: 'Dados de public.conversation_messages',
      rows: 18463,
      bytes: 7738490,
      databasesDone: 0,
      databasesTotal: 1,
      status: 'running',
      startedAt: '2026-11-26T12:01:00Z',
    );
    applyJob(<String, dynamic>{
      'id': snap.id,
      'kind': snap.kind,
      'connectionId': snap.connectionId,
      'database': snap.database,
      'stage': snap.stage,
      'rows': snap.rows,
      'bytes': snap.bytes,
      'databasesDone': snap.databasesDone,
      'databasesTotal': snap.databasesTotal,
      'status': snap.status,
      'startedAt': snap.startedAt,
    });
    return snap;
  }
}

class _RecordingStoragesNotifier extends StoragesNotifier {
  final List<StorageDestinationInput> savedInputs = <StorageDestinationInput>[];

  @override
  StoragesState build() => const StoragesState(
    loaded: true,
    meta: StoragesMetaResponse(localRoot: '/data/storage'),
    storages: <StorageDestinationResponse>[
      StorageDestinationResponse(
        id: 10,
        name: 'backups-database',
        provider: 'local',
        target: '/data/storage/databases-local',
      ),
      StorageDestinationResponse(
        id: 11,
        name: 's3-offsite',
        provider: 'aws_s3',
        target: 's3://empresa-backups',
      ),
    ],
  );

  @override
  Future<bool> fetchStorages() async => true;

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
