<template>
  <v-chip
    :color="meta.color"
    :size="size"
    variant="tonal"
    label
    :prepend-icon="meta.icon"
    class="font-weight-bold"
  >
    {{ meta.label }}
    <v-progress-circular
      v-if="meta.live"
      indeterminate
      size="12"
      width="2"
      class="ml-2"
    ></v-progress-circular>
  </v-chip>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { DeployRunStatus } from '@/bindings/DeployRunStatus'
import { runStatusMeta } from '@/utils/deployPresentation'

const props = withDefaults(defineProps<{ status: DeployRunStatus; size?: string }>(), {
  size: 'small',
})

const meta = computed(() => runStatusMeta(props.status))
</script>
