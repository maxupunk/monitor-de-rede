<template>
  <div class="mini-card">
    <button
      type="button"
      class="open"
      :aria-label="`Abrir o teste: ${title}`"
      @click="$emit('open')"
    >
      <span class="d-flex align-center ga-2">
        <v-progress-circular
          v-if="active"
          indeterminate
          size="16"
          width="2"
          color="primary"
        ></v-progress-circular>
        <v-icon v-else size="18" :color="status.color">{{ status.icon }}</v-icon>
        <span class="card-title flex-grow-1 text-truncate">
          {{ active ? 'Teste em andamento' : status.label }}
        </span>
        <v-chip v-if="others > 0" size="x-small" color="info" variant="flat" label>
          +{{ others }}
        </v-chip>
        <v-icon size="18">mdi-arrow-expand</v-icon>
      </span>
      <span class="card-leg text-truncate">{{ title }}</span>
      <span v-if="live" class="card-live">{{ live }}</span>
      <v-progress-linear
        v-if="active"
        :model-value="progress * 100"
        color="primary"
        height="4"
        rounded
        class="mt-1"
      ></v-progress-linear>
    </button>
    <v-btn
      v-if="dismissible && !active"
      icon="mdi-close"
      variant="text"
      size="x-small"
      class="dismiss"
      aria-label="Tirar o resultado do mapa"
      @click="$emit('dismiss')"
    ></v-btn>
  </div>
</template>

<script setup lang="ts">
/**
 * O teste numa pílula: estado, trecho atual, números ao vivo (ou o resultado)
 * e o andamento. Clicar abre a tela do teste. No mapa é o teste minimizado;
 * na aba "Testes", o que está rodando com o equipamento.
 */
import { computed } from 'vue'
import { useLinkTestContext } from '@/composables/linkTests/useLinkTestContext'
import { useLinkTestsStore } from '@/stores/linkTests'
import type { LinkTest } from '@/types/linkTests'
import {
  flowPairLabel,
  isActiveStatus,
  legTitle,
  testRouteTitle,
  testStatusPresentation,
} from '@/utils/linkTests/presentation'
import { currentLeg, legsDone, testProgress } from '@/utils/linkTests/progress'

const props = defineProps<{
  test: LinkTest
  /** Outros testes rodando ao mesmo tempo (o "+N"). */
  others?: number
  /** Mostra o "x" que tira o resultado do mapa. */
  dismissible?: boolean
}>()

defineEmits<{ open: []; dismiss: [] }>()

const store = useLinkTestsStore()
const ctx = useLinkTestContext()

const active = computed(() => isActiveStatus(props.test.status))
const status = computed(() => testStatusPresentation(props.test.status))
const others = computed(() => props.others ?? 0)

const title = computed(() => {
  const leg = currentLeg(props.test)
  if (!leg || !active.value || props.test.legs.length === 1) return testRouteTitle(props.test)
  return `Enlace ${legsDone(props.test) + 1} de ${props.test.legs.length} · ${legTitle(leg)}`
})

const live = computed(() => {
  const leg = currentLeg(props.test)
  if (!leg)
    return flowPairLabel(props.test.summary?.minForwardBps, props.test.summary?.minReverseBps)
  return flowPairLabel(
    store.liveBps(props.test.id, leg.seq, 'forward') ?? leg.forwardBps,
    store.liveBps(props.test.id, leg.seq, 'reverse') ?? leg.reverseBps
  )
})

const progress = computed(() => testProgress(props.test, ctx.nowFor(props.test)))
</script>

<style scoped>
.mini-card {
  position: relative;
}
.open {
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
.open:focus-visible {
  outline: 2px solid rgb(var(--v-theme-primary));
  outline-offset: 2px;
}
.dismiss {
  position: absolute;
  top: -10px;
  right: -10px;
  background: rgb(var(--v-theme-surface));
  border: 1px solid rgb(var(--v-theme-primary));
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
</style>
