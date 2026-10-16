<template>
  <TopologyLinkDialog
    v-model="dialogs.link.open"
    :editing-link-id="dialogs.link.editingId"
    :initial-source-device-id="dialogs.link.sourceId"
    :initial-target-device-id="dialogs.link.targetId"
    :initial-source-interface-id="dialogs.link.sourceInterfaceId"
    :initial-target-interface-id="dialogs.link.targetInterfaceId"
    :initial-link-type="dialogs.link.linkType"
    @saved="$emit('saved', 'Enlace salvo.')"
  />
  <UnmanagedSwitchDialog
    v-model="dialogs.unmanagedSwitch.open"
    :default-site-id="siteId"
    :switch-to-edit="dialogs.unmanagedSwitch.toEdit"
    @created="$emit('saved', 'Switch salvo.')"
  />
  <DeviceDialog
    v-model="dialogs.device.open"
    :device-to-edit="dialogs.device.toEdit"
    @saved="onDeviceSaved"
  />
</template>

<script setup lang="ts">
/** Diálogos de edição do mapa; quem abre é a página, pelo estado injetado. */
import { inject } from 'vue'
import { useTopologyStore } from '@/stores/topology'
import { TOPOLOGY_DIALOGS } from '@/composables/topology/useTopologyDialogs'
import TopologyLinkDialog from './TopologyLinkDialog.vue'
import UnmanagedSwitchDialog from './UnmanagedSwitchDialog.vue'
import DeviceDialog from '@/components/DeviceDialog.vue'

defineProps<{ siteId: number | null }>()
const emit = defineEmits<{ saved: [message: string] }>()

const injected = inject(TOPOLOGY_DIALOGS)
if (!injected) throw new Error('TopologyDialogs precisa do estado fornecido pela página')
const dialogs = injected
const store = useTopologyStore()

function onDeviceSaved() {
  store.refreshIfLoaded()
  emit('saved', 'Dispositivo salvo.')
}
</script>
