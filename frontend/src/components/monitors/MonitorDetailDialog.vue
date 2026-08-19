<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 1100"
    :fullscreen="$vuetify.display.xs"
    scrollable
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center justify-space-between pa-4 bg-primary text-white">
        <div class="d-flex align-center ga-2">
          <v-icon>mdi-heart-pulse</v-icon>
          <span>Detalhes do monitor</span>
        </div>
        <v-btn icon variant="text" color="white" aria-label="Fechar" @click="fechar">
          <v-icon>mdi-close</v-icon>
        </v-btn>
      </v-card-title>

      <v-card-text class="pa-4 pa-sm-6">
        <!--
          O `key` força a remontagem ao trocar de monitor: a view carrega o
          histórico no `onMounted`, e sem isto abrir um segundo monitor
          mostraria os dados do primeiro até a requisição voltar.
        -->
        <MonitorDetailView
          v-if="modelValue && monitorId"
          :key="monitorId"
          :monitor-id="monitorId"
          embedded
          @closed="fechar"
        />
      </v-card-text>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
/**
 * O detalhe do monitor, em diálogo.
 *
 * É a **única** forma de abrir `/monitors/{id}` no produto: a rota monta este
 * mesmo componente. Ter uma tela cheia e um diálogo mostrando a mesma coisa
 * garantiria que um dos dois envelhecesse.
 */
import MonitorDetailView from '@/components/monitors/MonitorDetailView.vue'

defineProps<{
  modelValue: boolean
  monitorId: number | null
}>()

const emit = defineEmits<{ (e: 'update:modelValue', value: boolean): void }>()

function fechar(): void {
  emit('update:modelValue', false)
}
</script>
