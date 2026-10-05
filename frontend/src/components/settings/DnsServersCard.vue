<template>
  <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4 d-flex flex-column fill-height">
    <v-card-title class="font-weight-bold d-flex align-center ga-2 px-0">
      <v-icon color="primary">mdi-dns-outline</v-icon>
      Servidores DNS
    </v-card-title>
    <v-card-text class="px-0 flex-grow-1">
      <p class="text-body-2 mb-3">
        Resolvedores oferecidos nos monitores de DNS. Os marcados entram na comparação de tempo de
        consulta do dashboard.
      </p>
      <v-progress-linear v-if="loading" indeterminate color="primary" class="mb-2" />
      <div v-else-if="store.servers.length === 0" class="text-body-2">
        Nenhum servidor cadastrado.
      </div>
      <v-list v-else density="compact" class="pa-0 bg-transparent">
        <v-list-item
          v-for="server in visibleServers"
          :key="server.id"
          class="px-0"
          :title="server.name"
          :subtitle="server.address"
        >
          <template #append>
            <div class="d-flex ga-1">
              <v-chip size="x-small" color="info" variant="tonal">
                {{ server.protocol.toUpperCase() }}
              </v-chip>
              <v-chip v-if="server.isDefault" size="x-small" color="success" variant="tonal">
                Comparação
              </v-chip>
            </div>
          </template>
        </v-list-item>
      </v-list>
      <div v-if="hiddenCount > 0" class="text-body-2 mt-1">
        e mais {{ hiddenCount }} servidor(es).
      </div>
    </v-card-text>
    <v-card-actions class="px-0">
      <v-spacer></v-spacer>
      <v-btn
        color="primary"
        variant="flat"
        prepend-icon="mdi-pencil-outline"
        @click="dialog = true"
      >
        Gerenciar servidores
      </v-btn>
    </v-card-actions>

    <DnsServersDialog v-model="dialog" @saved="store.fetchServers(true)" />
  </v-card>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import DnsServersDialog from '@/components/DnsServersDialog.vue'
import { useDnsServersStore } from '@/stores/dnsServers'

/** Quantos aparecem no cartão; o resto fica no diálogo. */
const VISIBLE = 5

const store = useDnsServersStore()
const dialog = ref(false)
const loading = ref(false)

const visibleServers = computed(() => store.servers.slice(0, VISIBLE))
const hiddenCount = computed(() => Math.max(0, store.servers.length - VISIBLE))

onMounted(async () => {
  loading.value = true
  await store.fetchServers()
  loading.value = false
})
</script>
