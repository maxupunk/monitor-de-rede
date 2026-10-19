<template>
  <QuickPingDialog
    v-model="open"
    :device-id="node?.id ?? null"
    :device-name="node?.name ?? null"
    :ip-address="node?.ipAddress ?? null"
    offer-full-test
  />
</template>

<script setup lang="ts">
/**
 * O diagnóstico rápido por ping sobre o mapa: registra a ação do nó e abre o
 * diálogo. O "teste de velocidade completo" do diálogo segue pelo
 * `linkTestRequests`, que a camada de teste de enlace já atende.
 */
import { onBeforeUnmount, ref, shallowRef } from 'vue'
import type { TopologyNode } from '@/stores/topology'
import QuickPingDialog from '@/components/diagnostics/QuickPingDialog.vue'
import { registerQuickPing } from './registerQuickPing'

const open = ref(false)
const node = shallowRef<TopologyNode | null>(null)

const unregister = registerQuickPing((target) => {
  node.value = target
  open.value = true
})
onBeforeUnmount(unregister)
</script>
