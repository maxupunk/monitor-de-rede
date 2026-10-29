<template>
  <DashboardCard
    title="Alertas ativos"
    icon="mdi-bell-outline"
    :icon-color="alertsStore.activeAlerts.length > 0 ? 'error' : 'success'"
    :subtitle="subtitle"
    to="/alerts"
    flush
  >
    <template #actions>
      <v-btn
        v-if="alertsStore.activeAlerts.length > 0"
        size="small"
        color="primary"
        variant="tonal"
        prepend-icon="mdi-refresh"
        :loading="verifyingAll"
        @click="verifyAll"
      >
        Verificar todos
      </v-btn>
    </template>

    <v-list v-if="alertsStore.activeAlerts.length > 0" lines="two" class="py-0">
      <v-list-item
        v-for="alert in alertsStore.activeAlerts.slice(0, MAX_ITEMS)"
        :key="alert.id"
        :title="alert.title"
        :subtitle="alert.message"
        class="px-5"
        @click="goToAlert(alert)"
      >
        <template #prepend>
          <v-avatar :color="severityColor(alert)" variant="tonal" size="36" rounded="lg">
            <v-icon size="20">mdi-alert</v-icon>
          </v-avatar>
        </template>
        <template #append>
          <div class="d-flex align-center ga-1">
            <v-chip :color="severityColor(alert)" size="x-small" variant="tonal" class="mr-1">
              {{ statusLabel(alert.status) }}
            </v-chip>
            <v-btn
              icon
              size="small"
              variant="text"
              color="info"
              aria-label="Verificar se resolveu"
              :loading="verifyingId === alert.id"
              @click.stop="verify(alert.id)"
            >
              <v-icon>mdi-refresh</v-icon>
              <v-tooltip activator="parent" location="top">Verificar se resolveu</v-tooltip>
            </v-btn>
            <v-btn
              size="small"
              variant="text"
              color="primary"
              icon
              aria-label="Reconhecer alerta"
              :disabled="alert.status === 'acknowledged'"
              :loading="verifyingId === alert.id"
              @click.stop="acknowledge(alert.id)"
            >
              <v-icon>mdi-check-circle-outline</v-icon>
              <v-tooltip activator="parent" location="top">
                Reconhecer (testa e remove se resolvido)
              </v-tooltip>
            </v-btn>
            <v-btn
              size="small"
              variant="text"
              color="warning"
              icon
              aria-label="Silenciar alerta"
              :disabled="alert.status === 'silenced'"
              @click.stop="openSilence(alert.id)"
            >
              <v-icon>mdi-bell-off-outline</v-icon>
              <v-tooltip activator="parent" location="top">Silenciar alerta</v-tooltip>
            </v-btn>
          </div>
        </template>
      </v-list-item>
    </v-list>

    <div v-else class="dashboard-empty">
      <v-icon size="40" color="success">mdi-check-all</v-icon>
      <div class="text-subtitle-2 font-weight-bold mt-2">Nenhum alerta ativo</div>
      <div class="text-body-2">Todos os sistemas estão funcionando normalmente.</div>
    </div>

    <AlertSilenceDialog v-model="silenceDialog" :alert-id="silenceTargetId" />
    <MonitorDetailDialog v-model="detalheAberto" :monitor-id="monitorEmDetalhe" />
    <v-snackbar v-model="feedback.show" :color="feedback.color" location="bottom right">
      {{ feedback.text }}
    </v-snackbar>
  </DashboardCard>
</template>

<script setup lang="ts">
import { computed, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import AlertSilenceDialog from '@/components/AlertSilenceDialog.vue'
import DashboardCard from '@/components/dashboard/DashboardCard.vue'
import MonitorDetailDialog from '@/components/monitors/MonitorDetailDialog.vue'
import { useMonitorDetail } from '@/composables/useMonitorDetail'
import { useAlertsStore, type AlertEvent } from '@/stores/alerts'
import { statusLabel } from '@/utils/alertPresentation'

/** Acima disso a lista vira rolagem de outra tela: "Ver tudo" leva a Alertas. */
const MAX_ITEMS = 5

const router = useRouter()
const alertsStore = useAlertsStore()
const { detalheAberto, monitorEmDetalhe, abrirDetalhe } = useMonitorDetail()

const verifyingId = ref<number | null>(null)
const verifyingAll = ref(false)
const silenceDialog = ref(false)
const silenceTargetId = ref<number | null>(null)
const feedback = reactive({ show: false, text: '', color: 'success' })

const subtitle = computed(() => {
  const total = alertsStore.activeAlerts.length
  if (total === 0) return 'Nada pendente'
  return total > MAX_ITEMS ? `${MAX_ITEMS} de ${total} em aberto` : `${total} em aberto`
})

function severityColor(alert: AlertEvent): string {
  return alert.severity === 'critical' || alert.severity === 'error' ? 'error' : 'warning'
}

function notify(text: string, color: string) {
  Object.assign(feedback, { show: true, text, color })
}

async function acknowledge(id: number) {
  verifyingId.value = id
  const result = await alertsStore.acknowledgeAlert(id)
  verifyingId.value = null
  if (result.resolved) notify(`Alerta #${id} verificado e resolvido automaticamente.`, 'success')
  else if (result.success) notify(`Alerta #${id} reconhecido (continua em falha).`, 'info')
  else notify(result.message || 'Erro ao reconhecer alerta.', 'error')
}

async function verify(id: number) {
  verifyingId.value = id
  const result = await alertsStore.verifyAlert(id)
  verifyingId.value = null
  if (result.resolved) notify(`Alerta #${id} verificado e resolvido.`, 'success')
  else if (result.success) notify(`Alerta #${id} re-verificado: continua com falha.`, 'warning')
  else notify(result.message || 'Erro ao verificar alerta.', 'error')
}

async function verifyAll() {
  verifyingAll.value = true
  const result = await alertsStore.verifyAllAlerts()
  verifyingAll.value = false
  if (result.success) {
    notify(
      result.message || 'Verificação concluída.',
      result.resolvedCount && result.resolvedCount > 0 ? 'success' : 'info'
    )
  } else {
    notify(result.message || 'Erro ao verificar alertas.', 'error')
  }
}

function openSilence(id: number) {
  silenceTargetId.value = id
  silenceDialog.value = true
}

/**
 * Leva o operador ao alvo do alerta. Monitor abre em diálogo — é a única
 * forma de abrir `/monitors/{id}` no produto; dispositivo continua sendo
 * navegação de página, porque ali o contexto **é** a página.
 */
function goToAlert(alert: AlertEvent) {
  if (alert.monitorId) {
    abrirDetalhe(alert.monitorId)
    return
  }
  void router.push(alert.deviceId ? `/devices/${alert.deviceId}` : '/alerts')
}
</script>
