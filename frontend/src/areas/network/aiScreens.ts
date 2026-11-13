import type { AiScreenMeta } from '@/utils/aiScreen'

/** Como as telas da área Rede se apresentam ao chat da IA (`meta.aiScreen`). */
export const networkScreens = {
  sites: {
    id: 'sites',
    label: 'Sites',
    icon: 'mdi-map-marker-multiple-outline',
    suggestions: [
      {
        title: 'Saúde por site',
        prompt: 'Como está cada site agora? Algum com equipamentos fora ou alertas abertos?',
        icon: 'mdi-map-marker-alert-outline',
        color: 'primary',
      },
    ],
  },
  networks: {
    id: 'networks',
    label: 'Redes',
    icon: 'mdi-ip-network-outline',
    suggestions: [
      {
        title: 'Redes e DNS',
        prompt: 'Resuma as redes cadastradas, os sites e os servidores DNS em uso',
        icon: 'mdi-ip-network-outline',
        color: 'info',
      },
    ],
  },
  wifi: {
    id: 'wifi',
    label: 'Clientes Wi-Fi',
    icon: 'mdi-wifi',
    suggestions: [
      {
        title: 'Clientes com problema',
        prompt:
          'Quais clientes Wi-Fi estão com sinal fraco, ruído ou interferência, e o que fazer?',
        icon: 'mdi-wifi-alert',
        color: 'error',
      },
      {
        title: 'Aparelhos vigiados',
        prompt: 'Algum aparelho vigiado saiu do Wi-Fi ou trocou de roteador?',
        icon: 'mdi-eye-outline',
        color: 'warning',
      },
    ],
  },
  topology: {
    id: 'topology',
    label: 'Topologia',
    icon: 'mdi-sitemap',
    suggestions: [
      {
        title: 'Enlaces com problema',
        prompt: 'Algum enlace da topologia está caído, saturado ou desatualizado?',
        icon: 'mdi-transit-connection-variant',
        color: 'warning',
      },
    ],
  },
  'link-tests': {
    id: 'link-tests',
    label: 'Testes de velocidade',
    icon: 'mdi-speedometer',
    suggestions: [
      {
        title: 'Resultados recentes',
        prompt: 'Resuma os testes de velocidade recentes: algum enlace abaixo do esperado?',
        icon: 'mdi-speedometer-slow',
        color: 'info',
      },
    ],
  },
  discovery: {
    id: 'discovery',
    label: 'Descoberta',
    icon: 'mdi-radar',
    suggestions: [
      {
        title: 'O que foi achado',
        prompt: 'O que as varreduras recentes encontraram que ainda não está cadastrado?',
        icon: 'mdi-radar',
        color: 'primary',
      },
    ],
  },
  'plugin-app': {
    id: 'plugin-app',
    label: 'App de plugin',
    icon: 'mdi-apps',
  },
  plugins: {
    id: 'plugins',
    label: 'Plugins',
    icon: 'mdi-puzzle-outline',
    suggestions: [
      {
        title: 'Plugins disponíveis',
        prompt: 'Quais plugins estão instalados e o que cada um permite fazer nos equipamentos?',
        icon: 'mdi-puzzle-outline',
        color: 'primary',
      },
    ],
  },
} satisfies Record<string, AiScreenMeta>
