<template>
  <div class="d-flex flex-column ga-3">
    <v-chip-group
      :model-value="filter.health"
      mandatory
      selected-class="font-weight-bold"
      aria-label="Filtrar pelo estado"
      @update:model-value="update({ health: $event })"
    >
      <v-chip value="all" color="primary" variant="tonal" filter> Todos ({{ counts.all }}) </v-chip>
      <v-chip value="attention" color="error" variant="tonal" filter>
        Precisam de atenção ({{ counts.attention }})
      </v-chip>
      <v-chip value="power_save" color="info" variant="tonal" filter>
        Em repouso ({{ counts.powerSave }})
      </v-chip>
    </v-chip-group>
    <v-chip
      v-if="watchedCount !== undefined"
      :model-value="filter.watchedOnly"
      color="primary"
      :variant="filter.watchedOnly ? 'flat' : 'tonal'"
      prepend-icon="mdi-eye"
      class="align-self-start"
      :aria-pressed="filter.watchedOnly"
      @click="update({ watchedOnly: !filter.watchedOnly })"
    >
      Só vigiados ({{ watchedCount }})
    </v-chip>

    <v-row dense>
      <v-col cols="12" :md="devices ? 3 : 6">
        <v-text-field
          :model-value="filter.search"
          prepend-inner-icon="mdi-magnify"
          label="Buscar"
          placeholder="Nome, IP, MAC, rede ou fabricante"
          density="compact"
          variant="outlined"
          hide-details
          clearable
          @update:model-value="update({ search: $event ?? '' })"
        ></v-text-field>
      </v-col>
      <v-col v-if="devices" cols="12" sm="6" md="3">
        <v-select
          :model-value="filter.deviceId"
          :items="devices"
          item-title="title"
          item-value="value"
          label="Roteador"
          density="compact"
          variant="outlined"
          hide-details
          @update:model-value="update({ deviceId: $event })"
        ></v-select>
      </v-col>
      <v-col v-if="ssids && ssids.length > 1" cols="12" sm="6" md="3">
        <v-select
          :model-value="filter.ssid"
          :items="ssidOptions"
          item-title="title"
          item-value="value"
          label="Rede (SSID)"
          density="compact"
          variant="outlined"
          hide-details
          @update:model-value="update({ ssid: $event })"
        ></v-select>
      </v-col>
      <v-col cols="12" :sm="devices ? 6 : 12" :md="devices ? 3 : 6">
        <v-select
          :model-value="filter.band"
          :items="BANDS"
          item-title="title"
          item-value="value"
          label="Banda"
          density="compact"
          variant="outlined"
          hide-details
          @update:model-value="update({ band: $event })"
        ></v-select>
      </v-col>
    </v-row>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { WifiBand } from '@/bindings/WifiBand'
import {
  BAND_LABEL,
  type WifiStationCounts,
  type WifiStationFilter,
} from '@/utils/wifiPresentation'

const props = defineProps<{
  counts: WifiStationCounts
  /** Opções de roteador; sem elas, o filtro não aparece (aba do equipamento). */
  devices?: { title: string; value: number | null }[]
  /** As redes vistas; com uma só, o filtro não aparece. */
  ssids?: string[]
  /** Quantos vigiados estão conectados; sem o número, o filtro não aparece. */
  watchedCount?: number
}>()

const filter = defineModel<WifiStationFilter>({ required: true })

const BANDS: { title: string; value: WifiBand | null }[] = [
  { title: 'Todas as bandas', value: null },
  { title: BAND_LABEL['2g'], value: '2g' },
  { title: BAND_LABEL['5g'], value: '5g' },
  { title: BAND_LABEL['6g'], value: '6g' },
]

const ssidOptions = computed(() => [
  { title: 'Todas as redes', value: null },
  ...(props.ssids ?? []).map((ssid) => ({ title: ssid, value: ssid })),
])

function update(change: Partial<WifiStationFilter>) {
  filter.value = { ...filter.value, ...change }
}
</script>
