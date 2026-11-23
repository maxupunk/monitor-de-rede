import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/features/servers/pages/deploys_page.dart';
import 'package:netmonitor_app/features/servers/pages/docker_compose_page.dart';
import 'package:netmonitor_app/features/servers/pages/docker_containers_page.dart';
import 'package:netmonitor_app/features/servers/pages/docker_history_page.dart';
import 'package:netmonitor_app/features/servers/pages/docker_images_page.dart';
import 'package:netmonitor_app/features/servers/pages/docker_layout_page.dart';
import 'package:netmonitor_app/features/servers/pages/docker_networks_page.dart';
import 'package:netmonitor_app/features/servers/pages/docker_overview_page.dart';
import 'package:netmonitor_app/features/servers/pages/docker_volumes_page.dart';
import 'package:netmonitor_app/features/servers/pages/registry_page.dart';
import 'package:netmonitor_app/features/servers/pages/remote_servers_page.dart';
import 'package:netmonitor_app/features/servers/providers/agents_provider.dart';
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/features/servers/providers/registry_provider.dart';
import 'package:netmonitor_app/features/servers/providers/storages_provider.dart';
import 'package:netmonitor_app/features/servers/widgets/database_info_card.dart';
import 'package:netmonitor_app/features/servers/widgets/databases_backup_tab.dart';
import 'package:netmonitor_app/features/servers/widgets/docker_summary_card.dart';
import 'package:netmonitor_app/features/servers/widgets/storages_backup_tab.dart';

final AiScreenContribution _remoteServersAiScreen = AiScreenContribution(
  id: 'remote-servers',
  title: 'Servidores remotos',
  description:
      'Agentes conectados à central, capacidades Docker/Compose e política AGENT_ALLOW.',
  contextBuilder: (ref, _) {
    final agents = ref.read(agentsProvider).agents;
    final connected = agents.where((a) => a.connected).length;
    final offlineNames = agents
        .where((a) => !a.connected && a.status != 'pending')
        .take(3)
        .map((a) => a.name)
        .toList();
    final pending = agents.where((a) => a.status == 'pending').length;
    return <String, dynamic>{
      'totalAgents': agents.length,
      'connectedCount': connected,
      'offlineSample': offlineNames,
      'pendingInstallCount': pending,
    };
  },
);

final AiScreenContribution _dockerAiScreen = AiScreenContribution(
  id: 'docker',
  title: 'Docker',
  description:
      'Containers, projetos Compose, volumes, redes, imagens e histórico do host.',
  contextBuilder: (ref, state) {
    final docker = ref.read(dockerProvider);
    return <String, dynamic>{
      'hostKey': docker.selectedHostKey,
      'hostName': docker.selectedHost?.name,
      'hostOnline': docker.selectedHost?.online,
      'tabPath': state.uri.path,
      'runningContainers': docker.runningContainers,
      'stoppedContainers': docker.stoppedContainers,
    };
  },
);

final AiScreenContribution _deploysAiScreen = AiScreenContribution(
  id: 'deploys',
  title: 'Deploy contínuo',
  description:
      'Aplicações, alvos, aprovações pendentes, compilações isoladas e histórico de deploys.',
  contextBuilder: (ref, _) {
    final deploys = ref.read(deploysProvider);
    final counts = deploys.overview?.counts;
    return <String, dynamic>{
      'applications': counts?.applications ?? deploys.applications.length,
      'targets': counts?.targets ?? 0,
      'running': counts?.running ?? 0,
      'succeeded24h': counts?.succeeded24h ?? 0,
      'failed24h': counts?.failed24h ?? 0,
      'awaitingApproval': deploys.awaitingApproval.length,
    };
  },
);

final AiScreenContribution _registryAiScreen = AiScreenContribution(
  id: 'registry',
  title: 'Registry',
  description:
      'Repositórios OCI internos, tags imutáveis, SBOM/OSV e tokens pessoais.',
  contextBuilder: (ref, _) {
    final overview = ref.read(registryProvider).overview;
    return <String, dynamic>{
      'repositories': overview?.repositories.length ?? 0,
      'usedBytes': overview?.usedBytes ?? 0,
      'quotaBytes': overview?.quotaBytes ?? 0,
    };
  },
);

/// Área Servidores (ADR 011 / 013 / 014 / 017 / 019 / 021 / 022 / 023):
/// Servidores remotos (area `core`), Docker dos hosts, Deploys contínuos, Registry,
/// Bancos de dados, Destinos de armazenamento e uso do banco interno.
final AreaContribution serversArea = AreaContribution(
  id: ProductArea.servers,
  routes: <RouteBase>[
    GoRoute(
      path: '/remote-servers',
      name: 'remote-servers',
      builder: (context, state) => const RemoteServersPage(),
    ),
    GoRoute(path: '/agents', redirect: (_, _) => '/remote-servers'),
    GoRoute(path: '/servers', redirect: (_, _) => '/remote-servers'),
    GoRoute(
      path: '/docker',
      name: 'docker-overview',
      builder: (context, state) => DockerLayoutScaffold(
        activeTab: DockerTab.overview,
        hostQueryParam: state.uri.queryParameters['host'],
        child: const DockerOverviewPage(),
      ),
    ),
    GoRoute(
      path: '/docker/containers',
      name: 'docker-containers',
      builder: (context, state) => DockerLayoutScaffold(
        activeTab: DockerTab.containers,
        hostQueryParam: state.uri.queryParameters['host'],
        child: DockerContainersPage(
          initialStateFilter: state.uri.queryParameters['state'],
        ),
      ),
    ),
    GoRoute(
      path: '/docker/compose',
      name: 'docker-compose',
      builder: (context, state) => DockerLayoutScaffold(
        activeTab: DockerTab.compose,
        hostQueryParam: state.uri.queryParameters['host'],
        child: const DockerComposePage(),
      ),
    ),
    GoRoute(
      path: '/docker/volumes',
      name: 'docker-volumes',
      builder: (context, state) => DockerLayoutScaffold(
        activeTab: DockerTab.volumes,
        hostQueryParam: state.uri.queryParameters['host'],
        child: const DockerVolumesPage(),
      ),
    ),
    GoRoute(
      path: '/docker/networks',
      name: 'docker-networks',
      builder: (context, state) => DockerLayoutScaffold(
        activeTab: DockerTab.networks,
        hostQueryParam: state.uri.queryParameters['host'],
        child: const DockerNetworksPage(),
      ),
    ),
    GoRoute(
      path: '/docker/images',
      name: 'docker-images',
      builder: (context, state) => DockerLayoutScaffold(
        activeTab: DockerTab.images,
        hostQueryParam: state.uri.queryParameters['host'],
        child: const DockerImagesPage(),
      ),
    ),
    GoRoute(
      path: '/docker/history',
      name: 'docker-history',
      builder: (context, state) => DockerLayoutScaffold(
        activeTab: DockerTab.history,
        hostQueryParam: state.uri.queryParameters['host'],
        child: const DockerHistoryPage(),
      ),
    ),
    GoRoute(
      path: '/deploys',
      name: 'deploys',
      builder: (context, state) =>
          DeploysPage(queryParams: state.uri.queryParameters),
    ),
    GoRoute(
      path: '/deploys/github',
      redirect: (_, state) {
        final query = state.uri.query;
        return query.isEmpty ? '/deploys' : '/deploys?$query';
      },
    ),
    GoRoute(
      path: '/registry',
      name: 'registry',
      builder: (context, state) => const RegistryPage(),
    ),
  ],
  // ADR 017 / ADR 019: `/remote-servers` pertence à área `core`; Docker/Deploys/Registry pertencem a `servers`.
  routeMeta: <String, AreaRouteMeta>{
    '/remote-servers': AreaRouteMeta(
      area: ProductArea.core,
      aiScreen: _remoteServersAiScreen,
    ),
    '/docker': AreaRouteMeta(
      area: ProductArea.servers,
      aiScreen: _dockerAiScreen,
    ),
    '/deploys': AreaRouteMeta(
      area: ProductArea.servers,
      aiScreen: _deploysAiScreen,
    ),
    '/registry': AreaRouteMeta(
      area: ProductArea.servers,
      aiScreen: _registryAiScreen,
    ),
  },
  nav: const <NavEntry>[
    NavEntry(
      area: ProductArea.servers,
      section: NavSectionId.servers,
      order: 10,
      item: NavItemSpec(
        title: 'Docker',
        icon: Icons.view_in_ar_outlined,
        to: '/docker',
      ),
    ),
    NavEntry(
      area: ProductArea.servers,
      section: NavSectionId.servers,
      order: 15,
      item: NavItemSpec(
        title: 'Deploys',
        icon: Icons.rocket_launch_outlined,
        to: '/deploys',
      ),
    ),
    NavEntry(
      area: ProductArea.servers,
      section: NavSectionId.servers,
      order: 16,
      item: NavItemSpec(
        title: 'Registry',
        icon: Icons.inventory_2_outlined,
        to: '/registry',
      ),
    ),
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.servers,
      order: 20,
      item: NavItemSpec(
        title: 'Servidores remotos',
        icon: Icons.dns_outlined,
        to: '/remote-servers',
      ),
    ),
  ],
  widgets: <String, DashboardWidgetContribution>{
    'docker_summary': DashboardWidgetContribution(
      type: 'docker_summary',
      builder: (context, config) => DockerSummaryCard(config: config),
    ),
  },
  aiScreens: <String, AiScreenContribution>{
    'remote-servers': _remoteServersAiScreen,
    'docker': _dockerAiScreen,
    'deploys': _deploysAiScreen,
    'registry': _registryAiScreen,
  },
  backupTabs: <BackupTabContribution>[
    BackupTabContribution(
      id: 'databases',
      title: 'Bancos de dados',
      icon: Icons.storage_outlined,
      order: 20,
      builder: (_) => const DatabasesBackupTab(),
    ),
    BackupTabContribution(
      id: 'storages',
      title: 'Destinos',
      icon: Icons.cloud_upload_outlined,
      order: 30,
      builder: (_) => const StoragesBackupTab(),
    ),
  ],
  settingsCards: <SettingsCardContribution>[
    SettingsCardContribution(
      tab: 'sistema',
      order: 20,
      area: ProductArea.core,
      builder: (_) => const DatabaseInfoCard(),
    ),
  ],
  // 10 rotas SSE da área `servers` (incluindo os 4 subtipos de `docker:*` = 10 eventos).
  sseBuilder: (ref) => <String, SseRoute>{
    'docker:*': SseRoute(
      ephemeral: true,
      handle: (data, event, _) {
        final docker = ref.read(dockerProvider.notifier);
        switch (event.type) {
          case 'docker:snapshot':
            docker.applyLiveSnapshot(data);
          case 'docker:inventory':
            docker.applyInventorySnapshot(data);
          case 'docker:operation':
            docker.applyOperationEvent(data);
          case 'docker:log':
            docker.applyLogStreamEvent(data);
        }
      },
    ),
    'deploys:run': SseRoute(
      ephemeral: true,
      handle: (data, _, _) =>
          ref.read(deploysProvider.notifier).applyRunPayload(data),
    ),
    'deploys:log': SseRoute(
      ephemeral: true,
      handle: (data, _, _) =>
          ref.read(deploysProvider.notifier).applyRunLinesPayload(data),
    ),
    'deploys:build': SseRoute(
      ephemeral: true,
      handle: (data, _, _) =>
          ref.read(deploysProvider.notifier).applyBuildPayload(data),
    ),
    'deploys:build-log': SseRoute(
      ephemeral: true,
      handle: (data, _, _) =>
          ref.read(deploysProvider.notifier).applyBuildLinesPayload(data),
    ),
    'deploys:updated': SseRoute(
      ephemeral: true,
      handle: (_, _, bus) => bus.scheduleRefresh(
        'deploys',
        () => ref.read(deploysProvider.notifier).refreshIfLoaded(),
      ),
    ),
    'registry:changed': SseRoute(
      ephemeral: true,
      handle: (_, _, bus) => bus.scheduleRefresh(
        'registry',
        () => ref.read(registryProvider.notifier).refreshIfLoaded(),
      ),
    ),
    'probe:status': SseRoute(
      ephemeral: false,
      handle: (data, _, bus) {
        ref.read(dockerProvider.notifier).applyAgentStatus(data);
        bus.scheduleRefresh(
          'deploys',
          () => ref.read(deploysProvider.notifier).refreshIfLoaded(),
        );
      },
    ),
  },
  // 5 rotas SSE de nível `core` (continuam ativas mesmo se a área `servers` for desligada — ADR 017).
  extraSseBuilders: <ProductArea, AreaSseRoutesBuilder>{
    ProductArea.core: (ref) => <String, SseRoute>{
      'probe:status': SseRoute(
        ephemeral: false,
        handle: (data, _, _) =>
            ref.read(agentsProvider.notifier).applyRealtimeStatus(data),
      ),
      'storages:updated': SseRoute(
        ephemeral: true,
        handle: (_, _, bus) => bus.scheduleRefresh(
          'storages',
          () => ref.read(storagesProvider.notifier).refreshIfLoaded(),
        ),
      ),
      'database_connections:updated': SseRoute(
        ephemeral: true,
        handle: (_, _, bus) => bus.scheduleRefresh(
          'database_connections',
          () => ref.read(databasesProvider.notifier).refreshIfLoaded(),
        ),
      ),
      'database_jobs:updated': SseRoute(
        ephemeral: true,
        handle: (data, _, _) =>
            ref.read(databasesProvider.notifier).applyJob(data),
      ),
      'backup:failed': SseRoute(
        ephemeral: false,
        handle: (_, _, bus) => bus.scheduleRefresh(
          'database_connections',
          () => ref.read(databasesProvider.notifier).refreshIfLoaded(),
        ),
      ),
    },
  },
);
