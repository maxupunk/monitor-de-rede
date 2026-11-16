import { defineArea, lazyComponent } from '../defineArea'
import { vpnScreens } from './aiScreens'
import { useVpnStore } from '@/stores/vpn'

/** VPN WireGuard: o servidor e os dispositivos que entram por ela. */
export const vpnArea = defineArea({
  id: 'vpn',
  routes: [
    {
      path: 'vpn',
      component: () => import('@/pages/vpn/VpnLayout.vue'),
      meta: { aiScreen: vpnScreens.vpn },
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
  serverConnectPaths: [
    {
      id: 'vpn',
      order: 20,
      title: 'Pela VPN',
      description:
        'Servidor em outra rede. Um script só sobe o túnel WireGuard até o IP público desta central e instala o agente, que passa a falar com ela apenas por dentro do túnel.',
      icon: 'mdi-shield-lock-outline',
      recommended: true,
      component: lazyComponent(() => import('@/components/VpnServerConnect.vue')),
      moveExisting: { label: 'Mover para a VPN', icon: 'mdi-shield-lock-outline' },
    },
  ],
  sse: {
    'vpn:peers_updated': (data) => useVpnStore().applyRealtimePeers(data),
  },
})
