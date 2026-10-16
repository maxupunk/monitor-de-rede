<template>
  <v-dialog
    :model-value="modelValue"
    max-width="480"
    @update:model-value="$emit('update:modelValue', $event)"
  >
    <v-card>
      <v-card-title class="d-flex align-center">
        <v-icon color="primary" class="mr-2">mdi-keyboard-outline</v-icon>
        Atalhos do mapa
      </v-card-title>
      <v-card-text>
        <div v-for="item in SHORTCUT_SHEET" :key="item.description" class="shortcut-row">
          <span class="keys">
            <kbd v-for="key in item.keys" :key="key">{{ key }}</kbd>
          </span>
          <span>{{ item.description }}</span>
        </div>
        <div class="mt-3 text-body-2">
          Clique com o botão direito (ou toque e segure) num dispositivo, enlace ou no fundo do mapa
          para ver as ações.
        </div>
      </v-card-text>
      <v-card-actions>
        <v-spacer></v-spacer>
        <v-btn color="primary" variant="flat" @click="$emit('update:modelValue', false)">
          Entendi
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
import { SHORTCUT_SHEET } from '@/utils/topology/shortcuts'

defineProps<{ modelValue: boolean }>()
defineEmits<{ 'update:modelValue': [value: boolean] }>()
</script>

<style scoped>
.shortcut-row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 5px 0;
  border-bottom: 1px solid rgba(var(--v-theme-on-surface), 0.1);
}
.keys {
  display: inline-flex;
  gap: 4px;
  min-width: 130px;
}
kbd {
  font-family: inherit;
  font-size: 0.75rem;
  font-weight: 700;
  padding: 2px 7px;
  border-radius: 4px;
  border: 1px solid rgba(var(--v-theme-on-surface), 0.45);
  color: rgb(var(--v-theme-on-surface));
}
</style>
