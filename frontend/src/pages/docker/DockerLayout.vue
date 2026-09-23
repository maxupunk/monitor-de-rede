<template>
  <div>
    <v-card variant="outlined" rounded="xl" class="mb-4 overflow-hidden">
      <div class="d-flex flex-wrap align-center pr-3">
        <v-tabs color="primary" show-arrows class="flex-grow-1">
          <v-tab
            v-for="item in tabs"
            :key="item.to"
            :to="{ path: item.to, query: hostQuery }"
            exact
          >
            <v-icon start>{{ item.icon }}</v-icon>
            {{ item.title }}
          </v-tab>
        </v-tabs>
        <v-select
          :model-value="docker.selectedHostKey"
          :items="hostItems"
          item-title="title"
          item-value="value"
          density="compact"
          variant="outlined"
          hide-details
          prepend-inner-icon="mdi-server"
          class="docker-host-select my-2 mx-2"
          aria-label="Host Docker"
          @update:model-value="changeHost"
        >
          <template #item="{ props: itemProps, item }">
            <v-list-item v-bind="itemProps" :title="item.title" :subtitle="item.subtitle">
              <template #prepend>
                <v-icon :color="item.online ? 'success' : 'error'" size="small">
                  mdi-circle
                </v-icon>
              </template>
            </v-list-item>
          </template>
        </v-select>
        <v-chip
          v-if="docker.available"
          color="success"
          size="small"
          variant="tonal"
          prepend-icon="mdi-access-point"
          class="docker-live-chip"
        >
          Tempo real
        </v-chip>
      </div>
    </v-card>
    <v-alert
      v-if="docker.selectedHost && !docker.selectedHost.online"
      type="warning"
      variant="tonal"
      class="mb-4"
      icon="mdi-lan-disconnect"
    >
      O servidor {{ docker.selectedHost.name }} não está conectado à central. O histórico continua
      disponível; inventário e ações voltam quando ele reconectar.
    </v-alert>
    <DockerUnavailableAlert
      v-else-if="docker.status && !docker.available"
      :reason="docker.status.reason || 'A Docker Engine não respondeu ao backend.'"
    ></DockerUnavailableAlert>
    <DockerOperationsPanel class="mb-4"></DockerOperationsPanel>
    <router-view />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import DockerOperationsPanel from '@/components/docker/DockerOperationsPanel.vue'
import DockerUnavailableAlert from '@/components/docker/DockerUnavailableAlert.vue'
import { LOCAL_HOST_KEY } from '@/services/dockerService'
import { useDockerStore } from '@/stores/docker'

const docker = useDockerStore()
const route = useRoute()
const router = useRouter()

const tabs = [
  { title: 'Visão geral', icon: 'mdi-view-dashboard-outline', to: '/docker' },
  { title: 'Containers', icon: 'mdi-cube-outline', to: '/docker/containers' },
  { title: 'Projetos', icon: 'mdi-file-cabinet', to: '/docker/compose' },
  { title: 'Volumes', icon: 'mdi-database-outline', to: '/docker/volumes' },
  { title: 'Redes', icon: 'mdi-lan', to: '/docker/networks' },
  { title: 'Imagens', icon: 'mdi-layers-outline', to: '/docker/images' },
  { title: 'Histórico', icon: 'mdi-chart-timeline-variant', to: '/docker/history' },
]

const hostQuery = computed(() =>
  docker.selectedHostKey === LOCAL_HOST_KEY ? {} : { host: docker.selectedHostKey }
)

const hostItems = computed(() => {
  const items = docker.hosts.map((host) => ({
    title: host.name,
    value: host.key,
    subtitle: host.kind === 'local' ? 'Docker desta central' : 'Servidor remoto',
    online: host.online,
  }))
  if (items.length === 0) {
    items.push({
      title: 'Este servidor',
      value: LOCAL_HOST_KEY,
      subtitle: 'Docker desta central',
      online: true,
    })
  }
  return items
})

function routeHost(): string {
  const value = route.query.host
  return typeof value === 'string' && value ? value : LOCAL_HOST_KEY
}

function changeHost(hostKey: string | null) {
  const key = hostKey || LOCAL_HOST_KEY
  const query = { ...route.query }
  if (key === LOCAL_HOST_KEY) {
    delete query.host
  } else {
    query.host = key
  }
  router.replace({ path: route.path, query })
}

watch(
  () => route.query.host,
  () => {
    const key = routeHost()
    if (key !== docker.selectedHostKey) docker.selectHost(key)
  },
  { immediate: true }
)

onMounted(() => {
  docker.fetchHosts()
})
</script>

<style scoped>
.docker-live-chip {
  flex: 0 0 auto;
}

.docker-host-select {
  flex: 0 0 240px;
  max-width: 280px;
}

@media (max-width: 700px) {
  .docker-live-chip {
    display: none;
  }

  .docker-host-select {
    flex: 1 1 100%;
    max-width: none;
  }
}
</style>
