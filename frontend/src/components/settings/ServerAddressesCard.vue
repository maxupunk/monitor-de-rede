<template>
  <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 d-flex flex-column">
    <v-card-title class="font-weight-bold d-flex align-center">
      <v-icon start color="primary">mdi-server-network</v-icon>
      Endereços deste servidor
    </v-card-title>
    <v-card-text class="mt-2 flex-grow-1">
      <p class="text-caption text-grey-darken-1 mb-4">
        Um servidor, várias portas de entrada. Cada equipamento alcança o NetMonitor pelo endereço
        da rede em que ele está — e é essa lista que aparece na hora de configurar o envio de logs.
      </p>

      <v-list density="compact" class="pa-0 bg-transparent">
        <v-list-item
          v-for="entrada in addressesStore.entries"
          :key="entrada.id"
          class="px-0"
          :title="entrada.label"
        >
          <template #prepend>
            <v-avatar
              :color="addressColor(entrada.kind)"
              size="30"
              rounded="lg"
              variant="tonal"
              class="mr-3"
            >
              <v-icon size="16">{{ addressIcon(entrada.kind) }}</v-icon>
            </v-avatar>
          </template>
          <template #subtitle>
            <span v-if="entrada.value" class="font-weight-medium">{{ entrada.value }}</span>
            <span v-else class="text-medium-emphasis">Não definido</span>
          </template>
        </v-list-item>
      </v-list>
    </v-card-text>
    <v-card-actions class="justify-end">
      <v-btn color="primary" variant="tonal" @click="emit('open-dialog')">
        <v-icon start>mdi-pencil-outline</v-icon>
        Gerenciar endereços
      </v-btn>
    </v-card-actions>
  </v-card>
</template>

<script setup lang="ts">
import { useServerAddressesStore, addressIcon, addressColor } from '@/stores/serverAddresses'

const emit = defineEmits<{
  'open-dialog': []
}>()

const addressesStore = useServerAddressesStore()
</script>
