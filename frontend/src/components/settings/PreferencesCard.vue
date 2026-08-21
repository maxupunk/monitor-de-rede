<template>
  <v-card elevation="2" class="rounded-lg pa-4">
    <v-card-title class="font-weight-bold d-flex align-center">
      <v-icon start color="primary">mdi-cog-outline</v-icon>
      Geral & Monitoramento
    </v-card-title>
    <v-card-text class="mt-2">
      <v-text-field
        v-model.number="form.defaultPingIntervalSeconds"
        label="Intervalo padrão de coleta por Ping"
        type="number"
        variant="outlined"
        suffix="segundos"
        :min="MIN_PING_INTERVAL_SECONDS"
        :max="MAX_PING_INTERVAL_SECONDS"
        :disabled="prefsStore.loading"
        hint="Aplicado a um monitor novo que não define o próprio intervalo. Monitores existentes não mudam."
        persistent-hint
        class="mb-4"
      />
      <v-text-field
        v-model="form.defaultSnmpCommunity"
        label="Comunidade SNMP padrão"
        variant="outlined"
        :disabled="prefsStore.loading"
        hint="Gravada no cadastro de um dispositivo novo com SNMP ligado que não informe a sua. Dispositivos já cadastrados mantêm a que têm."
        persistent-hint
        class="mb-2"
      />
      <v-switch
        v-model="form.autoDiscoveryEnabled"
        label="Varredura automática periódica das redes"
        color="primary"
        :disabled="prefsStore.loading"
        hint="Desligada, o agendador para de disparar varreduras sozinho. O botão “Escanear” de cada rede continua funcionando, e a configuração de cada uma fica intacta."
        persistent-hint
      />

      <v-alert
        v-if="prefsStore.error"
        type="error"
        variant="tonal"
        density="compact"
        class="mt-4"
        :text="prefsStore.error"
      ></v-alert>
    </v-card-text>
    <v-card-actions class="justify-end align-center">
      <span v-if="prefsDirty" class="text-caption text-medium-emphasis mr-2">
        Alterações não salvas
      </span>
      <v-btn variant="text" size="small" :disabled="prefsStore.saving" @click="restaurarPrefs">
        Restaurar padrões
      </v-btn>
      <v-btn color="primary" :loading="prefsStore.saving" @click="salvarPrefs">
        Salvar Preferências
      </v-btn>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { computed, reactive } from 'vue'
import {
  usePreferencesStore,
  defaultPreferences,
  MIN_PING_INTERVAL_SECONDS,
  MAX_PING_INTERVAL_SECONDS,
  type Preferences,
} from '@/stores/preferences'

const prefsStore = usePreferencesStore()
const form = reactive<Preferences>(defaultPreferences())

const prefsDirty = computed(
  () =>
    form.defaultPingIntervalSeconds !== prefsStore.preferences.defaultPingIntervalSeconds ||
    form.defaultSnmpCommunity !== prefsStore.preferences.defaultSnmpCommunity ||
    form.autoDiscoveryEnabled !== prefsStore.preferences.autoDiscoveryEnabled
)

function adotarPrefs(valores: Preferences): void {
  form.defaultPingIntervalSeconds = valores.defaultPingIntervalSeconds
  form.defaultSnmpCommunity = valores.defaultSnmpCommunity
  form.autoDiscoveryEnabled = valores.autoDiscoveryEnabled
}

const emit = defineEmits<{
  saved: []
}>()

async function salvarPrefs(): Promise<void> {
  const ok = await prefsStore.save({ ...form })
  if (!ok) return
  adotarPrefs(prefsStore.preferences)
  emit('saved')
}

function restaurarPrefs(): void {
  adotarPrefs(defaultPreferences())
}

defineExpose({
  adotarPrefs,
})
</script>
