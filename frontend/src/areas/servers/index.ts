import { defineArea, lazyComponent } from '../defineArea'
import { serversSse } from './sse'

/** Servidores: o Docker dos hosts (cada host é um agente, ADR 014). */
export const serversArea = defineArea({
  id: 'servers',
  routes: [
    {
      path: 'docker',
      component: () => import('@/pages/docker/DockerLayout.vue'),
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
  ],
  nav: [
    {
      section: 'servers',
      order: 10,
      item: { title: 'Docker', icon: 'mdi-docker', to: '/docker' },
    },
  ],
  widgets: {
    docker_summary: {
      component: lazyComponent(() => import('@/components/dashboard/DockerSummaryCard.vue')),
    },
  },
  sse: serversSse,
})
