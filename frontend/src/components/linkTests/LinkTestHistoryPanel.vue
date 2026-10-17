<template>
  <div>
    <div class="filters pa-3 pa-sm-4">
      <v-select
        v-model="filters.status"
        :items="statusItems"
        label="Status"
        variant="outlined"
        density="compact"
        hide-details
        clearable
        prepend-inner-icon="mdi-list-status"
      ></v-select>
      <v-select
        v-model="filters.kind"
        :items="kindItems"
        label="Tipo"
        variant="outlined"
        density="compact"
        hide-details
        clearable
        prepend-inner-icon="mdi-shape-outline"
      ></v-select>
      <v-autocomplete
        v-model="filters.deviceId"
        :items="deviceItems"
        label="Equipamento"
        variant="outlined"
        density="compact"
        hide-details
        clearable
        prepend-inner-icon="mdi-router-network"
      ></v-autocomplete>
      <v-select
        v-model="filters.period"
        :items="PERIODS"
        label="Período"
        variant="outlined"
        density="compact"
        hide-details
        prepend-inner-icon="mdi-calendar-range"
      ></v-select>
    </div>

    <v-alert
      v-if="store.pageError"
      type="error"
      variant="tonal"
      density="compact"
      class="mx-4 mb-3"
    >
      {{ store.pageError }}
    </v-alert>

    <div v-if="showEmpty" class="empty pa-8">
      <v-icon size="48" color="primary">mdi-speedometer</v-icon>
      <template v-if="hasFilters">
        <div class="text-title-medium font-weight-bold">Nenhum teste com esses filtros</div>
        <v-btn
          color="primary"
          variant="flat"
          prepend-icon="mdi-filter-remove"
          @click="clearFilters"
        >
          Limpar filtros
        </v-btn>
      </template>
      <template v-else>
        <div class="text-title-medium font-weight-bold">Nenhum teste de velocidade ainda</div>
        <p class="text-body-medium mb-2">
          Meça quanto passa entre dois equipamentos, enlace por enlace ao longo de um caminho ou do
          servidor até um roteador. Também dá para começar pelo mapa: clique com o botão direito num
          enlace.
        </p>
        <div class="d-flex flex-wrap justify-center ga-2">
          <v-btn
            v-if="auth.canWrite"
            color="primary"
            variant="flat"
            prepend-icon="mdi-plus"
            @click="$emit('new')"
          >
            Novo teste
          </v-btn>
          <v-btn color="secondary" variant="flat" prepend-icon="mdi-sitemap" to="/topology">
            Abrir a topologia
          </v-btn>
        </div>
      </template>
    </div>

    <template v-else>
      <ResponsiveDataTable
        :headers="headers"
        :items="items"
        :loading="store.pageLoading"
        :items-per-page="-1"
        hide-default-footer
        clickable
        no-data-text="Nenhum teste"
        :item-key="(item: LinkTest) => item.id"
        @click:row="(_event: MouseEvent, row: { item: LinkTest }) => $emit('open', row.item.id)"
      >
        <template #item.createdAt="{ item }">
          <div>{{ formatDateTime(item.createdAt) }}</div>
          <div class="text-body-small">{{ formatRelativeTime(item.createdAt) }}</div>
        </template>
        <template #item.title="{ item }">
          <div class="d-flex align-center ga-2">
            <v-icon :color="kindOf(item).color" size="18">{{ kindOf(item).icon }}</v-icon>
            <span class="font-weight-bold">{{ testTitle(item) }}</span>
          </div>
          <div class="text-body-small">{{ originOf(item) }}</div>
        </template>
        <template #item.result="{ item }">
          <span class="result">{{ resultOf(item) }}</span>
        </template>
        <template #item.status="{ item }">
          <LinkTestStatusChip :status="item.status" />
        </template>

        <template #mobile-item="{ item }">
          <div class="d-flex flex-column ga-1 mobile-item">
            <div class="d-flex align-start justify-space-between ga-2">
              <span class="font-weight-bold mobile-title">{{ testTitle(item) }}</span>
              <LinkTestStatusChip :status="item.status" size="x-small" class="flex-shrink-0" />
            </div>
            <div class="text-body-small">
              {{ formatDateTime(item.createdAt) }} · {{ originOf(item) }}
            </div>
            <div class="result">{{ resultOf(item) }}</div>
          </div>
        </template>
      </ResponsiveDataTable>

      <div v-if="pageCount > 1" class="d-flex justify-center pa-3">
        <v-pagination
          :model-value="store.page?.page ?? 1"
          :length="pageCount"
          :total-visible="5"
          color="primary"
          density="comfortable"
          @update:model-value="(page) => load(page)"
        ></v-pagination>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
/** Aba "Histórico": filtros, tabela paginada no servidor e abertura do detalhe. */
import { computed, onMounted, reactive, watch } from 'vue'
import { useAuthStore } from '@/stores/auth'
import { useDevicesStore } from '@/stores/devices'
import { HISTORY_PER_PAGE, useLinkTestsStore } from '@/stores/linkTests'
import type { LinkTest, LinkTestKind, LinkTestStatus } from '@/types/linkTests'
import { formatDateTime, formatRelativeTime } from '@/utils/formatters'
import {
  KIND_OPTIONS,
  TEST_KINDS,
  TEST_STATUS,
  flowPairLabel,
  testTitle,
} from '@/utils/linkTests/presentation'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import LinkTestStatusChip from './LinkTestStatusChip.vue'

const props = defineProps<{ initialDeviceId?: number | null }>()

defineEmits<{ open: [testId: number]; new: [] }>()

const store = useLinkTestsStore()
const devices = useDevicesStore()
const auth = useAuthStore()

const PERIODS = [
  { title: 'Últimas 24 horas', value: 1 },
  { title: 'Últimos 7 dias', value: 7 },
  { title: 'Últimos 30 dias', value: 30 },
  { title: 'Todo o período', value: 0 },
]

const headers = [
  { title: 'Quando', key: 'createdAt', sortable: false },
  { title: 'Teste', key: 'title', sortable: false },
  { title: 'Resultado (pior enlace)', key: 'result', sortable: false },
  { title: 'Status', key: 'status', sortable: false },
]

const filters = reactive({
  status: null as LinkTestStatus | null,
  kind: null as LinkTestKind | null,
  deviceId: props.initialDeviceId ?? (null as number | null),
  period: 30,
})

const statusItems = (Object.keys(TEST_STATUS) as LinkTestStatus[]).map((value) => ({
  title: TEST_STATUS[value].label,
  value,
}))
const kindItems = KIND_OPTIONS.map((option) => ({ title: option.label, value: option.value }))
const deviceItems = computed(() =>
  devices.devices
    .map((device) => ({ title: device.name, value: device.id }))
    .sort((a, b) => a.title.localeCompare(b.title, 'pt-BR'))
)

const items = computed(() => store.page?.items ?? [])
const pageCount = computed(() => {
  const page = store.page
  return page ? Math.ceil(page.total / Math.max(1, page.perPage)) : 0
})
const hasFilters = computed(
  () => filters.status !== null || filters.kind !== null || filters.deviceId !== null
)
const showEmpty = computed(
  () => !store.pageLoading && !store.pageError && store.page !== null && items.value.length === 0
)

function since(days: number): string | null {
  if (days <= 0) return null
  return new Date(Date.now() - days * 86_400_000).toISOString()
}

function load(page = 1) {
  void store.fetchPage({
    page,
    perPage: HISTORY_PER_PAGE,
    status: filters.status,
    kind: filters.kind,
    deviceId: filters.deviceId,
    since: since(filters.period),
  })
}

function clearFilters() {
  Object.assign(filters, { status: null, kind: null, deviceId: null, period: 0 })
}

function kindOf(test: LinkTest) {
  return TEST_KINDS[test.kind] ?? TEST_KINDS.link
}

function originOf(test: LinkTest): string {
  if (test.origin === 'schedule') return 'Agendamento'
  return test.requestedBy?.name ?? 'Usuário'
}

function resultOf(test: LinkTest): string {
  return flowPairLabel(test.summary?.minForwardBps, test.summary?.minReverseBps) ?? '—'
}

watch(
  () => ({ ...filters }),
  () => load(1),
  { deep: true }
)

watch(
  () => props.initialDeviceId,
  (deviceId) => {
    if (deviceId !== undefined && deviceId !== filters.deviceId) filters.deviceId = deviceId
  }
)

onMounted(() => {
  if (devices.devices.length === 0) void devices.fetchDevices()
  load(1)
})
</script>

<style scoped>
.filters {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 12px;
}
.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 8px;
  color: rgb(var(--v-theme-on-surface));
}
.empty p {
  max-width: 560px;
}
.mobile-item {
  min-width: 0;
}
/* Celular: o título quebra linha em vez de empurrar o estado para fora do cartão. */
.mobile-title {
  min-width: 0;
  overflow-wrap: anywhere;
}
.result {
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
</style>
