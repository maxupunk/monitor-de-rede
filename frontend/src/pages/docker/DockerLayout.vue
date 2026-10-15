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
      </div>
    </v-card>
    <v-alert
      v-if="docker.hostsLoaded && docker.hosts.length === 0"
      type="info"
      variant="tonal"
      class="mb-4"
      icon="mdi-docker"
    >
      Nenhum host Docker. O Docker deste servidor é gerenciado pelo agente local (serviço
      <code>netmonitor-agent</code> do <code>docker-compose.yml</code>); servidores remotos entram
      pela tela de agentes.
    </v-alert>
    <v-alert
      v-else-if="docker.selectedHost && !docker.selectedHost.online"
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
      :reason="docker.status.reason || 'A Docker Engine não respondeu ao agente.'"
      :local="docker.selectedHost?.kind === 'local'"
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

/** O host padrão (o desta central) não vai na URL; os demais, sim. */
const hostQuery = computed(() =>
  docker.selectedHostKey && docker.selectedHostKey !== docker.defaultHostKey
    ? { host: docker.selectedHostKey }
    : {}
)

const hostItems = computed(() =>
  docker.hosts.map((host) => ({
    title: host.name,
    value: host.key,
    subtitle: host.kind === 'local' ? 'Docker desta central' : 'Servidor remoto',
    online: host.online,
  }))
)

function routeHost(): string {
  const value = route.query.host
  return typeof value === 'string' ? value : ''
}

function changeHost(hostKey: string | null) {
  const query = { ...route.query }
  if (!hostKey || hostKey === docker.defaultHostKey) {
    delete query.host
  } else {
    query.host = hostKey
  }
  router.replace({ path: route.path, query })
}

/** A URL manda; sem `host`, o padrão — que só se conhece com a lista carregada. */
function syncHost() {
  const key = routeHost() || docker.defaultHostKey
  if (key && key !== docker.selectedHostKey) docker.selectHost(key)
}

watch(() => route.query.host, syncHost)

onMounted(async () => {
  await docker.fetchHosts()
  syncHost()
})
</script>

<style scoped>
.docker-host-select {
  flex: 0 0 240px;
  max-width: 280px;
}

@media (max-width: 700px) {
  .docker-host-select {
    flex: 1 1 100%;
    max-width: none;
  }
}
</style>
