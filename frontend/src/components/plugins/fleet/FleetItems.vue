<template>
  <div>
    <div class="d-flex align-center flex-wrap ga-2 mb-4">
      <v-icon color="primary">{{ matrix.icon ?? 'mdi-format-list-bulleted' }}</v-icon>
      <span class="text-h6 font-weight-bold">{{ matrix.title ?? 'Itens' }}</span>
      <v-chip size="small" color="primary" variant="tonal">{{ rows.length }}</v-chip>
      <v-spacer></v-spacer>
      <v-btn
        v-if="canWrite && matrix.add"
        color="primary"
        variant="flat"
        prepend-icon="mdi-plus"
        @click="emit('add')"
      >
        {{ addLabel }}
      </v-btn>
    </div>

    <v-alert v-if="!statusBatch" :type="reading ? 'info' : 'warning'" variant="tonal" class="mb-4">
      {{ reading ? 'Lendo os equipamentos…' : 'Ainda não há leitura dos equipamentos.' }}
      <template v-if="!reading && canWrite" #append>
        <v-btn color="primary" variant="flat" prepend-icon="mdi-refresh" @click="emit('refresh')">
          Ler agora
        </v-btn>
      </template>
    </v-alert>
    <v-alert v-else-if="rows.length === 0" type="info" variant="tonal" class="mb-4">
      Nenhuma {{ itemName }} nos equipamentos ainda.
    </v-alert>

    <v-row dense>
      <v-col v-for="row in rows" :key="row.key" cols="12" md="6" xl="4">
        <v-card
          border
          flat
          class="rounded-lg h-100"
          :link="canEdit"
          @click="canEdit && emit('edit', row)"
        >
          <v-card-item>
            <template #prepend>
              <v-avatar :color="row.active ? 'primary' : 'secondary'" variant="tonal" rounded="lg">
                <v-icon>{{ matrix.icon ?? 'mdi-circle-outline' }}</v-icon>
              </v-avatar>
            </template>
            <v-card-title class="font-weight-bold">{{ row.key }}</v-card-title>
            <v-card-subtitle>{{ row.subtitle }}</v-card-subtitle>
            <template v-if="canWrite && (matrix.edit || matrix.remove)" #append>
              <v-menu>
                <template #activator="{ props: menu }">
                  <v-btn
                    v-bind="menu"
                    icon="mdi-dots-vertical"
                    size="small"
                    variant="text"
                    color="primary"
                    :aria-label="`Opções de ${row.key}`"
                    @click.stop
                  />
                </template>
                <v-list density="compact">
                  <v-list-item
                    v-if="matrix.edit"
                    prepend-icon="mdi-pencil-outline"
                    title="Editar"
                    @click="emit('edit', row)"
                  ></v-list-item>
                  <v-list-item
                    v-if="matrix.remove"
                    prepend-icon="mdi-delete-outline"
                    base-color="error"
                    title="Remover"
                    @click="emit('remove', row)"
                  ></v-list-item>
                </v-list>
              </v-menu>
            </template>
          </v-card-item>
          <v-card-text class="pt-0">
            <div class="d-flex flex-wrap ga-1 mb-2">
              <v-chip
                v-for="entry in row.entries"
                :key="entry.deviceId"
                size="small"
                :color="stateColor(entry.state)"
                variant="tonal"
                prepend-icon="mdi-router-wireless"
              >
                {{ entry.name }}
              </v-chip>
            </div>
            <div class="text-body-2">
              Em {{ row.entries.length }} de {{ members }} equipamento(s)
              <template v-if="row.detailTotal !== null">
                · {{ row.detailTotal }} {{ detailLabel.toLowerCase() }}
              </template>
            </div>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { FleetMatrix } from '@/bindings/FleetMatrix'
import type { FleetView } from '@/bindings/FleetView'
import { usePluginAppsStore } from '@/stores/pluginApps'
import { itemRows, outputsByDevice, type ItemRow } from '@/utils/fleetContext'
import { outputLabel, stateColor } from '@/utils/pluginPresentation'

/**
 * Os itens que os equipamentos têm (ex.: as redes Wi-Fi), um cartão cada:
 * nome, o que importa, em quais equipamentos está. Clique edita.
 */
const props = defineProps<{ view: FleetView; matrix: FleetMatrix; canWrite: boolean }>()
const emit = defineEmits<{
  add: []
  edit: [row: ItemRow]
  remove: [row: ItemRow]
  refresh: []
}>()

const store = usePluginAppsStore()
const statusAction = computed(() => props.view.plugin.fleet?.statusAction ?? null)
const statusBatch = computed(() =>
  statusAction.value ? store.latestBatch(props.view.plugin.id, statusAction.value) : null
)
const reading = computed(() =>
  props.view.batches.some(
    (batch) => batch.action === statusAction.value && batch.status === 'running'
  )
)
const rows = computed(() =>
  itemRows(outputsByDevice(statusBatch.value), props.matrix, props.view.members)
)
const members = computed(() => props.view.members.length)
const itemName = computed(() => props.matrix.itemName ?? 'item')
const addLabel = computed(() => 'Nova ' + itemName.value)
const canEdit = computed(() => props.canWrite && Boolean(props.matrix.edit))
const detailLabel = computed(() => {
  const status = props.view.plugin.actions.find((action) => action.id === statusAction.value)
  return outputLabel(props.matrix.detail ?? '', status?.labels)
})
</script>
