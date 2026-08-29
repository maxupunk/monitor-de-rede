<template>
  <v-app>
    <!-- Navigation Drawer Principal -->
    <v-navigation-drawer
      v-model="drawer"
      elevation="2"
      class="border-none"
      :temporary="$vuetify.display.mdAndDown"
      :permanent="$vuetify.display.lgAndUp"
    >
      <div class="pa-4 d-flex align-center">
        <v-avatar color="primary" size="42" class="mr-3 elevation-2">
          <v-icon color="white" size="24">mdi-shield-network</v-icon>
        </v-avatar>
        <div>
          <div class="text-h6 font-weight-bold lh-1">NetMonitor</div>
          <div class="text-caption text-grey">Monitor de Rede v1.0</div>
        </div>
      </div>

      <v-divider class="mb-2" />

      <v-list density="compact" nav class="px-2">
        <template v-for="item in navItems" :key="item.title">
          <v-list-item
            v-if="!item.children"
            :prepend-icon="item.icon"
            :title="item.title"
            :to="item.to"
            :exact="item.to === '/'"
            color="primary"
            rounded="lg"
            class="mb-1 font-weight-medium"
            @click="item.click ? item.click() : undefined"
          />

          <v-list-group v-else :value="item.title">
            <template #activator="{ props }">
              <v-list-item
                v-bind="props"
                :prepend-icon="item.icon"
                :title="item.title"
                rounded="lg"
                class="mb-1 font-weight-medium"
              />
            </template>

            <v-list-item
              v-for="sub in item.children"
              :key="sub.title"
              :prepend-icon="sub.icon"
              :title="sub.title"
              :to="sub.to"
              :exact="sub.to ? true : undefined"
              color="primary"
              rounded="lg"
              class="mb-1 font-weight-medium pl-6"
              @click="sub.click ? sub.click() : undefined"
            />
          </v-list-group>
        </template>
      </v-list>
      <template #append>
        <div class="pa-4 text-center text-caption text-grey border-t">
          &copy; 2026 Master Sistemas
        </div>
      </template>
    </v-navigation-drawer>

    <!-- App Bar Superior -->
    <v-app-bar flat border="b" density="comfortable" class="px-2">
      <v-app-bar-nav-icon @click="drawer = !drawer" />

      <v-toolbar-title class="text-subtitle-1 font-weight-bold text-grey-darken-3 text-truncate">
        <span class="hidden-sm-and-down">Plataforma de Monitoramento de Redes</span>
        <span class="hidden-md-and-up">NetMonitor</span>
      </v-toolbar-title>

      <v-spacer />

      <!-- Status SSE Tempo Real -->
      <v-chip
        :color="eventsStore.isConnected ? 'success' : 'grey-darken-1'"
        size="small"
        class="mr-2 font-weight-bold"
        variant="tonal"
      >
        <v-icon start size="12">
          {{ eventsStore.isConnected ? 'mdi-radiobox-marked' : 'mdi-radiobox-blank' }}
        </v-icon>
        {{ eventsStore.isConnected ? 'Tempo Real Ativo' : 'Conectando SSE...' }}
      </v-chip>

      <!-- Botão de Notificações PWA / Web Push -->
      <v-tooltip
        location="bottom"
        :text="
          isSubscribed
            ? 'Web Push Ativo (Alertas em segundo plano)'
            : permissionState === 'granted'
              ? 'Notificações Ativas (Clique para configurar)'
              : 'Ativar Notificações Web Push'
        "
      >
        <template #activator="{ props: tooltipProps }">
          <v-btn
            v-bind="tooltipProps"
            icon
            size="small"
            variant="text"
            class="mr-2"
            :color="isSubscribed ? 'primary' : permissionState === 'granted' ? 'info' : 'grey'"
            @click="handleNotificationClick"
          >
            <v-icon>
              {{
                isSubscribed
                  ? 'mdi-bell-ring'
                  : permissionState === 'granted'
                    ? 'mdi-bell-ring-outline'
                    : 'mdi-bell-off-outline'
              }}
            </v-icon>
          </v-btn>
        </template>
      </v-tooltip>

      <!-- Menu do Usuário -->
      <v-menu location="bottom end" transition="scale-transition">
        <template #activator="{ props }">
          <v-btn v-bind="props" variant="text" class="rounded-pill pl-2 pr-3">
            <v-avatar color="primary" size="32" class="mr-2">
              <v-icon size="18" color="white">mdi-account</v-icon>
            </v-avatar>
            <span class="text-caption font-weight-bold hidden-xs">{{
              authStore.currentRoleLabel
            }}</span>
            <v-icon end size="16">mdi-chevron-down</v-icon>
          </v-btn>
        </template>
        <v-list width="220" rounded="lg" elevation="4">
          <v-list-item
            prepend-icon="mdi-account-circle-outline"
            title="Meu Perfil"
            :subtitle="authStore.user?.email || 'Conta autenticada'"
          />
          <v-list-item
            v-if="authStore.canWrite"
            prepend-icon="mdi-rocket-launch-outline"
            title="Assistente Inicial"
            subtitle="Configurações básicas"
            @click="onboardingStore.openWizard()"
          />
          <v-divider class="my-1" />
          <v-list-item
            prepend-icon="mdi-logout"
            title="Sair da Conta"
            color="error"
            class="text-error font-weight-bold"
            @click="handleLogout"
          />
        </v-list>
      </v-menu>
    </v-app-bar>

    <!-- Conteúdo Principal da Página -->
    <v-main class="bg-grey-lighten-4" :class="{ 'layout-full-bleed': route.meta.fullBleed }">
      <v-container
        fluid
        :class="[
          route.meta.fullBleed
            ? 'pa-0 fill-height d-flex flex-column'
            : 'px-2 px-md-6 py-3 py-md-6 max-w-1600',
        ]"
      >
        <v-alert
          v-if="!authStore.canWrite && !route.meta.fullBleed"
          type="info"
          variant="tonal"
          density="compact"
          icon="mdi-eye-outline"
          class="mb-4"
        >
          Modo somente visualização: você pode consultar os dados, mas alterações estão desativadas.
        </v-alert>
        <router-view />
      </v-container>
    </v-main>

    <!-- Modal Gerenciamento de Servidores DNS e Assistente Inicial -->
    <DnsServersDialog v-model="dnsServersDialog" />
    <ServerAddressesDialog v-model="serverAddressesDialog" />
    <InitialSetupDialog v-model="onboardingStore.showWizard" />

    <!-- Diálogo de Instruções de Instalação no iOS -->
    <v-dialog v-model="showIosDialog" max-width="420">
      <v-card class="rounded-xl pa-4">
        <v-card-title class="d-flex align-center font-weight-bold">
          <v-icon color="primary" class="mr-2">mdi-apple</v-icon>
          Instalar no iOS (Safari)
        </v-card-title>
        <v-card-text class="text-body-2 pt-2">
          <p class="mb-3 text-grey-darken-1">
            Para instalar o <strong>NetMonitor</strong> no seu iPhone ou iPad:
          </p>
          <ol class="pl-4 d-flex flex-column ga-2 text-caption font-weight-medium">
            <li>
              Toque no botão de <strong>Compartilhar</strong>
              <v-icon size="small" color="primary">mdi-export-variant</v-icon> na barra inferior do
              Safari.
            </li>
            <li>
              Role a lista para baixo e selecione <strong>"Adicionar à Tela de Início"</strong>
              <v-icon size="small" color="primary">mdi-plus-box-outline</v-icon>.
            </li>
            <li>Toque em <strong>"Adicionar"</strong> no canto superior direito para confirmar.</li>
          </ol>
        </v-card-text>
        <v-card-actions class="justify-end">
          <v-btn color="primary" variant="flat" size="small" @click="showIosDialog = false">
            Entendi
          </v-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </v-app>
</template>

<script setup lang="ts">
import { computed, ref, onMounted, onUnmounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useDisplay } from 'vuetify'
import { useEventsStore } from '@/stores/events'
import { useAuthStore } from '@/stores/auth'
import { useOnboardingStore } from '@/stores/onboarding'
import { useNotifications } from '@/composables/useNotifications'
import { usePwaInstall } from '@/composables/usePwaInstall'
import DnsServersDialog from '@/components/DnsServersDialog.vue'
import ServerAddressesDialog from '@/components/ServerAddressesDialog.vue'
import InitialSetupDialog from '@/components/InitialSetupDialog.vue'

interface NavSubItem {
  title: string
  icon: string
  to?: string
  click?: () => void
}

interface NavItem {
  title: string
  icon: string
  to?: string
  click?: () => void
  children?: NavSubItem[]
}

const drawer = ref(!useDisplay().mdAndDown)
const dnsServersDialog = ref(false)
const serverAddressesDialog = ref(false)
const eventsStore = useEventsStore()
const authStore = useAuthStore()
const onboardingStore = useOnboardingStore()
const router = useRouter()
const route = useRoute()
const { permissionState, isSubscribed, requestPermission } = useNotifications()
const { canInstall, isInstalled, showIosDialog, promptInstall } = usePwaInstall()

async function handleNotificationClick() {
  if (permissionState.value === 'default') {
    await requestPermission()
  } else {
    await router.push('/settings')
  }
}

const navItems = computed<NavItem[]>(() => [
  { title: 'Dashboard', icon: 'mdi-view-dashboard', to: '/' },
  { title: 'Dispositivos', icon: 'mdi-devices', to: '/devices' },
  { title: 'Monitores', icon: 'mdi-heart-pulse', to: '/monitors' },
  { title: 'Alertas', icon: 'mdi-bell-outline', to: '/alerts' },
  { title: 'Manutenção', icon: 'mdi-wrench-clock', to: '/maintenance-windows' },
  { title: 'Descoberta', icon: 'mdi-radar', to: '/discovery' },
  { title: 'Eventos', icon: 'mdi-history', to: '/events' },
  { title: 'Logs', icon: 'mdi-text-box-search-outline', to: '/logs' },
  {
    title: 'Infraestrutura',
    icon: 'mdi-server-network',
    children: [
      { title: 'Sites', icon: 'mdi-domain', to: '/sites' },
      { title: 'Redes', icon: 'mdi-lan', to: '/networks' },
      { title: 'Topologia', icon: 'mdi-sitemap', to: '/topology' },
      { title: 'Probes', icon: 'mdi-router-wireless', to: '/probes' },
      {
        title: 'Servidores DNS',
        icon: 'mdi-dns-outline',
        click: () => {
          dnsServersDialog.value = true
        },
      },
      {
        title: 'Endereços do servidor',
        icon: 'mdi-server-network',
        click: () => {
          serverAddressesDialog.value = true
        },
      },
    ],
  },
  {
    title: 'VPN WireGuard',
    icon: 'mdi-shield-lock-outline',
    children: [
      { title: 'Servidor VPN', icon: 'mdi-server-security', to: '/vpn' },
      { title: 'Dispositivos VPN', icon: 'mdi-lan-connect', to: '/vpn/devices' },
    ],
  },
  ...(authStore.isAdmin
    ? [
        { title: 'Usuários e acessos', icon: 'mdi-account-multiple-outline', to: '/users' },
        { title: 'Trilha de auditoria', icon: 'mdi-shield-account-outline', to: '/audit' },
      ]
    : []),
  { title: 'Configurações', icon: 'mdi-cog', to: '/settings' },
  ...(canInstall.value && !isInstalled.value
    ? [
        {
          title: 'Instalar aplicativo',
          icon: 'mdi-cellphone-arrow-down',
          click: () => {
            void promptInstall()
          },
        },
      ]
    : []),
])

onMounted(() => {
  void authStore.fetchMe()
  eventsStore.connect()
  void onboardingStore.checkAndOpenIfNeeded()
})

onUnmounted(() => {
  eventsStore.disconnect()
})

async function handleLogout() {
  // O `await` não é cosmético: `logout()` só limpa o token depois da chamada à
  // API. Navegar antes disso deixa o guard vendo uma sessão ainda válida, e ele
  // devolve o operador ao dashboard — a tela de saída que nunca sai.
  await authStore.logout()
  await router.push('/login')
}
</script>

<style scoped>
.lh-1 {
  line-height: 1.2;
}
.max-w-1600 {
  max-width: 1600px;
  margin: 0 auto;
}
.layout-full-bleed {
  height: calc(100vh - 64px);
  overflow: hidden;
}
.layout-full-bleed > .v-container {
  max-width: 100% !important;
  height: 100% !important;
  padding: 0 !important;
}
</style>
