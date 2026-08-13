<template>
  <div>
    <PageHeader
      title="Configurações do Sistema"
      subtitle="Preferências globais, parâmetros de monitoramento e notificações"
    />

    <v-row>
      <v-col cols="12" md="6">
        <v-card elevation="2" class="rounded-lg pa-4">
          <v-card-title class="font-weight-bold d-flex align-center">
            <v-icon start color="primary">mdi-cog-outline</v-icon>
            Geral & Monitoramento
          </v-card-title>
          <v-card-text class="mt-2">
            <v-text-field
              v-model="defaultPingInterval"
              label="Intervalo Padrão de Ping (segundos)"
              type="number"
              variant="outlined"
            />
            <v-text-field
              v-model="defaultSnmpCommunity"
              label="Comunidade SNMP Padrão"
              variant="outlined"
            />
            <v-switch
              v-model="autoDiscovery"
              label="Habilitar Descoberta Automática Periódica"
              color="primary"
            />
          </v-card-text>
          <v-card-actions class="justify-end">
            <v-btn color="primary" @click="saveSettings">Salvar Preferências</v-btn>
          </v-card-actions>
        </v-card>
      </v-col>

      <v-col cols="12" md="6">
        <v-card elevation="2" class="rounded-lg pa-4">
          <v-card-title class="font-weight-bold d-flex align-center">
            <v-icon start color="info">mdi-view-dashboard-variant-outline</v-icon>
            Sincronização do Dashboard
          </v-card-title>
          <v-card-text class="mt-2">
            <p class="text-caption text-grey-darken-1 mb-3">
              Escolha se deseja utilizar a organização de cards compartilhada no servidor ou manter
              uma distribuição customizada apenas neste navegador.
            </p>

            <v-radio-group
              :model-value="dashboardStore.syncMode"
              color="info"
              hide-details
              @update:model-value="(val) => dashboardStore.setSyncMode(val as any)"
            >
              <v-radio value="server">
                <template #label>
                  <div>
                    <div class="font-weight-bold text-subtitle-2">
                      Sincronizado com o Servidor (Global)
                    </div>
                    <div class="text-caption text-grey">
                      Recebe atualizações de layout em tempo real (SSE) emitidas por qualquer
                      dispositivo.
                    </div>
                  </div>
                </template>
              </v-radio>

              <v-radio value="local" class="mt-3">
                <template #label>
                  <div>
                    <div class="font-weight-bold text-subtitle-2">Personalizado Localmente</div>
                    <div class="text-caption text-grey">
                      Mantém a disposição dos cards exclusiva deste navegador/dispositivo.
                    </div>
                  </div>
                </template>
              </v-radio>
            </v-radio-group>
          </v-card-text>
          <v-card-actions class="justify-space-between flex-wrap ga-2 pt-0">
            <v-btn
              color="info"
              variant="tonal"
              size="small"
              prepend-icon="mdi-cloud-download"
              :loading="dashboardStore.loadingServer"
              @click="dashboardStore.fetchServerLayout"
            >
              Baixar do Servidor
            </v-btn>
            <v-btn
              color="primary"
              variant="flat"
              size="small"
              prepend-icon="mdi-cloud-upload"
              :loading="dashboardStore.savingGlobal"
              @click="dashboardStore.saveLayoutGlobally"
            >
              Salvar como Padrão Global
            </v-btn>
          </v-card-actions>
        </v-card>
      </v-col>

      <v-col cols="12" md="6">
        <v-card elevation="2" class="rounded-lg pa-4">
          <v-card-title class="font-weight-bold d-flex align-center">
            <v-icon start color="warning">mdi-bell-ring-outline</v-icon>
            Notificações PWA do Navegador
          </v-card-title>
          <v-card-text class="mt-2">
            <p class="text-caption text-grey-darken-1 mb-3">
              Configure as notificações do sistema PWA para receber alertas de queda e falhas
              críticas diretamente na área de trabalho ou dispositivo móvel.
            </p>

            <div class="d-flex align-center justify-space-between mb-4 pa-3 rounded border">
              <div>
                <div class="font-weight-bold text-subtitle-2">Status da Permissão</div>
                <div class="text-caption text-grey">
                  {{
                    permissionState === 'granted'
                      ? 'Permissão concedida no navegador'
                      : permissionState === 'denied'
                        ? 'Bloqueado no navegador'
                        : permissionState === 'unsupported'
                          ? 'Navegador não suporta notificações'
                          : 'Ainda não solicitado'
                  }}
                </div>
              </div>
              <v-chip
                :color="
                  permissionState === 'granted'
                    ? 'success'
                    : permissionState === 'denied'
                      ? 'error'
                      : 'warning'
                "
                size="small"
                variant="tonal"
                class="font-weight-bold"
              >
                {{ permissionState.toUpperCase() }}
              </v-chip>
            </div>

            <v-switch
              :model-value="notificationsEnabled"
              label="Ativar Notificações Nativas em Tempo Real"
              color="primary"
              :disabled="permissionState !== 'granted'"
              @update:model-value="(val) => setNotificationsEnabled(Boolean(val))"
            />
          </v-card-text>
          <v-card-actions class="justify-space-between flex-wrap ga-2 pt-0">
            <v-btn
              v-if="permissionState !== 'granted'"
              color="primary"
              variant="flat"
              size="small"
              prepend-icon="mdi-bell-check"
              :disabled="permissionState === 'unsupported'"
              @click="requestPermission"
            >
              Solicitar Permissão
            </v-btn>
            <v-btn
              v-else
              color="secondary"
              variant="tonal"
              size="small"
              prepend-icon="mdi-bell-ring"
              @click="testNotification"
            >
              Enviar Notificação de Teste
            </v-btn>
          </v-card-actions>
        </v-card>
      </v-col>

      <v-col cols="12">
        <v-card elevation="2" class="rounded-lg pa-4">
          <v-card-title class="font-weight-bold d-flex align-center">
            <v-icon start color="success">mdi-database-sync-outline</v-icon>
            Backup e Restauração
          </v-card-title>
          <v-card-text class="mt-2">
            <p class="text-caption text-grey-darken-1 mb-4">
              O backup grava as <strong>configurações</strong> do banco — sites, redes,
              dispositivos, interfaces, enlaces, monitores, regras de alerta, servidores DNS,
              servidor VPN com seus peers e as preferências do sistema. Histórico de coleta
              (métricas, resultados, eventos) e contas de acesso ficam de fora.
            </p>

            <v-alert
              v-if="backupStore.error"
              type="error"
              variant="tonal"
              density="compact"
              class="mb-4"
              closable
              @click:close="backupStore.error = null"
            >
              {{ backupStore.error }}
            </v-alert>

            <v-row>
              <v-col cols="12" md="6">
                <div class="pa-4 rounded-lg border h-100 d-flex flex-column">
                  <div class="font-weight-bold text-subtitle-2 mb-1">Exportar</div>
                  <div class="text-caption text-grey-darken-1 mb-4">
                    Baixa um arquivo JSON com a configuração atual. Ele carrega a community SNMP dos
                    dispositivos e as chaves da VPN cifradas — guarde-o como guardaria uma senha.
                  </div>
                  <v-spacer></v-spacer>
                  <div>
                    <v-btn
                      color="success"
                      variant="flat"
                      prepend-icon="mdi-download"
                      :loading="backupStore.exporting"
                      @click="onExport"
                    >
                      Baixar Backup
                    </v-btn>
                  </div>
                </div>
              </v-col>

              <v-col cols="12" md="6">
                <div class="pa-4 rounded-lg border h-100 d-flex flex-column">
                  <div class="font-weight-bold text-subtitle-2 mb-1">Restaurar</div>
                  <div class="text-caption text-grey-darken-1 mb-3">
                    Substitui toda a configuração atual pela do arquivo. O histórico de coleta dos
                    equipamentos atuais é descartado junto.
                  </div>

                  <v-file-input
                    v-model="selectedFile"
                    label="Arquivo de backup (.json)"
                    accept="application/json,.json"
                    variant="outlined"
                    density="comfortable"
                    prepend-icon=""
                    prepend-inner-icon="mdi-file-upload-outline"
                    hide-details
                    class="mb-3"
                    @update:model-value="onFileSelected"
                  ></v-file-input>

                  <div v-if="backupStore.pendingCounts" class="mb-3">
                    <div class="text-caption text-grey-darken-1 mb-1">
                      <strong>{{ backupStore.pendingName }}</strong> —
                      {{ backupStore.pendingCounts.totalRows }} registros
                    </div>
                    <v-chip
                      v-for="row in backupStore.pendingCounts.tables"
                      :key="row.table"
                      size="x-small"
                      variant="tonal"
                      color="info"
                      class="mr-1 mb-1"
                    >
                      {{ tableLabel(row.table) }}: {{ row.rows }}
                    </v-chip>
                  </div>

                  <v-spacer></v-spacer>
                  <div>
                    <v-btn
                      color="warning"
                      variant="flat"
                      prepend-icon="mdi-database-import-outline"
                      :disabled="!backupStore.pendingFile"
                      :loading="backupStore.restoring"
                      @click="confirmDialog = true"
                    >
                      Restaurar Configurações
                    </v-btn>
                  </div>
                </div>
              </v-col>
            </v-row>

            <v-alert
              v-if="backupStore.lastRestore"
              type="success"
              variant="tonal"
              density="compact"
              class="mt-4"
            >
              Restauração concluída — {{ backupStore.lastRestore.totalRows }} registros aplicados.
            </v-alert>
          </v-card-text>
        </v-card>
      </v-col>
    </v-row>

    <v-dialog v-model="confirmDialog" max-width="520">
      <v-card class="rounded-lg">
        <v-card-title class="font-weight-bold d-flex align-center">
          <v-icon start color="warning">mdi-alert-outline</v-icon>
          Confirmar restauração
        </v-card-title>
        <v-card-text>
          <p class="mb-3">
            Toda a configuração atual será apagada e substituída pela do arquivo
            <strong>{{ backupStore.pendingName }}</strong
            >. O histórico de coleta dos equipamentos atuais também é descartado.
          </p>
          <p class="text-caption text-grey-darken-1 mb-0">
            Usuários e sessões não são afetados — você continua logado.
          </p>
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn variant="text" @click="confirmDialog = false">Cancelar</v-btn>
          <v-btn color="warning" variant="flat" :loading="backupStore.restoring" @click="onRestore">
            Restaurar
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { useDashboardStore } from '@/stores/dashboard'
import { useBackupStore, tableLabel } from '@/stores/backup'
import { useNotifications } from '@/composables/useNotifications'
import PageHeader from '@/components/PageHeader.vue'

const dashboardStore = useDashboardStore()
const backupStore = useBackupStore()
const {
  permissionState,
  notificationsEnabled,
  requestPermission,
  setNotificationsEnabled,
  sendNotification,
} = useNotifications()

const defaultPingInterval = ref(60)
const defaultSnmpCommunity = ref('public')
const autoDiscovery = ref(true)

function saveSettings() {
  alert('Configurações salvas com sucesso!')
}

const selectedFile = ref<File | File[] | null>(null)
const confirmDialog = ref(false)

async function onExport() {
  await backupStore.exportConfig()
}

/**
 * O `v-model` do `v-file-input` do Vuetify 3 devolve `File[]` quando `multiple`
 * está ligado e `File | null` quando não está; normalizar aqui evita depender
 * dessa distinção no resto da tela.
 */
async function onFileSelected(value: File | File[] | null) {
  const file = Array.isArray(value) ? value[0] : value
  if (!file) {
    backupStore.clearFile()
    return
  }
  await backupStore.loadFile(file)
}

async function onRestore() {
  const ok = await backupStore.restoreConfig()
  confirmDialog.value = false
  if (ok) {
    selectedFile.value = null
    // A restauração trocou sites, redes, dispositivos e monitores por baixo de
    // todas as stores já carregadas. Recarregar a aplicação é mais honesto do
    // que tentar invalidar uma por uma e deixar alguma tela com dado morto.
    window.location.reload()
  }
}

function testNotification() {
  sendNotification('Notificação de Teste PWA', {
    body: 'Este é um teste de funcionamento das notificações em tempo real do NetMonitor.',
  })
}
</script>
