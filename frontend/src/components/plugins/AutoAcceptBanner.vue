<template>
  <div class="auto-accept">
    <v-alert
      v-if="aiStore.autoAccept?.active"
      type="error"
      variant="flat"
      density="compact"
      rounded="0"
      icon="mdi-lightning-bolt"
    >
      <div class="d-flex align-center ga-2 flex-wrap">
        <span class="text-body-2 font-weight-medium flex-grow-1">
          Modo automático ativo — a IA está executando comandos em {{ deviceName }} sem pedir
          confirmação.
        </span>
        <v-btn
          size="small"
          color="surface"
          variant="flat"
          prepend-icon="mdi-stop"
          :loading="busy"
          @click="stop"
        >
          Parar
        </v-btn>
      </div>
    </v-alert>

    <div v-else-if="aiStore.deviceContextId" class="d-flex align-center ga-2 px-3 py-1 border-b">
      <v-icon size="18" color="warning">mdi-shield-key-outline</v-icon>
      <span class="text-body-small flex-grow-1">
        Acessos da IA a {{ deviceName }} pedem sua aprovação um a um.
      </span>
      <v-btn
        size="small"
        variant="tonal"
        color="error"
        prepend-icon="mdi-lightning-bolt-outline"
        @click="dialog = true"
      >
        Aceitar automaticamente
      </v-btn>
    </div>

    <PluginAutoAcceptDialog
      v-model="dialog"
      :device-name="deviceName"
      :terms="aiStore.autoAccept?.terms ?? ''"
      :busy="busy"
      @accept="enable"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import { useAiStore } from '@/stores/ai'
import { useDevicesStore } from '@/stores/devices'
import PluginAutoAcceptDialog from './PluginAutoAcceptDialog.vue'

const aiStore = useAiStore()
const devicesStore = useDevicesStore()
const dialog = ref(false)
const busy = ref(false)

const deviceName = computed(() => {
  const id = aiStore.deviceContextId
  const device = devicesStore.devices.find((item) => item.id === id)
  return device?.name ?? `o dispositivo #${id}`
})

async function enable() {
  busy.value = true
  try {
    await aiStore.enableAutoAccept()
    dialog.value = false
  } finally {
    busy.value = false
  }
}

async function stop() {
  busy.value = true
  try {
    await aiStore.disableAutoAccept()
  } finally {
    busy.value = false
  }
}
</script>
