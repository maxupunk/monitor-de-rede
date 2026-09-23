import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { apiService } from '@/services/apiService'
import { useAgentsStore } from '@/stores/agents'
import type { AgentView } from '@/bindings/AgentView'

function agent(overrides: Partial<AgentView> = {}): AgentView {
  return {
    id: 7,
    name: 'srv-remoto',
    hostKey: 'agent-7',
    siteId: null,
    deviceId: 3,
    deviceName: 'srv-remoto',
    deviceIp: '10.8.0.5',
    status: 'offline',
    connected: false,
    version: '0.1.0',
    lastSeenAt: null,
    registeredAt: null,
    enforceTunnelIp: true,
    host: {
      hostname: 'srv-remoto',
      os: 'Debian',
      arch: 'x86_64',
      inContainer: false,
      policy: ['read', 'lifecycle'],
      docker: { available: true, version: '27.1.0', reason: null },
      compose: { available: false, version: null, reason: 'plugin ausente' },
    },
    createdAt: '2026-09-22T10:00:00Z',
    ...overrides,
  }
}

describe('agents store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('aplica probe:status do SSE sem consultar a API', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue([agent()])
    const store = useAgentsStore()
    await store.fetchAgents()

    store.applyRealtimeStatus({ id: 7, status: 'online', lastSeenAt: '2026-09-22T10:05:00Z' })

    expect(get).toHaveBeenCalledOnce()
    expect(store.agents[0].connected).toBe(true)
    expect(store.agents[0].status).toBe('online')
    expect(store.agents[0].lastSeenAt).toBe('2026-09-22T10:05:00Z')
  })

  it('cadastro devolve as instruções e entra na lista em ordem', async () => {
    vi.spyOn(apiService, 'get').mockResolvedValue([agent({ id: 9, name: 'zeta' })])
    const post = vi.spyOn(apiService, 'post').mockResolvedValue({
      agent: agent({ id: 10, name: 'alfa', status: 'pending' }),
      enrollment: {
        code: 'nma_x',
        expiresInSeconds: 900,
        commands: {
          addressId: 'vpn',
          serverUrl: 'http://10.8.0.1:3333',
          dockerCommand: 'docker run ...',
          systemdCommand: 'curl ...',
        },
      },
    })
    const store = useAgentsStore()
    await store.fetchAgents()

    const created = await store.createAgent({
      name: 'alfa',
      siteId: null,
      deviceId: null,
      enforceTunnelIp: true,
    })

    expect(post).toHaveBeenCalledWith('/agents', expect.objectContaining({ name: 'alfa' }))
    expect(created?.enrollment.code).toBe('nma_x')
    expect(store.agents.map((item) => item.name)).toEqual(['alfa', 'zeta'])
  })

  it('remover tira o agente da lista', async () => {
    vi.spyOn(apiService, 'get').mockResolvedValue([agent()])
    vi.spyOn(apiService, 'delete').mockResolvedValue(undefined)
    const store = useAgentsStore()
    await store.fetchAgents()

    expect(await store.removeAgent(7)).toBe(true)
    expect(store.agents).toEqual([])
  })

  it('regera os comandos para outro endereço com o mesmo código', async () => {
    const post = vi.spyOn(apiService, 'post').mockResolvedValue({
      addressId: 'lan',
      serverUrl: 'http://192.168.0.10:3333',
      dockerCommand: 'docker run ...',
      systemdCommand: 'curl ...',
    })
    const store = useAgentsStore()

    const commands = await store.installCommands('nma_x', 'lan')

    expect(post).toHaveBeenCalledWith('/agents/install-commands', {
      code: 'nma_x',
      addressId: 'lan',
    })
    expect(commands?.serverUrl).toBe('http://192.168.0.10:3333')
  })
})
