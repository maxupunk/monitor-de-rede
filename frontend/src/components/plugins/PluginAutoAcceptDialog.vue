<template>
  <v-dialog :model-value="modelValue" max-width="600" persistent>
    <v-card class="rounded-lg">
      <v-card-title class="d-flex align-center ga-2 text-wrap">
        <v-icon color="error">mdi-robot-angry-outline</v-icon>
        Ligar “Aceitar automaticamente”?
      </v-card-title>
      <v-card-subtitle class="text-wrap">
        Só nesta conversa e só para {{ deviceName }}. Vale por até 2 horas.
      </v-card-subtitle>
      <v-card-text>
        <v-alert type="error" variant="tonal" class="mb-4">
          <div class="terms text-body-2">{{ terms }}</div>
        </v-alert>
        <v-checkbox
          v-model="aware"
          color="error"
          hide-details
          label="Estou ciente dos riscos e assumo a responsabilidade pelas ações executadas"
        ></v-checkbox>
      </v-card-text>
      <v-card-actions>
        <v-spacer></v-spacer>
        <v-btn variant="text" color="secondary" @click="emit('update:modelValue', false)">
          Manter pedindo aprovação
        </v-btn>
        <v-btn
          color="error"
          variant="flat"
          prepend-icon="mdi-lightning-bolt"
          :disabled="!aware"
          :loading="busy"
          @click="emit('accept')"
        >
          Ligar modo automático
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'

const props = defineProps<{
  modelValue: boolean
  deviceName: string
  terms: string
  busy?: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: boolean]
  accept: []
}>()

const aware = ref(false)

watch(
  () => props.modelValue,
  () => {
    aware.value = false
  }
)
</script>

<style scoped>
.terms {
  white-space: pre-line;
}
</style>
