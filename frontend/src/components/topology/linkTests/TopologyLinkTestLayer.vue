<template>
  <div class="link-test-layer">
    <div v-if="cardTest && !panelOpen" class="lt-card-wrap" :class="{ narrow }">
      <button
        type="button"
        class="lt-card"
        :aria-label="`Abrir o teste em andamento: ${cardTitle}`"
        @click="store.focus(cardTest.id)"
      >
        <span class="d-flex align-center ga-2">
          <v-progress-circular
            v-if="cardActive"
            indeterminate
            size="16"
            width="2"
            color="primary"
          ></v-progress-circular>
          <v-icon v-else size="18" :color="cardStatus.color">{{ cardStatus.icon }}</v-icon>
          <span class="card-title flex-grow-1 text-truncate">
            {{ cardActive ? 'Teste em andamento' : cardStatus.label }}
          </span>
          <v-chip v-if="otherActive > 0" size="x-small" color="info" variant="flat" label>
            +{{ otherActive }}
          </v-chip>
          <v-icon size="18">mdi-arrow-expand</v-icon>
        </span>
        <span class="card-leg text-truncate">{{ cardTitle }}</span>
        <span v-if="cardLive" class="card-live">{{ cardLive }}</span>
        <v-progress-linear
          v-if="cardActive"
          :model-value="cardProgress * 100"
          color="primary"
          height="4"
          rounded
          class="mt-1"
        ></v-progress-linear>
      </button>
    </div>

    <div
      v-if="panelTest && !narrow && panelOpen"
      ref="panel"
      class="lt-panel"
      role="dialog"
      aria-modal="false"
      :aria-label="`Teste de velocidade: ${cardTitle}`"
    >
      <LinkTestRunPanel :test="panelTest" closable @close="closePanel">
        <template #actions>
          <v-btn
            color="primary"
            variant="outlined"
            size="small"
            prepend-icon="mdi-fit-to-screen"
            @click="fitPath(panelTest)"
          >
            Enquadrar caminho
          </v-btn>
        </template>
      </LinkTestRunPanel>
    </div>

    <v-bottom-sheet
      v-if="narrow"
      :model-value="sheets.isTop('link-test') && panelTest !== null"
      scrollable
      @update:model-value="(open) => !open && closePanel()"
    >
      <v-card v-if="panelTest" class="lt-sheet">
        <LinkTestRunPanel :test="panelTest" closable @close="closePanel" />
      </v-card>
    </v-bottom-sheet>

    <LinkTestWizard
      v-model="wizardOpen"
      :request="wizardRequest"
      @started="onStarted"
      @planned="(plan) => (plannedPreview = plan)"
    />
  </div>
</template>

<script setup lang="ts">
/**
 * O teste de enlace sobre o mapa: abre o assistente quando o mapa pede um
 * teste (`linkTestRequests`), mostra o cartão "teste em andamento", o painel
 * ao vivo do teste em foco e pinta a camada de sobreposição (caminho,
 * enlace em teste, resultado). Fechar o painel limpa o mapa.
 */
import { computed, nextTick, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import { useLinkTestContext } from '@/composables/linkTests/useLinkTestContext'
import { useSheetStack } from '@/composables/topology/useSheetStack'
import { useLinkTestsStore } from '@/stores/linkTests'
import { useLinkTestRequestsStore, type LinkTestRequest } from '@/stores/linkTestRequests'
import { useTopologyOverlayStore } from '@/stores/topologyOverlay'
import type { LinkTest, LinkTestPlan } from '@/types/linkTests'
import {
  flowPairLabel,
  isActiveStatus,
  legTitle,
  testStatusPresentation,
  testTitle,
} from '@/utils/linkTests/presentation'
import {
  overlayForPlan,
  overlayForTest,
  pathOf,
  type LinkTestOverlay,
} from '@/utils/linkTests/overlay'
import { currentLeg, legsDone, testProgress } from '@/utils/linkTests/progress'
import LinkTestRunPanel from '@/components/linkTests/LinkTestRunPanel.vue'
import LinkTestWizard from '@/components/linkTests/LinkTestWizard.vue'
import { registerLinkTestTopology } from './registerLinkTestTopology'

const props = defineProps<{
  /** Enquadra estes nós no mapa (a página repassa ao canvas). */
  fitNodes: (deviceIds: number[]) => void
  /** Celular: painel vira folha inferior. */
  narrow: boolean
}>()

registerLinkTestTopology()

/** Painel flutuante: largura e afastamento da borda (ao lado do zoom). */
const PANEL_WIDTH = 440
const PANEL_RIGHT = 76

const store = useLinkTestsStore()
const requests = useLinkTestRequestsStore()
const overlay = useTopologyOverlayStore()
const ctx = useLinkTestContext()
/** Celular: o painel do teste entra na pilha de folhas (uma por vez). */
const sheets = useSheetStack()

// --- Assistente ---------------------------------------------------------------

const wizardOpen = ref(false)
const wizardRequest = ref<LinkTestRequest | null>(null)
const plannedPreview = ref<LinkTestPlan | null>(null)

watch(
  () => requests.pending,
  (pending) => {
    if (!pending) return
    const taken = requests.take()
    if (!taken) return
    wizardRequest.value = {
      kind: taken.kind,
      linkId: taken.linkId,
      sourceDeviceId: taken.sourceDeviceId,
      targetDeviceId: taken.targetDeviceId,
      spec: taken.spec,
    }
    wizardOpen.value = true
  },
  { immediate: true }
)

watch(wizardOpen, (open) => {
  if (!open) plannedPreview.value = null
})

function onStarted(testId: number) {
  plannedPreview.value = null
  store.focus(testId)
}

// --- Cartão e painel ------------------------------------------------------------

const panelTest = computed(() => store.focused)
const panelOpen = computed(() => store.focusedId !== null)

watch(
  () => panelOpen.value && props.narrow,
  (open) => (open ? sheets.open('link-test') : sheets.close('link-test')),
  { immediate: true }
)
const cardTest = computed<LinkTest | null>(() => store.focused ?? store.active[0] ?? null)
const cardActive = computed(() => (cardTest.value ? isActiveStatus(cardTest.value.status) : false))
const cardStatus = computed(() => testStatusPresentation(cardTest.value?.status ?? 'queued'))
const otherActive = computed(
  () => store.active.filter((test) => test.id !== cardTest.value?.id).length
)

const cardTitle = computed(() => {
  const test = cardTest.value
  if (!test) return ''
  const leg = currentLeg(test)
  if (!leg || !cardActive.value) return testTitle(test)
  return `Perna ${legsDone(test) + 1} de ${test.legs.length} · ${legTitle(leg)}`
})

const cardLive = computed(() => {
  const test = cardTest.value
  if (!test) return null
  const leg = currentLeg(test)
  if (!leg) return flowPairLabel(test.summary?.minForwardBps, test.summary?.minReverseBps)
  return flowPairLabel(
    store.liveBps(test.id, leg.seq, 'forward') ?? leg.forwardBps,
    store.liveBps(test.id, leg.seq, 'reverse') ?? leg.reverseBps
  )
})

const cardProgress = computed(() =>
  cardTest.value ? testProgress(cardTest.value, ctx.nowFor(cardTest.value)) : 0
)

function closePanel() {
  store.focus(null)
}

/** Depois do painel abrir — a área livre do mapa (`occupiedRight`) muda com ele. */
function fitPath(test: LinkTest) {
  const path = pathOf(test.legs)
  if (path.nodeIds.length > 0) void nextTick(() => props.fitNodes(path.nodeIds))
}

/** Foco num teste concluído (aba, ação, histórico): o detalhe traz as amostras. */
watch(
  () => store.focusedId,
  (id) => {
    if (id === null) return
    const test = store.byId(id)
    if (!test || !isActiveStatus(test.status)) void store.fetchDetail(id).catch(() => null)
  }
)

/** Enquadra o caminho uma vez, quando o teste em foco ganha as pernas. */
let fittedFor: number | null = null
watch(
  () => [store.focusedId, panelTest.value?.legs.length ?? 0] as const,
  ([id, legs]) => {
    if (id === null || legs === 0 || fittedFor === id || !panelTest.value) return
    fittedFor = id
    fitPath(panelTest.value)
  },
  { immediate: true }
)

// --- Sobreposição no mapa -----------------------------------------------------------

const model = computed<LinkTestOverlay | null>(() => {
  if (plannedPreview.value) return overlayForPlan(plannedPreview.value)
  const test = panelTest.value
  if (!test) return null
  return overlayForTest(test, ctx.liveRate(test.id), ctx.edgeSource)
})

let applied: LinkTestOverlay | null = null

function sameIds(a: number[], b: number[]): boolean {
  return a.length === b.length && a.every((id, index) => id === b[index])
}

/** Aplica só a diferença — o canvas repinta o que mudou, não o mapa inteiro. */
function sync(next: LinkTestOverlay | null) {
  const previous = applied
  if (!next) {
    if (previous) overlay.clear()
    applied = null
    return
  }
  if (
    !previous ||
    !sameIds(previous.highlight.nodeIds, next.highlight.nodeIds) ||
    !sameIds(previous.highlight.edgeIds, next.highlight.edgeIds)
  ) {
    overlay.setPathHighlight(next.highlight)
  }
  for (const id of Object.keys(previous?.edges ?? {}).map(Number)) {
    if (!(id in next.edges)) overlay.setEdgeOverlay(id, null)
  }
  for (const [key, value] of Object.entries(next.edges)) {
    const id = Number(key)
    if (JSON.stringify(previous?.edges[id]) !== JSON.stringify(value)) {
      overlay.setEdgeOverlay(id, value)
    }
  }
  for (const id of Object.keys(previous?.nodes ?? {}).map(Number)) {
    if (!(id in next.nodes)) overlay.setNodeOverlay(id, null)
  }
  for (const [key, value] of Object.entries(next.nodes)) {
    const id = Number(key)
    if (JSON.stringify(previous?.nodes[id]) !== JSON.stringify(value)) {
      overlay.setNodeOverlay(id, value)
    }
  }
  applied = next
}

watch(model, sync, { immediate: true })

onBeforeUnmount(() => {
  sync(null)
  store.focus(null)
})

const dialogOpen = computed(() => wizardOpen.value)

/**
 * Largura real do painel (ele encolhe quando o inspetor também está aberto),
 * medida pelo próprio elemento — o enquadramento desconta exatamente isso.
 */
const panel = shallowRef<HTMLElement | null>(null)
const panelWidth = ref(PANEL_WIDTH)
let panelObserver: ResizeObserver | null = null
watch(panel, (element) => {
  panelObserver?.disconnect()
  panelObserver = null
  if (!element) return
  panelWidth.value = element.offsetWidth
  panelObserver = new ResizeObserver(() => (panelWidth.value = element.offsetWidth))
  panelObserver.observe(element)
})
onBeforeUnmount(() => panelObserver?.disconnect())

/** Largura que o painel flutuante tira do mapa (a página desconta ao enquadrar). */
const occupiedRight = computed(() =>
  panelOpen.value && panelTest.value !== null && !props.narrow ? PANEL_RIGHT + panelWidth.value : 0
)
/** Há teste (ou plano) pintado no mapa — o aviso "Caminho destacado" é dele, não do mapa. */
const overlayActive = computed(() => model.value !== null)
defineExpose({ dialogOpen, occupiedRight, overlayActive })
</script>

<style scoped>
.lt-card-wrap {
  position: absolute;
  right: 12px;
  bottom: 212px;
  z-index: 6;
  width: 300px;
  max-width: calc(100% - 24px);
}
/* Celular: acima do resumo e ao lado da coluna de zoom, sem cobrir nenhum. */
.lt-card-wrap.narrow {
  left: 12px;
  right: 76px;
  bottom: 64px;
  width: auto;
}
.lt-card {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 4px;
  text-align: left;
  padding: 10px 12px;
  border-radius: 12px;
  border: 1px solid rgb(var(--v-theme-primary));
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface));
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.28);
  cursor: pointer;
}
.lt-card:focus-visible {
  outline: 2px solid rgb(var(--v-theme-primary));
  outline-offset: 2px;
}
.card-title {
  font-weight: 700;
}
.card-leg {
  font-size: 0.8125rem;
}
.card-live {
  font-size: 1.05rem;
  font-weight: 800;
  font-variant-numeric: tabular-nums;
}
.lt-panel {
  position: absolute;
  top: 12px;
  right: v-bind('`${PANEL_RIGHT}px`');
  bottom: 12px;
  z-index: 7;
  /* Com o inspetor aberto o mapa estreita: o painel fica com no máximo metade. */
  width: min(v-bind('`${PANEL_WIDTH}px`'), 45%);
  min-width: 320px;
  overflow-y: auto;
  border-radius: 14px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.2);
  background: rgb(var(--v-theme-surface));
  box-shadow: 0 10px 28px rgba(0, 0, 0, 0.32);
}
.lt-sheet {
  max-height: 85vh;
  overflow-y: auto;
  border-radius: 16px 16px 0 0;
}
</style>
