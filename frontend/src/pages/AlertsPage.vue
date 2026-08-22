<template>
  <div>
    <PageHeader
      title="Central de Alertas"
      subtitle="Gerenciamento de eventos ativos e definição de regras de notificação"
    >
      <template #actions>
        <v-chip
          :color="eventsStore.isConnected ? 'success' : 'warning'"
          size="small"
          variant="tonal"
          class="font-weight-medium"
        >
          <v-icon start size="12">mdi-circle</v-icon>
          <span class="hidden-xs">{{
            eventsStore.isConnected ? 'Atualizando em tempo real' : 'Reconectando...'
          }}</span>
          <span class="hidden-sm-and-up">{{ eventsStore.isConnected ? 'Ao vivo' : 'Off' }}</span>
        </v-chip>
        <v-btn
          color="primary"
          variant="tonal"
          prepend-icon="mdi-playlist-check"
          @click="catalogDialog = true"
        >
          <span class="hidden-sm-and-down">Regras Pré-configuradas</span>
          <span class="hidden-md-and-up">Catálogo</span>
        </v-btn>
        <v-btn color="primary" prepend-icon="mdi-plus" @click="openRuleDialog()">
          <span class="hidden-sm-and-down">Nova Regra de Alerta</span>
          <span class="hidden-md-and-up">Nova</span>
        </v-btn>
      </template>
    </PageHeader>

    <!-- Banner de Causa Raiz Automática (RCA) -->
    <v-card
      v-for="cluster in correlatedClusters"
      :key="cluster.id"
      color="error"
      variant="tonal"
      class="mb-4 pa-4 rounded-lg border"
    >
      <div
        class="d-flex flex-column flex-md-row align-start align-md-center justify-space-between ga-3"
      >
        <div>
          <div class="d-flex align-center flex-wrap ga-2 mb-1">
            <v-chip size="small" color="error" variant="flat" class="font-weight-bold">
              <v-icon start size="small">mdi-alert-decagram</v-icon>
              Incidente em Cascata ({{ cluster.totalAlertsCount }} alertas)
            </v-chip>
            <v-chip size="small" color="primary" variant="outlined" class="font-weight-bold">
              <v-icon start size="small">mdi-source-branch</v-icon>
              {{ cluster.causalCategoryLabel }}
            </v-chip>
            <v-chip size="small" color="success" variant="outlined" class="font-weight-bold">
              {{ cluster.confidence }}% de Confiança
            </v-chip>
          </div>

          <div class="text-body-1 font-weight-medium text-high-emphasis">
            {{ cluster.explanation }}
          </div>
        </div>

        <v-btn
          color="error"
          variant="flat"
          size="small"
          prepend-icon="mdi-chart-tree"
          @click="openClusterCorrelation(cluster)"
        >
          Investigar Causa Raiz
        </v-btn>
      </div>
    </v-card>

    <v-card elevation="2" rounded="lg">
      <v-tabs v-model="tab" color="primary">
        <v-tab value="active">Alertas Pendentes ({{ alertsStore.activeAlerts.length }})</v-tab>
        <v-tab value="resolved">Alertas Resolvidos ({{ alertsStore.resolvedAlerts.length }})</v-tab>
        <v-tab value="rules">Regras Configuradas ({{ regrasVisiveis.length }})</v-tab>
        <v-tab value="history">Histórico Completo</v-tab>
      </v-tabs>
      <v-divider></v-divider>

      <v-card-text class="pa-4">
        <v-window v-model="tab">
          <v-window-item value="active">
            <ActiveAlertsTab
              v-model:sub-filter="activeSubFilter"
              :verifying-id="verifyingId"
              :verifying-all="verifyingAll"
              @acknowledge="handleAcknowledgeAlert"
              @verify="handleVerifyAlert"
              @silence="openSilenceDialog"
              @verify-all="handleVerifyAllAlerts"
              @correlate="openCorrelationDialog"
            />
          </v-window-item>

          <v-window-item value="resolved">
            <ResolvedAlertsTab />
          </v-window-item>

          <v-window-item value="rules">
            <AlertRulesTab
              :device-filter="filtroDeDispositivo"
              @clear-device-filter="limparFiltroDeDispositivo"
              @open-catalog="catalogDialog = true"
              @edit-rule="openRuleDialog"
              @delete-rule="confirmDeleteRule"
              @toggle-rule="toggleRule"
            />
          </v-window-item>

          <v-window-item value="history">
            <AlertHistoryTab />
          </v-window-item>
        </v-window>
      </v-card-text>
    </v-card>

    <AlertRuleCatalogDialog
      v-model="catalogDialog"
      allow-scope-choice
      @applied="onCatalogApplied"
    />

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="5000">
      {{ feedback.message }}
    </v-snackbar>

    <AlertSilenceDialog v-model="silenceDialog" :alert-id="silenceTargetId" />

    <AlertRuleFormDialog
      v-model="ruleDialog"
      :rule="editingRule"
      :default-device-id="filtroDeDispositivo"
      @saved="onRuleSaved"
    />

    <AlertCorrelationDialog v-model="correlationDialog" :alert-id="correlationTargetId" />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useAlertsStore, type AlertRule, type IncidentCluster } from '@/stores/alerts'
import { useEventsStore } from '@/stores/events'
import AlertRuleCatalogDialog from '@/components/AlertRuleCatalogDialog.vue'
import AlertRuleFormDialog from '@/components/AlertRuleFormDialog.vue'
import AlertSilenceDialog from '@/components/AlertSilenceDialog.vue'
import AlertCorrelationDialog from '@/components/alerts/AlertCorrelationDialog.vue'
import ActiveAlertsTab from '@/components/alerts/ActiveAlertsTab.vue'
import ResolvedAlertsTab from '@/components/alerts/ResolvedAlertsTab.vue'
import AlertRulesTab from '@/components/alerts/AlertRulesTab.vue'
import AlertHistoryTab from '@/components/alerts/AlertHistoryTab.vue'
import PageHeader from '@/components/PageHeader.vue'

const alertsStore = useAlertsStore()
const eventsStore = useEventsStore()

const route = useRoute()
const router = useRouter()

const tab = ref('active')
const catalogDialog = ref(false)

const abasValidas = ['active', 'resolved', 'rules', 'history']

const correlatedClusters = computed(() =>
  alertsStore.activeClusters.filter((c) => c.totalAlertsCount >= 2)
)

function openClusterCorrelation(cluster: IncidentCluster): void {
  const targetId = cluster.rootCauseEvent?.id ?? cluster.events[0]?.id
  if (targetId) {
    openCorrelationDialog(targetId)
  }
}

watch(
  () => route.query.tab,
  (pedida) => {
    const alvo = Array.isArray(pedida) ? pedida[0] : pedida
    if (typeof alvo === 'string' && abasValidas.includes(alvo)) tab.value = alvo
  },
  { immediate: true }
)

watch(tab, (atual) => {
  const naUrl = Array.isArray(route.query.tab) ? route.query.tab[0] : route.query.tab
  if (naUrl === atual) return
  void router.replace({ query: { ...route.query, tab: atual } })
})

const filtroDeDispositivo = computed<number | null>(() => {
  const bruto = route.query.deviceId
  const valor = Number(Array.isArray(bruto) ? bruto[0] : bruto)
  return Number.isFinite(valor) && valor > 0 ? valor : null
})

const regrasVisiveis = computed(() => {
  const alvo = filtroDeDispositivo.value
  if (alvo == null) return alertsStore.alertRules
  return alertsStore.alertRules.filter((regra) => regra.deviceId === alvo)
})

function limparFiltroDeDispositivo(): void {
  const query = { ...route.query }
  delete query.deviceId
  void router.replace({ query })
}

watch(
  [() => route.query.ruleId, () => alertsStore.alertRules],
  ([bruto, regras]) => {
    const id = Number(Array.isArray(bruto) ? bruto[0] : bruto)
    if (!Number.isFinite(id) || id <= 0) return
    const regra = regras.find((item) => item.id === id)
    if (regra) openRuleDialog(regra)
  },
  { immediate: true }
)

const verifyingId = ref<number | null>(null)
const verifyingAll = ref(false)
const activeSubFilter = ref<'all' | 'unacknowledged' | 'acknowledged'>('all')
const correlationDialog = ref(false)
const correlationTargetId = ref<number | null>(null)

function openCorrelationDialog(id: number): void {
  correlationTargetId.value = id
  correlationDialog.value = true
}

async function handleAcknowledgeAlert(id: number) {
  verifyingId.value = id
  const result = await alertsStore.acknowledgeAlert(id)
  verifyingId.value = null

  if (result.resolved) {
    notify(`Alerta #${id} verificado e resolvido automaticamente!`, 'success')
  } else if (result.success) {
    notify(`Alerta #${id} reconhecido (continua em falha).`, 'info')
  } else {
    notify(result.message || 'Erro ao reconhecer alerta.', 'error')
  }
}

async function handleVerifyAlert(id: number) {
  verifyingId.value = id
  const result = await alertsStore.verifyAlert(id)
  verifyingId.value = null

  if (result.resolved) {
    notify(`Alerta #${id} verificado e resolvido!`, 'success')
  } else if (result.success) {
    notify(`Alerta #${id} re-verificado: continua com falha.`, 'warning')
  } else {
    notify(result.message || 'Erro ao verificar alerta.', 'error')
  }
}

async function handleVerifyAllAlerts() {
  verifyingAll.value = true
  const result = await alertsStore.verifyAllAlerts()
  verifyingAll.value = false

  if (result.success) {
    notify(
      result.message || 'Verificação de alertas concluída.',
      result.resolvedCount ? 'success' : 'info'
    )
  } else {
    notify(result.message || 'Erro ao verificar alertas.', 'error')
  }
}

const feedback = reactive({ visible: false, message: '', color: 'success' })
const silenceDialog = ref(false)
const silenceTargetId = ref<number | null>(null)

const ruleDialog = ref(false)
const editingRule = ref<AlertRule | null>(null)

onMounted(() => {
  alertsStore.fetchActiveAlerts()
  alertsStore.fetchAlertRules()
  alertsStore.fetchRootCauseAnalysis()
})

watch(
  () => alertsStore.lastRealtimeUpdateAt,
  () => {
    void alertsStore.fetchRootCauseAnalysis()
  }
)

function notify(message: string, color = 'success') {
  feedback.message = message
  feedback.color = color
  feedback.visible = true
}

function onCatalogApplied(summary: { created: number; skipped: number }) {
  tab.value = 'rules'

  if (summary.created === 0) {
    notify('Nenhuma regra nova: as selecionadas já estavam configuradas.', 'info')
    return
  }

  const skippedNote =
    summary.skipped > 0 ? ` (${summary.skipped} já existia${summary.skipped > 1 ? 'm' : ''})` : ''
  notify(`${summary.created} regra(s) adicionada(s)${skippedNote}.`)
}

function openRuleDialog(rule?: AlertRule) {
  editingRule.value = rule ?? null
  ruleDialog.value = true
}

function openSilenceDialog(id: number) {
  silenceTargetId.value = id
  silenceDialog.value = true
}

async function onRuleSaved() {
  await alertsStore.fetchAlertRules()
  notify('Regra salva.')
}

async function toggleRule(rule: AlertRule, enabled: boolean | null) {
  await alertsStore.updateAlertRule(rule.id, { enabled: !!enabled })
}

async function confirmDeleteRule(rule: AlertRule) {
  const isLinkedToMonitor = !!rule.monitorId
  const promptMessage = isLinkedToMonitor
    ? 'Atenção: Esta regra está vinculada a um monitor de tráfego. Ao excluí-la, o monitor deixará de gerar alertas de tráfego. Deseja continuar?'
    : 'Deseja excluir esta regra de alerta?'

  if (confirm(promptMessage)) {
    await alertsStore.deleteAlertRule(rule.id)
  }
}
</script>
