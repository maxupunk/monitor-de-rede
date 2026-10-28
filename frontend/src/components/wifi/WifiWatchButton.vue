<template>
  <v-tooltip :text="tip" location="bottom">
    <template #activator="{ props: tipProps }">
      <span v-bind="tipProps" :class="{ 'd-block': block }">
        <v-btn
          v-if="watched"
          color="primary"
          variant="tonal"
          size="small"
          prepend-icon="mdi-eye"
          :block="block"
          :disabled="!auth.canWrite"
          @click="stopWatching(watched)"
        >
          Vigiado
        </v-btn>
        <v-btn
          v-else
          color="primary"
          variant="outlined"
          size="small"
          prepend-icon="mdi-eye-plus-outline"
          :block="block"
          :disabled="!auth.canWrite"
          @click="startWatching(row)"
        >
          Vigiar
        </v-btn>
      </span>
    </template>
  </v-tooltip>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useWifiWatch } from '@/composables/useWifiWatch'
import { useAuthStore } from '@/stores/auth'
import { useWifiClientsStore } from '@/stores/wifiClients'
import type { WifiStationRow } from '@/utils/wifiPresentation'

const props = defineProps<{ row: WifiStationRow; block?: boolean }>()

const auth = useAuthStore()
const store = useWifiClientsStore()
const { startWatching, stopWatching } = useWifiWatch()

const watched = computed(() => store.watchedByMac.get(props.row.mac) ?? null)

const tip = computed(() => {
  if (!auth.canWrite) return 'Seu perfil só consulta: vigiar cria alertas.'
  if (watched.value) return 'Vigiado: alerta se sumir do Wi-Fi. Clique para deixar de vigiar.'
  return 'Avisar se este aparelho sumir do Wi-Fi — para câmeras e IoT.'
})
</script>
