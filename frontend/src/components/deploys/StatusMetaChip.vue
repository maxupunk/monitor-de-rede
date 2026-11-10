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
/** O chip de um estado (deploy, compilação): rótulo, cor, ícone e "ao vivo". */
import type { StatusMeta } from '@/utils/deployPresentation'

withDefaults(defineProps<{ meta: StatusMeta; size?: string }>(), { size: 'small' })
</script>
