<template>
  <div>
    <PageHeader
      title="Configurações do Sistema"
      subtitle="Preferências globais, parâmetros de monitoramento e notificações"
    />

    <v-row dense>
      <v-col cols="12" md="6">
        <PreferencesCard
          ref="preferencesCard"
          @saved="
            notify('Preferências salvas — já valem para os próximos monitores e dispositivos.')
          "
        />
      </v-col>

      <v-col cols="12" md="6">
        <ServerAddressesCard @open-dialog="addressesDialog = true" />
      </v-col>

      <v-col cols="12" md="6">
        <DashboardSyncCard />
      </v-col>

      <v-col cols="12" md="6">
        <NotificationsCard @test-notification="testNotification" />
      </v-col>

      <v-col cols="12">
        <OnboardingCard />
      </v-col>

      <v-col cols="12">
        <BackupCard
          @export="onExport"
          @file-selected="onFileSelected"
          @confirm-restore="confirmDialog = true"
        />
      </v-col>

      <v-col cols="12">
        <DatabaseInfoCard />
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
import { onMounted, reactive, ref } from 'vue'
import { useServerAddressesStore } from '@/stores/serverAddresses'
import { usePreferencesStore } from '@/stores/preferences'
import { useBackupStore } from '@/stores/backup'
import { useNotifications } from '@/composables/useNotifications'
import ServerAddressesDialog from '@/components/ServerAddressesDialog.vue'
import PreferencesCard from '@/components/settings/PreferencesCard.vue'
import ServerAddressesCard from '@/components/settings/ServerAddressesCard.vue'
import DashboardSyncCard from '@/components/settings/DashboardSyncCard.vue'
import NotificationsCard from '@/components/settings/NotificationsCard.vue'
import OnboardingCard from '@/components/settings/OnboardingCard.vue'
import BackupCard from '@/components/settings/BackupCard.vue'
import DatabaseInfoCard from '@/components/settings/DatabaseInfoCard.vue'
import PageHeader from '@/components/PageHeader.vue'

const addressesStore = useServerAddressesStore()
const prefsStore = usePreferencesStore()
const backupStore = useBackupStore()
const addressesDialog = ref(false)
const confirmDialog = ref(false)
const preferencesCard = ref<InstanceType<typeof PreferencesCard> | null>(null)

const { sendLocalNotification } = useNotifications()

onMounted(async () => {
  void addressesStore.fetchAll()
  await prefsStore.fetchAll()
  preferencesCard.value?.adotarPrefs(prefsStore.preferences)
})

const feedback = reactive({ visible: false, message: '', color: 'success' })

function notify(message: string, color = 'success'): void {
  feedback.message = message
  feedback.color = color
  feedback.visible = true
}

async function onExport() {
  await backupStore.exportConfig()
}

async function onFileSelected(file: File | null) {
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
    // A restauração trocou sites, redes, dispositivos e monitores por baixo de
    // todas as stores já carregadas. Recarregar a aplicação é mais honesto do
    // que tentar invalidar uma por uma e deixar alguma tela com dado morto.
    window.location.reload()
  } else {
    notify(backupStore.error || 'Erro ao restaurar configurações.', 'error')
  }
}

function testNotification() {
  void sendLocalNotification('Notificação de Teste PWA', {
    body: 'Este é um teste de funcionamento das notificações em tempo real do NetMonitor.',
  })
}
</script>
