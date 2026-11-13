<template>
  <!--
    `/monitors/:id` **abre um diálogo**, e não uma tela cheia.
    O detalhe de um monitor é sempre consultado a partir de um contexto — a
    lista de monitores, a página do dispositivo, o dashboard — e tirar o
    operador desse contexto para mostrá-lo custava a ele o caminho de volta.
    A rota continua existindo para que um link colado no navegador funcione;
    ela apenas monta o mesmo diálogo que as tabelas montam.
  -->
  <MonitorDetailDialog :model-value="true" :monitor-id="monitorId" @update:model-value="sair" />
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import MonitorDetailDialog from '@/components/monitors/MonitorDetailDialog.vue'
import { useMonitorsStore } from '@/stores/monitors'
import { useAiScreenDetail } from '@/composables/useAiScreenDetail'

const route = useRoute()
const router = useRouter()

const monitorId = computed(() => Number(route.params.id))

const monitorsStore = useMonitorsStore()

/** O chat sabe qual monitor está aberto, de que tipo e em que estado. */
useAiScreenDetail(() => {
  const monitor = monitorsStore.currentMonitor
  if (!monitor || monitor.id !== monitorId.value) return `monitor #${monitorId.value}`
  const target = monitor.port ? `${monitor.target}:${monitor.port}` : monitor.target
  const parts = [
    `monitor ${monitor.name} (#${monitor.id})`,
    `tipo ${monitor.type}`,
    target ? `alvo ${target}` : null,
    monitor.isEnabled === false ? 'desativado' : `status ${monitor.status}`,
    monitor.device?.name ? `dispositivo ${monitor.device.name}` : null,
  ]
  return parts.filter(Boolean).join(', ')
})

/**
 * Fechar o diálogo tem de sair da rota — senão ele reabriria no próximo
 * render, porque o `model-value` é constante.
 *
 * O critério é `history.state.back`, que o Vue Router preenche com a rota
 * anterior **desta aplicação**: `window.history.length` contaria também as
 * páginas visitadas antes de entrar no sistema, e um `back()` ali jogaria o
 * operador para fora do produto.
 */
function sair(): void {
  const anterior = (window.history.state as { back?: string } | null)?.back
  if (anterior) router.back()
  else void router.push('/monitors')
}
</script>
