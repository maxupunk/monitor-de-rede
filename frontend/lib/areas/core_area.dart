import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/features/core/models/alert_models.dart';
import 'package:netmonitor_app/features/core/presentation/alerts/alerts_page.dart';
import 'package:netmonitor_app/features/core/presentation/audit/audit_page.dart';
import 'package:netmonitor_app/features/core/presentation/backup/backup_page.dart';
import 'package:netmonitor_app/features/core/presentation/dashboard/core_dashboard_widgets.dart';
import 'package:netmonitor_app/features/core/presentation/dashboard/dashboard_page.dart';
import 'package:netmonitor_app/features/core/presentation/device_tabs/device_core_tabs.dart';
import 'package:netmonitor_app/features/core/presentation/events/events_page.dart';
import 'package:netmonitor_app/features/core/presentation/logs/logs_page.dart';
import 'package:netmonitor_app/features/core/presentation/maintenance/maintenance_windows_page.dart';
import 'package:netmonitor_app/features/core/presentation/monitors/monitor_detail_page.dart';
import 'package:netmonitor_app/features/core/presentation/monitors/monitors_page.dart';
import 'package:netmonitor_app/features/core/presentation/onboarding/initial_setup_dialog.dart';
import 'package:netmonitor_app/features/core/presentation/scheduled_tasks/scheduled_tasks_page.dart';
import 'package:netmonitor_app/features/core/presentation/settings/core_settings_cards.dart';
import 'package:netmonitor_app/features/core/presentation/settings/settings_page.dart';
import 'package:netmonitor_app/features/core/presentation/users/users_page.dart';
import 'package:netmonitor_app/features/core/providers/alerts_provider.dart';
import 'package:netmonitor_app/features/core/providers/backup_provider.dart';
import 'package:netmonitor_app/features/core/providers/logs_provider.dart';
import 'package:netmonitor_app/features/core/providers/maintenance_windows_provider.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/features/core/providers/scheduled_tasks_provider.dart';

int _extractAlertId(Map<String, dynamic> data) =>
    ((data['id'] ?? data['alertEventId']) as num?)?.toInt() ?? 0;

bool _isScheduledTaskNews(Map<String, dynamic> data) {
  final status = data['status']?.toString();
  return status == 'failed' || status == 'skipped';
}

const Map<String, AiScreenContribution>
_coreAiScreens = <String, AiScreenContribution>{
  'dashboard': AiScreenContribution(
    id: 'dashboard',
    title: 'Dashboard',
    description:
        'Visão geral de saúde, monitores, alertas e eventos em tempo real.',
  ),
  'monitors': AiScreenContribution(
    id: 'monitors',
    title: 'Monitores',
    description: 'Monitores ICMP, TCP, HTTP/HTTPS, DNS, SNMP, SSH e TLS.',
  ),
  'monitor-detail': AiScreenContribution(
    id: 'monitor-detail',
    title: 'Detalhe do Monitor',
    description:
        'Latência, uptime, linha de base adaptativa e histórico do monitor.',
  ),
  'alerts': AiScreenContribution(
    id: 'alerts',
    title: 'Alertas',
    description: 'Incidentes ativos, análise de causa raiz e regras de alerta.',
  ),
  'events': AiScreenContribution(
    id: 'events',
    title: 'Eventos',
    description: 'Feed em tempo real SSE e histórico de eventos do sistema.',
  ),
  'logs': AiScreenContribution(
    id: 'logs',
    title: 'Logs Syslog',
    description: 'Logs recebidos via Syslog, padrões classificados e origens.',
  ),
  'maintenance-windows': AiScreenContribution(
    id: 'maintenance-windows',
    title: 'Janelas de Manutenção',
    description:
        'Janelas ativas, agendadas e encerradas para supressão de alertas.',
  ),
  'scheduled-tasks': AiScreenContribution(
    id: 'scheduled-tasks',
    title: 'Tarefas Agendadas',
    description: 'Rotinas automáticas periódicas e histórico de execuções.',
  ),
  'audit': AiScreenContribution(
    id: 'audit',
    title: 'Trilha de Auditoria',
    description: 'Registro de alterações, logins e execuções administrativas.',
  ),
};

/// Área Núcleo (ADR 017): nunca desliga.
///
/// Worker dono: Core (`lib/areas/core_area.dart` + `lib/features/core/`).
final AreaContribution coreArea = AreaContribution(
  id: ProductArea.core,
  routes: <RouteBase>[
    GoRoute(
      path: '/',
      name: 'dashboard',
      builder: (context, state) => const DashboardPage(),
    ),
    GoRoute(
      path: '/monitors',
      name: 'monitors',
      builder: (context, state) =>
          MonitorsPage(initialStatus: state.uri.queryParameters['status']),
    ),
    GoRoute(
      path: '/monitors/:id',
      name: 'monitor-detail',
      builder: (context, state) {
        final id = int.tryParse(state.pathParameters['id'] ?? '') ?? 0;
        return MonitorDetailPage(monitorId: id);
      },
    ),
    GoRoute(
      path: '/alerts',
      name: 'alerts',
      builder: (context, state) => AlertsPage(
        initialTab: state.uri.queryParameters['tab'],
        deviceId: int.tryParse(state.uri.queryParameters['deviceId'] ?? ''),
        ruleId: int.tryParse(state.uri.queryParameters['ruleId'] ?? ''),
        catalogKey: state.uri.queryParameters['catalog'],
      ),
    ),
    GoRoute(
      path: '/events',
      name: 'events',
      builder: (context, state) => const EventsPage(),
    ),
    GoRoute(
      path: '/logs',
      name: 'logs',
      builder: (context, state) => const LogsPage(),
    ),
    GoRoute(
      path: '/maintenance-windows',
      name: 'maintenance-windows',
      builder: (context, state) => const MaintenanceWindowsPage(),
    ),
    GoRoute(
      path: '/scheduled-tasks',
      name: 'scheduled-tasks',
      builder: (context, state) => const ScheduledTasksPage(),
    ),
    GoRoute(
      path: '/settings',
      name: 'settings',
      builder: (context, state) =>
          SettingsPage(initialTab: state.uri.queryParameters['tab']),
    ),
    GoRoute(
      path: '/users',
      name: 'users',
      builder: (context, state) => const UsersPage(),
    ),
    GoRoute(
      path: '/audit',
      name: 'audit',
      builder: (context, state) => const AuditPage(),
    ),
    GoRoute(
      path: '/backup',
      name: 'backup',
      builder: (context, state) =>
          BackupPage(initialTab: state.uri.queryParameters['tab']),
    ),
    GoRoute(path: '/storages', redirect: (_, _) => '/backup?tab=destinos'),
    GoRoute(path: '/databases', redirect: (_, _) => '/backup'),
  ],
  routeMeta: <String, AreaRouteMeta>{
    '/': AreaRouteMeta(
      area: ProductArea.core,
      aiScreen: _coreAiScreens['dashboard'],
    ),
    '/devices': const AreaRouteMeta(area: ProductArea.core),
    '/monitors': AreaRouteMeta(
      area: ProductArea.core,
      aiScreen: _coreAiScreens['monitors'],
    ),
    '/alerts': AreaRouteMeta(
      area: ProductArea.core,
      aiScreen: _coreAiScreens['alerts'],
    ),
    '/events': AreaRouteMeta(
      area: ProductArea.core,
      aiScreen: _coreAiScreens['events'],
    ),
    '/logs': AreaRouteMeta(
      area: ProductArea.core,
      aiScreen: _coreAiScreens['logs'],
    ),
    '/remote-servers': const AreaRouteMeta(area: ProductArea.core),
    '/maintenance-windows': AreaRouteMeta(
      area: ProductArea.core,
      aiScreen: _coreAiScreens['maintenance-windows'],
    ),
    '/scheduled-tasks': AreaRouteMeta(
      area: ProductArea.core,
      aiScreen: _coreAiScreens['scheduled-tasks'],
    ),
    '/ai-chat': const AreaRouteMeta(area: ProductArea.core, fullBleed: true),
    '/ai-benchmark': const AreaRouteMeta(area: ProductArea.core),
    '/settings': const AreaRouteMeta(area: ProductArea.core),
    '/users': const AreaRouteMeta(area: ProductArea.core, requiresAdmin: true),
    '/audit': AreaRouteMeta(
      area: ProductArea.core,
      requiresAdmin: true,
      aiScreen: _coreAiScreens['audit'],
    ),
    '/backup': const AreaRouteMeta(area: ProductArea.core, requiresAdmin: true),
  },
  nav: const <NavEntry>[
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.monitoring,
      order: 10,
      item: NavItemSpec(
        title: 'Dashboard',
        icon: Icons.dashboard_outlined,
        to: '/',
      ),
    ),
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.monitoring,
      order: 30,
      item: NavItemSpec(
        title: 'Dispositivos',
        icon: Icons.devices_outlined,
        to: '/devices',
      ),
    ),
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.monitoring,
      order: 40,
      item: NavItemSpec(
        title: 'Monitores',
        icon: Icons.monitor_heart_outlined,
        to: '/monitors',
      ),
    ),
    NavEntry(
      area: ProductArea.core,
      section: NavSectionId.monitoring,
      order: 50,
      item: NavItemSpec(
        title: 'Alertas',
        icon: Icons.notifications_outlined,
        to: '/alerts',
      ),
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
      item: NavItemSpec(
        title: 'Logs',
        icon: Icons.article_outlined,
        to: '/logs',
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
  widgets: buildCoreDashboardWidgets(),
  aiScreens: _coreAiScreens,
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
  settingsCards: buildCoreSettingsCards(),
  deviceDetailTabs: buildCoreDeviceDetailTabs(),
  shellOverlays: <WidgetBuilder>[(_) => const OnboardingShellOverlay()],
  sseBuilder: (ref) => <String, SseRoute>{
    'areas:updated': SseRoute(
      handle: (data, _, _) => ref.read(areasProvider.notifier).apply(data),
    ),
    'user:access_updated': SseRoute(
      handle: (data, _, _) => ref.read(authProvider.notifier).applyAccess(data),
    ),
    'user:menu_updated': SseRoute(
      handle: (data, _, _) =>
          ref.read(authProvider.notifier).applyMenuPrefs(data),
    ),
    'monitor:result': SseRoute(
      handle: (data, _, _) {
        final notifier = ref.read(monitorsProvider.notifier);
        notifier.applyRealtimeResult(data);
        notifier.applyRealtimeMetrics(data);
      },
    ),
    'metric:recorded': SseRoute(
      handle: (data, _, _) =>
          ref.read(monitorsProvider.notifier).applyRealtimeMetrics(data),
    ),
    'interface:traffic': SseRoute(
      ephemeral: true,
      handle: (data, _, _) =>
          ref.read(monitorsProvider.notifier).applyInterfaceTraffic(data),
    ),
    'alert:triggered': SseRoute(
      handle: (data, event, bus) {
        final notifier = ref.read(alertsProvider.notifier);
        bus.scheduleRefresh('alerts', notifier.fetchActiveAlerts);
        notifier.upsertAlertEvent(
          AlertEvent.fromJson(<String, dynamic>{
            ...data,
            'createdAt':
                data['createdAt'] ?? data['startedAt'] ?? event.timestamp,
          }),
        );
      },
    ),
    'alert:decider_triage': SseRoute(
      handle: (data, _, _) {
        final triage = data['deciderTriage'];
        if (triage is Map) {
          ref
              .read(alertsProvider.notifier)
              .applyDeciderTriage(
                _extractAlertId(data),
                Map<String, dynamic>.from(triage),
              );
        }
      },
    ),
    'alert:ai_summary': SseRoute(
      handle: (data, _, _) {
        final summary = data['aiSummary'];
        if (summary is Map) {
          ref
              .read(alertsProvider.notifier)
              .applyAiSummary(
                _extractAlertId(data),
                Map<String, dynamic>.from(summary),
              );
        }
      },
    ),
    'alert:updated': SseRoute(
      handle: (data, _, _) => ref
          .read(alertsProvider.notifier)
          .upsertAlertEvent(AlertEvent.fromJson(data)),
    ),
    'alert:resolved': SseRoute(
      handle: (data, _, _) => ref
          .read(alertsProvider.notifier)
          .patchAlertEvent(
            _extractAlertId(data),
            status: 'resolved',
            resolvedAt: data['resolvedAt'] as String?,
          ),
    ),
    'alert:acknowledged': SseRoute(
      handle: (data, _, _) => ref
          .read(alertsProvider.notifier)
          .patchAlertEvent(
            _extractAlertId(data),
            status: 'acknowledged',
            silencedUntil: data['silencedUntil'] as String?,
          ),
    ),
    'alert:silenced': SseRoute(
      handle: (data, _, _) => ref
          .read(alertsProvider.notifier)
          .patchAlertEvent(
            _extractAlertId(data),
            status: 'silenced',
            silencedUntil: data['silencedUntil'] as String?,
          ),
    ),
    'alert_rule:created': SseRoute(
      handle: (data, _, _) => ref
          .read(alertsProvider.notifier)
          .upsertAlertRule(AlertRule.fromJson(data)),
    ),
    'alert_rule:updated': SseRoute(
      handle: (data, _, _) => ref
          .read(alertsProvider.notifier)
          .upsertAlertRule(AlertRule.fromJson(data)),
    ),
    'alert_rule:deleted': SseRoute(
      handle: (data, _, _) {
        final id = (data['id'] as num?)?.toInt();
        if (id != null) ref.read(alertsProvider.notifier).removeAlertRule(id);
      },
    ),
    'logs:template_classified': SseRoute(
      handle: (data, _, _) =>
          ref.read(logsProvider.notifier).applyTemplateClassified(data),
    ),
    'maintenance_windows:updated': SseRoute(
      handle: (_, _, bus) {
        final notifier = ref.read(maintenanceWindowsProvider.notifier);
        bus.scheduleRefresh('maintenance_windows', notifier.fetchWindows);
      },
    ),
    'backup_plan:updated': SseRoute(
      ephemeral: true,
      handle: (_, _, bus) {
        final notifier = ref.read(backupProvider.notifier);
        bus.scheduleRefresh('backup_plan', notifier.refreshPlanIfLoaded);
      },
    ),
    'backup:failed': SseRoute(
      handle: (_, _, bus) {
        final notifier = ref.read(backupProvider.notifier);
        bus.scheduleRefresh('backup_plan', notifier.refreshPlanIfLoaded);
      },
    ),
    'scheduled_task:updated': SseRoute(
      isEphemeral: (data) => !_isScheduledTaskNews(data),
      handle: (data, _, _) =>
          ref.read(scheduledTasksProvider.notifier).applyUpdated(data),
    ),
  },
);
