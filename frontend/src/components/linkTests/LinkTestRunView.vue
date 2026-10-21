<template>
  <div class="run-view">
    <LinkTestPathMap
      :node-ids="path.nodeIds"
      :edge-ids="path.edgeIds"
      :overlay="overlay"
      :max-height="mapHeight"
      :health-labels="healthLabels"
    />

    <div class="sr-only" aria-live="polite" aria-atomic="true">{{ announcement }}</div>

    <v-alert v-if="active && test.error" type="error" variant="tonal" density="compact">
      {{ test.error }}
    </v-alert>

    <section v-if="active" class="live" aria-label="Medição em andamento">
      <div v-if="test.status === 'queued'" class="waiting">
        <v-progress-circular
          indeterminate
          color="primary"
          size="22"
          width="3"
        ></v-progress-circular>
        <span class="text-body-large">
          Aguardando a vez — no máximo dois testes rodam ao mesmo tempo.
        </span>
      </div>
      <template v-else-if="leg">
        <div class="now">
          <span class="text-title-small font-weight-bold">{{ legHeading }}</span>
          <ol class="pipeline" aria-label="Etapas da medição">
            <li
              v-for="stage in leg.stages"
              :key="stage.key"
              class="step"
              :class="`step-${stage.status}`"
              :title="stage.message ?? undefined"
            >
              <v-progress-circular
                v-if="stage.status === 'running'"
                indeterminate
                size="14"
                width="2"
                color="primary"
              ></v-progress-circular>
              <v-icon v-else size="16" :color="stepStatusPresentation(stage.status).color">
                {{
                  stage.status === 'pending'
                    ? STAGES[stage.key].icon
                    : stepStatusPresentation(stage.status).icon
                }}
              </v-icon>
              <span>{{ stageLabel(stage.key) }}</span>
            </li>
          </ol>
        </div>

        <div class="gauges">
          <div
            v-for="gauge in gauges"
            :key="gauge.flow"
            class="gauge"
            :class="{ active: gauge.active }"
          >
            <span class="gauge-label">
              <v-icon size="18" :color="gauge.flow === 'forward' ? 'success' : 'info'">
                {{ gauge.flow === 'forward' ? 'mdi-arrow-down-bold' : 'mdi-arrow-up-bold' }}
              </v-icon>
              {{ gauge.flow === 'forward' ? 'Ida' : 'Volta' }}
              <span class="gauge-route">{{ gauge.route }}</span>
            </span>
            <strong class="gauge-value">{{ gauge.value }}</strong>
          </div>
        </div>

        <LinkTestTrafficChart
          v-if="hasSamples || liveHealth.length > 0"
          :forward="forwardSamples"
          :reverse="reverseSamples"
          :health="liveHealth"
          :names="names"
          :order="path.nodeIds"
        />
      </template>
      <p v-else class="text-body-medium">Montando o plano do teste…</p>

      <v-alert type="warning" variant="tonal" density="compact" icon="mdi-speedometer">
        O enlace fica saturado enquanto mede — quem usa a rede pode sentir lentidão.
      </v-alert>
    </section>

    <LinkTestResultView
      v-else
      :test="test"
      :speed-of="ctx.speedOf"
      :names="names"
      :order="path.nodeIds"
      @requested="$emit('requested')"
    />

    <v-expansion-panels v-model="detailsOpen" variant="accordion" class="details">
      <v-expansion-panel value="details">
        <v-expansion-panel-title>
          <v-icon size="18" color="primary" class="mr-2">mdi-text-box-search-outline</v-icon>
          Detalhes técnicos
          <span class="details-meta text-body-small ml-2">
            {{ test.legs.length }} {{ test.legs.length === 1 ? 'perna' : 'pernas' }} ·
            {{ specText }}
          </span>
        </v-expansion-panel-title>
        <v-expansion-panel-text>
          <ol class="legs" aria-label="Pernas do teste">
            <LinkTestLegItem
              v-for="(item, index) in test.legs"
              :key="item.seq"
              :leg="item"
              :direction="test.spec.direction"
              :forward-samples="store.samplesFor(test.id, item.seq, 'forward')"
              :reverse-samples="store.samplesFor(test.id, item.seq, 'reverse')"
              :health="store.healthFor(test.id, item.seq)"
              :names="names"
              :order="path.nodeIds"
              :speed-bps="legSpeedBps(item, ctx.speedOf)"
              :bottleneck="!active && bottleneckSeq === item.seq && test.legs.length > 1"
              :last="index === test.legs.length - 1"
              @open-run="(runId, runTitle, device) => stageRun?.open(runId, runTitle, device)"
            />
          </ol>
          <div class="text-body-small mt-2">{{ originText }}</div>
        </v-expansion-panel-text>
      </v-expansion-panel>
    </v-expansion-panels>

    <LinkTestStageRun ref="stageRun" />
  </div>
</template>

<script setup lang="ts">
/**
 * O corpo da tela do teste (ao vivo ou concluído), usado pela topologia e
 * pelo histórico: o mini mapa animado do caminho em cima; durante a medição,
 * a etapa atual, os números grandes e o gráfico; no fim, o resultado com o
 * resumo primeiro. Pernas, etapas e transcrições ficam em "Detalhes
 * técnicos", recolhidos.
 */
import { computed, ref } from 'vue'
import { useDisplay } from 'vuetify'
import { useLinkTestContext } from '@/composables/linkTests/useLinkTestContext'
import { useLinkTestsStore } from '@/stores/linkTests'
import type { LinkTest, LinkTestStageKey } from '@/types/linkTests'
import { formatDateTime } from '@/utils/formatters'
import { legGauges } from '@/utils/linkTests/gauges'
import { overlayForTest, pathOf } from '@/utils/linkTests/overlay'
import { testDeviceNames, testHealthLabels } from '@/utils/linkTests/health'
import {
  STAGES,
  isActiveStatus,
  legTitle,
  stepStatusPresentation,
  testStatusPresentation,
} from '@/utils/linkTests/presentation'
import { currentLeg, legsDone } from '@/utils/linkTests/progress'
import { bottleneckLeg, legSpeedBps } from '@/utils/linkTests/results'
import { specSummary } from '@/utils/linkTests/spec'
import LinkTestLegItem from './LinkTestLegItem.vue'
import LinkTestPathMap from './LinkTestPathMap.vue'
import LinkTestResultView from './LinkTestResultView.vue'
import LinkTestStageRun from './LinkTestStageRun.vue'
import LinkTestTrafficChart from './LinkTestTrafficChart.vue'

const props = defineProps<{ test: LinkTest }>()
defineEmits<{ requested: [] }>()

const store = useLinkTestsStore()
const ctx = useLinkTestContext()
const { smAndDown } = useDisplay()
const stageRun = ref<InstanceType<typeof LinkTestStageRun> | null>(null)
const detailsOpen = ref<string[] | string | null>(null)

const active = computed(() => isActiveStatus(props.test.status))
const mapHeight = computed(() => (smAndDown.value ? 420 : 560))
const path = computed(() => pathOf(props.test.legs))
const overlay = computed(() =>
  overlayForTest(props.test, ctx.liveRate(props.test.id), ctx.edgeSource)
)

const leg = computed(() => currentLeg(props.test))
const legHeading = computed(() => {
  const current = leg.value
  if (!current) return ''
  if (props.test.legs.length === 1) return legTitle(current)
  return `Enlace ${legsDone(props.test) + 1} de ${props.test.legs.length} · ${legTitle(current)}`
})

/** Rótulo curto na linha de etapas (o completo fica nos detalhes). */
const SHORT_STAGES: Record<LinkTestStageKey, string> = {
  prepare_a: 'Conferir A',
  prepare_b: 'Conferir B',
  serve: 'Servidor',
  forward: 'Ida',
  reverse: 'Volta',
  stop: 'Encerrar',
}
function stageLabel(key: LinkTestStageKey): string {
  return SHORT_STAGES[key] ?? STAGES[key].label
}

const forwardSamples = computed(() =>
  leg.value ? store.samplesFor(props.test.id, leg.value.seq, 'forward') : []
)
const reverseSamples = computed(() =>
  leg.value ? store.samplesFor(props.test.id, leg.value.seq, 'reverse') : []
)
const gauges = computed(() =>
  leg.value
    ? legGauges(
        leg.value,
        props.test.spec.direction,
        forwardSamples.value,
        reverseSamples.value,
        legSpeedBps(leg.value, ctx.speedOf)
      )
    : []
)
const hasSamples = computed(
  () => forwardSamples.value.length > 0 || reverseSamples.value.length > 0
)
const liveHealth = computed(() => (leg.value ? store.healthFor(props.test.id, leg.value.seq) : []))
const names = computed(() => testDeviceNames(props.test))
/**
 * CPU/RAM no recorte do mapa, de todas as pernas: ao vivo, a leitura mais
 * recente de cada equipamento (não some entre uma perna e outra); no fim, o
 * antes e o pico (sem leitura, o equipamento mostra a do mapa).
 */
const healthLabels = computed(() =>
  testHealthLabels(
    props.test.legs.flatMap((item) => store.healthFor(props.test.id, item.seq)),
    active.value
  )
)

const bottleneckSeq = computed(() => bottleneckLeg(props.test)?.seq ?? null)
const specText = computed(() => specSummary(props.test.spec))
const originText = computed(() => {
  const by =
    props.test.origin === 'schedule' ? 'agendamento' : (props.test.requestedBy?.name ?? 'usuário')
  return `Pedido por ${by} em ${formatDateTime(props.test.createdAt)} · teste #${props.test.id}`
})

/** Anúncio para leitor de tela: muda a cada etapa, não a cada amostra. */
const announcement = computed(() => {
  const status = testStatusPresentation(props.test.status).label
  if (!active.value) return `Teste ${status.toLowerCase()}.`
  const running = leg.value?.stages.find((stage) => stage.status === 'running')
  return running ? `${status}: ${stageLabel(running.key)}.` : status
})
</script>

<style scoped>
.run-view {
  display: flex;
  flex-direction: column;
  gap: 16px;
  color: rgb(var(--v-theme-on-surface));
}
.live {
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.waiting {
  display: flex;
  align-items: center;
  gap: 10px;
}
.now {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.pipeline {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.step {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-radius: 999px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.24);
  font-size: 0.8125rem;
  font-weight: 600;
}
.step-running {
  border-color: rgb(var(--v-theme-primary));
  box-shadow: 0 0 0 1px rgb(var(--v-theme-primary));
}
.step-succeeded {
  border-color: rgb(var(--v-theme-success));
}
.step-failed {
  border-color: rgb(var(--v-theme-error));
}
.step-skipped {
  border-color: rgb(var(--v-theme-warning));
}
.gauges {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 12px;
}
.gauge {
  display: flex;
  flex-direction: column;
  padding: 14px 16px;
  border-radius: 14px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.2);
  transition:
    border-color 0.25s ease,
    box-shadow 0.25s ease;
}
.gauge.active {
  border-color: rgb(var(--v-theme-primary));
  box-shadow: 0 0 0 2px rgba(var(--v-theme-primary), 0.5);
}
.gauge-label {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  font-weight: 700;
}
.gauge-route {
  font-weight: 500;
  font-size: 0.8125rem;
}
.gauge-value {
  font-size: 2.4rem;
  font-weight: 800;
  line-height: 1.15;
  font-variant-numeric: tabular-nums;
}
.details-meta {
  color: rgb(var(--v-theme-on-surface));
}
.legs {
  padding: 0;
  margin: 0;
}
.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}
</style>
