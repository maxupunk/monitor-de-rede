<template>
  <v-snackbar
    v-model="feedback.show"
    :color="feedback.color"
    timeout="5000"
    location="bottom right"
  >
    {{ feedback.text }}
    <template v-if="feedback.action" #actions>
      <v-btn variant="flat" color="surface" @click="runAction">{{ feedback.action.label }}</v-btn>
    </template>
  </v-snackbar>
  <v-snackbar :model-value="Boolean(saveError)" color="error" timeout="-1" location="bottom right">
    <div class="font-weight-bold">Posições do mapa não salvas</div>
    <div>{{ saveError }}</div>
    <template #actions>
      <v-btn variant="flat" color="surface" prepend-icon="mdi-refresh" @click="$emit('retry-save')">
        Tentar de novo
      </v-btn>
    </template>
  </v-snackbar>
</template>

<script setup lang="ts">
/** Avisos do mapa: resultado das ações e falha ao gravar posições (com nova tentativa). */
import { inject } from 'vue'
import { TOPOLOGY_FEEDBACK } from '@/composables/topology/useTopologyHost'

defineProps<{ saveError: string | null }>()
defineEmits<{ 'retry-save': [] }>()

const injected = inject(TOPOLOGY_FEEDBACK)
if (!injected) throw new Error('TopologyFeedback precisa do estado fornecido pela página')
const feedback = injected

function runAction() {
  feedback.action?.run()
  feedback.show = false
}
</script>
