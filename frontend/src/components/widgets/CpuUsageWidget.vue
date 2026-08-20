<template>
  <ResourceUsageWidget :widget="widget" type="cpu" />
</template>

<script setup lang="ts">
import ResourceUsageWidget from './ResourceUsageWidget.vue'
import type { WidgetConfig } from '@/stores/dashboard'

defineProps<{
  widget: WidgetConfig
}>()
</script>
