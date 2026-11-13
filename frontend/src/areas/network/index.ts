import { defineArea, lazyComponent, withWidgetConfig } from '../defineArea'
import type { NavApp, NavItemSpec } from '@/navigation'
import { networkSse } from './sse'

/** Plugins com página de frota viram um grupo do menu. */
function appsGroup(apps: NavApp[]): NavItemSpec | null {
  if (apps.length === 0) return null
  return {
    title: 'Aplicativos',
    icon: 'mdi-apps',
    children: apps.map((app) => ({ title: app.title, icon: app.icon, to: '/apps/' + app.id })),
  }
}

/** Rede: topologia, descoberta, redes IP, sites, enlaces, Wi-Fi, sondas e plugins. */
export const networkArea = defineArea({
  id: 'network',
  routes: [
    {
      path: 'sites',
      name: 'sites',
      component: () => import('@/pages/SitesPage.vue'),
      meta: { aiScreen: { id: 'sites', label: 'Sites', icon: 'mdi-map-marker-multiple-outline' } },
    },
    {
      path: 'networks',
      name: 'networks',
      component: () => import('@/pages/NetworksPage.vue'),
      meta: { aiScreen: { id: 'networks', label: 'Redes', icon: 'mdi-ip-network-outline' } },
    },
    {
      path: 'wifi',
      name: 'wifi-clients',
      component: () => import('@/pages/WifiClientsPage.vue'),
      meta: { aiScreen: { id: 'wifi', label: 'Clientes Wi-Fi', icon: 'mdi-wifi' } },
    },
    {
      path: 'topology',
      name: 'topology',
      component: () => import('@/pages/TopologyPage.vue'),
      meta: {
        fullBleed: true,
        aiScreen: { id: 'topology', label: 'Topologia', icon: 'mdi-sitemap' },
      },
    },
    {
      path: 'link-tests',
      name: 'link-tests',
      component: () => import('@/pages/LinkTestsPage.vue'),
      meta: {
        aiScreen: { id: 'link-tests', label: 'Testes de velocidade', icon: 'mdi-speedometer' },
      },
    },
    {
      path: 'discovery',
      name: 'discovery',
      component: () => import('@/pages/DiscoveryPage.vue'),
      meta: { aiScreen: { id: 'discovery', label: 'Descoberta', icon: 'mdi-radar' } },
    },
    { path: 'probes', name: 'probes', component: () => import('@/pages/ProbesPage.vue') },
    {
      path: 'apps/:id',
      name: 'plugin-app',
      component: () => import('@/pages/PluginAppPage.vue'),
      meta: { aiScreen: { id: 'plugin-app', label: 'App de plugin', icon: 'mdi-apps' } },
    },
    {
      path: 'plugins',
      name: 'plugins',
      component: () => import('@/pages/PluginsPage.vue'),
      meta: {
        requiresAdmin: true,
        aiScreen: { id: 'plugins', label: 'Plugins', icon: 'mdi-puzzle-outline' },
      },
    },
  ],
  nav: [
    {
      section: 'monitoring',
      order: 20,
      item: { title: 'Topologia', icon: 'mdi-sitemap', to: '/topology' },
    },
    {
      section: 'network',
      order: 10,
      item: { title: 'Descoberta', icon: 'mdi-radar', to: '/discovery' },
    },
    {
      section: 'network',
      order: 20,
      item: { title: 'Redes IP', icon: 'mdi-ip-network-outline', to: '/networks' },
    },
    { section: 'network', order: 30, item: { title: 'Sites', icon: 'mdi-domain', to: '/sites' } },
    {
      section: 'network',
      order: 40,
      item: { title: 'Testes de velocidade', icon: 'mdi-speedometer', to: '/link-tests' },
    },
    {
      section: 'network',
      order: 50,
      item: { title: 'Sinais Wi-Fi', icon: 'mdi-wifi', to: '/wifi' },
    },
    {
      section: 'network',
      order: 60,
      item: { title: 'Sondas de rede', icon: 'mdi-access-point-network', to: '/probes' },
    },
    {
      section: 'network',
      order: 70,
      item: {
        title: 'Diagnóstico',
        icon: 'mdi-stethoscope',
        children: [
          { title: 'Playbooks', icon: 'mdi-clipboard-play-outline', action: 'playbook' },
          { title: 'Traceroute ICMP', icon: 'mdi-routes', action: 'traceroute' },
          {
            // "Testes de velocidade" mede entre equipamentos; este mede do navegador.
            title: 'Velocidade do navegador',
            icon: 'mdi-speedometer-medium',
            action: 'speedTest',
          },
        ],
      },
    },
    { section: 'network', order: 80, item: (ctx) => appsGroup(ctx.apps) },
    {
      section: 'admin',
      order: 30,
      item: {
        title: 'Plugins de dispositivo',
        icon: 'mdi-puzzle-outline',
        to: '/plugins',
        admin: true,
      },
    },
  ],
  widgets: {
    dns_latency: { component: lazyComponent(() => import('@/components/DnsLatencyCard.vue')) },
    saas_services: {
      component: lazyComponent(() => import('@/components/widgets/SaasServicesWidget.vue')),
    },
    saas_heatmap: {
      component: lazyComponent(() => import('@/components/widgets/SaasLatencyHeatmapWidget.vue')),
      props: (widget) => ({
        monitorId:
          typeof widget.config?.monitorId === 'number' ? widget.config.monitorId : undefined,
      }),
    },
    ether_bandwidth: {
      component: lazyComponent(() => import('@/components/widgets/EtherBandwidthWidget.vue')),
      props: withWidgetConfig,
    },
    bandwidth_vs_latency: {
      component: lazyComponent(() => import('@/components/widgets/BandwidthVsLatencyWidget.vue')),
      props: withWidgetConfig,
    },
  },
  sse: networkSse,
  settingsTabs: [{ value: 'rede', title: 'Rede', icon: 'mdi-lan', order: 30 }],
  settingsCards: [
    {
      tab: 'rede',
      order: 10,
      component: lazyComponent(() => import('@/components/settings/VendorRegistryCard.vue')),
    },
  ],
})
