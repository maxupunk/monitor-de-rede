<template>
  <div>
    <PageHeader
      title="Configurações do Sistema"
      subtitle="Preferências, rede deste servidor, notificações, inteligência artificial e sistema"
    />

    <v-card elevation="2" class="rounded-lg mb-4">
      <v-tabs v-model="tab" color="primary" show-arrows>
        <v-tab v-for="item in TABS" :key="item.value" :value="item.value" :prepend-icon="item.icon">
          {{ item.title }}
        </v-tab>
      </v-tabs>
    </v-card>

    <v-window v-model="tab" :touch="false">
      <!-- `eager`: o cartão de preferências recebe os valores no carregamento da página. -->
      <v-window-item value="geral" eager>
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
            <DashboardSyncCard />
          </v-col>
          <v-col cols="12">
            <OnboardingCard />
          </v-col>
        </v-row>
      </v-window-item>

      <v-window-item value="rede">
        <v-row dense>
          <v-col cols="12" md="6">
            <ServerAddressesCard @open-dialog="addressesDialog = true" />
          </v-col>
          <v-col cols="12" md="6">
            <DnsServersCard />
          </v-col>
          <v-col cols="12">
            <VendorRegistryCard @saved="notify" />
          </v-col>
        </v-row>
      </v-window-item>

      <v-window-item value="notificacoes">
        <v-row dense>
          <v-col cols="12" md="8" lg="6">
            <NotificationsCard @test-notification="testNotification" />
          </v-col>
        </v-row>
      </v-window-item>

      <v-window-item value="ia">
        <v-row dense>
          <v-col cols="12">
            <AiSettingsCard @saved="notify" />
          </v-col>
          <v-col cols="12">
            <LayaSettingsCard @saved="notify" />
          </v-col>
        </v-row>
      </v-window-item>

      <v-window-item value="sistema">
        <v-row dense>
          <v-col cols="12">
            <v-card elevation="2" class="rounded-lg pa-3 pa-sm-4">
              <div class="d-flex flex-wrap align-center ga-3">
                <v-avatar color="success" variant="tonal" rounded="lg" size="44">
                  <v-icon size="26">mdi-backup-restore</v-icon>
                </v-avatar>
                <div class="flex-grow-1">
                  <div class="font-weight-bold">Backup e restauração</div>
                  <div class="text-body-2 text-high-emphasis">
                    Cópias automáticas do NetMonitor e dos seus bancos de dados, e a restauração a
                    partir delas ou de um arquivo.
                  </div>
                </div>
                <v-btn color="success" variant="flat" :to="{ name: 'backup' }">Abrir Backup</v-btn>
              </div>
            </v-card>
          </v-col>
          <v-col cols="12">
            <DatabaseInfoCard />
          </v-col>
        </v-row>
      </v-window-item>
    </v-window>

    <ServerAddressesDialog v-model="addressesDialog" />

    <v-snackbar v-model="feedback.visible" :color="feedback.color" timeout="4000">
      {{ feedback.message }}
    </v-snackbar>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, reactive, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useServerAddressesStore } from '@/stores/serverAddresses'
import { usePreferencesStore } from '@/stores/preferences'
import { useNotifications } from '@/composables/useNotifications'
import ServerAddressesDialog from '@/components/ServerAddressesDialog.vue'
import PreferencesCard from '@/components/settings/PreferencesCard.vue'
import ServerAddressesCard from '@/components/settings/ServerAddressesCard.vue'
import DnsServersCard from '@/components/settings/DnsServersCard.vue'
import VendorRegistryCard from '@/components/settings/VendorRegistryCard.vue'
import DashboardSyncCard from '@/components/settings/DashboardSyncCard.vue'
import NotificationsCard from '@/components/settings/NotificationsCard.vue'
import OnboardingCard from '@/components/settings/OnboardingCard.vue'
import DatabaseInfoCard from '@/components/settings/DatabaseInfoCard.vue'
import AiSettingsCard from '@/components/settings/AiSettingsCard.vue'
import LayaSettingsCard from '@/components/settings/LayaSettingsCard.vue'
import PageHeader from '@/components/PageHeader.vue'

/** Abas da página; a escolhida fica na URL (`?tab=rede`) para links diretos. */
const TABS = [
  { value: 'geral', title: 'Geral', icon: 'mdi-tune-variant' },
  { value: 'rede', title: 'Rede', icon: 'mdi-lan' },
  { value: 'notificacoes', title: 'Notificações', icon: 'mdi-bell-outline' },
  { value: 'ia', title: 'Inteligência artificial', icon: 'mdi-robot-outline' },
  { value: 'sistema', title: 'Sistema', icon: 'mdi-database-cog-outline' },
] as const

type SettingsTab = (typeof TABS)[number]['value']

const route = useRoute()
const router = useRouter()
const tab = computed<SettingsTab>({
  get: () => TABS.find((item) => item.value === route.query.tab)?.value ?? 'geral',
  set: (value) => void router.replace({ query: { ...route.query, tab: value } }),
})

const addressesStore = useServerAddressesStore()
const prefsStore = usePreferencesStore()
const addressesDialog = ref(false)
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

function testNotification() {
  void sendLocalNotification('Notificação de Teste PWA', {
    body: 'Este é um teste de funcionamento das notificações em tempo real do NetMonitor.',
  })
}
</script>
