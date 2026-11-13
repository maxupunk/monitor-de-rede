import { defineArea, lazyComponent } from '../defineArea'
import { serversSse } from './sse'

/**
 * Servidores: o Docker dos hosts (cada host é um agente, ADR 014) e o deploy
 * contínuo das stacks deles (ADR 021).
 */
export const serversArea = defineArea({
  id: 'servers',
  routes: [
    {
      path: 'docker',
      component: () => import('@/pages/docker/DockerLayout.vue'),
      meta: { aiScreen: { id: 'docker', label: 'Docker', icon: 'mdi-docker' } },
      children: [
        {
          path: '',
          name: 'docker-overview',
          component: () => import('@/pages/docker/DockerOverviewPage.vue'),
        },
        {
          path: 'containers',
          name: 'docker-containers',
          component: () => import('@/pages/docker/DockerContainersPage.vue'),
        },
        {
          path: 'volumes',
          name: 'docker-volumes',
          component: () => import('@/pages/docker/DockerVolumesPage.vue'),
        },
        {
          path: 'networks',
          name: 'docker-networks',
          component: () => import('@/pages/docker/DockerNetworksPage.vue'),
        },
        {
          path: 'images',
          name: 'docker-images',
          component: () => import('@/pages/docker/DockerImagesPage.vue'),
        },
        {
          path: 'compose',
          name: 'docker-compose',
          component: () => import('@/pages/docker/DockerComposePage.vue'),
        },
        {
          path: 'history',
          name: 'docker-history',
          component: () => import('@/pages/docker/DockerHistoryPage.vue'),
        },
      ],
    },
    {
      path: 'deploys',
      name: 'deploys',
      component: () => import('@/pages/deploys/DeploysPage.vue'),
      meta: {
        aiScreen: { id: 'deploys', label: 'Deploy contínuo', icon: 'mdi-rocket-launch-outline' },
      },
    },
    {
      path: 'registry',
      name: 'registry',
      component: () => import('@/pages/registry/RegistryPage.vue'),
      meta: { aiScreen: { id: 'registry', label: 'Registry', icon: 'mdi-package-variant-closed' } },
    },
    {
      // A volta do GitHub ao criar e ao instalar o app (o manifest aponta
      // para cá): a página de deploys conclui pela query.
      path: 'deploys/github',
      redirect: (to) => ({ name: 'deploys', query: to.query }),
    },
  ],
  nav: [
    {
      section: 'servers',
      order: 10,
      item: { title: 'Docker', icon: 'mdi-docker', to: '/docker' },
    },
    {
      section: 'servers',
      order: 15,
      item: { title: 'Deploys', icon: 'mdi-rocket-launch-outline', to: '/deploys' },
    },
    {
      section: 'servers',
      order: 16,
      item: { title: 'Registry', icon: 'mdi-package-variant-closed', to: '/registry' },
    },
  ],
  widgets: {
    docker_summary: {
      component: lazyComponent(() => import('@/components/dashboard/DockerSummaryCard.vue')),
    },
  },
  sse: serversSse,
})
