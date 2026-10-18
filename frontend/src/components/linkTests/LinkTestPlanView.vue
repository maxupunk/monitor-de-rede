<template>
  <div class="plan-view">
    <LinkTestFixList :fixes="plan.fixes" :refreshing="refreshing" @fixed="emit('fixed')" />

    <div class="facts mb-3">
      <div class="fact">
        <v-icon color="primary">mdi-timer-outline</v-icon>
        <div>
          <div class="text-label-small">Tempo estimado</div>
          <div class="text-title-small font-weight-bold">{{ estimate }}</div>
        </div>
      </div>
      <div class="fact">
        <v-icon color="secondary">mdi-transit-connection-variant</v-icon>
        <div>
          <div class="text-label-small">Pernas</div>
          <div class="text-title-small font-weight-bold">
            {{ runnableCount }} de {{ plan.legs.length }} vão rodar
          </div>
        </div>
      </div>
      <div class="fact">
        <v-icon color="info">mdi-tune-variant</v-icon>
        <div>
          <div class="text-label-small">Parâmetros</div>
          <div class="text-title-small font-weight-bold">{{ specText }}</div>
        </div>
      </div>
    </div>

    <v-alert
      v-if="blockingHints.length > 0"
      type="error"
      variant="tonal"
      class="mb-3"
      title="O teste não pode rodar assim"
    >
      <ul class="issues">
        <li v-for="(hint, index) in blockingHints" :key="index">
          <span>{{ hint.text }}</span>
          <v-btn
            v-if="hint.to"
            :to="hint.to"
            color="error"
            variant="flat"
            size="x-small"
            class="ml-2"
            append-icon="mdi-arrow-right"
          >
            {{ hint.actionLabel }}
          </v-btn>
        </li>
      </ul>
    </v-alert>

    <v-alert
      v-if="runnableCount > 0"
      type="warning"
      variant="tonal"
      icon="mdi-speedometer"
      class="mb-3"
    >
      O teste <strong>satura o enlace durante a medição</strong>: por {{ estimate }}, quem usa a
      rede por ele pode sentir lentidão. Prefira horários de pouco uso.
    </v-alert>

    <v-alert
      v-if="plan.warnings.length > 0"
      type="warning"
      variant="tonal"
      class="mb-3"
      title="Atenção"
    >
      <ul class="issues">
        <li v-for="(warning, index) in plan.warnings" :key="index">{{ warning }}</li>
      </ul>
    </v-alert>

    <v-alert
      v-if="plan.willInstall.length > 0"
      type="info"
      variant="tonal"
      icon="mdi-package-down"
      class="mb-3"
      title="Vai instalar"
    >
      <ul class="issues">
        <li v-for="item in plan.willInstall" :key="item.deviceId">
          <strong>{{ item.packages.join(', ') }}</strong> em {{ item.deviceName }}
        </li>
      </ul>
    </v-alert>

    <div class="text-title-small font-weight-bold mb-2">Ordem das medições</div>
    <ol class="plan-legs">
      <li
        v-for="leg in plan.legs"
        :key="leg.seq"
        class="plan-leg"
        :class="{ blocked: !leg.runnable && !legNeedsSteps(leg), waiting: legNeedsSteps(leg) }"
      >
        <v-avatar :color="legColor(leg)" size="28" class="flex-shrink-0">
          <span class="seq">{{ leg.seq }}</span>
        </v-avatar>
        <div class="flex-grow-1 overflow-hidden">
          <div class="d-flex align-center flex-wrap ga-1">
            <v-icon size="18" color="primary">{{ endpointIcon(leg.a) }}</v-icon>
            <span class="font-weight-bold">{{ endpointLabel(leg.a) }}</span>
            <v-icon size="18">mdi-arrow-right</v-icon>
            <v-icon size="18" color="primary">{{ endpointIcon(leg.b) }}</v-icon>
            <span class="font-weight-bold">{{ endpointLabel(leg.b) }}</span>
          </div>
          <div class="d-flex align-center flex-wrap ga-1 mt-1">
            <v-chip
              v-if="leg.kind === 'end_to_end'"
              size="x-small"
              color="info"
              variant="flat"
              label
            >
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
              v-if="legNeedsSteps(leg)"
              size="x-small"
              color="warning"
              variant="flat"
              prepend-icon="mdi-format-list-checks"
              label
            >
              Falta um passo
            </v-chip>
            <v-chip v-else-if="!leg.runnable" size="x-small" color="error" variant="flat" label>
              Não vai rodar
            </v-chip>
            <span class="text-body-small meta">
              {{ legMeta(leg) }}
            </span>
          </div>
          <ul v-if="leg.issues.length > 0" class="issues mt-1">
            <li v-for="fix in leg.fixes" :key="fixKey(fix)" class="leg-issue">
              <v-icon size="14" color="warning">mdi-format-list-checks</v-icon>
              <span>{{ fix.message }} — resolva em “Antes de testar, falta”.</span>
            </li>
            <li v-for="(issue, index) in legOpenIssues(leg)" :key="index" class="leg-issue">
              <v-icon size="14" color="error">mdi-alert-circle</v-icon>
              <span>{{ issue }}</span>
              <v-btn
                v-if="legHint(leg, issue).to"
                :to="legHint(leg, issue).to ?? undefined"
                color="primary"
                variant="flat"
                size="x-small"
                class="ml-1"
                append-icon="mdi-arrow-right"
              >
                {{ legHint(leg, issue).actionLabel }}
              </v-btn>
            </li>
          </ul>
        </div>
      </li>
    </ol>
  </div>
</template>

<script setup lang="ts">
/**
 * O plano do backend, legível: o que falta fazer (com o botão de cada passo),
 * tempo, avisos, bloqueios e pernas.
 */
import { computed } from 'vue'
import type { LinkTestPlan, LinkTestPlanLeg, LinkTestSpec } from '@/types/linkTests'
import {
  ENGINES,
  directionLabel,
  endpointIcon,
  endpointLabel,
  engineLabel,
  serverSideLabel,
} from '@/utils/linkTests/presentation'
import { estimatedDurationText, specSummary } from '@/utils/linkTests/spec'
import {
  fixKey,
  issueHint,
  legNeedsSteps,
  legOpenIssues,
  planEndpoints,
} from '@/utils/linkTests/wizard'
import LinkTestFixList from './LinkTestFixList.vue'

const props = defineProps<{
  plan: LinkTestPlan
  spec: LinkTestSpec
  /** O plano está sendo refeito depois de um passo. */
  refreshing?: boolean
}>()

const emit = defineEmits<{
  /** Um passo foi feito: o assistente refaz o plano. */
  fixed: []
}>()

function legColor(leg: LinkTestPlanLeg): string {
  if (leg.runnable) return 'primary'
  return legNeedsSteps(leg) ? 'warning' : 'error'
}

const runnableCount = computed(() => props.plan.legs.filter((leg) => leg.runnable).length)
/** Nada roda: não há tempo a estimar (nem enlace a saturar). */
const estimate = computed(() =>
  runnableCount.value > 0 ? estimatedDurationText(props.plan.estimatedSeconds) : '—'
)
const specText = computed(() => specSummary(props.spec))

const blockingHints = computed(() =>
  props.plan.blocking.map((text) => issueHint(text, planEndpoints(props.plan)))
)

function legHint(leg: LinkTestPlanLeg, issue: string) {
  return issueHint(issue, [leg.a, leg.b])
}

function legMeta(leg: LinkTestPlanLeg): string {
  const parts: string[] = []
  const server = serverSideLabel(leg)
  if (server) parts.push(server)
  if (leg.runnable) parts.push(`${leg.durationS} s · ${directionLabel(props.spec.direction)}`)
  return parts.join(' · ')
}
</script>

<style scoped>
.plan-view {
  color: rgb(var(--v-theme-on-surface));
}
.facts {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
  gap: 8px;
}
.fact {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 12px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.22);
  border-radius: 10px;
}
.issues {
  padding-left: 18px;
  margin: 0;
}
.issues li + li {
  margin-top: 4px;
}
.plan-legs {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.plan-leg {
  display: flex;
  align-items: flex-start;
  gap: 12px;
  padding: 10px 12px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.22);
  border-radius: 10px;
}
.plan-leg.blocked {
  border-color: rgb(var(--v-theme-error));
}
.plan-leg.waiting {
  border-color: rgb(var(--v-theme-warning));
}
.leg-issue {
  list-style: none;
  margin-left: -18px;
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 4px;
  font-size: 0.8125rem;
}
.seq {
  color: #fff;
  font-weight: 700;
}
.meta {
  color: rgb(var(--v-theme-on-surface));
}
</style>
