import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { createMemoryHistory, createRouter } from 'vue-router'
import { routes } from '@/router'
import { areaOffRedirect } from '@/router/areaGuard'
import { coreSse } from '@/areas/core/sse'
import { apiService } from '@/services/apiService'
import { useAreasStore } from '@/stores/areas'
import { useAuthStore } from '@/stores/auth'
import { parseAreaRoles, roleInArea } from '@/utils/access'
import { areasSnapshot } from '../helpers/dashboardCatalog'

const stub = { default: { render: () => null } }
vi.mock('@/layouts/DefaultLayout.vue', () => stub)
vi.mock('@/pages/AreaOffPage.vue', () => stub)

function entrar(role: 'admin' | 'operator' | 'viewer', areaRoles = {}) {
  useAuthStore().user = { id: 7, email: 'noc@example.com', role, areaRoles }
}

describe('papel por área (ADR 019)', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    useAreasStore().apply(areasSnapshot({}))
  })

  it('a regra é a mesma do backend', () => {
    expect(roleInArea('admin', {}, 'vpn')).toBe('admin')
    expect(roleInArea('viewer', { servers: 'operator' }, 'core')).toBe('viewer')
    expect(roleInArea('viewer', { servers: 'operator' }, 'servers')).toBe('operator')
    expect(roleInArea('operator', {}, 'network')).toBeNull()
    expect(parseAreaRoles({ vpn: 'viewer', network: 'admin', lua: 'operator' })).toEqual({
      vpn: 'viewer',
    })
  })

  it('área sem acesso some, e o botão de mudar segue o papel da área aberta', () => {
    entrar('operator', { network: 'viewer' })
    const areas = useAreasStore()
    const auth = useAuthStore()
    expect(areas.isVisible('network')).toBe(true)
    expect(areas.isVisible('vpn')).toBe(false)
    expect(auth.canWrite).toBe(true)
    auth.currentArea = 'network'
    expect(auth.canWrite).toBe(false)
  })

  it('link para área sem acesso abre a página que explica, com o motivo', async () => {
    entrar('operator')
    const areas = useAreasStore()
    const auth = useAuthStore()
    const router = createRouter({ history: createMemoryHistory(), routes })
    router.beforeEach((to) => areaOffRedirect(to, areas.isEnabled, auth.hasAccess) ?? true)

    await router.push('/vpn')

    expect(router.currentRoute.value.name).toBe('area-off')
    expect(router.currentRoute.value.query.motivo).toBe('acesso')
  })

  it('o acesso liberado pelo SSE vale na hora, sem nenhuma chamada HTTP', () => {
    entrar('operator')
    const get = vi.spyOn(apiService, 'get')
    const areas = useAreasStore()
    expect(areas.isVisible('servers')).toBe(false)

    coreSse['user:access_updated']?.({
      userId: 7,
      role: 'operator',
      areaRoles: { servers: 'viewer' },
    })
    expect(areas.isVisible('servers')).toBe(true)

    coreSse['user:access_updated']?.({ userId: 8, role: 'admin', areaRoles: {} })
    expect(useAuthStore().user?.role).toBe('operator')
    expect(get).not.toHaveBeenCalled()
  })
})
