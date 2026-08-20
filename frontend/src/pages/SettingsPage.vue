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
            <!--
              Cada campo diz **onde** ele muda o comportamento. Uma preferência
              sem efeito declarado é indistinguível de uma que não funciona — e
              esta tela passou tempo demais sendo exatamente isso.
            -->
            <v-text-field
              v-model.number="form.defaultPingIntervalSeconds"
              label="Intervalo padrão de coleta por Ping"
              type="number"
              variant="outlined"
              suffix="segundos"
              :min="MIN_PING_INTERVAL_SECONDS"
              :max="MAX_PING_INTERVAL_SECONDS"
              :disabled="prefsStore.loading"
              hint="Aplicado a um monitor novo que não define o próprio intervalo. Monitores existentes não mudam."
              persistent-hint
              class="mb-4"
            />
            <v-text-field
              v-model="form.defaultSnmpCommunity"
              label="Comunidade SNMP padrão"
              variant="outlined"
              :disabled="prefsStore.loading"
              hint="Gravada no cadastro de um dispositivo novo com SNMP ligado que não informe a sua. Dispositivos já cadastrados mantêm a que têm."
              persistent-hint
              class="mb-2"
            />
            <v-switch
              v-model="form.autoDiscoveryEnabled"
              label="Varredura automática periódica das redes"
              color="primary"
              :disabled="prefsStore.loading"
              hint="Desligada, o agendador para de disparar varreduras sozinho. O botão “Escanear” de cada rede continua funcionando, e a configuração de cada uma fica intacta."
              persistent-hint
            />

            <v-alert
              v-if="prefsStore.error"
              type="error"
              variant="tonal"
              density="compact"
              class="mt-4"
              :text="prefsStore.error"
            ></v-alert>
          </v-card-text>
          <v-card-actions class="justify-end align-center">
            <!--
              O botão fica sempre ativo: desabilitá-lo até haver alteração o
              deixaria cinza, que nesta interface significa "quebrado". O aviso
              de pendência carrega o estado, e salvar valores iguais é
              inofensivo.
            -->
            <span v-if="prefsDirty" class="text-caption text-medium-emphasis mr-2">
              Alterações não salvas
            </span>
            <v-btn
              variant="text"
              size="small"
              :disabled="prefsStore.saving"
              @click="restaurarPrefs"
            >
              Restaurar padrões
            </v-btn>
            <v-btn color="primary" :loading="prefsStore.saving" @click="salvarPrefs">
              Salvar Preferências
            </v-btn>
          </v-card-actions>
        </v-card>
      </v-col>

      <v-col cols="12" md="6">
        <v-card elevation="2" class="rounded-lg pa-4 d-flex flex-column">
          <v-card-title class="font-weight-bold d-flex align-center">
            <v-icon start color="primary">mdi-server-network</v-icon>
            Endereços deste servidor
          </v-card-title>
          <v-card-text class="mt-2 flex-grow-1">
            <p class="text-caption text-grey-darken-1 mb-4">
              Um servidor, várias portas de entrada. Cada equipamento alcança o NetMonitor pelo
              endereço da rede em que ele está — e é essa lista que aparece na hora de configurar o
              envio de logs.
            </p>

            <v-list density="compact" class="pa-0 bg-transparent">
              <v-list-item
                v-for="entrada in addressesStore.entries"
                :key="entrada.id"
                class="px-0"
                :title="entrada.label"
              >
                <template #prepend>
                  <v-avatar
                    :color="addressColor(entrada.kind)"
                    size="30"
                    rounded="lg"
                    variant="tonal"
                    class="mr-3"
                  >
                    <v-icon size="16">{{ addressIcon(entrada.kind) }}</v-icon>
                  </v-avatar>
                </template>
                <template #subtitle>
                  <span v-if="entrada.value" class="font-weight-medium">{{ entrada.value }}</span>
                  <span v-else class="text-medium-emphasis">Não definido</span>
                </template>
              </v-list-item>
            </v-list>
          </v-card-text>
          <v-card-actions class="justify-end">
            <v-btn color="primary" variant="tonal" @click="addressesDialog = true">
              <v-icon start>mdi-pencil-outline</v-icon>
              Gerenciar endereços
            </v-btn>
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
            <v-icon start color="primary">mdi-rocket-launch-outline</v-icon>
            Assistente de Configuração Inicial
          </v-card-title>
          <v-card-text class="mt-2">
            <p class="text-caption text-grey-darken-1 mb-3">
              Execute novamente o assistente de primeiro acesso para cadastrar novos locais (Sites),
              sub-redes, servidores DNS, endereços deste servidor e ajustar parâmetros globais de
              forma guiada e rápida.
            </p>
          </v-card-text>
          <v-card-actions class="justify-end">
            <v-btn
              color="primary"
              variant="flat"
              prepend-icon="mdi-auto-fix"
              @click="onboardingStore.openWizard()"
            >
              Abrir Assistente de Configuração
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

    <ServerAddressesDialog v-model="addressesDialog" />

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="4000">
      {{ feedback.message }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { useDashboardStore } from '@/stores/dashboard'
import {
  usePreferencesStore,
  defaultPreferences,
  MIN_PING_INTERVAL_SECONDS,
  MAX_PING_INTERVAL_SECONDS,
  type Preferences,
} from '@/stores/preferences'
import { useServerAddressesStore, addressIcon, addressColor } from '@/stores/serverAddresses'
import { useOnboardingStore } from '@/stores/onboarding'
import ServerAddressesDialog from '@/components/ServerAddressesDialog.vue'
import { useBackupStore, tableLabel } from '@/stores/backup'
import { useNotifications } from '@/composables/useNotifications'
import PageHeader from '@/components/PageHeader.vue'

const dashboardStore = useDashboardStore()
const addressesStore = useServerAddressesStore()
const onboardingStore = useOnboardingStore()
const addressesDialog = ref(false)

onMounted(async () => {
  void addressesStore.fetchAll()
  await prefsStore.fetchAll()
  adotarPrefs(prefsStore.preferences)
})
const backupStore = useBackupStore()
const {
  permissionState,
  notificationsEnabled,
  requestPermission,
  setNotificationsEnabled,
  sendNotification,
} = useNotifications()

// --- Preferências globais ---------------------------------------------------

const prefsStore = usePreferencesStore()
const form = reactive<Preferences>(defaultPreferences())

/** Há edição pendente? Alimenta o aviso, não o estado do botão. */
const prefsDirty = computed(
  () =>
    form.defaultPingIntervalSeconds !== prefsStore.preferences.defaultPingIntervalSeconds ||
    form.defaultSnmpCommunity !== prefsStore.preferences.defaultSnmpCommunity ||
    form.autoDiscoveryEnabled !== prefsStore.preferences.autoDiscoveryEnabled
)

function adotarPrefs(valores: Preferences): void {
  form.defaultPingIntervalSeconds = valores.defaultPingIntervalSeconds
  form.defaultSnmpCommunity = valores.defaultSnmpCommunity
  form.autoDiscoveryEnabled = valores.autoDiscoveryEnabled
}

async function salvarPrefs(): Promise<void> {
  const ok = await prefsStore.save({ ...form })
  if (!ok) {
    notify(prefsStore.error || 'Não foi possível salvar as preferências.', 'error')
    return
  }
  // O servidor devolve o documento já validado e aparado; adotá-lo evita a tela
  // mostrar uma coisa e o sistema usar outra.
  adotarPrefs(prefsStore.preferences)
  notify('Preferências salvas — já valem para os próximos monitores e dispositivos.')
}

function restaurarPrefs(): void {
  adotarPrefs(defaultPreferences())
}

const feedback = reactive({ visible: false, message: '', color: 'success' })

function notify(message: string, color = 'success'): void {
  feedback.message = message
  feedback.color = color
  feedback.visible = true
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
