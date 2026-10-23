<template>
  <div>
    <PageHeader
      title="Benchmark dos modelos de decisão"
      subtitle="Como cada modelo respondeu às perguntas reais do NetMonitor — use para escolher o do Decisor"
    >
      <template #actions>
        <v-btn
          color="primary"
          prepend-icon="mdi-cog"
          :to="{ name: 'settings', query: { tab: 'ia' } }"
        >
          Configurar o Decisor
        </v-btn>
      </template>
    </PageHeader>

    <v-alert v-if="deciderStore.benchmarkError" type="error" variant="tonal" class="mb-4">
      {{ deciderStore.benchmarkError }}
    </v-alert>

    <template v-if="report">
      <!-- O recomendado, em destaque: a resposta para quem só quer escolher -->
      <v-card v-if="best" elevation="2" rounded="lg" class="mb-4" color="surface">
        <v-card-text class="d-flex flex-column flex-md-row align-start align-md-center ga-3">
          <v-avatar color="success" size="48">
            <v-icon size="28">mdi-trophy</v-icon>
          </v-avatar>
          <div class="flex-grow-1">
            <div class="text-overline text-success">Recomendado</div>
            <div class="text-h6 font-weight-bold text-break">{{ best.model }}</div>
            <div class="text-body-2">
              Acertou {{ percent(best.overall) }} das decisões e respondeu em até
              {{ formatLatency(best.p95Ms) }} em CPU (95% das vezes).
              <template v-if="best.bundled">Vem instalado com o NetMonitor.</template>
            </div>
          </div>
          <v-chip :color="accuracyColor(best.overall)" variant="flat" size="large">
            {{ percent(best.overall) }}
          </v-chip>
        </v-card-text>
      </v-card>

      <v-card elevation="2" rounded="lg">
        <ResponsiveDataTable
          :headers="headers"
          :items="rows"
          :loading="deciderStore.loadingBenchmark"
          :items-per-page="-1"
          hide-default-footer
          no-data-text="Nenhum modelo medido"
          :clickable="false"
        >
          <template #item.model="{ item }">
            <div class="py-2">
              <div class="font-weight-bold text-break">{{ item.model }}</div>
              <div class="d-flex flex-wrap ga-1 mt-1">
                <v-chip
                  v-for="tag in tagsOf(item)"
                  :key="tag.label"
                  size="x-small"
                  :color="tag.color"
                  variant="tonal"
                >
                  {{ tag.label }}
                </v-chip>
              </div>
            </div>
          </template>

          <template #item.overall="{ item }">
            <v-chip
              :color="accuracyColor(item.overall)"
              variant="flat"
              size="small"
              class="font-weight-bold"
            >
              {{ percent(item.overall) }}
            </v-chip>
            <div class="text-caption mt-1">{{ intervalOf(item) }}</div>
          </template>

          <template v-for="task in report.tasks" :key="task.id" #[`item.${task.id}`]="{ item }">
            <span :class="`text-${accuracyColor(scoreOf(item, task.id))} font-weight-medium`">
              {{ percent(scoreOf(item, task.id)) }}
            </span>
          </template>

          <template #item.p95Ms="{ item }">
            <div class="d-flex flex-column align-start py-1">
              <span class="font-weight-medium">{{ formatLatency(item.p95Ms) }}</span>
              <v-chip :color="speedOf(item).color" size="x-small" variant="tonal" class="mt-1">
                {{ speedOf(item).label }}
              </v-chip>
            </div>
          </template>

          <template #mobile-item="{ item }">
            <div class="d-flex flex-column ga-2">
              <div class="d-flex align-center justify-space-between ga-2">
                <span class="text-subtitle-1 font-weight-bold text-break">
                  {{ item.position }}º · {{ item.model }}
                </span>
                <v-chip :color="accuracyColor(item.overall)" variant="flat" size="small">
                  {{ percent(item.overall) }}
                </v-chip>
              </div>
              <div class="d-flex flex-wrap ga-1">
                <v-chip
                  v-for="tag in tagsOf(item)"
                  :key="tag.label"
                  size="x-small"
                  :color="tag.color"
                  variant="tonal"
                >
                  {{ tag.label }}
                </v-chip>
                <v-chip :color="speedOf(item).color" size="x-small" variant="tonal">
                  {{ formatLatency(item.p95Ms) }} · {{ speedOf(item).label }}
                </v-chip>
              </div>
              <div class="d-flex flex-wrap ga-1">
                <v-chip
                  v-for="task in report.tasks"
                  :key="task.id"
                  size="x-small"
                  variant="outlined"
                  :color="accuracyColor(scoreOf(item, task.id))"
                >
                  {{ task.label }}: {{ percent(scoreOf(item, task.id)) }}
                </v-chip>
              </div>
            </div>
          </template>
        </ResponsiveDataTable>
      </v-card>

      <!-- Como ler: curto, para quem quer entender a régua -->
      <v-card elevation="0" rounded="lg" class="mt-4" border>
        <v-card-text class="text-body-2">
          <div class="font-weight-bold mb-1">Como ler</div>
          <ul class="ps-4">
            <li>
              <strong>Nota</strong>: média das seis decisões, cada uma pesando igual; ao lado, o
              intervalo de 90% — notas cujos intervalos se cruzam estão empatadas.
            </li>
            <li>
              <strong>Resposta</strong>: 95% das decisões saíram até esse tempo. Acima de
              {{ formatLatency(report.timeoutMs) }} o NetMonitor segue sem a decisão do modelo.
            </li>
            <li>
              Medido em {{ formatDate(`${report.measuredOn}T12:00:00`) }} com
              {{ report.cases }} perguntas reais, rotuladas à mão, num {{ report.hardware }} (CPU,
              sem GPU). Os modelos de GPU não entram: em CPU eles passam do tempo máximo.
            </li>
          </ul>
        </v-card-text>
      </v-card>
    </template>

    <v-skeleton-loader v-else-if="deciderStore.loadingBenchmark" type="table" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted } from 'vue'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import { useAiDeciderStore } from '@/stores/aiDecider'
import type { DeciderBenchmarkRow } from '@/bindings/DeciderBenchmarkRow'
import { formatDate, formatLatency } from '@/utils/formatters'
import { accuracyColor, percent, type ModelFact } from '@/utils/decider'

/** Fração das decisões acima do tempo máximo que ainda conta como "rápido o bastante". */
const OVER_TIMEOUT_TOLERANCE = 0.05

type Row = DeciderBenchmarkRow & { position: number }

const deciderStore = useAiDeciderStore()
const report = computed(() => deciderStore.benchmarkReport)
const rows = computed<Row[]>(() =>
  (report.value?.models ?? []).map((model, index) => ({ ...model, position: index + 1 }))
)
/** O primeiro recomendado da lista (a mais alta nota entre eles). */
const best = computed(() => rows.value.find((row) => row.recommended) ?? null)

const headers = computed(() => [
  { title: '#', key: 'position', sortable: false, width: '48' },
  { title: 'Modelo', key: 'model', sortable: false },
  { title: 'Nota', key: 'overall', sortable: false },
  ...(report.value?.tasks ?? []).map((task) => ({
    title: task.label,
    key: task.id,
    sortable: false,
  })),
  { title: 'Resposta (p95)', key: 'p95Ms', sortable: false },
])

function scoreOf(row: DeciderBenchmarkRow, task: string): number {
  return row.tasks.find((item) => item.task === task)?.score ?? 0
}

function intervalOf(row: DeciderBenchmarkRow): string {
  return `${percent(row.overallLow)}–${percent(row.overallHigh)}`
}

function tagsOf(row: DeciderBenchmarkRow): { label: string; color: ModelFact['color'] }[] {
  const tags: { label: string; color: ModelFact['color'] }[] = []
  if (row.recommended) tags.push({ label: 'recomendado', color: 'success' })
  if (row.bundled) tags.push({ label: 'vem com o NetMonitor', color: 'info' })
  if (row.gpuRecommended) tags.push({ label: 'GPU', color: 'warning' })
  return tags
}

function speedOf(row: DeciderBenchmarkRow): { label: string; color: ModelFact['color'] } {
  const timeout = report.value?.timeoutMs ?? 4000
  if (row.p95Ms <= timeout && row.overTimeout <= OVER_TIMEOUT_TOLERANCE) {
    return { label: 'rápido o bastante', color: 'success' }
  }
  return { label: `${percent(row.overTimeout)} acima de ${formatLatency(timeout)}`, color: 'error' }
}

onMounted(() => deciderStore.loadBenchmark())
</script>
