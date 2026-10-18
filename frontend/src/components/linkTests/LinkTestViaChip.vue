<template>
  <v-chip
    v-if="label"
    class="via-chip"
    size="x-small"
    color="secondary"
    variant="outlined"
    prepend-icon="mdi-switch"
    label
    :title="description ?? undefined"
    :aria-label="description ?? undefined"
  >
    <span class="via-text">{{ label }}</span>
  </v-chip>
</template>

<script setup lang="ts">
/**
 * Chip "via switch-a › switch-b" da perna que atravessa equipamentos sem IP
 * (ADR 015 §5.2). Cadeia longa encurta com reticências; o texto inteiro fica
 * na dica e no rótulo acessível.
 */
import { computed } from 'vue'
import type { LinkTestPlanLeg } from '@/types/linkTests'
import { legViaDescription, legViaLabel } from '@/utils/linkTests/presentation'

const props = defineProps<{ leg: Pick<LinkTestPlanLeg, 'viaNames'> }>()

const label = computed(() => legViaLabel(props.leg))
const description = computed(() => legViaDescription(props.leg))
</script>

<style scoped>
.via-chip {
  max-width: 100%;
  min-width: 0;
}
.via-chip :deep(.v-chip__content) {
  min-width: 0;
}
.via-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
