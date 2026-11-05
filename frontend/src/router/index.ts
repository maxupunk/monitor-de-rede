import { createRouter, createWebHistory, type RouteRecordRaw } from 'vue-router'
import { useAuthStore } from '@/stores/auth'
import { useAreasStore } from '@/stores/areas'
import { AREA_ROUTES } from '@/areas'
import { areaOffRedirect } from './areaGuard'

export const routes: RouteRecordRaw[] = [
  {
    path: '/login',
    name: 'login',
    component: () => import('../pages/LoginPage.vue'),
    meta: { public: true },
  },
  {
    path: '/setup',
    name: 'setup',
    component: () => import('../pages/SetupPage.vue'),
    meta: { public: true },
  },
  {
    path: '/',
    component: () => import('@/layouts/DefaultLayout.vue'),
    // As rotas vêm das áreas (`src/areas/*`), cada uma com `meta.area`.
    children: AREA_ROUTES,
  },
]

export const router = createRouter({
  history: createWebHistory(),
  routes,
})

/**
 * Três destinos possíveis para quem chega, nesta ordem:
 *
 * 1. **Autenticado** — segue para onde pediu; login e cadastro inicial viram
 *    dashboard, porque não há o que fazer neles com sessão aberta. Rota de
 *    área desligada vira a `AreaOffPage`.
 * 2. **Instalação pendente** (banco sem nenhum usuário) — vai para `/setup`.
 *    Mandar para o login seria pedir uma credencial que ainda não existe, que
 *    é exatamente o beco sem saída de uma instalação nova.
 * 3. **Resto** — login, guardando o destino original em `?redirect` para
 *    devolver o operador ao lugar certo depois de entrar.
 */
router.beforeEach(async (to) => {
  const auth = useAuthStore()

  if (auth.isAuthenticated) {
    if (to.name === 'login' || to.name === 'setup') return { name: 'dashboard' }
    if (to.meta.requiresAdmin && !auth.isAdmin) {
      await auth.fetchMe()
      if (!auth.isAdmin) return { name: 'dashboard' }
    }
    // Área desligada: a página que explica e oferece ligar, nunca um 404.
    const areas = useAreasStore()
    return areaOffRedirect(to, areas.isEnabled) ?? true
  }

  if (await auth.ensureSetupStatus()) {
    return to.name === 'setup' ? true : { name: 'setup' }
  }

  // Instalação concluída: `/setup` não tem mais função.
  if (to.name === 'setup') return { name: 'login' }
  if (!to.meta.public) return { name: 'login', query: { redirect: to.fullPath } }
  return true
})
