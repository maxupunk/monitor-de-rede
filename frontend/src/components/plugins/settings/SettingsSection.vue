<template>
  <div class="mb-2">
    <div v-if="section.title" class="d-flex align-center ga-2 mt-2 mb-1">
      <span class="text-subtitle-2 font-weight-bold text-primary">{{ section.title }}</span>
      <v-divider></v-divider>
    </div>
    <v-row dense>
      <SettingsField
        v-for="field in section.fields"
        :key="field.name"
        :field="field"
        :value="value[field.name]"
        :disabled="disabled"
        :compact="compact"
        :suggestions="suggestions?.[field.name]"
        @update="(next) => emit('update', field.name, next)"
      />
    </v-row>
  </div>
</template>

<script setup lang="ts">
import type { FieldSection } from '@/utils/pluginSettings'
import SettingsField from './SettingsField.vue'

/** Uma seção do formulário (`group` no esquema), com título quando tem. */
defineProps<{
  section: FieldSection
  value: Record<string, unknown>
  disabled?: boolean
  compact?: boolean
  suggestions?: Record<string, string[]>
}>()

const emit = defineEmits<{ update: [name: string, value: unknown] }>()
</script>
