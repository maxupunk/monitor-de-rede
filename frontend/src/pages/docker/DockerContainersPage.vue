<template>
  <div>
    <PageHeader title="Containers" subtitle="Inventário, consumo, logs e controle do ciclo de vida">
      <template #actions>
        <v-btn
          color="primary"
          prepend-icon="mdi-refresh"
          :loading="docker.loading"
          @click="docker.refreshContainers()"
        >
          Atualizar
        </v-btn>
      </template>
    </PageHeader>

    <v-alert v-if="docker.error" type="error" variant="tonal" closable class="mb-4">
      {{ docker.error }}
    </v-alert>
    <v-card rounded="xl" variant="outlined">
      <div class="d-flex flex-column flex-md-row ga-3 pa-4">
        <v-text-field
          v-model="search"
          prepend-inner-icon="mdi-magnify"
          label="Buscar por nome, imagem ou projeto"
          variant="outlined"
          density="compact"
          hide-details
          clearable
        ></v-text-field>
        <v-select
          v-model="stateFilter"
          :items="stateOptions"
          label="Estado"
          variant="outlined"
          density="compact"
          hide-details
          class="docker-filter"
        ></v-select>
      </div>

      <ResponsiveDataTable
        :headers="headers"
        :items="filteredContainers"
        :loading="docker.loading"
        :items-per-page="15"
        no-data-text="Nenhum container encontrado"
        clickable
        @click:row="onRowClick"
      >
        <template #item.name="{ item }">
          <div class="py-2">
            <div class="font-weight-bold">{{ containerName(item) }}</div>
            <div class="text-caption text-medium-emphasis font-mono">{{ shortId(item.id) }}</div>
          </div>
        </template>
        <template #item.projectName="{ item }">
          {{ item.projectName || 'Avulso' }}
        </template>
        <template #item.state="{ item }">
          <v-chip :color="stateColor(item.state)" size="small" variant="tonal">
            {{ stateLabel(item.state) }}
          </v-chip>
        </template>
        <template #item.resources="{ item }">
          <div v-if="metricFor(item.id)" class="text-caption py-1">
            <div>CPU {{ metricFor(item.id)?.cpu.usagePercent.toFixed(1) }}%</div>
            <div>RAM {{ metricFor(item.id)?.memory.usagePercent.toFixed(1) }}%</div>
          </div>
          <span v-else class="text-medium-emphasis">—</span>
        </template>
        <template #item.actions="{ item }">
          <div class="d-flex ga-1 justify-end" @click.stop>
            <v-btn
              icon="mdi-eye-outline"
              size="small"
              variant="text"
              title="Inspecionar"
              @click="openDetail(item)"
            ></v-btn>
            <v-btn
              v-if="auth.isAdmin && item.state !== 'running'"
              icon="mdi-play"
              size="small"
              color="success"
              variant="text"
              title="Iniciar"
              :loading="docker.actionLoading"
              @click="runContainerAction(item, 'start')"
            ></v-btn>
            <v-btn
              v-if="auth.isAdmin && item.state === 'running'"
              icon="mdi-stop"
              size="small"
              color="warning"
              variant="text"
              title="Parar"
              :loading="docker.actionLoading"
              @click="runContainerAction(item, 'stop')"
            ></v-btn>
            <v-btn
              v-if="auth.isAdmin"
              icon="mdi-restart"
              size="small"
              color="info"
              variant="text"
              title="Reiniciar"
              :loading="docker.actionLoading"
              @click="runContainerAction(item, 'restart')"
            ></v-btn>
            <v-btn
              v-if="auth.isAdmin"
              icon="mdi-delete-outline"
              size="small"
              color="error"
              variant="text"
              title="Remover"
              :loading="docker.actionLoading"
              @click="runContainerAction(item, 'remove')"
            ></v-btn>
          </div>
        </template>
        <template #mobile-item="{ item }">
          <div class="d-flex align-start justify-space-between ga-2">
            <div class="min-w-0">
              <div class="font-weight-bold text-truncate">{{ containerName(item) }}</div>
              <div class="text-caption text-medium-emphasis text-truncate">{{ item.image }}</div>
              <div class="text-caption mt-1">{{ item.projectName || 'Avulso' }}</div>
            </div>
            <v-chip :color="stateColor(item.state)" size="small" variant="tonal">
              {{ stateLabel(item.state) }}
            </v-chip>
          </div>
        </template>
      </ResponsiveDataTable>
    </v-card>

    <v-dialog v-model="detailDialog" max-width="980" scrollable>
      <v-card rounded="xl">
        <v-card-title class="d-flex align-center ga-2">
          <v-icon color="primary">mdi-cube-outline</v-icon>
          <span class="text-truncate">{{ detail?.name || 'Container' }}</span>
          <v-spacer></v-spacer>
          <v-btn icon="mdi-close" variant="text" @click="detailDialog = false"></v-btn>
        </v-card-title>
        <v-tabs v-model="detailTab" color="primary" show-arrows>
          <v-tab value="overview">Detalhes</v-tab>
          <v-tab value="mounts">Volumes</v-tab>
          <v-tab value="networks">Redes</v-tab>
          <v-tab value="logs">Logs</v-tab>
        </v-tabs>
        <v-divider></v-divider>
        <v-card-text class="pa-0 docker-detail-body">
          <v-window v-model="detailTab">
            <v-window-item value="overview" class="pa-5">
              <v-skeleton-loader v-if="detailLoading" type="article"></v-skeleton-loader>
              <v-row v-else-if="detail">
                <v-col cols="12" md="6">
                  <v-list density="compact">
                    <v-list-item title="ID" :subtitle="detail.id"></v-list-item>
                    <v-list-item title="Imagem" :subtitle="detail.image"></v-list-item>
                    <v-list-item
                      title="Hostname"
                      :subtitle="detail.config.hostname || '—'"
                    ></v-list-item>
                    <v-list-item
                      title="Usuário"
                      :subtitle="detail.config.user || 'padrão da imagem'"
                    ></v-list-item>
                  </v-list>
                </v-col>
                <v-col cols="12" md="6">
                  <v-list density="compact">
                    <v-list-item title="Estado" :subtitle="detail.state.status"></v-list-item>
                    <v-list-item
                      title="PID"
                      :subtitle="String(detail.state.pid || '—')"
                    ></v-list-item>
                    <v-list-item
                      title="Reinício"
                      :subtitle="detail.hostConfig.restartPolicy.name || 'não'"
                    ></v-list-item>
                    <v-list-item
                      title="Modo de rede"
                      :subtitle="detail.hostConfig.networkMode || '—'"
                    ></v-list-item>
                  </v-list>
                </v-col>
                <v-col cols="12">
                  <div class="text-subtitle-2 mb-2">Comando</div>
                  <pre class="docker-code">{{ detail.config.command.join(' ') || '—' }}</pre>
                  <div class="text-subtitle-2 mt-4 mb-2">
                    Ambiente (segredos ocultados pelo servidor)
                  </div>
                  <pre class="docker-code">{{ detail.config.environment.join('\n') || '—' }}</pre>
                </v-col>
              </v-row>
            </v-window-item>
            <v-window-item value="mounts" class="pa-5">
              <v-table density="compact">
                <thead>
                  <tr>
                    <th>Tipo</th>
                    <th>Origem</th>
                    <th>Destino</th>
                    <th>Modo</th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="mount in detail?.mounts || []"
                    :key="`${mount.source}-${mount.destination}`"
                  >
                    <td>{{ mount.mountType }}</td>
                    <td>{{ mount.name || mount.source }}</td>
                    <td>{{ mount.destination }}</td>
                    <td>{{ mount.readWrite ? 'RW' : 'RO' }}</td>
                  </tr>
                </tbody>
              </v-table>
            </v-window-item>
            <v-window-item value="networks" class="pa-5">
              <v-table density="compact">
                <thead>
                  <tr>
                    <th>Rede</th>
                    <th>IP</th>
                    <th>Gateway</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="network in detail?.networks || []" :key="network.networkId">
                    <td>{{ network.networkName }}</td>
                    <td>{{ network.ipAddress || '—' }}</td>
                    <td>{{ network.gateway || '—' }}</td>
                  </tr>
                </tbody>
              </v-table>
            </v-window-item>
            <v-window-item value="logs" class="pa-4">
              <div class="d-flex align-center ga-2 mb-3">
                <v-select
                  v-model="logTail"
                  :items="logTailOptions"
                  label="Linhas"
                  variant="outlined"
                  density="compact"
                  hide-details
                  max-width="180"
                ></v-select>
                <v-btn prepend-icon="mdi-refresh" :loading="logsLoading" @click="loadLogs">
                  Atualizar
                </v-btn>
              </div>
              <pre class="docker-logs">{{ formattedLogs }}</pre>
            </v-window-item>
          </v-window>
        </v-card-text>
      </v-card>
    </v-dialog>

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="4500">
      {{ feedback.message }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import { confirm } from '@/composables/useConfirm'
import { dockerService } from '@/services/dockerService'
import { useAuthStore } from '@/stores/auth'
import { useDockerStore } from '@/stores/docker'
import type { DockerContainerDetail } from '@/bindings/DockerContainerDetail'
import type { DockerContainerSummary } from '@/bindings/DockerContainerSummary'
import type { DockerLogEntry } from '@/bindings/DockerLogEntry'

type ContainerActionName = 'start' | 'stop' | 'restart' | 'remove'

const docker = useDockerStore()
const auth = useAuthStore()
const search = ref('')
const stateFilter = ref('all')
const detailDialog = ref(false)
const detailLoading = ref(false)
const detail = ref<DockerContainerDetail | null>(null)
const detailTab = ref('overview')
const logs = ref<DockerLogEntry[]>([])
const logsLoading = ref(false)
const logTail = ref<number | 'all'>(500)
const feedback = ref({ visible: false, color: 'success', message: '' })

const headers = [
  { title: 'Container', key: 'name' },
  { title: 'Projeto', key: 'projectName' },
  { title: 'Imagem', key: 'image' },
  { title: 'Estado', key: 'state', width: '120px' },
  { title: 'Recursos', key: 'resources', width: '130px', sortable: false },
  { title: 'Ações', key: 'actions', width: '220px', sortable: false },
]
const stateOptions = [
  { title: 'Todos', value: 'all' },
  { title: 'Em execução', value: 'running' },
  { title: 'Parados', value: 'stopped' },
]
const logTailOptions = [100, 500, 1000, 5000, { title: 'Todos', value: 'all' }]

const filteredContainers = computed(() => {
  const term = search.value.trim().toLocaleLowerCase()
  return docker.containers.filter((container) => {
    const stateMatches =
      stateFilter.value === 'all' ||
      (stateFilter.value === 'running' && container.state === 'running') ||
      (stateFilter.value === 'stopped' && container.state !== 'running')
    const textMatches =
      !term ||
      [containerName(container), container.image, container.projectName || '']
        .join(' ')
        .toLocaleLowerCase()
        .includes(term)
    return stateMatches && textMatches
  })
})

const formattedLogs = computed(() =>
  logs.value.length
    ? logs.value
        .map(
          (entry) =>
            `${entry.timestamp ? `[${entry.timestamp}] ` : ''}${entry.stream}: ${entry.message}`
        )
        .join('\n')
    : 'Nenhum log encontrado.'
)

onMounted(() => void docker.refreshAll())

watch(detailTab, (tab) => {
  if (tab === 'logs' && logs.value.length === 0) void loadLogs()
})

function containerName(container: DockerContainerSummary): string {
  return container.names[0]?.replace(/^\//, '') || shortId(container.id)
}

function shortId(id: string): string {
  return id.replace(/^sha256:/, '').slice(0, 12)
}

function stateColor(state: string): string {
  if (state === 'running') return 'success'
  if (state === 'paused' || state === 'restarting') return 'warning'
  return 'grey'
}

function stateLabel(state: string): string {
  return (
    {
      running: 'Executando',
      exited: 'Parado',
      created: 'Criado',
      paused: 'Pausado',
      restarting: 'Reiniciando',
      dead: 'Inativo',
    }[state] || state
  )
}

function metricFor(id: string) {
  return docker.metrics?.containers.find((metric) => metric.containerId === id)
}

function onRowClick(_event: MouseEvent, row: { item: DockerContainerSummary }): void {
  void openDetail(row.item)
}

async function openDetail(container: DockerContainerSummary): Promise<void> {
  detailDialog.value = true
  detailTab.value = 'overview'
  detailLoading.value = true
  logs.value = []
  try {
    detail.value = await dockerService.container(container.id)
  } catch (reason: unknown) {
    notify(reason instanceof Error ? reason.message : 'Erro ao inspecionar container', 'error')
  } finally {
    detailLoading.value = false
  }
}

async function loadLogs(): Promise<void> {
  if (!detail.value) return
  logsLoading.value = true
  try {
    logs.value = await dockerService.logs(detail.value.id, {
      tail: logTail.value,
      timestamps: true,
    })
  } catch (reason: unknown) {
    notify(reason instanceof Error ? reason.message : 'Erro ao carregar logs', 'error')
  } finally {
    logsLoading.value = false
  }
}

async function runContainerAction(
  container: DockerContainerSummary,
  action: ContainerActionName
): Promise<void> {
  const name = containerName(container)
  const destructive = action === 'remove' || action === 'stop'
  if (destructive) {
    const accepted = await confirm({
      title: action === 'remove' ? 'Remover container' : 'Parar container',
      message: `${action === 'remove' ? 'Remover' : 'Parar'} o container "${name}"?`,
      confirmText: action === 'remove' ? 'Remover' : 'Parar',
      confirmColor: action === 'remove' ? 'error' : 'warning',
      icon: action === 'remove' ? 'mdi-delete-alert-outline' : 'mdi-stop-circle-outline',
    })
    if (!accepted) return
  }

  let responseMessage = ''
  const success = await docker.runAction(async () => {
    const response = await {
      start: () => dockerService.startContainer(container.id),
      stop: () => dockerService.stopContainer(container.id),
      restart: () => dockerService.restartContainer(container.id),
      remove: () => dockerService.removeContainer(container.id),
    }[action]()
    responseMessage = response.message
  }, docker.refreshContainers)
  notify(
    success ? responseMessage : docker.error || 'Operação não concluída',
    success ? 'success' : 'error'
  )
}

function notify(message: string, color: string): void {
  feedback.value = { visible: true, color, message }
}
</script>

<style scoped>
.docker-filter {
  max-width: 220px;
}
.docker-detail-body {
  min-height: 420px;
}
.docker-code,
.docker-logs {
  background: rgb(var(--v-theme-surface-variant));
  border-radius: 8px;
  font-family: ui-monospace, SFMono-Regular, Consolas, monospace;
  font-size: 0.78rem;
  overflow: auto;
  padding: 12px;
  white-space: pre-wrap;
  word-break: break-all;
}
.docker-logs {
  min-height: 340px;
  max-height: 55vh;
  white-space: pre;
}
</style>
