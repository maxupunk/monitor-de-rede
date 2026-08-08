<template>
  <v-dialog
    :model-value="modelValue"
    :max-width="$vuetify.display.xs ? undefined : 680"
    :fullscreen="$vuetify.display.xs"
    @update:model-value="onUpdateModelValue"
  >
    <v-card class="rounded-lg">
      <v-card-title class="font-weight-bold">Regras de firewall</v-card-title>
      <v-card-subtitle>
        Aplique no equipamento para liberar ICMP e SNMP na interface WireGuard.
      </v-card-subtitle>
      <v-card-text>
        <v-sheet class="rounded-lg pa-4" color="grey-darken-4">
          <pre class="script-content">{{ content }}</pre>
        </v-sheet>
      </v-card-text>
      <v-card-actions class="px-4 pb-4">
        <v-spacer></v-spacer>
        <v-btn variant="text" @click="close">Fechar</v-btn>
        <v-btn color="primary" variant="flat" prepend-icon="mdi-content-copy" @click="copy">
          Copiar regras
        </v-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
const props = defineProps<{
  modelValue: boolean
  content: string
}>()

const emit = defineEmits<{
  (e: 'update:modelValue', value: boolean): void
}>()

function onUpdateModelValue(value: boolean) {
  emit('update:modelValue', value)
}

function close() {
  emit('update:modelValue', false)
}

async function copy() {
  try {
    await navigator.clipboard.writeText(props.content)
  } catch {
    // navegador sem permissão de área de transferência
  }
}
</script>

<style scoped>
.script-content {
  color: #e0e0e0;
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 12px;
  line-height: 1.6;
  margin: 0;
  white-space: pre-wrap;
}
</style>
