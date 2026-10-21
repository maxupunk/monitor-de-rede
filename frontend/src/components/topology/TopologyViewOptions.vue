<template>
  <v-list density="comfortable" class="py-1" min-width="300">
    <v-list-item
      v-for="option in OPTIONS"
      :key="option.key"
      :title="option.title"
      :subtitle="option.subtitle"
      @click="toggle(option.key)"
    >
      <template #prepend>
        <v-icon color="primary">{{ option.icon }}</v-icon>
      </template>
      <template #append>
        <v-switch
          :model-value="props[option.key]"
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
/** "Exibir": o que o mapa mostra. Cada opção é uma chave de `prefs`. */
type ViewKey = 'legendOpen' | 'groupBySite' | 'orthogonalEdges' | 'alwaysDetail' | 'showHealth'

const OPTIONS: ReadonlyArray<{ key: ViewKey; title: string; subtitle: string; icon: string }> = [
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
    title: 'CPU e memória nos dispositivos',
    subtitle: 'Uma linha curta sob o nome, de perto (quando há coleta)',
    icon: 'mdi-chip',
  },
]

const props = defineProps<{
  legendOpen: boolean
  groupBySite: boolean
  orthogonalEdges: boolean
  alwaysDetail: boolean
  showHealth: boolean
}>()
const emit = defineEmits<{
  'update:legendOpen': [value: boolean]
  'update:groupBySite': [value: boolean]
  'update:alwaysDetail': [value: boolean]
  'update:orthogonalEdges': [value: boolean]
  'update:showHealth': [value: boolean]
}>()

function toggle(key: ViewKey) {
  const next = !props[key]
  if (key === 'legendOpen') emit('update:legendOpen', next)
  else if (key === 'groupBySite') emit('update:groupBySite', next)
  else if (key === 'orthogonalEdges') emit('update:orthogonalEdges', next)
  else if (key === 'alwaysDetail') emit('update:alwaysDetail', next)
  else emit('update:showHealth', next)
}
</script>
