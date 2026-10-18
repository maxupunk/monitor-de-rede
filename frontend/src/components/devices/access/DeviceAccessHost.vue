<template>
  <DeviceAccessDialog
    v-if="state.request"
    :model-value="state.open"
    :device-id="state.request.deviceId"
    :transport="state.request.transport"
    :reason="state.request.reason"
    @saved="settleAccess"
    @update:model-value="(open) => !open && settleAccess(null)"
  />
</template>

<script setup lang="ts">
/** O diálogo do acesso que `useDeviceAccess` abre de qualquer tela. Montado uma vez no `App.vue`. */
import { useDeviceAccess } from '@/composables/useDeviceAccess'
import DeviceAccessDialog from './DeviceAccessDialog.vue'

const { state, settleAccess } = useDeviceAccess()
</script>
