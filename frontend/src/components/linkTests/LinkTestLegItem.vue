<template>
  <li class="leg" :class="{ 'leg-bottleneck': bottleneck }" :aria-label="ariaLabel">
    <div class="leg-marker" aria-hidden="true">
      <v-avatar :color="status.color" size="30">
        <v-progress-circular
          v-if="leg.status === 'running'"
          indeterminate
          size="18"
          width="2"
          color="white"
        ></v-progress-circular>
        <span v-else class="seq">{{ leg.seq }}</span>
      </v-avatar>
      <span v-if="!last" class="rail"></span>
    </div>

    <div class="leg-body">
      <div class="d-flex align-center flex-wrap ga-1">
        <span class="leg-title text-title-small font-weight-bold">{{ title }}</span>
        <v-spacer></v-spacer>
        <LinkTestStatusChip :status="leg.status" scope="step" size="x-small" />
      </div>
      <div class="d-flex align-center flex-wrap ga-1 mt-1">
        <v-chip v-if="leg.kind === 'end_to_end'" size="x-small" color="info" variant="flat" label>
          Ponta a ponta
        </v-chip>
        <v-chip
          v-if="leg.engine"
          size="x-small"
          :color="ENGINES[leg.engine].color"
          variant="flat"
          prepend-icon="mdi-engine"
          label
        >
          {{ engineLabel(leg.engine) }}
        </v-chip>
        <v-chip
          v-if="bottleneck"
          size="x-small"
          color="warning"
          variant="flat"
          prepend-icon="mdi-alert-decagram"
          label
        >
          Gargalo
        </v-chip>
        <span v-if="serverText" class="meta text-body-small">{{ serverText }}</span>
      </div>

      <div v-if="gauges.length > 0" class="gauges mt-2">
        <div
          v-for="gauge in gauges"
          :key="gauge.flow"
          class="gauge"
          :class="{ 'gauge-active': gauge.active }"
        >
          <div class="gauge-label text-label-small">
            <v-icon size="14" :color="gauge.active ? 'primary' : undefined">{{
              gauge.icon
            }}</v-icon>
            {{ gauge.label }}
          </div>
          <div class="gauge-value">{{ gauge.value }}</div>
          <v-chip
            v-if="gauge.comparison"
            :color="gauge.comparison.color"
            size="x-small"
            variant="flat"
            label
            class="mt-1"
          >
            {{ gauge.comparison.text }}
          </v-chip>
        </div>
      </div>

      <div v-if="!compact && hasSamples" class="spark mt-2">
        <MonitorSparkline
          :in-data="forwardSeries"
          :out-data="reverseSeries"
          in-label="Ida"
          out-label="Volta"
          in-color="#4ade80"
          out-color="#38bdf8"
          width="100%"
          :height="44"
          unit="bps"
          :format-value="compactBps"
        />
      </div>

      <div v-if="leg.stages.length > 0" class="stages mt-2" role="list" aria-label="Etapas">
        <template v-for="stage in leg.stages" :key="stage.key">
          <v-chip
            role="listitem"
            size="small"
            :color="stepStatusPresentation(stage.status).color"
            :variant="stage.status === 'pending' ? 'outlined' : 'flat'"
            :prepend-icon="
              stage.status === 'running'
                ? undefined
                : stage.status === 'pending'
                  ? stageIcon(stage.key)
                  : stepStatusPresentation(stage.status).icon
            "
            :append-icon="stage.runId !== null ? 'mdi-console' : undefined"
            :title="stageTooltip(stage)"
            :aria-label="stageTooltip(stage)"
            label
            v-bind="stage.runId !== null ? { onClick: () => openRun(stage) } : {}"
          >
            <v-progress-circular
              v-if="stage.status === 'running'"
              indeterminate
              size="12"
              width="2"
              class="mr-1"
            ></v-progress-circular>
            {{ stage.label || STAGES[stage.key].label }}
          </v-chip>
        </template>
      </div>

      <ul v-if="notes.length > 0" class="notes mt-2">
        <li v-for="(note, index) in notes" :key="index" :class="`note-${note.color}`">
          <v-icon size="14" :color="note.color">{{ note.icon }}</v-icon>
          <span>{{ note.text }}</span>
        </li>
      </ul>
      <div v-if="leg.retransmits !== null && !compact" class="meta text-body-small mt-1">
        Retransmissões TCP: {{ leg.retransmits }}
      </div>
    </div>
  </li>
</template>

<script setup lang="ts">
/** Uma perna do teste: pontas, motor, medidores ao vivo, gráfico e etapas. */
import { computed } from 'vue'
import type { LinkTestFlow, LinkTestLeg, LinkTestSample, LinkTestStage } from '@/types/linkTests'
import type { ThemeColorToken } from '@/utils/topology/presentation'
import {
  ENGINES,
  STAGES,
  compactBps,
  endpointLabel,
  engineLabel,
  legTitle,
  serverSideLabel,
  stageEndpoint,
  stepStatusPresentation,
} from '@/utils/linkTests/presentation'
import { resultVsSpeed, type SpeedComparison } from '@/utils/linkTests/results'
import { runningLegFlow } from '@/utils/linkTests/overlay'
import MonitorSparkline from '@/components/MonitorSparkline.vue'
import LinkTestStatusChip from './LinkTestStatusChip.vue'

const props = defineProps<{
  leg: LinkTestLeg
  /** Direções pedidas no teste (só mostra os medidores pedidos). */
  direction: 'both' | 'forward' | 'reverse'
  forwardSamples: LinkTestSample[]
  reverseSamples: LinkTestSample[]
  speedBps: number | null
  bottleneck?: boolean
  last?: boolean
  compact?: boolean
}>()

const emit = defineEmits<{ 'open-run': [runId: number, title: string, deviceName: string] }>()

const status = computed(() => stepStatusPresentation(props.leg.status))
const title = computed(() => legTitle(props.leg))
const serverText = computed(() => serverSideLabel(props.leg))
const runningFlow = computed(() =>
  props.leg.status === 'running' ? runningLegFlow(props.leg) : null
)

function lastBps(list: LinkTestSample[]): number | null {
  return list.length > 0 ? list[list.length - 1].bps : null
}

interface Gauge {
  flow: LinkTestFlow
  label: string
  icon: string
  value: string
  active: boolean
  comparison: SpeedComparison | null
}

const gauges = computed<Gauge[]>(() => {
  const leg = props.leg
  const running = leg.status === 'running'
  if (!running && leg.forwardBps === null && leg.reverseBps === null) return []
  const flows: LinkTestFlow[] =
    props.direction === 'both' ? ['forward', 'reverse'] : [props.direction]
  return flows.map((flow) => {
    const final = flow === 'forward' ? leg.forwardBps : leg.reverseBps
    const live = lastBps(flow === 'forward' ? props.forwardSamples : props.reverseSamples)
    const active = runningFlow.value === flow
    const shown = active ? (live ?? final) : (final ?? (running ? live : null))
    return {
      flow,
      label:
        flow === 'forward'
          ? `↓ Ida · ${endpointLabel(leg.a)} → ${endpointLabel(leg.b)}`
          : `↑ Volta · ${endpointLabel(leg.b)} → ${endpointLabel(leg.a)}`,
      icon: flow === 'forward' ? 'mdi-arrow-right-bold' : 'mdi-arrow-left-bold',
      value:
        shown !== null ? compactBps(shown) : active ? 'medindo…' : running ? 'aguardando' : '—',
      active,
      comparison: !running && final !== null ? resultVsSpeed(final, props.speedBps) : null,
    }
  })
})

const hasSamples = computed(
  () => props.forwardSamples.length > 0 || props.reverseSamples.length > 0
)
const forwardSeries = computed(() =>
  props.forwardSamples.map((sample) => ({ value: sample.bps, recordedAt: '' }))
)
const reverseSeries = computed(() =>
  props.reverseSamples.map((sample) => ({ value: sample.bps, recordedAt: '' }))
)

function stageIcon(key: LinkTestStage['key']): string {
  return STAGES[key]?.icon ?? 'mdi-circle-outline'
}

function stageTooltip(stage: LinkTestStage): string {
  const parts = [
    `${stage.label || STAGES[stage.key].label}: ${stepStatusPresentation(stage.status).label}`,
  ]
  if (stage.message) parts.push(stage.message)
  if (stage.runId !== null) parts.push('clique para ver a execução')
  return parts.join(' — ')
}

function openRun(stage: LinkTestStage) {
  if (stage.runId === null) return
  const endpoint = stageEndpoint(props.leg, stage.key)
  emit(
    'open-run',
    stage.runId,
    `${stage.label || STAGES[stage.key].label} · ${title.value}`,
    endpointLabel(endpoint)
  )
}

interface Note {
  text: string
  icon: string
  color: ThemeColorToken
}

const notes = computed<Note[]>(() => {
  const list: Note[] = []
  if (props.leg.error)
    list.push({ text: props.leg.error, icon: 'mdi-close-circle', color: 'error' })
  for (const issue of props.leg.issues) {
    list.push({ text: issue, icon: 'mdi-alert', color: 'warning' })
  }
  for (const stage of props.leg.stages) {
    if (stage.message && (stage.status === 'failed' || stage.status === 'skipped')) {
      list.push({
        text: `${stage.label || STAGES[stage.key].label}: ${stage.message}`,
        icon: stage.status === 'failed' ? 'mdi-close-circle' : 'mdi-debug-step-over',
        color: stage.status === 'failed' ? 'error' : 'warning',
      })
    }
  }
  return list
})

const ariaLabel = computed(() => `Perna ${props.leg.seq}: ${title.value}, ${status.value.label}`)
</script>

<style scoped>
.leg {
  display: flex;
  gap: 12px;
  list-style: none;
}
.leg-marker {
  display: flex;
  flex-direction: column;
  align-items: center;
  flex-shrink: 0;
}
.seq {
  color: #fff;
  font-weight: 700;
  font-size: 0.85rem;
}
.rail {
  flex: 1;
  width: 2px;
  min-height: 16px;
  margin: 4px 0;
  background: rgba(var(--v-theme-on-surface), 0.28);
}
.leg-body {
  flex: 1;
  min-width: 0;
  padding-bottom: 16px;
}
.leg-bottleneck .leg-body {
  border-left: 3px solid rgb(var(--v-theme-warning));
  padding-left: 8px;
}
.leg-title {
  color: rgb(var(--v-theme-on-surface));
  word-break: break-word;
}
.meta {
  color: rgb(var(--v-theme-on-surface));
}
.gauges {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 8px;
}
.gauge {
  border: 1px solid rgba(var(--v-theme-on-surface), 0.22);
  border-radius: 10px;
  padding: 8px 10px;
  background: rgb(var(--v-theme-surface));
}
.gauge-active {
  border-color: rgb(var(--v-theme-primary));
  box-shadow: 0 0 0 1px rgb(var(--v-theme-primary));
}
.gauge-label {
  display: flex;
  align-items: center;
  gap: 4px;
  color: rgb(var(--v-theme-on-surface));
  word-break: break-word;
}
.gauge-value {
  font-size: 1.35rem;
  font-weight: 800;
  line-height: 1.3;
  font-variant-numeric: tabular-nums;
  color: rgb(var(--v-theme-on-surface));
}
.stages {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.notes {
  list-style: none;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.notes li {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  font-size: 0.8125rem;
  color: rgb(var(--v-theme-on-surface));
}
</style>
