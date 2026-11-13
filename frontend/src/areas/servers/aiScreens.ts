import type { AiScreenMeta } from '@/utils/aiScreen'

/** Como as telas da área Servidores se apresentam ao chat da IA (`meta.aiScreen`). */
export const serversScreens = {
  docker: {
    id: 'docker',
    label: 'Docker',
    icon: 'mdi-docker',
    suggestions: [
      {
        title: 'Containers com problema',
        prompt: 'Algum container deste host está parado, reiniciando ou usando recursos demais?',
        icon: 'mdi-cube-off-outline',
        color: 'error',
      },
      {
        title: 'Uso de recursos',
        prompt: 'Quais containers mais consomem CPU, memória e disco neste host?',
        icon: 'mdi-chart-donut',
        color: 'info',
      },
    ],
  },
  deploys: {
    id: 'deploys',
    label: 'Deploy contínuo',
    icon: 'mdi-rocket-launch-outline',
    suggestions: [
      {
        title: 'Último deploy',
        prompt: 'Como foram os últimos deploys? Algum falhou, e por quê?',
        icon: 'mdi-rocket-launch-outline',
        color: 'primary',
      },
    ],
  },
  registry: {
    id: 'registry',
    label: 'Registry',
    icon: 'mdi-package-variant-closed',
  },
} satisfies Record<string, AiScreenMeta>
