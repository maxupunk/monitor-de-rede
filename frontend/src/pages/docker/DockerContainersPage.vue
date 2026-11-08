<template>
  <div>
    <DockerSectionBar
      description="Agrupados por projeto compose. Clique num container para ver consumo, configuração, redes, volumes e logs."
    />

    <v-alert v-if="docker.error" type="error" variant="tonal" closable class="mb-4">
      {{ docker.error }}
    </v-alert>
    <v-alert
      v-if="docker.metrics?.failedContainerCount"
      type="warning"
      variant="tonal"
      class="mb-4"
    >
      Métricas parciais: {{ docker.metrics.failedContainerCount }} container(s) sem amostra.
    </v-alert>

    <v-card rounded="lg" elevation="2" class="mb-4">
      <div class="d-flex flex-column flex-md-row align-md-center ga-3 pa-3 pa-md-4">
        <v-text-field
          v-model="search"
          prepend-inner-icon="mdi-magnify"
          label="Buscar por nome, imagem ou projeto"
          variant="outlined"
          density="compact"
          hide-details
          clearable
          class="flex-grow-1"
        ></v-text-field>
        <v-chip-group
          v-model="stateFilter"
          mandatory
          selected-class="font-weight-bold"
          aria-label="Filtrar por estado"
        >
          <v-chip
            v-for="option in CONTAINER_STATE_FILTERS"
            :key="option.value"
            :value="option.value"
            :color="option.color"
            :base-color="option.color"
            variant="outlined"
            filter
          >
            {{ option.label }}
            <v-badge :content="counts[option.value]" :color="option.color" inline></v-badge>
          </v-chip>
        </v-chip-group>
      </div>
    </v-card>

    <v-expansion-panels v-if="groups.length" v-model="expandedGroups" multiple>
      <v-expansion-panel
        v-for="group in groups"
        :key="group.key"
        :value="group.key"
        elevation="2"
        rounded="lg"
        class="mb-3"
      >
        <v-expansion-panel-title>
          <div class="d-flex flex-wrap align-center ga-2 w-100 pr-2">
            <v-avatar :color="group.standalone ? 'info' : 'primary'" variant="tonal" size="32">
              <v-icon size="18">
                {{ group.standalone ? 'mdi-cube-outline' : 'mdi-folder-multiple-outline' }}
              </v-icon>
            </v-avatar>
            <div class="font-weight-bold">{{ group.label }}</div>
            <v-chip
              size="x-small"
              :color="group.running === group.containers.length ? 'success' : 'warning'"
              variant="flat"
            >
              {{ group.running }}/{{ group.containers.length }} rodando
            </v-chip>
            <v-spacer></v-spacer>
            <div class="docker-group-totals text-caption text-high-emphasis">
              CPU {{ formatPercent(group.cpuPercent, 1) }} · RAM
              {{ formatBinaryBytes(group.memoryBytes) }}
            </div>
          </div>
        </v-expansion-panel-title>
        <v-expansion-panel-text>
          <ResponsiveDataTable
            :headers="headers"
            :items="group.containers"
            :loading="docker.loading"
            :items-per-page="15"
            no-data-text="Nenhum container encontrado"
            clickable
            @click:row="onRowClick"
          >
            <template #item.name="{ item }">
              <div class="py-2">
                <div class="font-weight-bold">{{ containerName(item) }}</div>
                <div class="text-caption text-high-emphasis font-mono">{{ shortId(item.id) }}</div>
              </div>
            </template>
            <template #item.state="{ item }">
              <v-chip
                :color="containerState(item.state).color"
                :prepend-icon="containerState(item.state).icon"
                size="small"
                variant="tonal"
              >
                {{ containerState(item.state).label }}
              </v-chip>
            </template>
            <template #item.resources="{ item }">
              <div v-if="metricFor(item.id)" class="resource-cell py-1">
                <v-icon size="14" color="info">mdi-cpu-64-bit</v-icon>
                <span>CPU</span>
                <span class="resource-cell__value">
                  {{ formatPercent(metricFor(item.id)?.cpu.usagePercent, 1) }}
                </span>
                <v-icon size="14" color="primary">mdi-memory</v-icon>
                <span>RAM</span>
                <span class="resource-cell__value">
                  {{ formatBinaryBytes(metricFor(item.id)?.memory.usageBytes) }}
                </span>
              </div>
              <span v-else>—</span>
            </template>
            <template #item.actions="{ item }">
              <DockerContainerActions :container="item" @done="onActionDone" />
            </template>
            <template #mobile-item="{ item }">
              <div class="d-flex flex-wrap align-start justify-space-between ga-2">
                <div class="min-w-0">
                  <div class="font-weight-bold text-truncate">{{ containerName(item) }}</div>
                  <div class="text-caption text-high-emphasis text-truncate">{{ item.image }}</div>
                  <div v-if="metricFor(item.id)" class="text-caption text-high-emphasis">
                    CPU {{ formatPercent(metricFor(item.id)?.cpu.usagePercent, 1) }} · RAM
                    {{ formatBinaryBytes(metricFor(item.id)?.memory.usageBytes) }}
                  </div>
                </div>
                <v-chip
                  :color="containerState(item.state).color"
                  :prepend-icon="containerState(item.state).icon"
                  size="small"
                  variant="tonal"
                >
                  {{ containerState(item.state).label }}
                </v-chip>
              </div>
              <DockerContainerActions :container="item" class="mt-2" @done="onActionDone" />
            </template>
          </ResponsiveDataTable>
        </v-expansion-panel-text>
      </v-expansion-panel>
    </v-expansion-panels>
    <v-alert v-else-if="!docker.loading" type="info" variant="tonal">
      {{
        docker.containers.length
          ? 'Nenhum container combina com a busca e o filtro.'
          : 'Nenhum container neste host.'
      }}
    </v-alert>

    <DockerContainerDetailDialog v-model="detailOpen" :container="selected" @notify="notify" />

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="4500">
      {{ feedback.message }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import DockerContainerActions from '@/components/docker/DockerContainerActions.vue'
import DockerContainerDetailDialog from '@/components/docker/DockerContainerDetailDialog.vue'
import DockerSectionBar from '@/components/docker/DockerSectionBar.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import type { ActionOutcome } from '@/composables/useContainerActions'
import { useDockerStore } from '@/stores/docker'
import type { DockerContainerSummary } from '@/bindings/DockerContainerSummary'
import {
  CONTAINER_STATE_FILTERS,
  containerName,
  containerState,
  filterContainers,
  groupContainers,
  isContainerStateFilter,
  shortId,
  stateFilterCounts,
  type ContainerStateFilter,
} from '@/utils/dockerPresentation'
import { formatBinaryBytes, formatPercent } from '@/utils/formatters'

const docker = useDockerStore()
const route = useRoute()
const router = useRouter()

const search = ref('')
/** O filtro vem da URL: a visão geral leva direto aos parados ou aos ativos. */
const stateFilter = ref<ContainerStateFilter>(
  isContainerStateFilter(route.query.state) ? route.query.state : 'all'
)
const expandedGroups = ref<string[]>([])
const detailOpen = ref(false)
const selected = ref<DockerContainerSummary | null>(null)
const feedback = ref({ visible: false, color: 'success', message: '' })

const headers = [
  { title: 'Container', key: 'name' },
  { title: 'Imagem', key: 'image' },
  { title: 'Estado', key: 'state', width: '140px' },
  { title: 'Recursos', key: 'resources', width: '160px', sortable: false },
  { title: 'Ações', key: 'actions', width: '200px', sortable: false, align: 'end' as const },
]

const counts = computed(() => stateFilterCounts(docker.containers))
const groups = computed(() =>
  groupContainers(filterContainers(docker.containers, search.value, stateFilter.value), metricFor)
)

// Grupo novo entra aberto; o que o operador fechou continua fechado.
watch(
  () => groups.value.map((group) => group.key),
  (keys, previous) => {
    const known = new Set(previous ?? [])
    const current = expandedGroups.value.filter((key) => keys.includes(key))
    expandedGroups.value = [...new Set([...current, ...keys.filter((key) => !known.has(key))])]
  },
  { immediate: true }
)

watch(stateFilter, (state) => {
  const query = { ...route.query }
  if (state === 'all') delete query.state
  else query.state = state
  void router.replace({ query })
})

function metricFor(id: string) {
  return docker.metrics?.containers.find((metric) => metric.containerId === id)
}

function onRowClick(_event: MouseEvent, row: { item: DockerContainerSummary }): void {
  selected.value = row.item
  detailOpen.value = true
}

function onActionDone(outcome: ActionOutcome) {
  notify(outcome.message, outcome.ok ? 'success' : 'error')
}

function notify(message: string, color: string): void {
  feedback.value = { visible: true, color, message }
}
</script>

<style scoped>
.resource-cell {
  display: grid;
  grid-template-columns: 14px auto 1fr;
  align-items: center;
  column-gap: 6px;
  row-gap: 2px;
  font-size: 0.8125rem;
}

.resource-cell__value {
  text-align: right;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}

@media (max-width: 700px) {
  .docker-group-totals {
    display: none;
  }
}
</style>
