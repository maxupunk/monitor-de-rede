<template>
  <PluginItemList
    :list="list"
    :rows="rows"
    :actions="actionInfo"
    :can-write="canWrite"
    :loading="reading"
    :members="view.members.length"
    :presentation="statusAction"
    @add="emit('add')"
    @edit="(row) => emit('edit', row)"
    @remove="(row) => emit('remove', row)"
    @action="(target, row) => emit('action', target, row)"
    @toolbar="(id) => emit('toolbar', id)"
  >
    <template #status>
      <v-alert v-if="!statusBatch && !reading" type="warning" variant="tonal" class="mb-4">
        Ainda não há leitura dos equipamentos.
        <template v-if="canWrite" #append>
          <v-btn color="primary" variant="flat" prepend-icon="mdi-refresh" @click="emit('refresh')">
            Ler agora
          </v-btn>
        </template>
      </v-alert>
    </template>
    <template #empty>
      <v-alert v-if="statusBatch && answered === 0" type="warning" variant="tonal">
        Nenhum equipamento respondeu à última leitura — veja o motivo na aba Equipamentos.
      </v-alert>
      <template v-else-if="statusBatch">
        {{ (list.title ?? 'Lista') + ': nada nos equipamentos ainda.' }}
      </template>
    </template>
  </PluginItemList>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import type { FleetView } from '@/bindings/FleetView'
import type { ItemAction } from '@/bindings/ItemAction'
import type { ItemList } from '@/bindings/ItemList'
import PluginItemList from '@/components/plugins/items/PluginItemList.vue'
import { usePluginAppsStore } from '@/stores/pluginApps'
import { outputsByDevice } from '@/utils/fleetContext'
import { fleetRows, type ItemRow, type ListActionInfo } from '@/utils/itemList'

/**
 * Os itens que os equipamentos têm (ex.: as redes Wi-Fi), lidos da última
 * execução da ação de estado: cada item uma vez, com os equipamentos que o
 * têm. Os botões citam ações de frota; quem as abre é a página.
 */
const props = defineProps<{ view: FleetView; list: ItemList; canWrite: boolean }>()
const emit = defineEmits<{
  add: []
  edit: [row: ItemRow]
  remove: [row: ItemRow]
  action: [target: ItemAction, row: ItemRow]
  toolbar: [fleetActionId: string]
  refresh: []
}>()

const store = usePluginAppsStore()
const statusActionId = computed(() => props.view.plugin.fleet?.statusAction ?? null)
const statusAction = computed(
  () => props.view.plugin.actions.find((action) => action.id === statusActionId.value) ?? null
)
const statusBatch = computed(() =>
  statusActionId.value ? store.latestBatch(props.view.plugin.id, statusActionId.value) : null
)
const reading = computed(() =>
  props.view.batches.some(
    (batch) => batch.action === statusActionId.value && batch.status === 'running'
  )
)
const outputs = computed(() => outputsByDevice(statusBatch.value))
/** Quantos equipamentos responderam à leitura (sem nenhum, a lista vazia não diz nada). */
const answered = computed(() => outputs.value.size)
const rows = computed(() => fleetRows(outputs.value, props.list, props.view.members))
/** Cada ação de frota com o efeito da ação que ela roda nos equipamentos. */
const actionInfo = computed<ListActionInfo[]>(() =>
  (props.view.plugin.fleet?.actions ?? []).map((action) => ({
    id: action.id,
    title: action.title,
    icon: action.icon,
    effect:
      props.view.plugin.actions.find((device) => device.id === action.action)?.effect ?? 'read',
  }))
)
</script>
