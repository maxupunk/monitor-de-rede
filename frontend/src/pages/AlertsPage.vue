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
        <v-tab value="rules">Regras Configuradas ({{ alertsStore.alertRules.length }})</v-tab>
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
                <div>{{ item.message }}</div>
                <div
                  v-if="item.status === 'recovering' && recoveringInfo(item)"
                  class="text-caption text-warning"
                >
                  {{ recoveringInfo(item) }}
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
                      </div>
                      <div class="text-subtitle-1 font-weight-bold mt-1">{{ item.title }}</div>
                      <div class="text-body-2 text-grey-darken-1">{{ item.message }}</div>
                      <div
                        v-if="item.status === 'recovering' && recoveringInfo(item)"
                        class="text-caption text-warning"
                      >
                        {{ recoveringInfo(item) }}
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
              :items="alertsStore.alertRules"
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
                      <td class="font-weight-medium">{{ alert.title }}</td>
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
    <AlertRuleCatalogDialog v-model="catalogDialog" @applied="onCatalogApplied" />

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="5000">
      {{ feedback.message }}
    </v-snackbar>

    <!-- Modal Silenciar Alerta -->
    <AlertSilenceDialog v-model="silenceDialog" :alert-id="silenceTargetId" />

    <!-- Modal Form de Regra -->
    <v-dialog
      v-model="ruleDialog"
      :max-width="$vuetify.display.xs ? undefined : 620"
      :fullscreen="$vuetify.display.xs"
    >
      <v-card class="rounded-lg pa-4">
        <v-card-title class="font-weight-bold">
          {{ editingRuleId ? 'Editar Regra de Alerta' : 'Cadastrar Regra de Alerta' }}
        </v-card-title>
        <v-card-subtitle class="pb-2">
          Monte a regra em linguagem simples: escolha o que medir, como comparar e a partir de qual
          valor o alerta deve disparar.
        </v-card-subtitle>

        <v-card-text>
          <v-form ref="formRef" @submit.prevent="saveRule">
            <v-text-field
              v-model="form.name"
              label="Nome da Regra"
              placeholder="Ex.: Latência alta no link principal"
              variant="outlined"
              :rules="[(v: string) => !!v || 'Informe um nome para a regra']"
            ></v-text-field>

            <v-select
              v-model="form.field"
              :items="ALERT_METRICS"
              item-title="title"
              item-value="field"
              label="O que monitorar (métrica alvo)"
              :hint="selectedMetric?.hint"
              persistent-hint
              variant="outlined"
              class="mb-4"
              @update:model-value="onMetricChange"
            ></v-select>

            <v-row dense>
              <v-col cols="12" sm="6">
                <v-select
                  v-model="form.operator"
                  :items="availableOperators"
                  item-title="title"
                  item-value="value"
                  label="Quando o valor..."
                  variant="outlined"
                ></v-select>
              </v-col>

              <v-col cols="12" sm="6">
                <v-select
                  v-if="selectedMetric?.kind === 'enum'"
                  v-model="form.value"
                  :items="selectedMetric.options"
                  item-title="title"
                  item-value="value"
                  label="Valor de referência"
                  variant="outlined"
                ></v-select>
                <v-text-field
                  v-else-if="selectedMetric?.kind === 'text'"
                  v-model="form.value"
                  label="Valor de referência"
                  placeholder="Ex.: uplink"
                  variant="outlined"
                ></v-text-field>
                <DataRateInput
                  v-else-if="selectedMetric?.unit === 'bps'"
                  v-model="bpsValue"
                  label="Valor de referência"
                ></DataRateInput>
                <v-text-field
                  v-else
                  v-model.number="form.value"
                  label="Valor de referência"
                  type="number"
                  :suffix="selectedMetric?.unit"
                  variant="outlined"
                ></v-text-field>
              </v-col>
            </v-row>

            <v-select
              v-model="form.durationSeconds"
              :items="ALERT_DURATIONS"
              item-title="title"
              item-value="value"
              label="Tolerância antes de disparar"
              hint="Evita alertas por oscilações momentâneas da rede."
              persistent-hint
              variant="outlined"
              class="mb-4"
            ></v-select>

            <v-select
              v-model="form.recoveryWindowSeconds"
              :items="RECOVERY_WINDOWS"
              item-title="title"
              item-value="value"
              label="Estabilização antes de resolver"
              hint="Só resolve depois que o alvo se mantém estável por esse período; cada recaída reinicia a contagem."
              persistent-hint
              variant="outlined"
              class="mb-4"
            ></v-select>

            <v-select
              v-model="form.severity"
              :items="ALERT_SEVERITIES"
              item-title="title"
              item-value="value"
              label="Nível de severidade"
              variant="outlined"
            ></v-select>

            <v-alert type="info" variant="tonal" density="comfortable" class="mt-2">
              <div class="text-caption font-weight-bold mb-1">Resumo da regra</div>
              <div class="text-body-2">{{ rulePreview }}</div>
            </v-alert>
          </v-form>
        </v-card-text>

        <v-card-actions class="justify-end">
          <v-btn variant="text" @click="ruleDialog = false">Cancelar</v-btn>
          <v-btn color="primary" :loading="saving" @click="saveRule">
            {{ editingRuleId ? 'Salvar Alterações' : 'Salvar Regra' }}
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted } from 'vue'
import { useAlertsStore, type AlertEvent, type AlertRule } from '@/stores/alerts'
import { useEventsStore } from '@/stores/events'
import { useInfiniteList } from '@/composables/useInfiniteList'
import AlertRuleCatalogDialog from '@/components/AlertRuleCatalogDialog.vue'
import AlertSilenceDialog from '@/components/AlertSilenceDialog.vue'
import DataRateInput from '@/components/DataRateInput.vue'
import PageHeader from '@/components/PageHeader.vue'
import ResponsiveDataTable from '@/components/ResponsiveDataTable.vue'
import {
  ALERT_METRICS,
  ALERT_DURATIONS,
  RECOVERY_WINDOWS,
  ALERT_SEVERITIES,
  findMetric,
  operatorsForMetric,
  metricLabel,
  operatorLabel,
  severityLabel,
  severityColor,
  statusLabel,
  statusColor,
  formatConditionValue,
  describeRule,
  type AlertOperator,
} from '@/utils/alertPresentation'
import { formatDateTime, formatRelativeTime } from '@/utils/formatters'

const alertsStore = useAlertsStore()
const eventsStore = useEventsStore()

const tab = ref('active')
const catalogDialog = ref(false)

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
const editingRuleId = ref<number | null>(null)
const saving = ref(false)
const formRef = ref()

const form = reactive({
  name: '',
  field: 'latencyMs',
  operator: 'gt' as AlertOperator,
  value: 150 as number | string,
  durationSeconds: 0,
  recoveryWindowSeconds: 0,
  severity: 'warning' as AlertRule['severity'],
})

const selectedMetric = computed(() => findMetric(form.field))
const availableOperators = computed(() => operatorsForMetric(form.field))

/** Ponte tipada para o DataRateInput, que trabalha só em bps (number | null) */
const bpsValue = computed<number | null>({
  get: () => (typeof form.value === 'number' ? form.value : Number(form.value) || null),
  set: (value) => {
    form.value = value ?? 0
  },
})
const rulePreview = computed(() =>
  describeRule(
    { field: form.field, operator: form.operator, value: form.value },
    form.durationSeconds,
    form.recoveryWindowSeconds
  )
)

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

/** Linha informativa do estado "Estabilizando": último problema e recaídas */
function recoveringInfo(alert: AlertEvent): string {
  const parts: string[] = []
  if (alert.data?.lastProblemAt) {
    parts.push(`último problema ${formatRelativeTime(alert.data.lastProblemAt)}`)
  }
  const recurrences = alert.data?.recurrenceCount ?? 0
  if (recurrences > 0) {
    parts.push(`${recurrences} ${recurrences === 1 ? 'recaída' : 'recaídas'}`)
  }
  return parts.join(' · ')
}

function onMetricChange(field: string) {
  const metric = findMetric(field)
  if (!metric) return
  form.operator = metric.defaultOperator
  form.value = metric.defaultValue
}

function openRuleDialog(rule?: AlertRule) {
  if (rule) {
    editingRuleId.value = rule.id
    form.name = rule.name
    form.field = rule.condition?.field ?? 'latencyMs'
    form.operator = (rule.condition?.operator ?? 'gt') as AlertOperator
    form.value = rule.condition?.value ?? 0
    form.durationSeconds = rule.durationSeconds ?? 0
    form.recoveryWindowSeconds = rule.recoveryWindowSeconds ?? 0
    form.severity = rule.severity ?? 'warning'
  } else {
    editingRuleId.value = null
    form.name = ''
    form.field = 'latencyMs'
    form.operator = 'gt'
    form.value = 150
    form.durationSeconds = 0
    form.recoveryWindowSeconds = 0
    form.severity = 'warning'
  }
  ruleDialog.value = true
}

function openSilenceDialog(id: number) {
  silenceTargetId.value = id
  silenceDialog.value = true
}

function buildPayload(): Partial<AlertRule> {
  const metric = selectedMetric.value
  // Métricas numéricas precisam chegar como número para o comparador do backend
  const value =
    metric?.kind === 'number' && form.value !== '' ? Number(form.value) : String(form.value)

  return {
    name: form.name,
    type: 'custom',
    condition: { field: form.field, operator: form.operator, value },
    durationSeconds: form.durationSeconds,
    recoveryWindowSeconds: form.recoveryWindowSeconds,
    severity: form.severity,
    enabled: true,
  }
}

async function saveRule() {
  const validation = await formRef.value?.validate()
  if (validation && validation.valid === false) return
  if (!form.name) return

  saving.value = true
  const ok = editingRuleId.value
    ? await alertsStore.updateAlertRule(editingRuleId.value, buildPayload())
    : await alertsStore.createAlertRule(buildPayload())
  saving.value = false

  if (ok) ruleDialog.value = false
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
