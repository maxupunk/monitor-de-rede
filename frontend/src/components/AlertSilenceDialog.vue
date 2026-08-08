<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 400"
    :fullscreen="$vuetify.display.xs"
    @update:model-value="emit('update:modelValue', $event)"
  >
    <v-card class="rounded-lg pa-4">
      <v-card-title class="font-weight-bold">Silenciar Alerta</v-card-title>
      <v-card-text>
        <v-select
          v-model="duration"
          :items="[
            { title: '15 minutos', value: 15 },
            { title: '1 hora', value: 60 },
            { title: '4 horas', value: 240 },
            { title: '24 horas', value: 1440 },
          ]"
          label="Duração do Silenciamento"
          variant="outlined"
        ></v-select>
      </v-card-text>
      <v-card-actions class="justify-end">
        <v-btn variant="text" @click="emit('update:modelValue', false)">Cancelar</v-btn>
        <v-btn color="warning" :loading="loading" @click="confirm">Confirmar Silêncio</v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useAlertsStore } from '@/stores/alerts'

const props = defineProps<{
  modelValue: boolean
  alertId: number | null
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
  (e: 'silenced'): void
}>()

const alertsStore = useAlertsStore()
const duration = ref(60)
const loading = ref(false)

async function confirm() {
  if (props.alertId) {
    loading.value = true
    await alertsStore.silenceAlert(props.alertId, duration.value)
    loading.value = false
    emit('silenced')
  }
  emit('update:modelValue', false)
}
</script>
