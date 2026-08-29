import { createRouter, createWebHistory } from 'vue-router'
import { useAuthStore } from '@/stores/auth'

const routes = [
  {
    path: '/login',
    name: 'login',
    component: () => import('../pages/LoginPage.vue'),
    meta: { public: true },
  },
  {
    path: '/setup',
    name: 'setup',
    component: () => import('../pages/SetupPage.vue'),
    meta: { public: true },
  },
  {
    path: '/',
    component: () => import('../layouts/DefaultLayout.vue'),
    children: [
      { path: '', name: 'dashboard', component: () => import('../pages/DashboardPage.vue') },
      { path: 'sites', name: 'sites', component: () => import('../pages/SitesPage.vue') },
      { path: 'networks', name: 'networks', component: () => import('../pages/NetworksPage.vue') },
      { path: 'devices', name: 'devices', component: () => import('../pages/DevicesPage.vue') },
      {
        path: 'devices/:id',
        name: 'device-detail',
        component: () => import('../pages/DeviceDetailPage.vue'),
      },
      {
        path: 'topology',
        name: 'topology',
        component: () => import('../pages/TopologyPage.vue'),
        meta: { fullBleed: true },
      },
      {
        path: 'discovery',
        name: 'discovery',
        component: () => import('../pages/DiscoveryPage.vue'),
      },
      { path: 'monitors', name: 'monitors', component: () => import('../pages/MonitorsPage.vue') },
      {
        path: 'monitors/:id',
        name: 'monitor-detail',
        component: () => import('../pages/MonitorDetailPage.vue'),
      },
      { path: 'alerts', name: 'alerts', component: () => import('../pages/AlertsPage.vue') },
      {
        path: 'maintenance-windows',
        name: 'maintenance-windows',
        component: () => import('../pages/MaintenanceWindowsPage.vue'),
      },
      { path: 'events', name: 'events', component: () => import('../pages/EventsPage.vue') },
      { path: 'logs', name: 'logs', component: () => import('../pages/LogsPage.vue') },
      { path: 'probes', name: 'probes', component: () => import('../pages/ProbesPage.vue') },
      {
        path: 'vpn',
        name: 'vpn-server',
        component: () => import('../pages/vpn/VpnServerPage.vue'),
      },
      {
        path: 'vpn/devices',
        name: 'vpn-devices',
        component: () => import('../pages/vpn/VpnDevicesPage.vue'),
      },
      { path: 'settings', name: 'settings', component: () => import('../pages/SettingsPage.vue') },
      {
        path: 'users',
        name: 'users',
        component: () => import('../pages/UsersPage.vue'),
        meta: { requiresAdmin: true },
      },
      {
        path: 'audit',
        name: 'audit',
        component: () => import('../pages/AuditPage.vue'),
        meta: { requiresAdmin: true },
      },
    ],
  },
]

export const router = createRouter({
  history: createWebHistory(),
  routes,
})

/**
 * Três destinos possíveis para quem chega, nesta ordem:
 *
 * 1. **Autenticado** — segue para onde pediu; login e cadastro inicial viram
 *    dashboard, porque não há o que fazer neles com sessão aberta.
 * 2. **Instalação pendente** (banco sem nenhum usuário) — vai para `/setup`.
 *    Mandar para o login seria pedir uma credencial que ainda não existe, que
 *    é exatamente o beco sem saída de uma instalação nova.
 * 3. **Resto** — login, guardando o destino original em `?redirect` para
 *    devolver o operador ao lugar certo depois de entrar.
 */
router.beforeEach(async (to) => {
  const auth = useAuthStore()

  if (auth.isAuthenticated) {
    if (to.name === 'login' || to.name === 'setup') return { name: 'dashboard' }
    if (to.meta.requiresAdmin && !auth.isAdmin) {
      await auth.fetchMe()
      if (!auth.isAdmin) return { name: 'dashboard' }
    }
    return true
  }

  if (await auth.ensureSetupStatus()) {
    return to.name === 'setup' ? true : { name: 'setup' }
  }

  // Instalação concluída: `/setup` não tem mais função.
  if (to.name === 'setup') return { name: 'login' }
  if (!to.meta.public) return { name: 'login', query: { redirect: to.fullPath } }
  return true
})
