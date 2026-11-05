import { defineArea, lazyWidget, withWidgetConfig } from '../defineArea'
import { coreSse } from './sse'

/**
 * Núcleo: o que toda instalação tem — dispositivos, monitores, alertas, logs,
 * IA, agentes, rotinas, backup e administração. Nunca desliga.
 */
export const coreArea = defineArea({
  id: 'core',
  routes: [
    { path: '', name: 'dashboard', component: () => import('@/pages/DashboardPage.vue') },
    { path: 'devices', name: 'devices', component: () => import('@/pages/DevicesPage.vue') },
    {
      path: 'devices/:id',
      name: 'device-detail',
      component: () => import('@/pages/DeviceDetailPage.vue'),
    },
    {
      path: 'scheduled-tasks',
      name: 'scheduled-tasks',
      component: () => import('@/pages/ScheduledTasksPage.vue'),
    },
    { path: 'monitors', name: 'monitors', component: () => import('@/pages/MonitorsPage.vue') },
    {
      path: 'monitors/:id',
      name: 'monitor-detail',
      component: () => import('@/pages/MonitorDetailPage.vue'),
    },
    { path: 'alerts', name: 'alerts', component: () => import('@/pages/AlertsPage.vue') },
    {
      path: 'maintenance-windows',
      name: 'maintenance-windows',
      component: () => import('@/pages/MaintenanceWindowsPage.vue'),
    },
    { path: 'events', name: 'events', component: () => import('@/pages/EventsPage.vue') },
    { path: 'logs', name: 'logs', component: () => import('@/pages/LogsPage.vue') },
    {
      path: 'remote-servers',
      name: 'remote-servers',
      component: () => import('@/pages/remote-servers/RemoteServersPage.vue'),
    },
    { path: 'agents', redirect: '/remote-servers' },
    {
      path: 'ai-chat',
      name: 'ai-chat',
      component: () => import('@/pages/AiChatPage.vue'),
      meta: { fullBleed: true },
    },
    { path: 'settings', name: 'settings', component: () => import('@/pages/SettingsPage.vue') },
    {
      path: 'ai-benchmark',
      name: 'decider-benchmark',
      component: () => import('@/pages/DeciderBenchmarkPage.vue'),
    },
    {
      path: 'users',
      name: 'users',
      component: () => import('@/pages/UsersPage.vue'),
      meta: { requiresAdmin: true },
    },
    {
      path: 'audit',
      name: 'audit',
      component: () => import('@/pages/AuditPage.vue'),
      meta: { requiresAdmin: true },
    },
    {
      path: 'backup',
      name: 'backup',
      component: () => import('@/pages/BackupPage.vue'),
      meta: { requiresAdmin: true },
    },
    // Endereços antigos, de antes de o backup virar uma tela só.
    { path: 'storages', redirect: { name: 'backup', query: { tab: 'destinos' } } },
    { path: 'databases', redirect: { name: 'backup' } },
    // Destino do guard quando a rota pedida é de uma área desligada.
    {
      path: 'area-desligada/:area',
      name: 'area-off',
      component: () => import('@/pages/AreaOffPage.vue'),
    },
  ],
  nav: [
    {
      section: 'monitoring',
      order: 10,
      item: { title: 'Dashboard', icon: 'mdi-view-dashboard', to: '/' },
    },
    {
      section: 'monitoring',
      order: 30,
      item: { title: 'Dispositivos', icon: 'mdi-devices', to: '/devices' },
    },
    {
      section: 'monitoring',
      order: 40,
      item: { title: 'Monitores', icon: 'mdi-heart-pulse', to: '/monitors' },
    },
    {
      section: 'monitoring',
      order: 50,
      item: { title: 'Alertas', icon: 'mdi-bell-outline', to: '/alerts' },
    },
    {
      section: 'monitoring',
      order: 60,
      item: { title: 'Eventos', icon: 'mdi-history', to: '/events' },
    },
    {
      section: 'monitoring',
      order: 70,
      item: { title: 'Logs', icon: 'mdi-text-box-search-outline', to: '/logs' },
    },
    // Os agentes servem às duas áreas (Docker e coleta remota): são do núcleo.
    {
      section: 'servers',
      order: 20,
      item: { title: 'Servidores remotos', icon: 'mdi-server', to: '/remote-servers' },
    },
    {
      section: 'operations',
      order: 10,
      item: { title: 'Manutenção', icon: 'mdi-wrench-clock', to: '/maintenance-windows' },
    },
    {
      section: 'operations',
      order: 20,
      item: { title: 'Tarefas agendadas', icon: 'mdi-calendar-clock', to: '/scheduled-tasks' },
    },
    {
      section: 'admin',
      order: 10,
      item: {
        title: 'Usuários e acessos',
        icon: 'mdi-account-multiple-outline',
        to: '/users',
        admin: true,
      },
    },
    {
      section: 'admin',
      order: 20,
      item: {
        title: 'Trilha de auditoria',
        icon: 'mdi-shield-account-outline',
        to: '/audit',
        admin: true,
      },
    },
    {
      section: 'admin',
      order: 40,
      item: { title: 'Backup', icon: 'mdi-backup-restore', to: '/backup', admin: true },
    },
    {
      section: 'admin',
      order: 50,
      item: { title: 'Configurações', icon: 'mdi-cog', to: '/settings' },
    },
    {
      section: 'admin',
      order: 60,
      item: {
        title: 'Instalar aplicativo',
        icon: 'mdi-cellphone-arrow-down',
        action: 'installPwa',
        pwa: true,
      },
    },
  ],
  widgets: {
    stat_cards: {
      component: lazyWidget(() => import('@/components/dashboard/StatCardsWidget.vue')),
    },
    health_gauge: {
      component: lazyWidget(() => import('@/components/widgets/GaugeHealthWidget.vue')),
    },
    active_alerts: {
      component: lazyWidget(() => import('@/components/dashboard/ActiveAlertsWidget.vue')),
    },
    latency_time_series: {
      component: lazyWidget(() => import('@/components/widgets/LatencyTimeSeriesWidget.vue')),
      props: withWidgetConfig,
    },
    event_distribution: {
      component: lazyWidget(() => import('@/components/widgets/EventDistributionWidget.vue')),
    },
    unstable_targets: {
      component: lazyWidget(() => import('@/components/widgets/UnstableTargetsWidget.vue')),
    },
    events_feed: {
      component: lazyWidget(() => import('@/components/dashboard/EventsFeedWidget.vue')),
    },
    network_monitors: {
      component: lazyWidget(() => import('@/components/dashboard/NetworkMonitorsWidget.vue')),
    },
    cpu_usage: {
      component: lazyWidget(() => import('@/components/widgets/CpuUsageWidget.vue')),
      props: withWidgetConfig,
    },
    ram_usage: {
      component: lazyWidget(() => import('@/components/widgets/RamUsageWidget.vue')),
      props: withWidgetConfig,
    },
    binary_status: {
      component: lazyWidget(() => import('@/components/widgets/BinaryStatusWidget.vue')),
      props: withWidgetConfig,
    },
  },
  sse: coreSse,
})
