import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { NAV_ENTRIES } from '@/areas'
import { coreSse } from '@/areas/core/sse'
import { buildNavSections, navLeaves, PINNED_SECTION, type NavContext } from '@/navigation'
import { apiService } from '@/services/apiService'
import { useAuthStore } from '@/stores/auth'
import { useMenuPrefsStore } from '@/stores/menuPrefs'

const ctx: NavContext = { isAdmin: true, apps: [], canInstallPwa: false }
const paths = (sections: ReturnType<typeof buildNavSections>) =>
  navLeaves(sections).map((leaf) => leaf.to)

describe('menu por usuário (Fase 16)', () => {
  beforeEach(() => setActivePinia(createPinia()))

  it('oculto sai do menu e fixado aparece no topo, na ordem fixada', () => {
    const sections = buildNavSections(
      { ...ctx, menu: { pinned: ['/logs', '/topology'], hidden: ['/events'] } },
      NAV_ENTRIES
    )
    expect(sections[0]?.title).toBe(PINNED_SECTION)
    expect(sections[0]?.items.map((item) => item.to)).toEqual(['/logs', '/topology'])
    expect(paths(sections)).not.toContain('/events')
    expect(paths(sections).filter((path) => path === '/logs')).toHaveLength(2)
  })

  it('fixar não traz de volta o que a área ou o papel escondem', () => {
    const sections = buildNavSections(
      { ...ctx, enabledAreas: new Set(['core']), menu: { pinned: ['/topology'], hidden: [] } },
      NAV_ENTRIES
    )
    expect(sections[0]?.title).not.toBe(PINNED_SECTION)
    expect(paths(sections)).not.toContain('/topology')
  })

  it('o menu salvo em outro aparelho chega pelo SSE, sem chamada HTTP', () => {
    useAuthStore().user = { id: 7, email: 'noc@example.com', role: 'viewer' }
    const get = vi.spyOn(apiService, 'get')
    coreSse['user:menu_updated']?.({ userId: 7, pinned: ['/logs'], hidden: [] })
    expect(useMenuPrefsStore().prefs.pinned).toEqual(['/logs'])
    coreSse['user:menu_updated']?.({ userId: 8, pinned: [], hidden: ['/logs'] })
    expect(useMenuPrefsStore().prefs.hidden).toEqual([])
    expect(get).not.toHaveBeenCalled()
  })
})
