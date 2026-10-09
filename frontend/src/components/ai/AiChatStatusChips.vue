<template>
  <v-chip
    size="x-small"
    color="primary"
    variant="tonal"
    :prepend-icon="responseStyle.icon"
    :title="responseStyle.hint"
  >
    {{ responseStyle.title }}
  </v-chip>
  <v-chip
    v-if="aiStore.settings"
    size="x-small"
    :color="activeTests.color"
    variant="tonal"
    :prepend-icon="activeTests.icon"
    :title="activeTests.hint"
  >
    {{ activeTests.title }}
  </v-chip>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useAiStore } from '@/stores/ai'
import { responseStyleOption } from './aiResponseStyle'
import { activeTestsPresentation } from './aiActiveTests'

/** Como a IA vai responder e o que ela pode testar — junto do campo da pergunta. */
const aiStore = useAiStore()

const responseStyle = computed(() => responseStyleOption(aiStore.settings?.responseStyle))
const activeTests = computed(() => activeTestsPresentation(aiStore.settings))
</script>
