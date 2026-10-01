<template>
  <div>
    <!-- Ações da frota -->
    <div class="d-flex flex-wrap align-center ga-2 mb-4">
      <v-btn
        v-if="spec?.statusAction"
        color="primary"
        variant="flat"
        prepend-icon="mdi-refresh"
        :loading="refreshing"
        :disabled="!canWrite || view.members.length === 0"
        @click="emit('run', spec.statusAction)"
      >
        Atualizar estado
      </v-btn>
      <v-btn
        v-for="item in spec?.actions ?? []"
        :key="item.id"
        :color="colorOf(item)"
        variant="flat"
        :prepend-icon="item.icon ?? effectPresentation(effectOf(item)).icon"
        :disabled="!canWrite || view.members.length === 0"
        @click="emit('run', item.id)"
      >
        {{ item.title }}
      </v-btn>
      <v-spacer></v-spacer>
      <span v-if="statusBatch" class="text-body-2">
        Estado de {{ formatRelativeTime(statusBatch.finishedAt) }}
      </span>
    </div>

    <v-alert v-if="view.members.length === 0" type="info" variant="tonal" class="mb-4">
      Nenhum equipamento neste aplicativo ainda. Adicione os equipamentos na aba
      <strong>Equipamentos</strong>; o acesso de cada um fica no cadastro do próprio dispositivo.
    </v-alert>

    <!-- Grade equipamentos × itens (ex.: roteadores × SSIDs) -->
    <v-card v-else border flat class="rounded-lg">
      <v-table density="comfortable">
        <thead>
          <tr>
            <th class="font-weight-bold">Equipamento</th>
            <th class="font-weight-bold">Estado</th>
            <th v-for="column in columns" :key="column" class="font-weight-bold text-center">
              {{ column }}
            </th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in rows" :key="row.member.deviceId">
            <td>
              <router-link
                :to="{ path: '/devices/' + row.member.deviceId }"
                class="font-weight-bold text-primary text-decoration-none"
              >
                {{ row.member.name }}
              </router-link>
              <div class="text-body-small">
                {{ [row.member.ip, row.member.firmware].filter(Boolean).join(' · ') }}
              </div>
            </td>
            <td>
              <v-chip
                v-if="!row.member.credentialsReady"
                size="small"
                color="warning"
                variant="flat"
              >
                Sem credencial
              </v-chip>
              <v-chip v-else-if="row.error" size="small" color="error" variant="flat">
                <v-icon start size="14">mdi-alert-circle</v-icon>
                Falhou
              </v-chip>
              <v-chip
                v-else-if="row.state"
                size="small"
                :color="stateColor(row.state)"
                variant="flat"
              >
                {{ stateLabel(row.state) }}
              </v-chip>
              <span v-else class="text-body-2">—</span>
              <div v-if="row.error" class="text-body-small text-error mt-1">{{ row.error }}</div>
            </td>
            <td v-for="column in columns" :key="column" class="text-center">
              <template v-if="row.cells[column]">
                <v-chip
                  size="small"
                  :color="stateColor(row.cells[column].state)"
                  variant="tonal"
                  class="font-weight-bold"
                >
                  {{ stateLabel(row.cells[column].state) }}
                </v-chip>
                <div v-if="row.cells[column].detail !== undefined" class="text-body-small mt-1">
                  {{ detailLabel }}: {{ row.cells[column].detail }}
                </div>
              </template>
              <span v-else class="text-body-2">—</span>
            </td>
          </tr>
        </tbody>
      </v-table>
      <v-card-text v-if="!statusBatch && spec?.statusAction" class="text-body-2">
        Ainda sem leitura dos equipamentos. Clique em <strong>Atualizar estado</strong>.
      </v-card-text>
    </v-card>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { FleetAction } from '@/bindings/FleetAction'
import type { FleetMember } from '@/bindings/FleetMember'
import type { FleetView } from '@/bindings/FleetView'
import { usePluginAppsStore } from '@/stores/pluginApps'
import { formatRelativeTime } from '@/utils/formatters'
import { effectPresentation, outputLabel, stateColor, stateLabel } from '@/utils/pluginPresentation'

const props = defineProps<{ view: FleetView; canWrite: boolean }>()
const emit = defineEmits<{ run: [fleetActionId: string] }>()

const store = usePluginAppsStore()

type Cell = { state: unknown; detail?: unknown }
interface Row {
  member: FleetMember
  state: unknown
  error: string | null
  cells: Record<string, Cell>
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

const spec = computed(() => props.view.plugin.fleet)
const matrix = computed(() => spec.value?.matrix ?? null)
const statusLabels = computed(
  () => props.view.plugin.actions.find((action) => action.id === spec.value?.statusAction)?.labels
)
const detailLabel = computed(() => outputLabel(matrix.value?.detail ?? '', statusLabels.value))

const statusBatch = computed(() =>
  spec.value?.statusAction ? store.latestBatch(props.view.plugin.id, spec.value.statusAction) : null
)
const refreshing = computed(() =>
  props.view.batches.some(
    (batch) => batch.action === spec.value?.statusAction && batch.status === 'running'
  )
)

const rows = computed<Row[]>(() =>
  props.view.members.map((member) => {
    const device = statusBatch.value?.devices.find((item) => item.deviceId === member.deviceId)
    const output = isRecord(device?.output) ? device.output : {}
    const cells: Record<string, Cell> = {}
    const config = matrix.value
    const list = config ? output[config.field] : undefined
    if (config && Array.isArray(list)) {
      for (const item of list) {
        if (!isRecord(item)) continue
        cells[String(item[config.key])] = {
          state: item[config.state],
          detail: config.detail ? item[config.detail] : undefined,
        }
      }
    }
    return { member, state: output.state, error: device?.error ?? null, cells }
  })
)

/** As colunas na ordem em que aparecem nos equipamentos. */
const columns = computed(() => {
  const keys: string[] = []
  for (const row of rows.value) {
    for (const key of Object.keys(row.cells)) if (!keys.includes(key)) keys.push(key)
  }
  return keys
})

function effectOf(item: FleetAction) {
  return props.view.plugin.actions.find((action) => action.id === item.action)?.effect ?? 'read'
}

function colorOf(item: FleetAction): string {
  if (effectOf(item) === 'write') return 'error'
  return item.reduce ? 'secondary' : 'primary'
}
</script>
