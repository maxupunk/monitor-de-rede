<template>
  <div class="d-flex align-center flex-wrap ga-1">
    <template v-if="driver === 'ollama'">
      <v-chip v-if="item.isInstalled" size="x-small" color="success" variant="tonal">
        <v-icon start size="12">mdi-check</v-icon>
        Instalado{{ item.size ? ` (${item.size})` : '' }}
      </v-chip>
      <v-chip v-else-if="isTopPick" size="x-small" color="primary" variant="flat">
        <v-icon start size="12">mdi-star</v-icon>
        Mais recomendado
      </v-chip>
      <v-chip v-else-if="item.isRecommended" size="x-small" color="primary" variant="tonal">
        Recomendado
      </v-chip>
      <v-chip v-if="item.parameterSize" size="x-small" color="secondary" variant="tonal">
        {{ item.parameterSize }}
      </v-chip>
    </template>
    <template v-else>
      <v-chip v-if="item.isFree" size="x-small" color="success" variant="tonal">
        {{ item.id === DEFAULT_MODELS.openrouter ? 'Roteador gratuito' : 'Gratuito' }}
      </v-chip>
      <v-chip v-else size="x-small" color="primary" variant="outlined">Pago / créditos</v-chip>
    </template>

    <v-chip
      v-if="context"
      size="x-small"
      color="info"
      variant="tonal"
      title="Janela de contexto do modelo"
    >
      {{ context }}
    </v-chip>
    <v-chip
      v-if="item.supportsTools"
      size="x-small"
      color="warning"
      variant="tonal"
      title="Executa ferramentas: ping, traceroute e diagnósticos de rede"
    >
      <v-icon start size="12">mdi-tools</v-icon>
      Ferramentas
    </v-chip>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { contextLabel, type AiModelChoice } from './aiModelCatalog'
import { DEFAULT_MODELS, type AiDriver } from './aiProviders'

/** Selos de um modelo (gratuito, instalado, contexto, ferramentas), iguais em toda a tela. */
const props = defineProps<{
  item: AiModelChoice
  driver: AiDriver
}>()

const context = computed(() => contextLabel(props.item))
const isTopPick = computed(
  () => props.item.isRecommended && props.item.id === DEFAULT_MODELS.ollama
)
</script>
