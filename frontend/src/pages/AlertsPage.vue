<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6 flex-wrap ga-4">
      <div>
        <h1 class="text-h4 font-weight-bold">Central de Alertas</h1>
        <p class="text-subtitle-1 text-grey-darken-1">
          Gerenciamento de eventos ativos e definição de regras de notificação
        </p>
      </div>
      <div class="d-flex align-center ga-3">
        <v-chip
          :color="eventsStore.isConnected ? 'success' : 'warning'"
          size="small"
          variant="tonal"
          class="font-weight-medium"
        >
          <v-icon start size="12">mdi-circle</v-icon>
          {{ eventsStore.isConnected ? 'Atualizando em tempo real' : 'Reconectando...' }}
        </v-chip>
        <v-btn
          color="primary"
          variant="tonal"
          prepend-icon="mdi-playlist-check"
          @click="catalogDialog = true"
        >
          Regras Pré-configuradas
        </v-btn>
        <v-btn color="primary" prepend-icon="mdi-plus" @click="openRuleDialog()">
          Nova Regra de Alerta
        </v-btn>
      </div>
    </div>

    <!-- Abas: Alertas Ativos & Regras de Alerta -->
    <v-card elevation="2" class="rounded-lg">
      <v-tabs v-model="tab" color="primary">
        <v-tab value="active">Alertas Ativos ({{ alertsStore.activeAlerts.length }})</v-tab>
        <v-tab value="rules">Regras Configuradas ({{ alertsStore.alertRules.length }})</v-tab>
      </v-tabs>
      <v-divider></v-divider>

      <v-card-text class="pa-4">
        <v-window v-model="tab">
          <!-- Alertas Ativos -->
          <v-window-item value="active">
            <v-data-table
              :headers="activeHeaders"
              :items="alertsStore.activeAlerts"
              :loading="alertsStore.loading"
              :items-per-page="-1"
              hide-default-footer
              no-data-text="Nenhum alerta ativo no momento!"
            >
              <template #item.severity="{ item }">
                <v-chip :color="severityColor(item.severity)" size="small">
                  {{ severityLabel(item.severity) }}
                </v-chip>
              </template>

              <template #item.status="{ item }">
                <v-chip variant="outlined" size="small">
                  {{ statusLabel(item.status) }}
                </v-chip>
              </template>

              <template #item.createdAt="{ item }">
                {{ formatDateTime(item.startedAt || item.createdAt) }}
              </template>

              <template #item.actions="{ item }">
                <div class="d-flex ga-2">
                  <v-btn
                    size="small"
                    color="primary"
                    variant="tonal"
                    :disabled="item.status === 'acknowledged'"
                    @click="alertsStore.acknowledgeAlert(item.id)"
                  >
                    Reconhecer
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
            </v-data-table>
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

            <v-data-table
              :headers="rulesHeaders"
              :items="alertsStore.alertRules"
              :loading="alertsStore.loading"
              :items-per-page="-1"
              hide-default-footer
              no-data-text="Nenhuma regra configurada"
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
                    @click="confirmDeleteRule(item.id)"
                  >
                    <v-icon>mdi-delete</v-icon>
                  </v-btn>
                </div>
              </template>
            </v-data-table>
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
    <v-dialog v-model="ruleDialog" max-width="620">
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
import { useAlertsStore, type AlertRule } from '@/stores/alerts'
import { useEventsStore } from '@/stores/events'
import AlertRuleCatalogDialog from '@/components/AlertRuleCatalogDialog.vue'
import AlertSilenceDialog from '@/components/AlertSilenceDialog.vue'
import {
  ALERT_METRICS,
  ALERT_DURATIONS,
  ALERT_SEVERITIES,
  findMetric,
  operatorsForMetric,
  metricLabel,
  operatorLabel,
  severityLabel,
  severityColor,
  statusLabel,
  formatConditionValue,
  describeRule,
  type AlertOperator,
} from '@/utils/alertPresentation'
import { formatDateTime } from '@/utils/formatters'

const alertsStore = useAlertsStore()
const eventsStore = useEventsStore()

const tab = ref('active')
const catalogDialog = ref(false)
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
  severity: 'warning' as AlertRule['severity'],
})

const selectedMetric = computed(() => findMetric(form.field))
const availableOperators = computed(() => operatorsForMetric(form.field))
const rulePreview = computed(() =>
  describeRule(
    { field: form.field, operator: form.operator, value: form.value },
    form.durationSeconds
  )
)

const activeHeaders = [
  { title: 'Severidade', key: 'severity', width: '120px' },
  { title: 'Título', key: 'title' },
  { title: 'Mensagem', key: 'message' },
  { title: 'Status', key: 'status', width: '130px' },
  { title: 'Data/Hora', key: 'createdAt', width: '170px' },
  { title: 'Ações', key: 'actions', sortable: false, width: '220px' },
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
    form.severity = rule.severity ?? 'warning'
  } else {
    editingRuleId.value = null
    form.name = ''
    form.field = 'latencyMs'
    form.operator = 'gt'
    form.value = 150
    form.durationSeconds = 0
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

async function confirmDeleteRule(id: number) {
  if (confirm('Deseja excluir esta regra de alerta?')) {
    await alertsStore.deleteAlertRule(id)
  }
}
</script>
