<template>
  <div
    class="d-flex flex-column flex-md-row align-start align-md-center justify-space-between mb-4 mb-md-6 gap-4 px-3 px-md-0"
  >
    <div class="flex-grow-1">
      <h1 class="text-h5 text-md-h4 font-weight-bold text-break">{{ title }}</h1>
      <p
        v-if="subtitle"
        class="text-subtitle-2 text-md-subtitle-1 text-grey-darken-1 mt-1 text-break"
      >
        {{ subtitle }}
      </p>
    </div>
    <div class="d-flex flex-wrap align-center gap-2 w-100 w-md-auto justify-start justify-md-end">
      <slot name="actions" />
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  title: string
  subtitle?: string
}>()
</script>

<style scoped>
.gap-4 {
  gap: 16px;
}
.gap-2 {
  gap: 8px;
}
</style>
