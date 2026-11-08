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
          <div class="text-caption text-high-emphasis">Monitor de Rede v1.0</div>
        </div>
      </div>

      <v-divider class="mb-2" />

      <v-list v-model:opened="openedGroups" density="compact" nav class="nav-list px-2">
        <template v-for="(section, sectionIndex) in navSections" :key="section.title">
          <v-list-subheader
            class="nav-section-title font-weight-bold text-uppercase"
            :class="{ 'mt-2': sectionIndex > 0 }"
          >
            {{ section.title }}
          </v-list-subheader>
          <template v-for="item in section.items" :key="item.title">
            <v-list-item
              v-if="!item.children"
              :prepend-icon="item.icon"
              :title="item.title"
              :to="item.to"
              :exact="item.to === '/'"
              color="primary"
              rounded="lg"
              class="mb-1 font-weight-medium"
              @click="item.action ? runAction(item.action) : undefined"
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
                @click="sub.action ? runAction(sub.action) : undefined"
              />
            </v-list-group>
          </template>
        </template>
      </v-list>
      <template #append>
        <div class="px-3 pb-2">
          <v-btn
            block
            color="primary"
            variant="tonal"
            prepend-icon="mdi-menu-open"
            @click="menuDialog = true"
          >
            Personalizar menu
          </v-btn>
        </div>
        <div class="app-footer-bar justify-center px-4 text-caption text-high-emphasis border-t">
          &copy; 2026 Master Sistemas
        </div>
      </template>
    </v-navigation-drawer>
    <MenuCustomizeDialog v-model="menuDialog" :base-sections="baseNavSections" />

    <!-- App Bar Superior -->
    <v-app-bar flat border="b" density="comfortable" class="px-2 pwa-app-bar">
      <v-app-bar-nav-icon @click="drawer = !drawer" />

      <v-spacer />

      <!-- Status SSE Tempo Real -->
      <v-chip
        :color="eventsStore.isConnected ? 'success' : 'grey'"
        size="small"
        class="mr-2 font-weight-bold"
        variant="tonal"
      >
        <v-icon start size="12">
          {{ eventsStore.isConnected ? 'mdi-radiobox-marked' : 'mdi-radiobox-blank' }}
        </v-icon>
        {{ eventsStore.isConnected ? 'Tempo Real Ativo' : 'Conectando SSE...' }}
      </v-chip>

      <!-- Botão Assistente IA -->
      <!-- Sem dica no toque: ela abria no tap e ficava presa sobre o chat. -->
      <v-tooltip
        location="bottom"
        text="Assistente IA (Diagnóstico & Suporte)"
        :disabled="display.mdAndDown.value"
      >
        <template #activator="{ props: tooltipProps }">
          <v-btn
            v-bind="tooltipProps"
            icon
            size="small"
            variant="text"
            class="mr-2"
            color="primary"
            @click="aiStore.toggleDrawer()"
          >
            <v-icon>mdi-robot-outline</v-icon>
          </v-btn>
        </template>
      </v-tooltip>

      <!-- Botão de Notificações PWA / Web Push -->
      <v-tooltip
        location="bottom"
        :disabled="display.mdAndDown.value"
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
    <v-main :class="{ 'layout-full-bleed': route.meta.fullBleed }">
      <v-container
        fluid
        :class="[
          route.meta.fullBleed
            ? 'pa-0 fill-height d-flex flex-column'
            : 'px-1 px-sm-3 px-md-6 py-2 py-md-4 max-w-1600',
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
        <AreaSuggestions class="mb-4" />
        <router-view />
      </v-container>
    </v-main>

    <!-- Modal Gerenciamento de Servidores DNS e Assistente Inicial -->
    <InitialSetupDialog v-model="onboardingStore.showWizard" />

    <!-- Modais de Diagnóstico de Rede -->
    <TracerouteDialog v-model="tracerouteDialog" />
    <SpeedTestDialog v-model="speedTestDialog" />
    <DiagnosticPlaybookDialog v-model="playbookDialog" />
    <AiChatDrawer />
    <PluginApprovalDialog />
    <AiApprovalNotice />

    <!-- Diálogo de Instruções de Instalação no iOS -->
    <v-dialog v-model="showIosDialog" :fullscreen="$vuetify.display.xs" max-width="420">
      <v-card class="rounded-lg pa-4">
        <v-card-title class="d-flex align-center font-weight-bold">
          <v-icon color="primary" class="mr-2">mdi-apple</v-icon>
          Instalar no iOS (Safari)
        </v-card-title>
        <v-card-text class="text-body-2 pt-2">
          <p class="mb-3 text-high-emphasis">
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
import { computed, ref, onMounted, onUnmounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useDisplay } from 'vuetify'
import { useEventsStore } from '@/stores/events'
import { useAuthStore } from '@/stores/auth'
import { useOnboardingStore } from '@/stores/onboarding'
import { useNotifications } from '@/composables/useNotifications'
import { usePwaInstall } from '@/composables/usePwaInstall'
import InitialSetupDialog from '@/components/InitialSetupDialog.vue'
import TracerouteDialog from '@/components/TracerouteDialog.vue'
import SpeedTestDialog from '@/components/SpeedTestDialog.vue'
import DiagnosticPlaybookDialog from '@/components/DiagnosticPlaybookDialog.vue'
import AiChatDrawer from '@/components/ai/AiChatDrawer.vue'
import PluginApprovalDialog from '@/components/plugins/PluginApprovalDialog.vue'
import AiApprovalNotice from '@/components/ai/AiApprovalNotice.vue'
import AreaSuggestions from '@/components/areas/AreaSuggestions.vue'
import { useAiStore } from '@/stores/ai'
import { usePluginAppsStore } from '@/stores/pluginApps'
import { buildBaseSections, buildNavSections, groupOfPath, type NavAction } from '@/navigation'
import MenuCustomizeDialog from '@/components/menu/MenuCustomizeDialog.vue'
import { useMenuPrefsStore } from '@/stores/menuPrefs'
import { NAV_ENTRIES } from '@/areas'
import { installAreaRealtime } from '@/areas/realtime'
import { useAreasStore } from '@/stores/areas'
import { areaOffRedirect } from '@/router/areaGuard'

const display = useDisplay()
const drawer = ref(!display.mdAndDown.value)
const tracerouteDialog = ref(false)
const speedTestDialog = ref(false)
const playbookDialog = ref(false)
const aiStore = useAiStore()
const pluginAppsStore = usePluginAppsStore()
const eventsStore = useEventsStore()
const authStore = useAuthStore()
const areasStore = useAreasStore()
const menuPrefsStore = useMenuPrefsStore()
const onboardingStore = useOnboardingStore()
const router = useRouter()
const route = useRoute()
const { permissionState, isSubscribed, requestPermission } = useNotifications()
const { canInstall, isInstalled, showIosDialog, promptInstall } = usePwaInstall()

/*
 * No celular e no tablet o menu e o Assistente IA são sobreposições da tela
 * inteira: abrir um fecha o outro, e trocar de página fecha os dois. Sem isso
 * o chat ficava por cima do menu e da página nova, sem saída aparente.
 */
watch(drawer, (open) => {
  if (open && display.mdAndDown.value) aiStore.isDrawerOpen = false
})
watch(
  () => aiStore.isDrawerOpen,
  (open) => {
    if (open && display.mdAndDown.value) drawer.value = false
  }
)
watch(
  () => route.fullPath,
  () => {
    if (display.mdAndDown.value) aiStore.isDrawerOpen = false
  }
)

async function handleNotificationClick() {
  if (permissionState.value === 'default') {
    await requestPermission()
  } else {
    await router.push('/settings?tab=notificacoes')
  }
}

/** O menu vem das áreas (`src/areas`); o layout só executa as ações. */
const navContext = computed(() => ({
  isAdmin: authStore.isAdmin,
  apps: pluginAppsStore.apps,
  canInstallPwa: canInstall.value && !isInstalled.value,
  enabledAreas: areasStore.visibleAreas,
}))
/** O menu de áreas e papéis — o que o usuário pode personalizar. */
const baseNavSections = computed(() => buildBaseSections(navContext.value, NAV_ENTRIES))
/** O menu desenhado: o de áreas e papéis com o menu pessoal aplicado (Fase 16). */
const navSections = computed(() =>
  buildNavSections({ ...navContext.value, menu: menuPrefsStore.prefs }, NAV_ENTRIES)
)
const menuDialog = ref(false)

/*
 * A área da página aberta desligou (aqui ou em outra aba, pelo SSE): a página
 * dá lugar à que explica e oferece ligar — o mesmo destino do guard.
 */
watch(
  () => areasStore.visibleAreas,
  () => {
    const target = areaOffRedirect(route, areasStore.isEnabled, authStore.hasAccess)
    if (target) void router.replace(target)
  }
)

const ACTIONS: Record<NavAction, () => void> = {
  playbook: () => {
    playbookDialog.value = true
  },
  traceroute: () => {
    tracerouteDialog.value = true
  },
  speedTest: () => {
    speedTestDialog.value = true
  },
  installPwa: () => {
    void promptInstall()
  },
}

function runAction(action: NavAction) {
  ACTIONS[action]()
}

/** Grupos abertos no menu; o da página atual abre sozinho ao navegar. */
const openedGroups = ref<string[]>([])

watch(
  () => route.path,
  (path) => {
    const group = groupOfPath(navSections.value, path)
    if (group && !openedGroups.value.includes(group)) {
      openedGroups.value = [...openedGroups.value, group]
    }
  },
  { immediate: true }
)

// As rotas SSE de cada área ligada; antes do `connect`, para o snapshot inicial.
let uninstallAreaRealtime: (() => void) | null = null

onMounted(() => {
  void authStore.fetchMe()
  uninstallAreaRealtime = installAreaRealtime()
  eventsStore.connect()
  void onboardingStore.checkAndOpenIfNeeded()
  void pluginAppsStore.fetchApps()
})

onUnmounted(() => {
  eventsStore.disconnect()
  uninstallAreaRealtime?.()
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
/* Item de submenu recuado não cabe numa linha ("Velocidade do navegador"): quebra em vez de "…". */
.nav-list :deep(.v-list-item-title) {
  white-space: normal;
  line-height: 1.25;
}
.lh-1 {
  line-height: 1.2;
}
.nav-section-title {
  font-size: 0.68rem;
  letter-spacing: 0.08em;
  min-height: 28px;
  color: rgb(var(--v-theme-primary));
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
