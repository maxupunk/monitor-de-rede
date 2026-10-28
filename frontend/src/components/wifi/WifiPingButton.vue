<template>
  <v-tooltip
    v-if="!unavailable"
    :text="tip"
    :disabled="!tip"
    location="top"
    :open-on-click="Boolean(disabledReason)"
  >
    <template #activator="{ props: tipProps }">
      <span v-bind="tipProps" :class="{ 'd-block': block }">
        <v-btn
          v-if="compact && measured"
          icon="mdi-refresh"
          size="small"
          variant="tonal"
          color="primary"
          aria-label="Medir a latência de novo"
          :loading="state?.loading"
          :disabled="Boolean(disabledReason)"
          @click="store.ping(row)"
        ></v-btn>
        <v-btn
          v-else
          size="small"
          variant="tonal"
          color="primary"
          prepend-icon="mdi-pulse"
          :block="block"
          :loading="state?.loading"
          :disabled="Boolean(disabledReason)"
          @click="store.ping(row)"
        >
          {{ label }}
        </v-btn>
      </span>
    </template>
  </v-tooltip>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useAuthStore } from '@/stores/auth'
import { useWifiClientsStore } from '@/stores/wifiClients'
import { pingUnavailableReason, type WifiStationRow } from '@/utils/wifiPresentation'

const props = defineProps<{
  row: WifiStationRow
  /** Na tabela: depois da primeira medida, vira só o ícone de repetir. */
  compact?: boolean
  block?: boolean
}>()

const store = useWifiClientsStore()
const auth = useAuthStore()

const state = computed(() => store.pingOf(props.row.deviceId, props.row.mac))
const measured = computed(() => Boolean(state.value?.result || state.value?.error))

/** Sem como medir, o botão nem aparece — quem explica é o `WifiPingResult`. */
const unavailable = computed(() => Boolean(pingUnavailableReason(props.row)))

const disabledReason = computed(() =>
  auth.canWrite ? '' : 'Seu perfil só consulta: medir roda um comando no roteador.'
)

const label = computed(() => {
  if (props.compact) return 'Medir'
  return measured.value ? 'Medir de novo' : 'Medir latência'
})

const tip = computed(() => disabledReason.value || (props.compact ? 'Medir a latência' : ''))
</script>
