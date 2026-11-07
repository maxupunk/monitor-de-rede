import { defineArea } from '../defineArea'
import { useVpnStore } from '@/stores/vpn'

/** VPN WireGuard: o servidor e os dispositivos que entram por ela. */
export const vpnArea = defineArea({
  id: 'vpn',
  routes: [
    {
      path: 'vpn',
      component: () => import('@/pages/vpn/VpnLayout.vue'),
      children: [
        {
          path: '',
          name: 'vpn-server',
          component: () => import('@/pages/vpn/VpnServerPage.vue'),
        },
        {
          path: 'devices',
          name: 'vpn-devices',
          component: () => import('@/pages/vpn/VpnDevicesPage.vue'),
        },
      ],
    },
  ],
  nav: [
    {
      section: 'vpn',
      order: 10,
      item: { title: 'VPN WireGuard', icon: 'mdi-shield-lock-outline', to: '/vpn' },
    },
  ],
  sse: {
    'vpn:peers_updated': (data) => useVpnStore().applyRealtimePeers(data),
  },
})
