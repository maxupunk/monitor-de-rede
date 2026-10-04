<template>
  <div>
    <div v-if="hosts.length === 0" class="empty-state text-center pa-6">
      <v-avatar color="primary" variant="tonal" size="64" class="mb-3">
        <v-icon size="34">mdi-radar</v-icon>
      </v-avatar>
      <div class="text-h6 font-weight-bold">
        {{ scanning ? 'Procurando equipamentos…' : 'Nenhum equipamento encontrado ainda' }}
      </div>
      <p class="text-body-2 mb-4 empty-state__text">
        {{
          scanning
            ? 'Os hosts aparecem aqui assim que respondem.'
            : 'Escolha uma rede cadastrada e inicie a varredura. Roteadores, câmeras, impressoras, IoT, TVs e computadores são identificados automaticamente.'
        }}
      </p>
      <v-btn
        v-if="!scanning"
        color="primary"
        variant="flat"
        prepend-icon="mdi-radar"
        :disabled="!canScan"
        @click="emit('scan')"
      >
        Escanear agora
      </v-btn>
    </div>

    <template v-else>
      <div class="d-flex flex-column flex-md-row align-md-center ga-3 mb-3">
        <v-text-field
          v-model="search"
          placeholder="Buscar por IP, nome, fabricante ou MAC"
          prepend-inner-icon="mdi-magnify"
          variant="outlined"
          density="compact"
          hide-details
          clearable
          class="flex-grow-1"
        ></v-text-field>
        <v-switch
          v-model="hideAdded"
          color="primary"
          density="compact"
          hide-details
          inset
          class="flex-grow-0"
          :label="`Ocultar cadastrados (${addedCount})`"
        ></v-switch>
      </div>

      <v-chip-group v-model="typeFilter" mandatory :column="$vuetify.display.mdAndUp" class="mb-2">
        <v-chip
          :value="ALL"
          :variant="typeFilter === ALL ? 'flat' : 'tonal'"
          color="primary"
          size="small"
          filter
        >
          Todos · {{ hosts.length }}
        </v-chip>
        <v-chip
          v-for="group in typeGroups"
          :key="group.meta.id"
          :value="group.meta.id"
          :color="group.meta.color"
          :prepend-icon="group.meta.icon"
          :variant="typeFilter === group.meta.id ? 'flat' : 'tonal'"
          size="small"
          filter
        >
          {{ group.meta.label }} · {{ group.count }}
        </v-chip>
      </v-chip-group>

      <div v-if="visibleHosts.length === 0" class="text-center pa-6">
        <div class="text-body-1 font-weight-medium mb-2">Nenhum equipamento com esses filtros.</div>
        <v-btn color="primary" variant="tonal" prepend-icon="mdi-filter-off" @click="resetFilters">
          Limpar filtros
        </v-btn>
      </div>

      <v-row v-else dense>
        <v-col v-for="host in visibleHosts" :key="host.ipAddress" cols="12" sm="6" lg="4" xxl="3">
          <DiscoveryHostCard
            :host="host"
            :added="addedIps.has(host.ipAddress)"
            :is-gateway="host.ipAddress === gateway"
            @open="emit('open', $event)"
            @add="emit('add', $event)"
          />
        </v-col>
      </v-row>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import DiscoveryHostCard from './DiscoveryHostCard.vue'
import type { StreamedDiscoveryHost } from '@/stores/discovery'
import type { DeviceTypePresentation } from '@/utils/deviceTypes'
import { discoverySearchText, discoveryTypeMeta } from '@/utils/discoveryPresentation'

const props = defineProps<{
  hosts: StreamedDiscoveryHost[]
  addedIps: ReadonlySet<string>
  gateway?: string | null
  scanning: boolean
  canScan: boolean
}>()

const emit = defineEmits<{
  open: [host: StreamedDiscoveryHost]
  add: [host: StreamedDiscoveryHost]
  scan: []
}>()

const ALL = 'all'
const search = ref<string | null>('')
const typeFilter = ref<string>(ALL)
const hideAdded = ref(false)

const addedCount = computed(
  () => props.hosts.filter((host) => props.addedIps.has(host.ipAddress)).length
)

/** Um chip por tipo encontrado, do mais numeroso para o menos. */
const typeGroups = computed(() => {
  const groups = new Map<string, { meta: DeviceTypePresentation; count: number }>()
  for (const host of props.hosts) {
    const meta = discoveryTypeMeta(host)
    const group = groups.get(meta.id) ?? { meta, count: 0 }
    group.count += 1
    groups.set(meta.id, group)
  }
  return [...groups.values()].sort((a, b) => b.count - a.count)
})

const visibleHosts = computed(() => {
  const term = (search.value ?? '').trim().toLowerCase()
  return props.hosts.filter((host) => {
    if (hideAdded.value && props.addedIps.has(host.ipAddress)) return false
    if (typeFilter.value !== ALL && discoveryTypeMeta(host).id !== typeFilter.value) return false
    return !term || discoverySearchText(host).includes(term)
  })
})

// Uma varredura nova pode não ter o tipo que estava filtrado.
watch(typeGroups, (groups) => {
  if (typeFilter.value !== ALL && !groups.some((group) => group.meta.id === typeFilter.value)) {
    typeFilter.value = ALL
  }
})

function resetFilters() {
  search.value = ''
  typeFilter.value = ALL
  hideAdded.value = false
}
</script>

<style scoped>
.empty-state__text {
  max-width: 560px;
  margin-inline: auto;
}
</style>
