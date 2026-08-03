<template>
  <v-app>
    <!-- Navigation Drawer Principal -->
    <v-navigation-drawer v-model="drawer" elevation="2" class="border-none">
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
            color="primary"
            rounded="lg"
            class="mb-1 font-weight-medium"
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
              :key="sub.to"
              :prepend-icon="sub.icon"
              :title="sub.title"
              :to="sub.to"
              color="primary"
              rounded="lg"
              class="mb-1 font-weight-medium pl-6"
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

      <v-toolbar-title class="text-subtitle-1 font-weight-bold text-grey-darken-3">
        Plataforma de Monitoramento de Redes
      </v-toolbar-title>

      <v-spacer />

      <!-- Status SSE Tempo Real -->
      <v-chip
        :color="eventsStore.isConnected ? 'success' : 'grey-darken-1'"
        size="small"
        class="mr-3 font-weight-bold"
        variant="tonal"
      >
        <v-icon start size="12">
          {{ eventsStore.isConnected ? 'mdi-radiobox-marked' : 'mdi-radiobox-blank' }}
        </v-icon>
        {{ eventsStore.isConnected ? 'Tempo Real Ativo' : 'Conectando SSE...' }}
      </v-chip>

      <!-- Menu do Usuário -->
      <v-menu location="bottom end" transition="scale-transition">
        <template #activator="{ props }">
          <v-btn v-bind="props" variant="text" class="rounded-pill pl-2 pr-3">
            <v-avatar color="primary" size="32" class="mr-2">
              <v-icon size="18" color="white">mdi-account</v-icon>
            </v-avatar>
            <span class="text-caption font-weight-bold hidden-xs">Administrador</span>
            <v-icon end size="16">mdi-chevron-down</v-icon>
          </v-btn>
        </template>
        <v-list width="200" rounded="lg" elevation="4">
          <v-list-item
            prepend-icon="mdi-account-circle-outline"
            title="Meu Perfil"
            subtitle="admin@monitor.local"
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
    <v-main class="bg-grey-lighten-4">
      <v-container fluid class="pa-6 max-w-1600">
        <router-view />
      </v-container>
    </v-main>
  </v-app>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useEventsStore } from '@/stores/events'
import { useAuthStore } from '@/stores/auth'

interface NavSubItem {
  title: string
  icon: string
  to: string
}

interface NavItem {
  title: string
  icon: string
  to?: string
  children?: NavSubItem[]
}

const drawer = ref(true)
const eventsStore = useEventsStore()
const authStore = useAuthStore()
const router = useRouter()

const navItems: NavItem[] = [
  { title: 'Dashboard', icon: 'mdi-view-dashboard', to: '/' },
  { title: 'Dispositivos', icon: 'mdi-devices', to: '/devices' },
  { title: 'Monitores', icon: 'mdi-heart-pulse', to: '/monitors' },
  { title: 'Alertas', icon: 'mdi-bell-outline', to: '/alerts' },
  { title: 'Descoberta', icon: 'mdi-radar', to: '/discovery' },
  { title: 'Eventos', icon: 'mdi-history', to: '/events' },
  {
    title: 'Infraestrutura',
    icon: 'mdi-server-network',
    children: [
      { title: 'Sites', icon: 'mdi-domain', to: '/sites' },
      { title: 'Redes', icon: 'mdi-lan', to: '/networks' },
      { title: 'Topologia', icon: 'mdi-sitemap', to: '/topology' },
      { title: 'Probes', icon: 'mdi-router-wireless', to: '/probes' },
    ],
  },
  { title: 'Configurações', icon: 'mdi-cog', to: '/settings' },
]

onMounted(() => {
  eventsStore.connect()
})

onUnmounted(() => {
  eventsStore.disconnect()
})

function handleLogout() {
  authStore.logout()
  router.push('/login')
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
</style>
