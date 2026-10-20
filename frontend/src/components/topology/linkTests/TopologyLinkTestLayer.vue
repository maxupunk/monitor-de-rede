<template>
  <div class="link-test-layer">
    <div v-if="cardTest" class="lt-card-wrap" :class="{ narrow }">
      <LinkTestMiniCard
        :test="cardTest"
        :others="otherActive"
        :dismissible="cardTest.id === store.focusedId"
        @open="openTest(cardTest.id)"
        @dismiss="closeTest"
      />
    </div>

    <LinkTestRunDialog
      :model-value="runOpen"
      :test="focused"
      minimizable
      @minimize="minimize"
      @close="closeTest"
    />

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
 * O teste de velocidade sobre o mapa: abre o assistente quando o mapa pede um
 * teste (`linkTestRequests`), mostra o teste em tela cheia
 * (`LinkTestRunDialog`) e pinta a camada de sobreposição no mapa (caminho,
 * enlace em teste, resultado). Minimizar devolve o mapa com a pílula do
 * teste; fechar limpa o mapa.
 */
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import { useLinkTestContext } from '@/composables/linkTests/useLinkTestContext'
import { useLinkTestsStore } from '@/stores/linkTests'
import { useLinkTestRequestsStore, type LinkTestRequest } from '@/stores/linkTestRequests'
import { useTopologyOverlayStore } from '@/stores/topologyOverlay'
import type { LinkTest, LinkTestPlan } from '@/types/linkTests'
import { isActiveStatus } from '@/utils/linkTests/presentation'
import {
  overlayForPlan,
  overlayForTest,
  pathOf,
  type LinkTestOverlay,
} from '@/utils/linkTests/overlay'
import LinkTestMiniCard from '@/components/linkTests/LinkTestMiniCard.vue'
import LinkTestRunDialog from '@/components/linkTests/LinkTestRunDialog.vue'
import LinkTestWizard from '@/components/linkTests/LinkTestWizard.vue'
import { registerLinkTestTopology } from './registerLinkTestTopology'

const props = defineProps<{
  /** Enquadra estes nós no mapa (a página repassa ao canvas). */
  fitNodes: (deviceIds: number[]) => void
  /** Celular: a pílula fica acima do resumo do mapa. */
  narrow: boolean
}>()

registerLinkTestTopology()

const store = useLinkTestsStore()
const requests = useLinkTestRequestsStore()
const overlay = useTopologyOverlayStore()
const ctx = useLinkTestContext()

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
  openTest(testId)
}

// --- Tela do teste e pílula -----------------------------------------------------

/** O teste em foco está minimizado: a pílula no mapa, a tela fechada. */
const minimized = ref(false)
const focused = computed(() => store.focused)
const runOpen = computed(() => focused.value !== null && !minimized.value)

/** Pílula: o teste minimizado ou, sem nenhum em foco, o que estiver rodando. */
const cardTest = computed<LinkTest | null>(() => {
  if (focused.value) return minimized.value ? focused.value : null
  return store.active[0] ?? null
})
const otherActive = computed(
  () => store.active.filter((test) => test.id !== cardTest.value?.id).length
)

function openTest(id: number) {
  store.focus(id)
}

/** Todo pedido de abrir (pílula, aba, histórico) reabre a tela do teste. */
watch(
  () => store.focusSeq,
  () => (minimized.value = false)
)

function closeTest() {
  minimized.value = false
  store.focus(null)
}

/** Devolve o mapa com o caminho do teste enquadrado. */
function minimize() {
  minimized.value = true
  const test = focused.value
  if (!test) return
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

// --- Sobreposição no mapa -----------------------------------------------------------

const model = computed<LinkTestOverlay | null>(() => {
  if (plannedPreview.value) return overlayForPlan(plannedPreview.value)
  const test = focused.value
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

/** Um diálogo do teste está aberto (a página suspende os atalhos do mapa). */
const dialogOpen = computed(() => wizardOpen.value || runOpen.value)
/** Há teste (ou plano) pintado no mapa — o aviso "Caminho destacado" é dele, não do mapa. */
const overlayActive = computed(() => model.value !== null)
defineExpose({ dialogOpen, overlayActive })
</script>

<style scoped>
.lt-card-wrap {
  position: absolute;
  right: 76px;
  /* Acima do resumo do mapa (dispositivos, enlaces, estado). */
  bottom: 72px;
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
</style>
