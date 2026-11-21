import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';

/// Área Núcleo (ADR 017): nunca desliga.
///
/// Worker dono: Core (`lib/areas/core_area.dart` + `lib/features/core/`).
/// Substitua os `FeaturePlaceholderPage` pelas telas reais e acrescente os
/// handlers SSE, widgets de dashboard e cartões de Configurações do núcleo.
final AreaContribution coreArea = AreaContribution(
  id: ProductArea.core,
  routes: <RouteBase>[
    GoRoute(
      path: '/',
      name: 'dashboard',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Dashboard',
        area: ProductArea.core,
        icon: Icons.dashboard_outlined,
      ),
    ),
    GoRoute(
      path: '/devices',
      name: 'devices',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Dispositivos',
        area: ProductArea.core,
        icon: Icons.devices_outlined,
      ),
    ),
    GoRoute(
      path: '/devices/:id',
      name: 'device-detail',
      builder: (context, state) => FeaturePlaceholderPage(
        title: 'Dispositivo #${state.pathParameters['id'] ?? ''}',
        area: ProductArea.core,
        icon: Icons.router_outlined,
      ),
    ),
    GoRoute(
      path: '/monitors',
      name: 'monitors',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Monitores',
        area: ProductArea.core,
        icon: Icons.monitor_heart_outlined,
      ),
    ),
    GoRoute(
      path: '/monitors/:id',
      name: 'monitor-detail',
      builder: (context, state) => FeaturePlaceholderPage(
        title: 'Monitor #${state.pathParameters['id'] ?? ''}',
        area: ProductArea.core,
        icon: Icons.monitor_heart_outlined,
      ),
    ),
    GoRoute(
      path: '/alerts',
      name: 'alerts',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Alertas',
        area: ProductArea.core,
        icon: Icons.notifications_active_outlined,
      ),
    ),
    GoRoute(
      path: '/events',
      name: 'events',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Eventos',
        area: ProductArea.core,
        icon: Icons.history,
      ),
    ),
    GoRoute(
      path: '/logs',
      name: 'logs',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Logs',
        area: ProductArea.core,
        icon: Icons.article_outlined,
      ),
    ),
    GoRoute(
      path: '/remote-servers',
      name: 'remote-servers',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Servidores remotos',
        area: ProductArea.core,
        icon: Icons.dns_outlined,
      ),
    ),
    GoRoute(
      path: '/agents',
      redirect: (_, _) => '/remote-servers',
    ),
    GoRoute(
      path: '/maintenance-windows',
      name: 'maintenance-windows',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Manutenção',
        area: ProductArea.core,
        icon: Icons.build_circle_outlined,
      ),
    ),
    GoRoute(
      path: '/scheduled-tasks',
      name: 'scheduled-tasks',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Tarefas agendadas',
        area: ProductArea.core,
        icon: Icons.event_repeat_outlined,
      ),
    ),
    GoRoute(
      path: '/ai-chat',
      name: 'ai-chat',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Assistente IA',
        area: ProductArea.core,
        icon: Icons.smart_toy_outlined,
      ),
    ),
    GoRoute(
      path: '/ai-benchmark',
      name: 'decider-benchmark',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Benchmark IA',
        area: ProductArea.core,
        icon: Icons.speed_outlined,
      ),
    ),
    GoRoute(
      path: '/settings',
      name: 'settings',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Configurações',
        area: ProductArea.core,
        icon: Icons.settings_outlined,
      ),
    ),
    GoRoute(
      path: '/users',
      name: 'users',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Usuários e acessos',
        area: ProductArea.core,
        icon: Icons.people_outline,
      ),
    ),
    GoRoute(
      path: '/audit',
      name: 'audit',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Trilha de auditoria',
        area: ProductArea.core,
        icon: Icons.verified_user_outlined,
      ),
    ),
    GoRoute(
      path: '/backup',
      name: 'backup',
      builder: (context, state) => const FeaturePlaceholderPage(
        title: 'Backup',
        area: ProductArea.core,
        icon: Icons.settings_backup_restore,
      ),
    ),
    GoRoute(
      path: '/storages',
      redirect: (_, _) => '/backup?tab=destinos',
    ),
    GoRoute(
      path: '/databases',
      redirect: (_, _) => '/backup',
    ),
  ],
  routeMeta: const <String, AreaRouteMeta>{
    '/': AreaRouteMeta(area: ProductArea.core),
    '/devices': AreaRouteMeta(area: ProductArea.core),
    '/monitors': AreaRouteMeta(area: ProductArea.core),
    '/alerts': AreaRouteMeta(area: ProductArea.core),
    '/events': AreaRouteMeta(area: ProductArea.core),
    '/logs': AreaRouteMeta(area: ProductArea.core),
    '/remote-servers': AreaRouteMeta(area: ProductArea.core),
    '/maintenance-windows': AreaRouteMeta(area: ProductArea.core),
    '/scheduled-tasks': AreaRouteMeta(area: ProductArea.core),
    '/ai-chat': AreaRouteMeta(area: ProductArea.core, fullBleed: true),
    '/ai-benchmark': AreaRouteMeta(area: ProductArea.core),
    '/settings': AreaRouteMeta(area: ProductArea.core),
    '/users': AreaRouteMeta(area: ProductArea.core, requiresAdmin: true),
    '/audit': AreaRouteMeta(area: ProductArea.core, requiresAdmin: true),
    '/backup': AreaRouteMeta(area: ProductArea.core, requiresAdmin: true),
  },
  nav: const <NavEntry>[
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.monitoring,
      order: 10,
      item: NavItemSpec(title: 'Dashboard', icon: Icons.dashboard_outlined, to: '/'),
    ),
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.monitoring,
      order: 30,
      item: NavItemSpec(title: 'Dispositivos', icon: Icons.devices_outlined, to: '/devices'),
    ),
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.monitoring,
      order: 40,
      item: NavItemSpec(title: 'Monitores', icon: Icons.monitor_heart_outlined, to: '/monitors'),
    ),
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.monitoring,
      order: 50,
      item: NavItemSpec(title: 'Alertas', icon: Icons.notifications_outlined, to: '/alerts'),
    ),
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.monitoring,
      order: 60,
      item: NavItemSpec(title: 'Eventos', icon: Icons.history, to: '/events'),
    ),
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.monitoring,
      order: 70,
      item: NavItemSpec(title: 'Logs', icon: Icons.article_outlined, to: '/logs'),
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
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.operations,
      order: 10,
      item: NavItemSpec(
        title: 'Manutenção',
        icon: Icons.build_circle_outlined,
        to: '/maintenance-windows',
      ),
    ),
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.operations,
      order: 20,
      item: NavItemSpec(
        title: 'Tarefas agendadas',
        icon: Icons.event_repeat_outlined,
        to: '/scheduled-tasks',
      ),
    ),
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.admin,
      order: 10,
      item: NavItemSpec(
        title: 'Usuários e acessos',
        icon: Icons.people_outline,
        to: '/users',
        admin: true,
      ),
    ),
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.admin,
      order: 20,
      item: NavItemSpec(
        title: 'Trilha de auditoria',
        icon: Icons.verified_user_outlined,
        to: '/audit',
        admin: true,
      ),
    ),
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.admin,
      order: 40,
      item: NavItemSpec(
        title: 'Backup',
        icon: Icons.settings_backup_restore,
        to: '/backup',
        admin: true,
      ),
    ),
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.admin,
      order: 50,
      item: NavItemSpec(
        title: 'Configurações',
        icon: Icons.settings_outlined,
        to: '/settings',
      ),
    ),
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.admin,
      order: 60,
      item: NavItemSpec(
        title: 'Instalar aplicativo',
        icon: Icons.install_mobile_outlined,
        action: NavAction.installPwa,
        pwa: true,
      ),
    ),
  ],
  serverConnectPaths: const <ServerConnectPathContribution>[
    ServerConnectPathContribution(
      id: 'lan',
      order: 10,
      title: 'Na mesma rede',
      description:
          'O servidor está na rede local desta central. Conexão direta, sem VPN e sem passar pela internet.',
      icon: Icons.lan_outlined,
      addressKinds: <String>['lan'],
    ),
    ServerConnectPathContribution(
      id: 'internet',
      order: 30,
      title: 'Pela internet (HTTPS)',
      description:
          'Sem VPN: o servidor disca para o domínio desta central. Use só com HTTPS — é ele que cifra o canal.',
      icon: Icons.public,
      addressKinds: <String>['domain', 'public', 'custom'],
    ),
  ],
  settingsTabs: const <SettingsTabContribution>[
    SettingsTabContribution(
      value: 'geral',
      title: 'Geral',
      icon: Icons.tune,
      order: 10,
    ),
    SettingsTabContribution(
      value: 'conectividade',
      title: 'Conectividade',
      icon: Icons.settings_ethernet,
      order: 20,
    ),
    SettingsTabContribution(
      value: 'notificacoes',
      title: 'Notificações',
      icon: Icons.notifications_outlined,
      order: 40,
    ),
    SettingsTabContribution(
      value: 'ia',
      title: 'Inteligência artificial',
      icon: Icons.smart_toy_outlined,
      order: 50,
    ),
    SettingsTabContribution(
      value: 'sistema',
      title: 'Sistema',
      icon: Icons.storage_outlined,
      order: 60,
    ),
  ],
  sseBuilder: (ref) => <String, SseRoute>{
    'areas:updated': SseRoute(
      handle: (data, _, _) => ref.read(areasProvider.notifier).apply(data),
    ),
    'user:access_updated': SseRoute(
      handle: (data, _, _) => ref.read(authProvider.notifier).applyAccess(data),
    ),
    'user:menu_updated': SseRoute(
      handle: (data, _, _) => ref.read(authProvider.notifier).applyMenuPrefs(data),
    ),
  },
);
