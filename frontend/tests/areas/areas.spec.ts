import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { apiService } from '@/services/apiService'
import { installAreaRealtime } from '@/areas/realtime'
import { AREAS, NAV_ENTRIES } from '@/areas'
import { buildNavSections, type NavSection } from '@/navigation'
import { useAreasStore } from '@/stores/areas'
import { useEventsStore } from '@/stores/events'
import { useDockerStore } from '@/stores/docker'
import { areasSnapshot } from '../helpers/dashboardCatalog'

class FakeEventSource {
  static instances: FakeEventSource[] = []
  onopen: (() => void) | null = null
  onmessage: ((event: MessageEvent<string>) => void) | null = null
  onerror: (() => void) | null = null
  closed = false

  constructor(readonly url: string) {
    FakeEventSource.instances.push(this)
  }

  static get latest(): FakeEventSource | undefined {
    return FakeEventSource.instances.at(-1)
  }

  close() {
    this.closed = true
  }
}

function send(type: string, data: unknown) {
  FakeEventSource.latest?.onmessage?.({
    data: JSON.stringify({ type, timestamp: '2026-11-05T12:00:00Z', data }),
  } as MessageEvent<string>)
}

function leaves(sections: NavSection[]): string[] {
  return sections.flatMap((section) =>
    section.items.flatMap((item) => (item.children ? item.children : [item]).map((l) => l.title))
  )
}

describe('áreas na SPA', () => {
  let uninstall: () => void

  beforeEach(() => {
    setActivePinia(createPinia())
    FakeEventSource.instances = []
    vi.stubGlobal('EventSource', FakeEventSource)
    vi.stubGlobal('localStorage', {
      getItem: vi.fn(() => null),
      setItem: vi.fn(),
      removeItem: vi.fn(),
      clear: vi.fn(),
    })
    uninstall = installAreaRealtime()
  })

  afterEach(() => {
    uninstall()
    useEventsStore().disconnect()
    vi.useRealTimers()
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('areas:updated muda as áreas e o menu sem nenhuma chamada HTTP', () => {
    const get = vi.spyOn(apiService, 'get')
    const put = vi.spyOn(apiService, 'put')
    const post = vi.spyOn(apiService, 'post')
    const areas = useAreasStore()
    const menu = () =>
      leaves(
        buildNavSections(
          { isAdmin: true, apps: [], canInstallPwa: false, enabledAreas: areas.enabledAreas },
          NAV_ENTRIES
        )
      )
    useEventsStore().connect()

    // Antes do snapshot, tudo ligado: nada some por engano.
    expect(menu()).toContain('Topologia')
    expect(menu()).toContain('Docker')

    send('areas:updated', areasSnapshot({ network: false, vpn: false }))

    expect([...areas.enabledAreas].sort()).toEqual(['core', 'servers'])
    expect(menu()).not.toContain('Topologia')
    expect(menu()).not.toContain('VPN WireGuard')
    expect(menu()).not.toContain('Plugins de dispositivo')
    expect(menu()).toContain('Docker')
    expect(menu()).toContain('Monitores')
    expect(get).not.toHaveBeenCalled()
    expect(put).not.toHaveBeenCalled()
    expect(post).not.toHaveBeenCalled()
  })

  it('área desligada não registra handlers SSE; religada, volta a registrar', () => {
    const events = useEventsStore()
    events.connect()
    expect(events.hasRoute('docker:snapshot')).toBe(true)
    expect(events.hasRoute('vpn:peers_updated')).toBe(true)

    send('areas:updated', areasSnapshot({ servers: false, vpn: false }))

    expect(events.hasRoute('docker:snapshot')).toBe(false)
    expect(events.hasRoute('docker:operation')).toBe(false)
    expect(events.hasRoute('vpn:peers_updated')).toBe(false)
    // O núcleo fica: é ele que recebe a próxima mudança de área.
    expect(events.hasRoute('areas:updated')).toBe(true)
    expect(events.hasRoute('alert:triggered')).toBe(true)

    const docker = useDockerStore()
    const apply = vi.spyOn(docker, 'applyLiveSnapshot')
    send('docker:snapshot', { hostKey: 'agent-1', containers: [] })
    expect(apply).not.toHaveBeenCalled()

    send('areas:updated', areasSnapshot({}))
    expect(events.hasRoute('docker:snapshot')).toBe(true)
    expect(events.hasRoute('vpn:peers_updated')).toBe(true)
  })

  it('o núcleo não tem rota de outra área e toda área declara as suas', () => {
    const core = AREAS.find((area) => area.id === 'core')!
    expect(Object.keys(core.sse).some((key) => key.startsWith('docker:'))).toBe(false)
    expect(Object.keys(core.sse).some((key) => key.startsWith('vpn:'))).toBe(false)
    for (const area of AREAS) {
      for (const route of area.routes) expect(route.meta?.area).toBe(area.id)
      for (const entry of area.nav) expect(entry.area).toBe(area.id)
    }
  })

  it('mudar o conjunto de interesses reabre o stream uma vez, com a URL certa', () => {
    vi.useFakeTimers()
    const events = useEventsStore()
    events.connect()
    expect(FakeEventSource.instances).toHaveLength(1)
    expect(FakeEventSource.latest?.url).toBe('/api/events/stream')

    // Duas telas pedem o mesmo dado em rajada: uma reconexão só.
    const releaseLayout = events.acquireInterest('docker')
    const releaseCard = events.acquireInterest('docker')
    vi.advanceTimersByTime(299)
    expect(FakeEventSource.instances).toHaveLength(1)
    vi.advanceTimersByTime(1)
    expect(FakeEventSource.instances).toHaveLength(2)
    expect(FakeEventSource.instances[0].closed).toBe(true)
    expect(FakeEventSource.latest?.url).toBe('/api/events/stream?interest=docker')

    // Ainda há quem use: o conjunto não muda, nada reconecta.
    releaseCard()
    vi.advanceTimersByTime(1000)
    expect(FakeEventSource.instances).toHaveLength(2)

    // O último sai: volta ao stream sem interesse, de novo uma vez só.
    releaseLayout()
    releaseLayout()
    vi.advanceTimersByTime(1000)
    expect(FakeEventSource.instances).toHaveLength(3)
    expect(FakeEventSource.latest?.url).toBe('/api/events/stream')
  })

  it('interesse declarado antes de conectar já vai na primeira URL', () => {
    vi.useFakeTimers()
    const events = useEventsStore()
    events.acquireInterest('docker')
    events.connect()
    vi.advanceTimersByTime(1000)
    expect(FakeEventSource.instances).toHaveLength(1)
    expect(FakeEventSource.latest?.url).toBe('/api/events/stream?interest=docker')
  })
})
