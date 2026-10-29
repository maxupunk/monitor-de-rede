<template>
  <v-list
    density="comfortable"
    lines="two"
    class="view-options py-1"
    min-width="300"
    max-width="380"
  >
    <v-list-item
      v-for="option in OPTIONS"
      :key="option.key"
      :title="option.title"
      :subtitle="option.subtitle"
      @click="emit('set', option.key, !view[option.key])"
    >
      <template #prepend>
        <v-icon color="primary">{{ option.icon }}</v-icon>
      </template>
      <template #append>
        <v-switch
          :model-value="view[option.key]"
          color="primary"
          density="compact"
          hide-details
          inset
          readonly
          :aria-label="option.title"
        ></v-switch>
      </template>
    </v-list-item>
  </v-list>
</template>

<script setup lang="ts">
/**
 * "Exibir": o que o mapa mostra. Cada opção é uma chave de `prefs`; opção
 * nova entra só na lista abaixo (e em `ViewPrefKey`), sem prop nova no caminho.
 */
import type { ViewPrefKey, ViewPrefs } from '@/utils/topology/prefs'

const OPTIONS: ReadonlyArray<{ key: ViewPrefKey; title: string; subtitle: string; icon: string }> =
  [
    {
      key: 'legendOpen',
      title: 'Legenda',
      subtitle: 'Cores e linhas do mapa (L)',
      icon: 'mdi-map-legend',
    },
    {
      key: 'groupBySite',
      title: 'Caixas por site',
      subtitle: 'Agrupa os dispositivos de cada site',
      icon: 'mdi-office-building-marker-outline',
    },
    {
      key: 'orthogonalEdges',
      title: 'Enlaces em ângulo reto',
      subtitle: 'Traçado de árvore entre níveis (desligado: curvas)',
      icon: 'mdi-vector-polyline',
    },
    {
      key: 'alwaysDetail',
      title: 'Tráfego sempre visível',
      subtitle: 'Rótulo de tráfego em todos os enlaces',
      icon: 'mdi-label-multiple-outline',
    },
    {
      key: 'showHealth',
      title: 'CPU e memória',
      subtitle: 'Sob o nome: laranja a partir de 70%, vermelho de 90%',
      icon: 'mdi-chip',
    },
  ]

defineProps<{ view: ViewPrefs }>()
const emit = defineEmits<{ set: [key: ViewPrefKey, value: boolean] }>()
</script>

<style scoped>
/* Título e explicação inteiros: num menu de opções, reticências escondem o que a chave faz. */
.view-options :deep(.v-list-item-title),
.view-options :deep(.v-list-item-subtitle) {
  white-space: normal;
  -webkit-line-clamp: unset;
  line-clamp: unset;
}
</style>
