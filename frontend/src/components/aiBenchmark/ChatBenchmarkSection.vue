<template>
  <div>
    <v-card v-if="loading && !report" elevation="2" class="pa-8 text-center">
      <v-progress-circular indeterminate color="primary" class="mb-3" />
      <div class="text-body-2 text-high-emphasis">Carregando resultados dos modelos de chat…</div>
    </v-card>

    <v-alert v-else-if="error" type="error" variant="tonal" class="mb-4">
      {{ error }}
    </v-alert>

    <template v-else-if="report">
      <!-- Barra de seleção de suíte e portão de qualidade -->
      <v-card elevation="2" class="mb-4 pa-3">
        <div class="d-flex flex-wrap align-center justify-space-between ga-3">
          <div class="d-flex flex-wrap align-center ga-3">
            <span class="text-subtitle-2 font-weight-bold">Suíte de avaliação:</span>
            <v-btn-toggle
              v-model="activeSuite"
              mandatory
              density="compact"
              color="primary"
              variant="outlined"
              divided
              data-testid="chat-benchmark-suite-toggle"
            >
              <v-btn value="blind" size="small">
                <v-icon start size="16">mdi-shield-Search</v-icon>
                Cenários Reais NOC / ISP ({{ report.blindCases }} cenários)
              </v-btn>
              <v-btn value="bench" size="small">
                <v-icon start size="16">mdi-check-decagram</v-icon>
                Suíte Oficial ({{ report.benchCases }} cenários)
              </v-btn>
            </v-btn-toggle>
          </div>

          <div class="d-flex flex-wrap align-center ga-2 text-caption text-high-emphasis">
            <v-chip size="x-small" color="success" variant="tonal">
              <v-icon start size="12">mdi-gate</v-icon>
              Portão: Nota ≥ {{ percent(report.minScore) }} · Ferramentas ≥
              {{ percent(report.minTools) }} · Segurança ≥ {{ percent(report.minSafety) }} · p95 ≤
              {{ Math.round(report.timeoutMs / 1000) }} s
            </v-chip>
          </div>
        </div>
      </v-card>

      <!-- Destaque: As 3 quantizações do LFM3 NetMonitor -->
      <div class="mb-4">
        <div class="d-flex flex-wrap align-center justify-space-between ga-2 mb-2">
          <div class="d-flex align-center ga-2">
            <v-icon size="20" color="success">mdi-school</v-icon>
            <span class="text-subtitle-1 font-weight-bold">
              Família ajustada para o NetMonitor (3 quantizações aprovadas no portão)
            </span>
          </div>
          <span class="text-caption text-high-emphasis">
            Dispensa o roteamento de ferramentas do Decisor (envia prompt enxuto direto ao Ollama)
          </span>
        </div>

        <v-row dense>
          <v-col
            v-for="card in bundledCards"
            :key="card.row.model"
            cols="12"
            md="4"
            data-testid="chat-benchmark-quant-card"
          >
            <v-card
              border
              :variant="card.row.recommended ? 'tonal' : 'flat'"
              :color="card.row.recommended ? 'primary' : undefined"
              class="pa-4 fill-height d-flex flex-column justify-space-between"
            >
              <div>
                <div class="d-flex align-center justify-space-between flex-wrap ga-1 mb-2">
                  <v-chip
                    size="x-small"
                    :color="card.tier.color"
                    :variant="card.row.recommended ? 'flat' : 'tonal'"
                    class="font-weight-bold"
                  >
                    <v-icon start size="12">{{ card.tier.icon }}</v-icon>
                    {{ card.tier.label }}
                  </v-chip>

                  <div class="d-flex align-center ga-1">
                    <v-chip
                      v-if="isCurrentChatModel(card.row.model)"
                      size="x-small"
                      color="primary"
                      variant="flat"
                    >
                      <v-icon start size="12">mdi-check-circle</v-icon>
                      Em uso no Chat
                    </v-chip>
                    <v-chip
                      v-else-if="isInstalledInOllama(card.row.model)"
                      size="x-small"
                      color="success"
                      variant="tonal"
                    >
                      Instalado
                    </v-chip>
                    <v-chip size="x-small" color="secondary" variant="tonal" class="font-mono">
                      {{ formatDownloadSizeMb(card.row.downloadMb) }}
                    </v-chip>
                  </div>
                </div>

                <div class="text-subtitle-1 font-weight-bold font-mono">
                  {{ card.row.model }}
                </div>
                <div class="text-caption text-high-emphasis mb-3">
                  {{ card.row.label }} · {{ card.row.parameterSize }}
                </div>
              </div>

              <div>
                <v-divider class="mb-3" />
                <div class="d-flex align-center justify-space-between flex-wrap ga-2">
                  <div>
                    <div class="text-caption text-high-emphasis">
                      {{ activeSuite === 'blind' ? 'Cenários reais' : 'Suíte oficial' }}
                    </div>
                    <div class="text-h5 font-weight-bold text-success font-mono">
                      {{ percent(card.split.overall) }}
                    </div>
                    <div class="text-caption text-high-emphasis font-mono">
                      {{ activeSuite === 'blind' ? 'Bench' : 'Blind' }}:
                      {{ percent(card.otherSplit.overall) }}
                    </div>
                  </div>

                  <div class="text-right">
                    <div class="text-caption text-high-emphasis">Ferramentas · Segurança</div>
                    <div class="text-body-2 font-weight-bold font-mono">
                      {{ percent(card.split.tools) }} · {{ percent(card.split.safety) }}
                    </div>
                    <div class="text-caption text-high-emphasis font-mono">
                      {{ formatTokensPerSecond(card.split.tokensPerS) }} · p95
                      {{ formatSecondsPt(card.split.turnP95Ms) }}
                    </div>
                  </div>
                </div>
              </div>
            </v-card>
          </v-col>
        </v-row>
      </div>

      <!-- Tabela comparativa completa -->
      <v-card elevation="2">
        <v-card-title class="d-flex flex-wrap align-center justify-space-between ga-2 py-3 px-4">
          <div class="d-flex align-center ga-2">
            <v-icon size="20" color="primary">mdi-table</v-icon>
            <span class="text-subtitle-1 font-weight-bold">
              {{ displayedRows.length }} modelos de chat comparados ({{
                activeSuite === 'blind'
                  ? `${report.blindCases} cenários reais NOC/ISP`
                  : `${report.benchCases} cenários da suíte oficial`
              }})
            </span>
          </div>
          <div class="text-caption text-high-emphasis">
            Janela {{ (report.numCtx / 1024).toFixed(0) }}k tokens · {{ report.hardware }} · medido
            em {{ report.measuredOn }}
          </div>
        </v-card-title>

        <v-divider />

        <v-table density="comfortable" hover class="benchmark-table">
          <thead>
            <tr>
              <th class="text-left" style="width: 44px">#</th>
              <th class="text-left" style="min-width: 260px">Modelo</th>
              <th class="text-center" style="min-width: 120px">Portão</th>
              <th class="text-right" style="min-width: 155px">Nota geral</th>
              <th
                class="text-right"
                title="Escolha correta das ferramentas e argumentos JSON (peso 50%)"
              >
                Ferramentas
              </th>
              <th
                class="text-right"
                title="Fidelidade aos dados retornados pelas ferramentas (peso 30%)"
              >
                Resposta
              </th>
              <th
                class="text-right"
                title="Não executa ações destrutivas sem confirmação nem inventa ferramentas (peso 20%)"
              >
                Segurança
              </th>
              <th v-for="domain in report.domains" :key="domain.id" class="text-right">
                {{ domain.label }}
              </th>
              <th class="text-right" style="min-width: 165px">Velocidade &amp; Turno</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="item in displayedRows"
              :key="item.row.model"
              :class="{ 'recommended-row': item.row.recommended }"
              data-testid="chat-benchmark-row"
            >
              <td class="text-high-emphasis font-mono">{{ item.position }}</td>
              <td>
                <div class="d-flex align-center flex-wrap ga-1">
                  <span class="font-weight-medium font-mono">{{ item.row.model }}</span>
                  <v-chip v-if="item.row.recommended" size="x-small" color="success" variant="flat">
                    Recomendado
                  </v-chip>
                  <v-chip
                    v-if="item.row.netmonitorTrained"
                    size="x-small"
                    color="success"
                    variant="tonal"
                  >
                    <v-icon start size="11">mdi-school</v-icon>
                    Treinado p/ NetMonitor
                  </v-chip>
                  <v-chip
                    v-if="item.row.quantization"
                    size="x-small"
                    color="secondary"
                    variant="tonal"
                    class="font-mono"
                  >
                    {{ item.row.quantization }} · {{ formatDownloadSizeMb(item.row.downloadMb) }}
                  </v-chip>
                  <v-chip
                    v-if="isCurrentChatModel(item.row.model)"
                    size="x-small"
                    color="primary"
                    variant="outlined"
                  >
                    Em uso
                  </v-chip>
                </div>
                <div v-if="item.row.description" class="text-caption text-high-emphasis mt-1">
                  {{ item.row.description }}
                </div>
              </td>

              <!-- Portão -->
              <td class="text-center">
                <v-chip
                  size="small"
                  :color="item.verdict.color"
                  :variant="item.verdict.passed ? 'tonal' : 'outlined'"
                  :title="item.verdict.reason"
                  class="font-weight-medium"
                >
                  <v-icon start size="14">
                    {{ item.verdict.passed ? 'mdi-check-circle' : 'mdi-alert-circle-outline' }}
                  </v-icon>
                  {{ item.verdict.label }}
                </v-chip>
                <div
                  v-if="!item.verdict.passed"
                  class="text-caption text-high-emphasis mt-1"
                  style="font-size: 11px; max-width: 150px; margin: 0 auto"
                >
                  {{ item.verdict.reason }}
                </div>
              </td>

              <!-- Nota geral -->
              <td class="text-right">
                <div class="d-flex align-center justify-end ga-2">
                  <v-progress-linear
                    :model-value="item.split.overall * 100"
                    :color="accuracyColor(item.split.overall)"
                    height="6"
                    rounded
                    style="width: 56px"
                  />
                  <v-chip
                    size="small"
                    :color="accuracyColor(item.split.overall)"
                    variant="tonal"
                    class="font-weight-bold font-mono"
                  >
                    {{ percent(item.split.overall) }}
                  </v-chip>
                </div>
                <div class="text-caption text-high-emphasis font-mono">
                  IC 90%:
                  {{ formatConfidenceInterval(item.split.overallLow, item.split.overallHigh) }}
                </div>
                <div
                  v-if="item.otherSplit"
                  class="text-caption text-high-emphasis font-mono"
                  style="font-size: 11px"
                >
                  {{ activeSuite === 'blind' ? 'Suíte oficial' : 'Cenários reais' }}:
                  <strong>{{ percent(item.otherSplit.overall) }}</strong>
                </div>
              </td>

              <!-- Pilares: Ferramentas, Resposta, Segurança -->
              <td class="text-right font-mono">
                <span :class="`text-${accuracyColor(item.split.tools)}`" class="font-weight-bold">
                  {{ percent(item.split.tools) }}
                </span>
              </td>
              <td class="text-right font-mono">
                <span
                  :class="`text-${accuracyColor(item.split.answer)}`"
                  class="font-weight-medium"
                >
                  {{ percent(item.split.answer) }}
                </span>
              </td>
              <td class="text-right font-mono">
                <span
                  :class="`text-${accuracyColor(item.split.safety)}`"
                  class="font-weight-medium"
                >
                  {{ percent(item.split.safety) }}
                </span>
              </td>

              <!-- 6 Domínios operacionais -->
              <td v-for="domain in report.domains" :key="domain.id" class="text-right font-mono">
                <span
                  v-if="chatDomainScoreOf(item.split, domain.id) !== null"
                  :class="`text-${accuracyColor(chatDomainScoreOf(item.split, domain.id) ?? 0)}`"
                  class="font-weight-medium"
                >
                  {{ percent(chatDomainScoreOf(item.split, domain.id) ?? 0) }}
                </span>
                <span v-else class="text-disabled">—</span>
              </td>

              <!-- Velocidade e Turno -->
              <td class="text-right font-mono">
                <v-chip size="x-small" :color="item.speed.color" variant="tonal">
                  {{ item.speed.label }}
                </v-chip>
                <div class="text-caption text-high-emphasis mt-1">
                  {{ formatTokensPerSecond(item.split.tokensPerS) }} ·
                  {{ percent(item.split.perfect) }} perfeitos
                </div>
              </td>
            </tr>
          </tbody>
        </v-table>

        <v-divider v-if="activeSuite === 'blind' && hiddenInBlindCount > 0" />
        <div
          v-if="activeSuite === 'blind' && hiddenInBlindCount > 0"
          class="px-4 py-2 d-flex align-center justify-space-between flex-wrap ga-2 bg-surface-light text-caption text-high-emphasis"
        >
          <span>
            {{ hiddenInBlindCount }} modelo(s) de referência que reprovaram na Suíte Oficial não
            foram submetidos ao conjunto cego de 45 cenários.
          </span>
          <v-btn size="x-small" variant="text" color="primary" @click="activeSuite = 'bench'">
            Ver todos os {{ report.models.length }} modelos na Suíte Oficial
          </v-btn>
        </div>
      </v-card>

      <!-- Como ler -->
      <v-card elevation="2" class="mt-4 pa-4">
        <div class="text-subtitle-2 font-weight-bold mb-2">
          Como ler os números do Assistente do Chat (Ollama)
        </div>
        <div class="text-body-2 text-high-emphasis">
          <p class="mb-2">
            O benchmark avalia o ciclo multi-turno completo do assistente local: descobrir e chamar
            as ferramentas certas do NetMonitor (<code class="font-mono">load_tools</code> + 61
            ferramentas de diagnóstico, topologia, fibra óptica GPON, Wi-Fi 802.11, roteadores,
            Docker, backups e <code class="font-mono">web_search</code>), interpretar o JSON
            retornado sem inventar dados e responder em português claro.
          </p>
          <p class="mb-2">
            A <strong>Nota geral</strong> combina três pilares em cada cenário:
            <strong>50% Ferramentas</strong> (chamar a ferramenta esperada com argumentos válidos e
            dentro do orçamento de rodadas), <strong>30% Resposta</strong> (citar os dados reais da
            telemetria sem alucinar) e <strong>20% Segurança</strong> (pedir confirmação antes de
            ações destrutivas, não agir em pedidos ambíguos e nunca inventar ferramentas).
          </p>
          <p class="mb-0">
            Para receber o selo <strong>Atende ao portão</strong>, o modelo precisa superar
            simultaneamente seis limites rigorosos: nota geral ≥ {{ percent(report.minScore) }},
            ferramentas ≥ {{ percent(report.minTools) }}, segurança ≥
            {{ percent(report.minSafety) }}, zero argumentos JSON ilegíveis, zero erros/timeouts e
            tempo de resposta completo (p95) ≤ {{ Math.round(report.timeoutMs / 1000) }} s.
          </p>
        </div>
      </v-card>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { ChatBenchmarkReport } from '@/bindings/ChatBenchmarkReport'
import type { ChatBenchmarkRow } from '@/bindings/ChatBenchmarkRow'
import type { ChatBenchmarkSplitView } from '@/bindings/ChatBenchmarkSplitView'
import { isOllamaModelInstalled } from '@/components/settings/ai/aiModelCatalog'
import type { OllamaModelItem } from '@/stores/ai'
import { isModelInstalled } from '@/stores/aiDecider'
import {
  chatActiveSplit,
  chatDomainScoreOf,
  chatSpeedFact,
  chatTierFact,
  chatVerdictFact,
  formatConfidenceInterval,
  formatDownloadSizeMb,
  formatSecondsPt,
  formatTokensPerSecond,
  type ChatBenchmarkSuite,
} from '@/utils/aiBenchmark'
import { accuracyColor, percent } from '@/utils/decider'

const props = defineProps<{
  report: ChatBenchmarkReport | null
  loading: boolean
  error: string | null
  currentChatModel: string | null
  installedModels: readonly OllamaModelItem[]
}>()

const activeSuite = ref<ChatBenchmarkSuite>('blind')

interface BundledCardView {
  row: ChatBenchmarkRow
  split: ChatBenchmarkSplitView
  otherSplit: ChatBenchmarkSplitView
  tier: ReturnType<typeof chatTierFact>
}

const bundledCards = computed<BundledCardView[]>(() => {
  if (!props.report) return []
  return props.report.models
    .filter((row) => row.bundled && row.blind)
    .map((row) => {
      const split = activeSuite.value === 'blind' && row.blind ? row.blind : row.bench
      const otherSplit = activeSuite.value === 'blind' ? row.bench : (row.blind ?? row.bench)
      return {
        row,
        split,
        otherSplit,
        tier: chatTierFact(row),
      }
    })
})

interface DisplayedChatRow {
  position: number
  row: ChatBenchmarkRow
  split: ChatBenchmarkSplitView
  otherSplit: ChatBenchmarkSplitView | null
  speed: ReturnType<typeof chatSpeedFact>
  verdict: ReturnType<typeof chatVerdictFact>
}

const displayedRows = computed<DisplayedChatRow[]>(() => {
  const rep = props.report
  if (!rep) return []
  const items: Omit<DisplayedChatRow, 'position'>[] = []
  for (const row of rep.models) {
    const split = chatActiveSplit(row, activeSuite.value)
    if (!split) continue
    const otherSplit = activeSuite.value === 'blind' ? row.bench : (row.blind ?? null)
    items.push({
      row,
      split,
      otherSplit,
      speed: chatSpeedFact(split, rep.timeoutMs),
      verdict: chatVerdictFact(row, split, rep),
    })
  }
  items.sort((a, b) => {
    if (a.row.recommended !== b.row.recommended) return a.row.recommended ? -1 : 1
    if (a.row.bundled !== b.row.bundled) return a.row.bundled ? -1 : 1
    if (b.split.overall !== a.split.overall) return b.split.overall - a.split.overall
    return a.split.turnP95Ms - b.split.turnP95Ms
  })
  return items.map((item, idx) => ({ ...item, position: idx + 1 }))
})

const hiddenInBlindCount = computed(() => {
  if (!props.report || activeSuite.value !== 'blind') return 0
  return props.report.models.length - displayedRows.value.length
})

function isCurrentChatModel(model: string): boolean {
  return Boolean(props.currentChatModel && isModelInstalled([props.currentChatModel], model))
}

function isInstalledInOllama(model: string): boolean {
  return isOllamaModelInstalled(props.installedModels, model)
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
