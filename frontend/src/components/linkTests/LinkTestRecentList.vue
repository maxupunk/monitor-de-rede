<template>
  <ul class="recent" aria-label="Testes recentes">
    <li v-for="test in tests" :key="test.id">
      <button
        type="button"
        class="recent-item"
        :class="{ focused: focusedId === test.id }"
        :aria-label="`${testTitle(test)}, ${testStatusPresentation(test.status).label}, ${formatRelativeTime(test.createdAt)}`"
        @click="$emit('open', test.id)"
      >
        <v-icon :color="testStatusPresentation(test.status).color" size="20">
          {{ testStatusPresentation(test.status).icon }}
        </v-icon>
        <span class="flex-grow-1 overflow-hidden">
          <span class="d-block font-weight-bold title" :title="testTitle(test)">
            {{ testTitle(test) }}
          </span>
          <span class="d-block text-body-small meta">
            {{ testStatusPresentation(test.status).label }} ·
            {{ formatRelativeTime(test.createdAt) }}
          </span>
          <span v-if="resultOf(test)" class="d-block result">{{ resultOf(test) }}</span>
        </span>
        <v-icon size="18">mdi-chevron-right</v-icon>
      </button>
    </li>
  </ul>
</template>

<script setup lang="ts">
/** Lista curta de testes (aba da topologia): estado, quando e o resultado. */
import type { LinkTest } from '@/types/linkTests'
import { formatRelativeTime } from '@/utils/formatters'
import { flowPairLabel, testStatusPresentation, testTitle } from '@/utils/linkTests/presentation'

const props = defineProps<{
  tests: LinkTest[]
  /** Enlace em foco: o resultado mostrado é o da perna dele. */
  edgeId?: number | null
  focusedId?: number | null
}>()

defineEmits<{ open: [testId: number] }>()

function resultOf(test: LinkTest): string | null {
  if (props.edgeId !== null && props.edgeId !== undefined) {
    const leg = test.legs.find((item) => item.edgeId === props.edgeId)
    if (leg) return flowPairLabel(leg.forwardBps, leg.reverseBps)
  }
  return flowPairLabel(test.summary?.minForwardBps, test.summary?.minReverseBps)
}
</script>

<style scoped>
.recent {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.recent-item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: 10px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.22);
  background: rgb(var(--v-theme-surface));
  color: rgb(var(--v-theme-on-surface));
  text-align: left;
  cursor: pointer;
}
.recent-item:hover,
.recent-item.focused {
  border-color: rgb(var(--v-theme-primary));
}
.recent-item:focus-visible {
  outline: 2px solid rgb(var(--v-theme-primary));
  outline-offset: 2px;
}
.meta {
  color: rgb(var(--v-theme-on-surface));
}
/* O nome do teste é o que diferencia um do outro: até duas linhas, sem cortar cedo. */
.title {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.result {
  font-weight: 700;
  font-size: 0.8125rem;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}
</style>
