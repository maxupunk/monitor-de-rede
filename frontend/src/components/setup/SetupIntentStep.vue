<template>
  <div>
    <div class="mb-4">
      <div class="text-h6 font-weight-bold">O que você vai gerenciar?</div>
      <div class="text-body-2">
        Escolha uma ou mais áreas. O menu, o painel e as próximas etapas mostram só o que você
        escolher; dá para mudar depois em Configurações › Áreas.
      </div>
    </div>

    <v-sheet
      v-for="option in options"
      :key="option.key"
      border
      rounded="lg"
      class="pa-4 mb-3 bg-surface d-flex align-center ga-3 cursor-pointer intent-option"
      :class="{ 'intent-option--on': model[option.key] }"
      :style="model[option.key] ? { borderColor: 'rgb(var(--v-theme-success))' } : undefined"
      @click="toggle(option.key)"
    >
      <v-avatar :color="option.color" size="40" rounded="lg" variant="tonal">
        <v-icon size="22">{{ option.icon }}</v-icon>
      </v-avatar>
      <div class="flex-grow-1 min-w-0">
        <div class="font-weight-bold text-subtitle-1">{{ option.title }}</div>
        <div class="text-body-2">{{ option.description }}</div>
      </div>
      <v-switch
        :model-value="model[option.key]"
        color="success"
        inset
        hide-details
        :aria-label="'Gerenciar ' + option.title"
        @click.stop
        @update:model-value="(on) => set(option.key, Boolean(on))"
      ></v-switch>
    </v-sheet>

    <v-alert v-if="!valid" type="warning" variant="tonal" density="compact" class="mt-1">
      Escolha <strong>Rede</strong> ou <strong>Servidores e Docker</strong> para continuar.
    </v-alert>
    <v-alert v-else-if="error" type="error" variant="tonal" density="compact" class="mt-1">
      {{ error }}
    </v-alert>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { OnboardingIntentInput } from '@/bindings/OnboardingIntentInput'
import { isIntentValid } from './steps'

type IntentKey = keyof OnboardingIntentInput

defineProps<{
  /** Recusa da API ao gravar a escolha (409/422). */
  error?: string | null
}>()

const model = defineModel<OnboardingIntentInput>({ required: true })

const options: {
  key: IntentKey
  title: string
  description: string
  icon: string
  color: string
}[] = [
  {
    key: 'network',
    title: 'Rede',
    description: 'Roteadores, switches, APs, Wi-Fi e enlaces: descoberta, SNMP e topologia.',
    icon: 'mdi-lan',
    color: 'primary',
  },
  {
    key: 'servers',
    title: 'Servidores e Docker',
    description: 'Hosts, containers, compose e bancos, pelo agente instalado em cada servidor.',
    icon: 'mdi-docker',
    color: 'info',
  },
  {
    key: 'vpn',
    title: 'VPN WireGuard',
    description: 'Túneis seguros entre esta central e filiais, roteadores ou servidores remotos.',
    icon: 'mdi-shield-lock-outline',
    color: 'deep-purple',
  },
]

const valid = computed(() => isIntentValid(model.value))

// Muda o campo no próprio objeto: dois cliques seguidos não leem um valor antigo.
function set(key: IntentKey, on: boolean) {
  model.value[key] = on
}

function toggle(key: IntentKey) {
  set(key, !model.value[key])
}
</script>

<style scoped>
.min-w-0 {
  min-width: 0;
}
.cursor-pointer {
  cursor: pointer;
}
.intent-option {
  transition: border-color 0.15s ease;
}
.intent-option--on {
  border-width: 2px !important;
}
</style>
