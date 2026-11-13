<template>
  <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 d-flex flex-column fill-height">
    <v-card-title class="font-weight-bold d-flex align-center ga-2 px-0">
      <v-icon color="primary">mdi-wifi-cog</v-icon>
      Mapa Wi-Fi e roaming
    </v-card-title>
    <v-card-text class="px-0 flex-grow-1">
      <p class="text-body-2 mb-4">
        Quando os roteadores são lidos, como o mapa estima a distância de cada aparelho e quando a
        troca de AP vira alerta.
      </p>

      <v-alert
        v-if="store.error"
        type="error"
        variant="tonal"
        density="compact"
        class="mb-3"
        closable
        :text="store.error"
        @click:close="store.error = null"
      ></v-alert>

      <v-skeleton-loader v-if="!form" type="article, article"></v-skeleton-loader>

      <v-form
        v-else
        ref="formRef"
        :disabled="!auth.canWrite"
        @submit.prevent="save"
        @update:model-value="saved = false"
      >
        <div class="section-title">Atualização</div>
        <v-row dense>
          <v-col cols="12" sm="6">
            <v-text-field
              v-model.number="form.liveIntervalS"
              type="number"
              :min="LIMITS.live[0]"
              :max="LIMITS.live[1]"
              label="Com o mapa aberto (segundos)"
              :hint="`Leitura contínua só enquanto alguém olha o mapa. Entre ${LIMITS.live[0]} e ${LIMITS.live[1]}.`"
              persistent-hint
              variant="outlined"
              density="compact"
              :rules="[range(LIMITS.live)]"
            ></v-text-field>
          </v-col>
          <v-col cols="12" sm="6">
            <v-text-field
              v-model.number="form.backgroundIntervalS"
              type="number"
              :min="LIMITS.background[0]"
              :max="LIMITS.background[1]"
              label="Sem ninguém olhando (segundos)"
              :hint="`Leitura leve que mantém o histórico de roaming e os alertas. Entre ${LIMITS.background[0]} e ${LIMITS.background[1]}.`"
              persistent-hint
              variant="outlined"
              density="compact"
              :rules="[range(LIMITS.background)]"
            ></v-text-field>
          </v-col>
        </v-row>

        <div class="section-title mt-5">Distância estimada</div>
        <p class="text-body-2 mb-3">
          A distância sai do sinal (modelo de perda de percurso) e depende do ambiente: paredes
          encurtam o alcance. É sempre uma faixa aproximada, nunca uma medida.
        </p>
        <v-select
          v-model="form.environment"
          :items="ENVIRONMENTS"
          item-title="title"
          item-value="value"
          label="Ambiente padrão dos APs"
          variant="outlined"
          density="compact"
          hide-details
          class="mb-3"
        ></v-select>
        <div v-if="wifi.devices.length" class="d-flex flex-column ga-2">
          <div class="text-body-2 font-weight-medium">Ajuste por AP (opcional)</div>
          <v-select
            v-for="device in wifi.devices"
            :key="device.deviceId"
            :model-value="environmentOf(device.deviceId)"
            :items="apOptions"
            item-title="title"
            item-value="value"
            :label="device.deviceName"
            variant="outlined"
            density="compact"
            hide-details
            @update:model-value="setEnvironment(device.deviceId, $event)"
          ></v-select>
        </div>

        <div class="section-title mt-5">Roaming</div>
        <v-row dense>
          <v-col cols="12" sm="4">
            <v-text-field
              v-model.number="form.pingPongCount"
              type="number"
              :min="LIMITS.count[0]"
              :max="LIMITS.count[1]"
              label="Ping-pong: trocas"
              variant="outlined"
              density="compact"
              :rules="[range(LIMITS.count)]"
            ></v-text-field>
          </v-col>
          <v-col cols="12" sm="4">
            <v-text-field
              v-model.number="form.pingPongWindowMin"
              type="number"
              :min="LIMITS.window[0]"
              :max="LIMITS.window[1]"
              label="em quantos minutos"
              variant="outlined"
              density="compact"
              :rules="[range(LIMITS.window)]"
            ></v-text-field>
          </v-col>
          <v-col cols="12" sm="4">
            <v-text-field
              v-model.number="form.roamingRetentionDays"
              type="number"
              :min="LIMITS.retention[0]"
              :max="LIMITS.retention[1]"
              label="Guardar o histórico (dias)"
              variant="outlined"
              density="compact"
              :rules="[range(LIMITS.retention)]"
            ></v-text-field>
          </v-col>
        </v-row>
        <p class="text-body-2">
          {{ form.pingPongCount }} trocas de AP em {{ form.pingPongWindowMin }} min viram o alerta
          <strong>Aparelho em ping-pong entre APs</strong> — severidade e canais na tela de regras.
        </p>

        <div v-if="auth.canWrite" class="d-flex align-center justify-end ga-3 mt-4">
          <span v-if="saved" class="text-success font-weight-bold d-inline-flex align-center ga-1">
            <v-icon size="18">mdi-check-circle</v-icon> Salvo
          </span>
          <v-btn
            type="submit"
            color="primary"
            variant="flat"
            prepend-icon="mdi-content-save"
            :loading="store.saving"
          >
            Salvar
          </v-btn>
        </div>
      </v-form>
    </v-card-text>
  </v-card>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import type { WifiEnvironment } from '@/bindings/WifiEnvironment'
import type { WifiSettings } from '@/bindings/WifiSettings'
import { useAuthStore } from '@/stores/auth'
import { useWifiClientsStore } from '@/stores/wifiClients'
import { useWifiSettingsStore } from '@/stores/wifiSettings'
import { ENVIRONMENT_LABEL } from '@/utils/wifiPresentation'

/** Os mesmos limites do servidor (`settings.rs`). */
const LIMITS = {
  live: [5, 300],
  background: [60, 900],
  retention: [1, 365],
  count: [2, 30],
  window: [1, 120],
} as const satisfies Record<string, readonly [number, number]>

const ENVIRONMENTS = (Object.keys(ENVIRONMENT_LABEL) as WifiEnvironment[]).map((value) => ({
  value,
  title: ENVIRONMENT_LABEL[value],
}))

const auth = useAuthStore()
const store = useWifiSettingsStore()
const wifi = useWifiClientsStore()
/** Confirmação discreta depois de salvar; some na próxima edição. */
const saved = ref(false)
const form = ref<WifiSettings | null>(null)
const formRef = ref<{ validate: () => Promise<{ valid: boolean }> } | null>(null)

const apOptions = computed(() => [
  { value: null, title: `Padrão (${form.value ? ENVIRONMENT_LABEL[form.value.environment] : ''})` },
  ...ENVIRONMENTS,
])

function range([min, max]: readonly [number, number]) {
  return (value: number) =>
    (Number.isInteger(value) && value >= min && value <= max) || `Entre ${min} e ${max}.`
}

function environmentOf(deviceId: number): WifiEnvironment | null {
  return form.value?.apEnvironments.find((item) => item.deviceId === deviceId)?.environment ?? null
}

function setEnvironment(deviceId: number, environment: WifiEnvironment | null) {
  if (!form.value) return
  const others = form.value.apEnvironments.filter((item) => item.deviceId !== deviceId)
  form.value.apEnvironments = environment ? [...others, { deviceId, environment }] : others
}

async function save() {
  if (!form.value) return
  const result = await formRef.value?.validate()
  if (result && !result.valid) return
  if (await store.save(form.value)) {
    form.value = store.settings ? structuredClone(store.settings) : form.value
    saved.value = true
  }
}

onMounted(async () => {
  const loaded = await store.fetch()
  if (loaded) form.value = structuredClone(loaded)
})
</script>

<style scoped>
.section-title {
  font-size: 0.75rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  margin-bottom: 8px;
}
</style>
