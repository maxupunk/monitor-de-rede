<template>
  <div>
    <div class="d-flex align-center justify-space-between mb-6">
      <div>
        <h1 class="text-h4 font-weight-bold">Central de Alertas</h1>
        <p class="text-subtitle-1 text-grey-darken-1">Gerenciamento de eventos ativos e definição de regras de notificação</p>
      </div>
      <v-btn color="primary" prepend-icon="mdi-plus" @click="ruleDialog = true">
        Nova Regra de Alerta
      </v-btn>
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
              no-data-text="Nenhum alerta ativo no momento!"
            >
              <template #item.severity="{ item }">
                <v-chip :color="getSeverityColor(item.severity)" size="small">
                  {{ (item.severity || 'INFO').toUpperCase() }}
                </v-chip>
              </template>

              <template #item.status="{ item }">
                <v-chip variant="outlined" size="small">
                  {{ (item.status || 'ACTIVE').toUpperCase() }}
                </v-chip>
              </template>

              <template #item.actions="{ item }">
                <div class="d-flex ga-2" style="gap: 8px;">
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
            <v-data-table
              :headers="rulesHeaders"
              :items="alertsStore.alertRules"
              :loading="alertsStore.loading"
              no-data-text="Nenhuma regra configurada"
            >
              <template #item.severity="{ item }">
                <v-chip :color="getSeverityColor(item.severity)" size="small">
                  {{ (item.severity || 'INFO').toUpperCase() }}
                </v-chip>
              </template>

              <template #item.actions="{ item }">
                <v-btn icon size="small" variant="text" color="error" @click="confirmDeleteRule(item.id)">
                  <v-icon>mdi-delete</v-icon>
                </v-btn>
              </template>
            </v-data-table>
          </v-window-item>
        </v-window>
      </v-card-text>
    </v-card>

    <!-- Modal Silenciar Alerta -->
    <v-dialog v-model="silenceDialog" max-width="400">
      <v-card class="rounded-lg pa-4">
        <v-card-title class="font-weight-bold">Silenciar Alerta</v-card-title>
        <v-card-text>
          <v-select
            v-model="silenceDuration"
            :items="[
              { title: '15 minutos', value: 15 },
              { title: '1 hora', value: 60 },
              { title: '4 horas', value: 240 },
              { title: '24 horas', value: 1440 },
            ]"
            label="Duração do Silenciamento"
            variant="outlined"
          ></v-select>
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" @click="silenceDialog = false">Cancelar</v-btn>
          <v-btn color="warning" @click="confirmSilence">Confirmar Silêncio</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Modal Form de Regra -->
    <v-dialog v-model="ruleDialog" max-width="500">
      <v-card class="rounded-lg pa-4">
        <v-card-title class="font-weight-bold">Cadastrar Regra de Alerta</v-card-title>
        <v-card-text>
          <v-form @submit.prevent="saveRule">
            <v-text-field v-model="newRule.name" label="Nome da Regra" variant="outlined" required></v-text-field>
            <v-select
              v-model="newRule.metric"
              :items="['latency_ms', 'packet_loss', 'http_status', 'interface_status']"
              label="Métrica Alvo"
              variant="outlined"
            ></v-select>
            <v-select
              v-model="newRule.condition"
              :items="['gt', 'gte', 'lt', 'lte', 'eq', 'neq']"
              label="Condição"
              variant="outlined"
            ></v-select>
            <v-text-field v-model.number="newRule.threshold" label="Valor Limite (Threshold)" type="number" variant="outlined"></v-text-field>
            <v-select
              v-model="newRule.severity"
              :items="['info', 'warning', 'error', 'critical']"
              label="Severidade"
              variant="outlined"
            ></v-select>
          </v-form>
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" @click="ruleDialog = false">Cancelar</v-btn>
          <v-btn color="primary" @click="saveRule">Salvar Regra</v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, reactive } from 'vue'
import { useAlertsStore } from '@/stores/alerts'

const alertsStore = useAlertsStore()
const tab = ref('active')
const silenceDialog = ref(false)
const silenceTargetId = ref<number | null>(null)
const silenceDuration = ref(60)

const ruleDialog = ref(false)
const newRule = reactive<{
  name: string
  metric: string
  condition: 'gt' | 'gte' | 'lt' | 'lte' | 'eq' | 'neq'
  threshold: number
  severity: 'info' | 'warning' | 'error' | 'critical'
}>({
  name: '',
  metric: 'latency_ms',
  condition: 'gt',
  threshold: 150,
  severity: 'warning',
})

const activeHeaders = [
  { title: 'Severidade', key: 'severity', width: '110px' },
  { title: 'Título', key: 'title' },
  { title: 'Mensagem', key: 'message' },
  { title: 'Status', key: 'status', width: '130px' },
  { title: 'Data/Hora', key: 'createdAt' },
  { title: 'Ações', key: 'actions', sortable: false, width: '220px' },
]

const rulesHeaders = [
  { title: 'ID', key: 'id', width: '60px' },
  { title: 'Nome da Regra', key: 'name' },
  { title: 'Métrica', key: 'metric' },
  { title: 'Condição', key: 'condition' },
  { title: 'Threshold', key: 'threshold' },
  { title: 'Severidade', key: 'severity' },
  { title: 'Ações', key: 'actions', sortable: false, width: '100px' },
]

onMounted(() => {
  alertsStore.fetchActiveAlerts()
  alertsStore.fetchAlertRules()
})

function getSeverityColor(sev: string) {
  switch (sev) {
    case 'critical': return 'error'
    case 'error': return 'deep-orange'
    case 'warning': return 'warning'
    default: return 'info'
  }
}

function openSilenceDialog(id: number) {
  silenceTargetId.value = id
  silenceDialog.value = true
}

async function confirmSilence() {
  if (silenceTargetId.value) {
    await alertsStore.silenceAlert(silenceTargetId.value, silenceDuration.value)
  }
  silenceDialog.value = false
}

async function saveRule() {
  if (!newRule.name) return
  await alertsStore.createAlertRule({ ...newRule, isEnabled: true })
  ruleDialog.value = false
}

async function confirmDeleteRule(id: number) {
  if (confirm('Deseja excluir esta regra de alerta?')) {
    await alertsStore.deleteAlertRule(id)
  }
}
</script>
