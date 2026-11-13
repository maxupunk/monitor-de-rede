import type { AiScreenMeta } from '@/utils/aiScreen'

/** Como as telas da área VPN se apresentam ao chat da IA (`meta.aiScreen`). */
export const vpnScreens = {
  vpn: {
    id: 'vpn',
    label: 'VPN',
    icon: 'mdi-shield-lock-outline',
    suggestions: [
      {
        title: 'Peers da VPN',
        prompt: 'Algum peer da VPN está sem handshake recente ou sem tráfego?',
        icon: 'mdi-shield-alert-outline',
        color: 'warning',
      },
    ],
  },
} satisfies Record<string, AiScreenMeta>
