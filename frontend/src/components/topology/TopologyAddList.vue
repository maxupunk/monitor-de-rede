<template>
  <v-list density="comfortable" class="py-1" min-width="320">
    <v-list-item
      v-for="item in ITEMS"
      :key="item.kind"
      :prepend-icon="item.icon"
      :title="item.title"
      :subtitle="item.subtitle"
      base-color="primary"
      @click="$emit('add', item.kind)"
    ></v-list-item>
  </v-list>
</template>

<script lang="ts">
export type AddKind = 'link' | 'switch' | 'discover' | 'device'

const ITEMS: ReadonlyArray<{ kind: AddKind; title: string; subtitle: string; icon: string }> = [
  {
    kind: 'link',
    title: 'Enlace entre dispositivos',
    subtitle: 'Escolha as portas de cada ponta',
    icon: 'mdi-vector-polyline-plus',
  },
  {
    kind: 'switch',
    title: 'Switch não gerenciável',
    subtitle: 'Nó de derivação sem IP nem SNMP',
    icon: 'mdi-hub',
  },
  {
    kind: 'discover',
    title: 'Descobrir enlaces',
    subtitle: 'LLDP/CDP e inferência por sub-rede',
    icon: 'mdi-radar',
  },
  {
    kind: 'device',
    title: 'Cadastrar dispositivo',
    subtitle: 'Abre a tela de dispositivos',
    icon: 'mdi-plus-network',
  },
]
</script>

<script setup lang="ts">
defineEmits<{ add: [kind: AddKind] }>()
</script>
