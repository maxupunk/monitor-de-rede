import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { createMemoryHistory, createRouter } from 'vue-router'
import { routes } from '@/router'
import { areaOffRedirect } from '@/router/areaGuard'
import { useAreasStore } from '@/stores/areas'
import { areasSnapshot } from '../helpers/dashboardCatalog'

// Só os componentes que a navegação carrega: o teste é do roteamento.
const stub = { default: { render: () => null } }
vi.mock('@/layouts/DefaultLayout.vue', () => stub)
vi.mock('@/pages/AreaOffPage.vue', () => stub)

function testRouter() {
  const areas = useAreasStore()
  const router = createRouter({ history: createMemoryHistory(), routes })
  router.beforeEach((to) => areaOffRedirect(to, areas.isEnabled) ?? true)
  return router
}

describe('guard de área', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('link profundo para /docker com Servidores desligado abre a AreaOffPage', async () => {
    useAreasStore().apply(areasSnapshot({ servers: false }))
    const router = testRouter()

    await router.push('/docker/containers?host=agent-2')

    const current = router.currentRoute.value
    expect(current.name).toBe('area-off')
    expect(current.params.area).toBe('servers')
    expect(current.query.from).toBe('/docker/containers?host=agent-2')
  })

  it('toda rota do layout declara a área; as do núcleo nunca desviam', () => {
    useAreasStore().apply(areasSnapshot({ network: false, servers: false, vpn: false }))
    const router = testRouter()

    for (const path of ['/', '/devices/3', '/monitors', '/settings', '/remote-servers']) {
      const resolved = router.resolve(path)
      expect(resolved.meta.area).toBe('core')
      expect(areaOffRedirect(resolved, useAreasStore().isEnabled)).toBeNull()
    }
    expect(router.resolve('/topology').meta.area).toBe('network')
    expect(router.resolve('/vpn/devices').meta.area).toBe('vpn')
    expect(areaOffRedirect(router.resolve('/vpn/devices'), useAreasStore().isEnabled)).toEqual({
      name: 'area-off',
      params: { area: 'vpn' },
      query: { from: '/vpn/devices' },
    })
  })

  it('com a área ligada a rota segue normalmente', () => {
    useAreasStore().apply(areasSnapshot({}))
    const router = testRouter()
    expect(areaOffRedirect(router.resolve('/docker'), useAreasStore().isEnabled)).toBeNull()
  })
})
