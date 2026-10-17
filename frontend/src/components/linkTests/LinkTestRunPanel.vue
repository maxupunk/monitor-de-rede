<template>
  <section class="run-panel" :aria-label="`Teste de velocidade: ${title}`">
    <header class="run-header">
      <div class="d-flex align-start ga-3">
        <v-avatar :color="kind.color" size="40" rounded="lg">
          <v-icon color="white">{{ kind.icon }}</v-icon>
        </v-avatar>
        <div class="flex-grow-1 overflow-hidden">
          <div class="text-title-medium font-weight-bold title">{{ title }}</div>
          <div class="d-flex align-center flex-wrap ga-2 mt-1">
            <LinkTestStatusChip :status="test.status" />
            <span class="meta text-body-small">{{ timing }}</span>
          </div>
        </div>
        <v-btn
          v-if="closable"
          icon="mdi-close"
          variant="text"
          size="small"
          aria-label="Fechar o painel do teste"
          @click="$emit('close')"
        ></v-btn>
      </div>

      <v-progress-linear
        v-if="active"
        :model-value="progress * 100"
        color="primary"
        height="6"
        rounded
        class="mt-3"
        :aria-label="`Andamento do teste: ${Math.round(progress * 100)}%`"
      ></v-progress-linear>

      <div class="d-flex align-center flex-wrap ga-2 mt-3">
        <span class="meta text-body-small">{{ specText }}</span>
        <v-spacer></v-spacer>
        <v-tooltip v-if="active" :text="READ_ONLY_REASON" :disabled="auth.canWrite">
          <template #activator="{ props: tip }">
            <span v-bind="tip">
              <v-btn
                color="error"
                variant="flat"
                size="small"
                prepend-icon="mdi-stop"
                :loading="cancelling"
                :disabled="!auth.canWrite"
                @click="cancel"
              >
                Cancelar teste
              </v-btn>
            </span>
          </template>
        </v-tooltip>
        <slot name="actions"></slot>
      </div>
    </header>

    <div class="sr-only" aria-live="polite" aria-atomic="true">{{ announcement }}</div>

    <div class="run-body">
      <v-alert
        v-if="active"
        type="warning"
        variant="tonal"
        density="compact"
        icon="mdi-speedometer"
        class="mb-3"
      >
        O enlace fica saturado enquanto mede — quem usa a rede pode sentir lentidão.
      </v-alert>
      <v-alert v-if="test.error" type="error" variant="tonal" density="compact" class="mb-3">
        {{ test.error }}
      </v-alert>
      <v-alert v-if="cancelError" type="error" variant="tonal" density="compact" class="mb-3">
        {{ cancelError }}
      </v-alert>

      <LinkTestResultSummary v-if="!active" :test="test" :speed-of="ctx.speedOf" class="mb-4" />

      <ol class="legs" aria-label="Pernas do teste">
        <LinkTestLegItem
          v-for="(leg, index) in shownLegs"
          :key="leg.seq"
          :leg="leg"
          :direction="test.spec.direction"
          :forward-samples="store.samplesFor(test.id, leg.seq, 'forward')"
          :reverse-samples="store.samplesFor(test.id, leg.seq, 'reverse')"
          :speed-bps="leg.edgeId !== null ? ctx.speedOf(leg.edgeId) : null"
          :bottleneck="!active && bottleneckSeq === leg.seq && test.legs.length > 1"
          :last="index === shownLegs.length - 1"
          :compact="compact"
          @open-run="(runId, runTitle, device) => stageRun?.open(runId, runTitle, device)"
        />
      </ol>
      <p v-if="test.legs.length === 0" class="meta text-body-medium">Montando o plano do teste…</p>
      <p v-else-if="shownLegs.length < test.legs.length" class="meta text-body-small">
        {{ legsDone(test) }} de {{ test.legs.length }} pernas concluídas — o painel do mapa mostra
        todas.
      </p>
      <div class="meta text-body-small mt-2">
        {{ originText }}
      </div>
    </div>

    <LinkTestStageRun ref="stageRun" />
  </section>
</template>

<script setup lang="ts">
/**
 * O teste ao vivo (ou concluído): cabeçalho com estado, tempo e "Cancelar"
 * sempre à vista, pernas como linha do tempo com medidores, gráfico e
 * etapas, e o resumo com o gargalo no fim. Usado no mapa e no histórico.
 */
import { computed, ref } from 'vue'
import { confirm } from '@/composables/useConfirm'
import { useLinkTestContext } from '@/composables/linkTests/useLinkTestContext'
import { useAuthStore } from '@/stores/auth'
import { useLinkTestsStore } from '@/stores/linkTests'
import type { LinkTest } from '@/types/linkTests'
import { formatDateTime, formatElapsedMs, formatShortDateTime } from '@/utils/formatters'
import {
  TEST_KINDS,
  isActiveStatus,
  legTitle,
  testStatusPresentation,
  testTitle,
} from '@/utils/linkTests/presentation'
import {
  currentLeg,
  legsDone,
  testElapsedMs,
  testProgress,
  testRemainingMs,
} from '@/utils/linkTests/progress'
import { bottleneckLeg } from '@/utils/linkTests/results'
import { specSummary } from '@/utils/linkTests/spec'
import { READ_ONLY_REASON } from '@/utils/topology/inspectorRegistry'
import LinkTestLegItem from './LinkTestLegItem.vue'
import LinkTestResultSummary from './LinkTestResultSummary.vue'
import LinkTestStageRun from './LinkTestStageRun.vue'
import LinkTestStatusChip from './LinkTestStatusChip.vue'

const props = defineProps<{
  test: LinkTest
  /** Versão enxuta (aba do inspetor): sem gráfico nem retransmissões. */
  compact?: boolean
  closable?: boolean
}>()

defineEmits<{ close: [] }>()

const store = useLinkTestsStore()
const auth = useAuthStore()
const ctx = useLinkTestContext()
const stageRun = ref<InstanceType<typeof LinkTestStageRun> | null>(null)
const cancelling = ref(false)
const cancelError = ref<string | null>(null)

const kind = computed(() => TEST_KINDS[props.test.kind] ?? TEST_KINDS.link)
const title = computed(() => testTitle(props.test))
const active = computed(() => isActiveStatus(props.test.status))
const now = computed(() => ctx.nowFor(props.test))
const progress = computed(() => testProgress(props.test, now.value))
/** Enxuto e em andamento: só a perna atual (o painel do mapa mostra todas). */
const shownLegs = computed(() => {
  if (!props.compact || !active.value) return props.test.legs
  const leg = currentLeg(props.test)
  return leg ? [leg] : props.test.legs
})
const bottleneckSeq = computed(() => bottleneckLeg(props.test)?.seq ?? null)
const specText = computed(() => specSummary(props.test.spec))

const timing = computed(() => {
  const elapsed = testElapsedMs(props.test, now.value)
  if (props.test.status === 'queued') return 'aguardando a vez (no máximo 2 testes ao mesmo tempo)'
  if (active.value) {
    const remaining = testRemainingMs(props.test, now.value)
    const parts = [`${formatElapsedMs(elapsed)} decorridos`]
    if (remaining !== null && remaining > 0) parts.push(`faltam ~${formatElapsedMs(remaining)}`)
    return parts.join(' · ')
  }
  // Hora absoluta: o painel não tem relógio próprio, e um "há 0s" congelaria.
  const when = props.test.finishedAt ?? props.test.createdAt
  return `terminou em ${formatShortDateTime(when)}${elapsed !== null ? ` · durou ${formatElapsedMs(elapsed)}` : ''}`
})

const originText = computed(() => {
  const by =
    props.test.origin === 'schedule' ? 'agendamento' : (props.test.requestedBy?.name ?? 'usuário')
  return `Pedido por ${by} em ${formatDateTime(props.test.createdAt)} · teste #${props.test.id}`
})

/** Anúncio para leitor de tela: muda a cada etapa, não a cada amostra. */
const announcement = computed(() => {
  const status = testStatusPresentation(props.test.status).label
  if (!active.value) return `Teste ${status.toLowerCase()}.`
  const leg = currentLeg(props.test)
  if (!leg) return status
  return `${status}: perna ${legsDone(props.test) + 1} de ${props.test.legs.length}, ${legTitle(leg)}.`
})

async function cancel() {
  const ok = await confirm({
    title: 'Cancelar o teste',
    message:
      'Interromper a medição agora? O que já foi medido fica registrado e os servidores de teste são desligados nos equipamentos.',
    confirmText: 'Cancelar teste',
    cancelText: 'Continuar medindo',
    confirmColor: 'error',
  })
  if (!ok) return
  cancelling.value = true
  cancelError.value = null
  try {
    await store.cancel(props.test.id)
  } catch (err: unknown) {
    cancelError.value =
      err instanceof Error && err.message ? err.message : 'Não foi possível cancelar o teste.'
  } finally {
    cancelling.value = false
  }
}
</script>

<style scoped>
.run-panel {
  display: flex;
  flex-direction: column;
  min-height: 0;
  color: rgb(var(--v-theme-on-surface));
}
.run-header {
  padding: 12px 16px;
  border-bottom: 1px solid rgba(var(--v-theme-on-surface), 0.14);
}
.run-body {
  padding: 12px 16px;
}
.title {
  word-break: break-word;
}
.meta {
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
