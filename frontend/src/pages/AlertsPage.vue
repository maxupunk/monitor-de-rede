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

    <!-- Abas: Alertas Ativos, Resolvidos, Regras e Histórico -->
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
          <!-- Alertas Ativos / Pendentes -->
          <v-window-item value="active">
            <div
              class="d-flex flex-column flex-sm-row align-start align-sm-center justify-space-between ga-3 mb-4"
            >
              <v-btn-toggle
                v-model="activeSubFilter"
                density="compact"
                variant="outlined"
                color="primary"
                mandatory
              >
                <v-btn value="all" size="small">
                  Todos ({{ alertsStore.activeAlerts.length }})
                </v-btn>
                <v-btn value="unacknowledged" size="small">
                  Não Reconhecidos ({{ alertsStore.pendingAlerts.length }})
                </v-btn>
                <v-btn value="acknowledged" size="small">
                  Reconhecidos ({{ alertsStore.acknowledgedAlerts.length }})
                </v-btn>
              </v-btn-toggle>

              <v-btn
                v-if="alertsStore.activeAlerts.length > 0"
                size="small"
                color="primary"
                variant="tonal"
                prepend-icon="mdi-refresh"
                :loading="verifyingAll"
                @click="handleVerifyAllAlerts"
              >
                Verificar Todos os Pendentes
              </v-btn>
            </div>

            <ResponsiveDataTable
              :headers="activeHeaders"
              :items="filteredActiveAlerts"
              :loading="alertsStore.loading"
              :items-per-page="-1"
              hide-default-footer
              no-data-text="Nenhum alerta pendente no momento!"
              :clickable="false"
            >
              <template #item.severity="{ item }">
                <v-chip :color="severityColor(item.severity)" size="small">
                  {{ severityLabel(item.severity) }}
                </v-chip>
              </template>

              <template #item.status="{ item }">
                <v-chip :color="statusColor(item.status)" variant="outlined" size="small">
                  {{ statusLabel(item.status) }}
                </v-chip>
              </template>

              <template #item.message="{ item }">
                <div>
                  <v-chip
                    v-if="problemKindLabel(item.data?.problemKind)"
                    size="x-small"
                    variant="tonal"
                    color="grey"
                    class="mr-2"
                  >
                    {{ problemKindLabel(item.data?.problemKind) }}
                  </v-chip>
                  {{ item.message }}
                </div>
                <div v-if="episodeInfo(item)" class="text-caption text-warning">
                  {{ episodeInfo(item) }}
                </div>
              </template>

              <template #item.createdAt="{ item }">
                {{ formatDateTime(item.startedAt || item.createdAt) }}
              </template>

              <template #item.actions="{ item }">
                <div class="d-flex align-center ga-1 flex-wrap">
                  <v-btn
                    size="small"
                    color="primary"
                    variant="tonal"
                    :disabled="item.status === 'acknowledged'"
                    :loading="verifyingId === item.id"
                    @click="handleAcknowledgeAlert(item.id)"
                  >
                    Reconhecer
                  </v-btn>
                  <v-btn
                    size="small"
                    color="info"
                    variant="outlined"
                    :loading="verifyingId === item.id"
                    @click="handleVerifyAlert(item.id)"
                  >
                    Verificar
                  </v-btn>
                  <v-btn
                    size="small"
                    color="warning"
                    variant="outlined"
                    :disabled="item.status === 'silenced'"
                    @click="openSilenceDialog(item.id)"
                  >
                    Silenciar
                  </v-btn>
                </div>
              </template>

              <template #mobile-item="{ item }">
                <div class="d-flex flex-column ga-2">
                  <div class="d-flex align-start justify-space-between ga-2">
                    <div class="flex-grow-1 text-break">
                      <div class="d-flex flex-wrap align-center ga-2">
                        <v-chip :color="severityColor(item.severity)" size="x-small">
                          {{ severityLabel(item.severity) }}
                        </v-chip>
                        <v-chip :color="statusColor(item.status)" variant="outlined" size="x-small">
                          {{ statusLabel(item.status) }}
                        </v-chip>
                        <v-chip
                          v-if="problemKindLabel(item.data?.problemKind)"
                          size="x-small"
                          variant="tonal"
                          color="grey"
                        >
                          {{ problemKindLabel(item.data?.problemKind) }}
                        </v-chip>
                      </div>
                      <div class="text-subtitle-1 font-weight-bold mt-1">{{ item.title }}</div>
                      <div class="text-body-2 text-grey-darken-1">{{ item.message }}</div>
                      <div v-if="episodeInfo(item)" class="text-caption text-warning">
                        {{ episodeInfo(item) }}
                      </div>
                      <div class="text-caption text-grey mt-1">
                        {{ formatDateTime(item.startedAt || item.createdAt) }}
                      </div>
                    </div>
                  </div>
                  <div class="d-flex align-center ga-1 flex-wrap mt-1">
                    <v-btn
                      size="small"
                      color="primary"
                      variant="tonal"
                      :disabled="item.status === 'acknowledged'"
                      :loading="verifyingId === item.id"
                      @click="handleAcknowledgeAlert(item.id)"
                    >
                      Reconhecer
                    </v-btn>
                    <v-btn
                      size="small"
                      color="info"
                      variant="outlined"
                      :loading="verifyingId === item.id"
                      @click="handleVerifyAlert(item.id)"
                    >
                      Verificar
                    </v-btn>
                    <v-btn
                      size="small"
                      color="warning"
                      variant="outlined"
                      :disabled="item.status === 'silenced'"
                      @click="openSilenceDialog(item.id)"
                    >
                      Silenciar
                    </v-btn>
                  </div>
                </div>
              </template>
            </ResponsiveDataTable>
          </v-window-item>

          <!-- Alertas Resolvidos -->
          <v-window-item value="resolved">
            <ResponsiveDataTable
              :headers="resolvedHeaders"
              :items="alertsStore.resolvedAlerts"
              :loading="alertsStore.loading"
              :items-per-page="-1"
              hide-default-footer
              no-data-text="Nenhum alerta resolvido na sessão atual."
              :clickable="false"
            >
              <template #item.severity="{ item }">
                <v-chip :color="severityColor(item.severity)" size="small">
                  {{ severityLabel(item.severity) }}
                </v-chip>
              </template>

              <template #item.createdAt="{ item }">
                {{ formatDateTime(item.startedAt || item.createdAt) }}
              </template>

              <template #item.resolvedAt="{ item }">
                <v-chip color="success" variant="tonal" size="small">
                  <v-icon start size="14">mdi-check-circle</v-icon>
                  {{ item.resolvedAt ? formatDateTime(item.resolvedAt) : 'Resolvido' }}
                </v-chip>
              </template>

              <template #mobile-item="{ item }">
                <div class="d-flex flex-column ga-2 pa-1">
                  <div class="d-flex align-center ga-2">
                    <v-chip :color="severityColor(item.severity)" size="x-small">
                      {{ severityLabel(item.severity) }}
                    </v-chip>
                    <v-chip color="success" variant="tonal" size="x-small"> Resolvido </v-chip>
                  </div>
                  <div class="text-subtitle-1 font-weight-bold">{{ item.title }}</div>
                  <div class="text-body-2 text-grey-darken-1">{{ item.message }}</div>
                  <div class="text-caption text-grey">
                    Início: {{ formatDateTime(item.startedAt || item.createdAt) }} | Resolvido:
                    {{ item.resolvedAt ? formatDateTime(item.resolvedAt) : 'Sim' }}
                  </div>
                </div>
              </template>
            </ResponsiveDataTable>
          </v-window-item>

          <!-- Regras Configuradas -->
          <v-window-item value="rules">
            <!--
              O recorte vindo da URL precisa estar **visível**: uma lista
              filtrada que não se anuncia parece uma lista curta, e o operador
              conclui que perdeu regras.
            -->
            <v-alert
              v-if="filtroDeDispositivo"
              type="info"
              variant="tonal"
              density="comfortable"
              class="mb-4 rounded-lg"
            >
              <div class="d-flex flex-wrap align-center justify-space-between ga-2">
                <span>
                  Mostrando apenas as regras de
                  <strong>{{ deviceName(filtroDeDispositivo) }}</strong
                  >.
                </span>
                <div class="d-flex ga-2">
                  <v-btn
                    size="small"
                    variant="text"
                    :to="`/devices/${filtroDeDispositivo}?tab=rules`"
                  >
                    Abrir o dispositivo
                  </v-btn>
                  <v-btn size="small" variant="tonal" @click="limparFiltroDeDispositivo">
                    Ver todas
                  </v-btn>
                </div>
              </div>
            </v-alert>
            <v-alert
              v-if="!alertsStore.loading && alertsStore.alertRules.length === 0"
              type="info"
              variant="tonal"
              density="comfortable"
              class="mb-4"
            >
              Nenhuma regra configurada. Comece pelas
              <a
                class="font-weight-bold text-primary"
                href="#"
                @click.prevent="catalogDialog = true"
                >regras pré-configuradas</a
              >
              para cobrir indisponibilidade, latência, perda de pacotes e quedas de interface.
            </v-alert>

            <ResponsiveDataTable
              :headers="rulesHeaders"
              :items="regrasVisiveis"
              :loading="alertsStore.loading"
              :items-per-page="-1"
              hide-default-footer
              no-data-text="Nenhuma regra configurada"
              :clickable="false"
            >
              <template #item.name="{ item }">
                <div class="d-flex align-center ga-2">
                  <span>{{ item.name }}</span>
                  <v-tooltip v-if="item.templateKey" text="Criada a partir do catálogo de regras">
                    <template #activator="{ props: tooltipProps }">
                      <v-icon v-bind="tooltipProps" size="16" color="primary">
                        mdi-playlist-check
                      </v-icon>
                    </template>
                  </v-tooltip>
                </div>
              </template>

              <!--
                O escopo diz **de quem** é a regra. Sem ele, duas regras do
                mesmo template — uma do servidor, outra de um roteador —
                apareciam com nome idêntico e nada as distinguia na lista.
              -->
              <template #item.scope="{ item }">
                <div v-if="item.deviceId || item.monitorId" class="d-flex flex-column">
                  <RouterLink
                    v-if="item.deviceId"
                    class="text-primary text-decoration-none font-weight-medium"
                    :to="`/devices/${item.deviceId}?tab=rules`"
                  >
                    <v-icon size="14" class="mr-1">mdi-router-network</v-icon>
                    {{ deviceName(item.deviceId) }}
                  </RouterLink>
                  <span v-if="item.monitorId" class="text-caption text-grey">
                    Monitor #{{ item.monitorId }}
                  </span>
                </div>
                <span v-else class="text-caption text-grey">Todos os dispositivos</span>
              </template>

              <template #item.metric="{ item }">
                {{ metricLabel(item.condition?.field) }}
              </template>

              <template #item.criteria="{ item }">
                <span class="text-body-2">
                  {{ operatorLabel(item.condition?.operator).toLowerCase() }}
                  <strong>
                    {{ formatConditionValue(item.condition?.field, item.condition?.value) }}
                  </strong>
                </span>
              </template>

              <template #item.durationSeconds="{ item }">
                {{ durationLabel(item.durationSeconds) }}
              </template>

              <template #item.severity="{ item }">
                <v-chip :color="severityColor(item.severity)" size="small">
                  {{ severityLabel(item.severity) }}
                </v-chip>
              </template>

              <template #item.enabled="{ item }">
                <v-switch
                  :model-value="item.isEnabled ?? item.enabled"
                  color="success"
                  density="compact"
                  hide-details
                  @update:model-value="toggleRule(item, $event)"
                ></v-switch>
              </template>

              <template #item.actions="{ item }">
                <div class="d-flex ga-1">
                  <v-btn icon size="small" variant="text" @click="openRuleDialog(item)">
                    <v-icon>mdi-pencil</v-icon>
                  </v-btn>
                  <v-btn
                    icon
                    size="small"
                    variant="text"
                    color="error"
                    @click="confirmDeleteRule(item)"
                  >
                    <v-icon>mdi-delete</v-icon>
                  </v-btn>
                </div>
              </template>

              <template #mobile-item="{ item }">
                <div class="d-flex flex-column ga-2">
                  <div class="d-flex align-start justify-space-between ga-2">
                    <div class="flex-grow-1 text-break">
                      <div class="d-flex flex-wrap align-center ga-2">
                        <span class="text-subtitle-2 font-weight-bold">{{ item.name }}</span>
                        <v-chip :color="severityColor(item.severity)" size="x-small">
                          {{ severityLabel(item.severity) }}
                        </v-chip>
                      </div>
                      <div class="text-body-2 text-grey-darken-1 mt-1">
                        {{ metricLabel(item.condition?.field) }}
                        {{ operatorLabel(item.condition?.operator).toLowerCase() }}
                        <strong>
                          {{ formatConditionValue(item.condition?.field, item.condition?.value) }}
                        </strong>
                      </div>
                      <div class="text-caption text-grey mt-1">
                        Tolerância: {{ durationLabel(item.durationSeconds) }}
                      </div>
                    </div>
                    <v-switch
                      :model-value="item.isEnabled ?? item.enabled"
                      color="success"
                      density="compact"
                      hide-details
                      style="transform: scale(0.85); transform-origin: right top"
                      @update:model-value="toggleRule(item, $event)"
                    ></v-switch>
                  </div>
                  <div class="d-flex ga-1 mt-1">
                    <v-btn icon size="small" variant="text" @click="openRuleDialog(item)">
                      <v-icon>mdi-pencil</v-icon>
                    </v-btn>
                    <v-btn
                      icon
                      size="small"
                      variant="text"
                      color="error"
                      @click="confirmDeleteRule(item)"
                    >
                      <v-icon>mdi-delete</v-icon>
                    </v-btn>
                  </div>
                </div>
              </template>
            </ResponsiveDataTable>
          </v-window-item>

          <!-- Histórico completo, incluindo alertas já normalizados -->
          <v-window-item value="history">
            <div class="d-flex align-center justify-space-between mb-3 flex-wrap ga-2">
              <div class="text-body-2 text-grey-darken-1">
                Todos os alertas já registrados, do mais recente para o mais antigo.
              </div>
              <v-chip
                v-if="history.total.value > 0"
                size="small"
                variant="outlined"
                color="primary"
              >
                {{ history.items.value.length }} de {{ history.total.value }}
              </v-chip>
            </div>

            <v-infinite-scroll :key="history.scrollKey.value" @load="history.load">
              <div class="table-responsive">
                <v-table hover density="comfortable" class="rounded-lg border">
                  <thead>
                    <tr>
                      <th style="width: 110px">Severidade</th>
                      <th style="width: 120px">Situação</th>
                      <th>Alerta</th>
                      <th>Mensagem</th>
                      <th style="width: 170px">Início</th>
                      <th style="width: 170px">Normalizado em</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr v-for="alert in history.items.value" :key="alert.id">
                      <td>
                        <v-chip :color="severityColor(alert.severity)" size="x-small">
                          {{ severityLabel(alert.severity) }}
                        </v-chip>
                      </td>
                      <td>
                        <v-chip
                          :color="statusColor(alert.status)"
                          variant="outlined"
                          size="x-small"
                        >
                          {{ statusLabel(alert.status) }}
                        </v-chip>
                      </td>
                      <td class="font-weight-medium">
                        {{ alert.title }}
                        <v-chip
                          v-if="problemKindLabel(alert.data?.problemKind)"
                          size="x-small"
                          variant="tonal"
                          color="grey"
                          class="ml-2"
                        >
                          {{ problemKindLabel(alert.data?.problemKind) }}
                        </v-chip>
                      </td>
                      <td class="text-body-2">{{ alert.message || '—' }}</td>
                      <td>{{ formatDateTime(alert.startedAt || alert.createdAt) }}</td>
                      <td>
                        <span v-if="alert.resolvedAt">{{ formatDateTime(alert.resolvedAt) }}</span>
                        <span v-else class="text-grey">Em aberto</span>
                      </td>
                    </tr>
                  </tbody>
                </v-table>
              </div>
              <template #empty>
                <div class="text-caption text-grey text-center py-4">
                  Nenhum outro alerta no histórico.
                </div>
              </template>
            </v-infinite-scroll>
          </v-window-item>
        </v-window>
      </v-card-text>
    </v-card>

    <!-- Modal Regras Pré-configuradas -->
    <!--
      Aqui o dispositivo é **escolhido**; aberto pela página do equipamento, ele
      já vem fixado. É o mesmo componente nos dois casos — o que muda é só o
      escopo que ele recebe.
    -->
    <AlertRuleCatalogDialog
      v-model="catalogDialog"
      allow-scope-choice
      @applied="onCatalogApplied"
    />

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="5000">
      {{ feedback.message }}
    </v-snackbar>

    <!-- Modal Silenciar Alerta -->
    <AlertSilenceDialog v-model="silenceDialog" :alert-id="silenceTargetId" />

    <!--
      O **mesmo** formulário da aba Regras do dispositivo. Ele carrega o
      seletor de escopo, com "Todos os dispositivos" entre as opções.
    -->
    <AlertRuleFormDialog
      v-model="ruleDialog"
      :rule="editingRule"
      :default-device-id="filtroDeDispositivo"
      @saved="onRuleSaved"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useAlertsStore, type AlertEvent, type AlertRule } from '@/stores/alerts'
import { useDevicesStore } from '@/stores/devices'
import { useEventsStore } from '@/stores/events'
import { useInfiniteList } from '@/composables/useInfiniteList'
import AlertRuleCatalogDialog from '@/components/AlertRuleCatalogDialog.vue'
import AlertRuleFormDialog from '@/components/AlertRuleFormDialog.vue'
import AlertSilenceDialog from '@/components/AlertSilenceDialog.vue'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import {
  ALERT_DURATIONS,
  metricLabel,
  operatorLabel,
  severityLabel,
  severityColor,
  statusLabel,
  statusColor,
  problemKindLabel,
  formatConditionValue,
} from '@/utils/alertPresentation'
import { formatDateTime, formatRelativeTime } from '@/utils/formatters'

const alertsStore = useAlertsStore()
const eventsStore = useEventsStore()

const route = useRoute()
const router = useRouter()

const tab = ref('active')
const catalogDialog = ref(false)

const devicesStore = useDevicesStore()

/** Nome do dispositivo pelo id, com o próprio id como último recurso. */
function deviceName(id: number): string {
  return devicesStore.devices.find((device) => device.id === id)?.name ?? `Dispositivo #${id}`
}

/**
 * Recorte da lista de regras vindo da URL.
 *
 * `/alerts?tab=rules&deviceId=1` é o atalho que a página do dispositivo usa —
 * e é a mesma Central de Alertas, só filtrada. Sem isto o link abria a aba
 * errada e mostrava o parque inteiro.
 */
const filtroDeDispositivo = computed<number | null>(() => {
  const bruto = route.query.deviceId
  const valor = Number(Array.isArray(bruto) ? bruto[0] : bruto)
  return Number.isFinite(valor) && valor > 0 ? valor : null
})

const abasValidas = ['active', 'resolved', 'rules', 'history']

/** A aba pedida na URL manda; a URL acompanha a aba escolhida na tela. */
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

/** Remove o recorte sem sair da aba. */
function limparFiltroDeDispositivo(): void {
  const query = { ...route.query }
  delete query.deviceId
  void router.replace({ query })
}

/** As regras mostradas na aba, já recortadas pelo filtro da URL. */
const regrasVisiveis = computed(() => {
  const alvo = filtroDeDispositivo.value
  if (alvo == null) return alertsStore.alertRules
  return alertsStore.alertRules.filter((regra) => regra.deviceId === alvo)
})

/** Abre a regra pedida em `?ruleId=` — o atalho de edição vindo do dispositivo. */
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

const filteredActiveAlerts = computed(() => {
  if (activeSubFilter.value === 'unacknowledged') {
    return alertsStore.pendingAlerts
  }
  if (activeSubFilter.value === 'acknowledged') {
    return alertsStore.acknowledgedAlerts
  }
  return alertsStore.activeAlerts
})

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

/** Histórico completo: cresce sem teto, então vem paginado do servidor */
const history = useInfiniteList<AlertEvent>(() => '/alerts', { label: 'histórico de alertas' })
const feedback = reactive({ visible: false, message: '', color: 'success' })
const silenceDialog = ref(false)
const silenceTargetId = ref<number | null>(null)

const ruleDialog = ref(false)
const editingRule = ref<AlertRule | null>(null)

const activeHeaders = [
  { title: 'Severidade', key: 'severity', width: '120px' },
  { title: 'Título', key: 'title' },
  { title: 'Mensagem', key: 'message' },
  { title: 'Status', key: 'status', width: '130px' },
  { title: 'Data/Hora', key: 'createdAt', width: '170px' },
  { title: 'Ações', key: 'actions', sortable: false, width: '280px' },
]

const resolvedHeaders = [
  { title: 'Severidade', key: 'severity', width: '120px' },
  { title: 'Título', key: 'title' },
  { title: 'Mensagem', key: 'message' },
  { title: 'Data de Início', key: 'createdAt', width: '170px' },
  { title: 'Resolvido Em', key: 'resolvedAt', width: '170px' },
]

const rulesHeaders = [
  { title: 'Nome da Regra', key: 'name' },
  { title: 'Escopo', key: 'scope', sortable: false, width: '200px' },
  { title: 'Métrica Monitorada', key: 'metric', sortable: false },
  { title: 'Critério de Disparo', key: 'criteria', sortable: false },
  { title: 'Tolerância', key: 'durationSeconds', width: '150px' },
  { title: 'Severidade', key: 'severity', width: '120px' },
  { title: 'Ativa', key: 'enabled', sortable: false, width: '90px' },
  { title: 'Ações', key: 'actions', sortable: false, width: '110px' },
]

onMounted(() => {
  alertsStore.fetchActiveAlerts()
  alertsStore.fetchAlertRules()
  // A coluna de escopo e o seletor do catálogo mostram **nome**, não id: um
  // "Dispositivo #7" na lista de regras não diz nada a quem opera.
  void devicesStore.fetchDevices()
})

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

function durationLabel(seconds?: number): string {
  return ALERT_DURATIONS.find((d) => d.value === (seconds ?? 0))?.title ?? `${seconds}s`
}

/**
 * Linha informativa do episódio: último problema, recaídas e, quando o alvo foi
 * declarado oscilante, desde quando. Vale para "Estabilizando" e "Oscilando" —
 * os dois estados abertos em que a história do episódio explica a tela; nos
 * demais não há episódio a contar e a função devolve string vazia.
 */
function episodeInfo(alert: AlertEvent): string {
  if (alert.status !== 'recovering' && alert.status !== 'flapping') return ''
  const parts: string[] = []
  if (alert.status === 'flapping' && alert.data?.flappingSince) {
    parts.push(`oscilando desde ${formatRelativeTime(alert.data.flappingSince)}`)
  }
  if (alert.data?.lastProblemAt) {
    parts.push(`último problema ${formatRelativeTime(alert.data.lastProblemAt)}`)
  }
  const recurrences = alert.data?.recurrenceCount ?? 0
  if (recurrences > 0) {
    parts.push(`${recurrences} ${recurrences === 1 ? 'recaída' : 'recaídas'}`)
  }
  return parts.join(' · ')
}

/**
 * Abre o formulário compartilhado.
 *
 * Sem regra, é cadastro novo — e o escopo começa no dispositivo filtrado pela
 * URL, quando houver: quem chegou por `/alerts?tab=rules&deviceId=1` está
 * olhando as regras daquele equipamento e quase certamente quer criar mais uma
 * para ele.
 */
function openRuleDialog(rule?: AlertRule) {
  editingRule.value = rule ?? null
  ruleDialog.value = true
}

function openSilenceDialog(id: number) {
  silenceTargetId.value = id
  silenceDialog.value = true
}

/** O componente já persistiu; aqui só se recarrega o que a tela mostra. */
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
