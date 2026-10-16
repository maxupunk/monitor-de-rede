<template>
  <v-chip :color="presentation.color" :size="size" variant="flat" label>
    <v-progress-circular
      v-if="status === 'running'"
      indeterminate
      size="12"
      width="2"
      class="mr-1"
    ></v-progress-circular>
    <v-icon v-else start size="14">{{ presentation.icon }}</v-icon>
    {{ presentation.label }}
  </v-chip>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { LinkTestStatus, LinkTestStepStatus } from '@/types/linkTests'
import { stepStatusPresentation, testStatusPresentation } from '@/utils/linkTests/presentation'

const props = withDefaults(
  defineProps<{
    status: LinkTestStatus | LinkTestStepStatus
    /** `step` para perna/etapa (rótulos no feminino), `test` para o teste. */
    scope?: 'test' | 'step'
    size?: 'x-small' | 'small' | 'default'
  }>(),
  { scope: 'test', size: 'small' }
)

const presentation = computed(() =>
  props.scope === 'step'
    ? stepStatusPresentation(props.status)
    : testStatusPresentation(props.status)
)
</script>
