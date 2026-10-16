<template>
  <v-list density="comfortable" class="py-1" min-width="300">
    <v-list-item
      title="Legenda"
      subtitle="Cores e linhas do mapa (L)"
      @click="toggle('legendOpen')"
    >
      <template #prepend>
        <v-icon color="primary">mdi-map-legend</v-icon>
      </template>
      <template #append>
        <v-switch
          :model-value="legendOpen"
          color="primary"
          density="compact"
          hide-details
          inset
          readonly
          aria-label="Mostrar legenda"
        ></v-switch>
      </template>
    </v-list-item>
    <v-list-item
      title="Caixas por site"
      subtitle="Agrupa os dispositivos de cada site"
      @click="toggle('groupBySite')"
    >
      <template #prepend>
        <v-icon color="primary">mdi-office-building-marker-outline</v-icon>
      </template>
      <template #append>
        <v-switch
          :model-value="groupBySite"
          color="primary"
          density="compact"
          hide-details
          inset
          readonly
          aria-label="Agrupar por site"
        ></v-switch>
      </template>
    </v-list-item>
    <v-list-item
      title="Tráfego sempre visível"
      subtitle="Rótulo de tráfego em todos os enlaces"
      @click="toggle('alwaysDetail')"
    >
      <template #prepend>
        <v-icon color="primary">mdi-label-multiple-outline</v-icon>
      </template>
      <template #append>
        <v-switch
          :model-value="alwaysDetail"
          color="primary"
          density="compact"
          hide-details
          inset
          readonly
          aria-label="Tráfego sempre visível"
        ></v-switch>
      </template>
    </v-list-item>
  </v-list>
</template>

<script setup lang="ts">
const props = defineProps<{ legendOpen: boolean; groupBySite: boolean; alwaysDetail: boolean }>()
const emit = defineEmits<{
  'update:legendOpen': [value: boolean]
  'update:groupBySite': [value: boolean]
  'update:alwaysDetail': [value: boolean]
}>()

function toggle(key: 'legendOpen' | 'groupBySite' | 'alwaysDetail') {
  if (key === 'legendOpen') emit('update:legendOpen', !props.legendOpen)
  else if (key === 'groupBySite') emit('update:groupBySite', !props.groupBySite)
  else emit('update:alwaysDetail', !props.alwaysDetail)
}
</script>
