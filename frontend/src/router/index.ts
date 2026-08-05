import { createRouter, createWebHistory } from 'vue-router'

const routes = [
  {
    path: '/login',
    name: 'login',
    component: () => import('../pages/LoginPage.vue'),
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
      { path: 'topology', name: 'topology', component: () => import('../pages/TopologyPage.vue') },
      {
        path: 'zabbix-templates',
        name: 'zabbix-templates',
        component: () => import('../pages/ZabbixTemplatesPage.vue'),
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
      { path: 'events', name: 'events', component: () => import('../pages/EventsPage.vue') },
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
    ],
  },
]

export const router = createRouter({
  history: createWebHistory(),
  routes,
})
