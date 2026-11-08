<template>
  <div>
    <!-- Cabeçalho único: de qual host se trata, como a Engine está e as abas. -->
    <v-card elevation="2" rounded="lg" class="mb-4 overflow-hidden">
      <div class="d-flex flex-wrap align-center ga-3 pa-3 pa-md-4">
        <v-avatar color="primary" variant="tonal" size="44" class="flex-shrink-0">
          <v-icon size="26">mdi-docker</v-icon>
        </v-avatar>
        <div class="flex-grow-1 min-w-0">
          <h1 class="text-h6 text-md-h5 font-weight-bold ma-0">Docker</h1>
          <div class="d-flex flex-wrap align-center ga-2 mt-1">
            <v-chip
              v-for="chip in statusChips"
              :key="chip.label"
              size="small"
              :color="chip.color"
              :prepend-icon="chip.icon"
              variant="tonal"
            >
              {{ chip.label }}
            </v-chip>
            <LiveIndicator :updated-at="docker.lastUpdatedAt" />
          </div>
        </div>
        <div class="docker-header__actions d-flex flex-wrap align-center ga-2">
          <v-select
            :model-value="docker.selectedHostKey"
            :items="hostItems"
            item-title="title"
            item-value="value"
            label="Host"
            density="compact"
            variant="outlined"
            hide-details
            prepend-inner-icon="mdi-server"
            class="docker-host-select"
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
          <v-btn
            color="primary"
            variant="flat"
            prepend-icon="mdi-refresh"
            :loading="docker.loading"
            :disabled="!docker.selectedHost?.online"
            @click="docker.refreshAll()"
          >
            Atualizar
          </v-btn>
        </div>
      </div>
      <v-divider></v-divider>
      <v-tabs color="primary" show-arrows density="comfortable">
        <v-tab
          v-for="item in tabs"
          :key="item.to"
          :to="{ path: item.to, query: docker.hostQuery }"
          :prepend-icon="item.icon"
          exact
        >
          {{ item.title }}
          <v-badge v-if="item.count != null" :content="item.count" color="primary" inline></v-badge>
        </v-tab>
      </v-tabs>
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
import LiveIndicator from '@/components/LiveIndicator.vue'
import { useDockerStore } from '@/stores/docker'
import { useLiveInterest } from '@/composables/useLiveInterest'

const docker = useDockerStore()
const route = useRoute()
const router = useRouter()

// Enquanto as telas Docker estiverem abertas, o backend mantém o snapshot ao
// vivo dos agentes (ARQ-05); ao sair, deixa de pedir.
useLiveInterest('docker')

/** Quantos itens a aba tem — só onde o número diz algo e a Engine respondeu. */
function count(value: number): number | null {
  return docker.available ? value : null
}

const tabs = computed(() => [
  { title: 'Visão geral', icon: 'mdi-view-dashboard-outline', to: '/docker', count: null },
  {
    title: 'Containers',
    icon: 'mdi-cube-outline',
    to: '/docker/containers',
    count: count(docker.containers.length),
  },
  { title: 'Projetos', icon: 'mdi-file-cabinet', to: '/docker/compose', count: null },
  {
    title: 'Volumes',
    icon: 'mdi-database-outline',
    to: '/docker/volumes',
    count: count(docker.volumes.length),
  },
  { title: 'Redes', icon: 'mdi-lan', to: '/docker/networks', count: count(docker.networks.length) },
  {
    title: 'Imagens',
    icon: 'mdi-layers-outline',
    to: '/docker/images',
    count: count(docker.images.length),
  },
  { title: 'Histórico', icon: 'mdi-chart-timeline-variant', to: '/docker/history', count: null },
])

/** O estado do host num relance: conexão, Engine e containers. */
const statusChips = computed(() => {
  const host = docker.selectedHost
  if (!host) return []
  if (!host.online) {
    return [{ label: 'Agente desconectado', color: 'error', icon: 'mdi-lan-disconnect' }]
  }
  if (docker.status && !docker.available) {
    return [{ label: 'Engine indisponível', color: 'warning', icon: 'mdi-alert' }]
  }
  const status = docker.status
  return [
    {
      label: host.kind === 'local' ? 'Docker desta central' : 'Servidor remoto',
      color: 'primary',
      icon: host.kind === 'local' ? 'mdi-server' : 'mdi-transit-connection-variant',
    },
    ...(status?.engineVersion
      ? [{ label: `Engine ${status.engineVersion}`, color: 'info', icon: 'mdi-docker' }]
      : []),
    ...(status
      ? [
          {
            label: `${docker.runningContainers} rodando`,
            color: 'success',
            icon: 'mdi-play-circle-outline',
          },
          ...(docker.stoppedContainers
            ? [
                {
                  label: `${docker.stoppedContainers} parados`,
                  color: 'warning',
                  icon: 'mdi-stop-circle-outline',
                },
              ]
            : []),
        ]
      : []),
  ]
})

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
  width: 260px;
}

@media (max-width: 700px) {
  .docker-header__actions,
  .docker-host-select {
    width: 100%;
  }

  .docker-header__actions > * {
    flex: 1 1 auto;
  }
}
</style>
