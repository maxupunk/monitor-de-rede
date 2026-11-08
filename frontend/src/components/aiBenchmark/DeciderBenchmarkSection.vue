<template>
  <div>
    <v-card v-if="loading && !report" elevation="2" class="pa-8 text-center">
      <v-progress-circular indeterminate color="primary" class="mb-3" />
      <div class="text-body-2 text-high-emphasis">Carregando resultados do Decisor…</div>
    </v-card>

    <v-alert v-else-if="error" type="error" variant="tonal" class="mb-4">
      {{ error }}
    </v-alert>

    <template v-else-if="report">
      <!-- Destaque: o recomendado -->
      <v-card
        v-if="recommended"
        variant="tonal"
        color="primary"
        class="mb-4 pa-4"
        data-testid="benchmark-recommended"
      >
        <div class="d-flex flex-wrap align-center justify-space-between ga-4">
          <div style="min-width: 260px; flex: 1">
            <div class="d-flex align-center flex-wrap ga-2 mb-1">
              <v-chip size="small" color="success" variant="flat" class="font-weight-bold">
                <v-icon start size="14">mdi-star</v-icon>
                Recomendado · Padrão do Decisor
              </v-chip>
              <v-chip v-if="recommended.bundled" size="small" color="info" variant="tonal">
                Baixado sob demanda da release
              </v-chip>
              <v-chip
                v-if="isCurrentModel(recommended.model)"
                size="small"
                color="primary"
                variant="outlined"
              >
                Em uso no Decisor
              </v-chip>
            </div>
            <div class="text-h6 font-weight-bold font-mono">{{ recommended.model }}</div>
            <div v-if="recommended.description" class="text-body-2 text-high-emphasis mt-1">
              {{ recommended.description }}
            </div>
          </div>

          <div class="d-flex flex-wrap ga-6 align-center">
            <div>
              <div class="text-caption text-high-emphasis">Nota geral</div>
              <div class="text-h4 font-weight-bold text-success">
                {{ percent(recommended.overall) }}
              </div>
              <div class="text-caption text-high-emphasis">
                IC 90%:
                {{ formatConfidenceInterval(recommended.overallLow, recommended.overallHigh) }}
              </div>
            </div>
            <div>
              <div class="text-caption text-high-emphasis">Sugestões na tela</div>
              <div class="text-h5 font-weight-bold">{{ percent(recommended.screen) }}</div>
              <div class="text-caption text-high-emphasis">acima da confiança mínima</div>
            </div>
            <div>
              <div class="text-caption text-high-emphasis">Tempo em CPU</div>
              <div class="text-h5 font-weight-bold">
                {{ deciderSpeedOf(recommended, report.timeoutMs).label }}
              </div>
              <div class="text-caption text-high-emphasis">
                mediana e 95% das decisões
                <span v-if="deciderLatencyNoteOf(recommended)">
                  · {{ deciderLatencyNoteOf(recommended) }}
                </span>
              </div>
            </div>
          </div>
        </div>
      </v-card>

      <!-- Tabela completa -->
      <v-card elevation="2">
        <v-card-title class="d-flex flex-wrap align-center justify-space-between ga-2 py-3 px-4">
          <div class="d-flex align-center ga-2">
            <v-icon size="20" color="primary">mdi-table</v-icon>
            <span class="text-subtitle-1 font-weight-bold">
              {{ report.models.length }} modelos de decisão comparados
            </span>
          </div>
          <div class="text-caption text-high-emphasis">
            {{ report.cases }} perguntas do NetMonitor · {{ report.hardware }} · tempo máximo de
            {{ Math.round(report.timeoutMs / 1000) }} s por decisão · medido em
            {{ report.measuredOn }}
          </div>
        </v-card-title>

        <v-divider />

        <v-table density="comfortable" hover class="benchmark-table">
          <thead>
            <tr>
              <th class="text-left" style="width: 44px">#</th>
              <th class="text-left" style="min-width: 240px">Modelo</th>
              <th class="text-right" style="min-width: 150px">Nota geral</th>
              <th
                class="text-right"
                title="Considerando só as sugestões que chegam à tela (acima da confiança mínima)"
              >
                Na tela
              </th>
              <th v-for="task in report.tasks" :key="task.id" class="text-right">
                {{ task.label }}
              </th>
              <th class="text-right" style="min-width: 170px">Tempo em CPU</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="row in rows"
              :key="row.model"
              :class="{ 'recommended-row': row.recommended }"
              data-testid="benchmark-row"
            >
              <td class="text-high-emphasis font-mono">{{ row.position }}</td>
              <td>
                <div class="d-flex align-center flex-wrap ga-1">
                  <span class="font-weight-medium font-mono">{{ row.model }}</span>
                  <v-chip
                    v-for="tag in deciderTagsOf(row)"
                    :key="tag.label"
                    size="x-small"
                    :color="tag.color"
                    variant="tonal"
                  >
                    {{ tag.label }}
                  </v-chip>
                  <v-chip
                    v-if="isCurrentModel(row.model)"
                    size="x-small"
                    color="primary"
                    variant="outlined"
                  >
                    Em uso
                  </v-chip>
                </div>
                <div v-if="row.description" class="text-caption text-high-emphasis mt-1">
                  {{ row.description }}
                </div>
              </td>

              <td class="text-right">
                <div class="d-flex align-center justify-end ga-2">
                  <v-progress-linear
                    :model-value="row.overall * 100"
                    :color="accuracyColor(row.overall)"
                    height="6"
                    rounded
                    style="width: 64px"
                  />
                  <v-chip
                    size="small"
                    :color="accuracyColor(row.overall)"
                    variant="tonal"
                    class="font-weight-bold font-mono"
                  >
                    {{ percent(row.overall) }}
                  </v-chip>
                </div>
                <div class="text-caption text-high-emphasis font-mono">
                  {{ formatConfidenceInterval(row.overallLow, row.overallHigh) }}
                </div>
              </td>

              <td class="text-right font-mono">
                <span :class="`text-${accuracyColor(row.screen)}`" class="font-weight-medium">
                  {{ percent(row.screen) }}
                </span>
              </td>

              <td v-for="task in report.tasks" :key="task.id" class="text-right font-mono">
                <span
                  :class="`text-${accuracyColor(deciderTaskScoreOf(row, task.id))}`"
                  class="font-weight-medium"
                >
                  {{ percent(deciderTaskScoreOf(row, task.id)) }}
                </span>
              </td>

              <td class="text-right font-mono">
                <v-chip
                  size="x-small"
                  :color="deciderSpeedOf(row, report.timeoutMs).color"
                  variant="tonal"
                >
                  {{ deciderSpeedOf(row, report.timeoutMs).label }}
                </v-chip>
                <div
                  v-if="deciderLatencyNoteOf(row)"
                  class="text-caption text-high-emphasis mt-1"
                  style="white-space: normal"
                >
                  {{ deciderLatencyNoteOf(row) }}
                </div>
              </td>
            </tr>
          </tbody>
        </v-table>
      </v-card>

      <!-- Como ler -->
      <v-card elevation="2" class="mt-4 pa-4">
        <div class="text-subtitle-2 font-weight-bold mb-2">
          Como ler os números do Decisor (Ollaya)
        </div>
        <div class="text-body-2 text-high-emphasis">
          <p class="mb-2">
            Cada modelo respondeu às mesmas <strong>{{ report.cases }}</strong> perguntas reais das
            seis frentes em que o Decisor atua no NetMonitor, em português, rodando em CPU sem placa
            de vídeo dedicada. A <strong>nota geral</strong> é a média das seis frentes com peso
            igual; o intervalo <strong>IC 90%</strong> é a margem de incerteza estatística da
            amostra.
          </p>
          <p class="mb-0">
            <strong>Na tela</strong> mede a precisão das sugestões que realmente aparecem para você
            (as que ficam abaixo da confiança mínima configurada são descartadas). Em
            <strong>Tempo em CPU</strong>, quando uma decisão passa de
            {{ Math.round(report.timeoutMs / 1000) }} segundos, o NetMonitor segue sem ela para não
            atrasar o chat nem a descoberta de rede.
          </p>
        </div>
      </v-card>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { DeciderBenchmarkReport } from '@/bindings/DeciderBenchmarkReport'
import type { DeciderBenchmarkRow } from '@/bindings/DeciderBenchmarkRow'
import { isModelInstalled } from '@/stores/aiDecider'
import {
  deciderLatencyNoteOf,
  deciderSpeedOf,
  deciderTagsOf,
  deciderTaskScoreOf,
  formatConfidenceInterval,
} from '@/utils/aiBenchmark'
import { accuracyColor, percent } from '@/utils/decider'

const props = defineProps<{
  report: DeciderBenchmarkReport | null
  loading: boolean
  error: string | null
  currentModel: string | null
}>()

type RankedDeciderRow = DeciderBenchmarkRow & { position: number }

const recommended = computed(
  () => props.report?.models.find((row) => row.recommended) ?? props.report?.models[0] ?? null
)

const rows = computed<RankedDeciderRow[]>(() =>
  (props.report?.models ?? []).map((row, idx) => ({ ...row, position: idx + 1 }))
)

function isCurrentModel(model: string): boolean {
  return Boolean(props.currentModel && isModelInstalled([props.currentModel], model))
}
</script>

<style scoped>
.benchmark-table th {
  white-space: nowrap;
}

.recommended-row {
  background: rgba(var(--v-theme-primary), 0.06);
}
</style>
