import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import type { Pinia } from 'pinia'
import type { FleetMember } from '@/bindings/FleetMember'
import type { FleetView } from '@/bindings/FleetView'
import type { PluginSummary } from '@/bindings/PluginSummary'
import FleetMembers from '@/components/plugins/fleet/FleetMembers.vue'
import { settleAccess, useDeviceAccess } from '@/composables/useDeviceAccess'
import { askMemberAccess } from '@/composables/useFleetAccess'
import { apiService } from '@/services/apiService'
import { usePluginAppsStore } from '@/stores/pluginApps'
import { button, mountWith, setRole, setupDom, teardownDom } from './domHarness'

let pinia: Pinia

function member(patch: Partial<FleetMember> = {}): FleetMember {
  return {
    deviceId: 7,
    name: 'AP-terraço',
    ip: '10.0.0.7',
    platform: 'openwrt',
    firmware: '23.05.0',
    compat: 'likely',
    reasons: [],
    installedAt: '2026-10-17T10:00:00Z',
    credentialsReady: false,
    missingAccess: 'ssh',
    settings: null,
    ...patch,
  }
}

function fleet(members: FleetMember[]): FleetView {
  const plugin: PluginSummary = {
    id: 3,
    slug: 'openwrt-wifi',
    name: 'Wi-Fi OpenWrt',
    version: '1.0.0',
    description: null,
    scope: 'model',
    deviceId: null,
    source: 'builtin',
    status: 'active',
    transports: ['ssh'],
    actions: [],
    matcher: { platforms: ['openwrt'] },
    list: null,
    surfaces: ['fleet'],
    settings: null,
    fleet: null,
    risk: null,
    lastTestAt: null,
    lastTestOk: true,
    validatedCount: 0,
    updatedAt: '2026-10-17T10:00:00Z',
  }
  return { plugin, settings: null, members, candidates: [], batches: [] }
}

describe('aplicativo de frota: o acesso que falta, pedido ali mesmo', () => {
  beforeEach(() => {
    pinia = setupDom()
  })

  afterEach(() => {
    settleAccess(null)
    teardownDom()
  })

  it('o cartão sem acesso abre o formulário do transporte que falta, sem sair da página', async () => {
    setRole('admin')
    const wrapper = mountWith(FleetMembers, { view: fleet([member()]), canWrite: true }, pinia)
    await flushPromises()
    expect(wrapper.text()).toContain('Sem acesso')
    expect(wrapper.find('a[href^="/devices/7"]').exists()).toBe(false)
    button('Cadastrar o acesso (SSH)')?.click()
    await flushPromises()
    const { state } = useDeviceAccess()
    expect(state.open).toBe(true)
    expect(state.request).toEqual({
      deviceId: 7,
      transport: 'ssh',
      reason: 'Para gerenciar AP-terraço o NetMonitor precisa do acesso SSH dele.',
    })
  })

  it('quem não administra vê a quem pedir, não um botão', async () => {
    setRole('operator')
    const wrapper = mountWith(FleetMembers, { view: fleet([member()]), canWrite: false }, pinia)
    await flushPromises()
    expect(wrapper.text()).toContain('Peça a um administrador para cadastrar o acesso SSH')
    expect(button('Cadastrar o acesso')).toBeUndefined()
  })

  it('recém-adicionado sem acesso: pede na hora e relê a frota ao salvar; com acesso, não pede', async () => {
    setRole('admin')
    const store = usePluginAppsStore()
    store.fleets[3] = fleet([member({ missingAccess: 'telnet' })])
    const get = vi.spyOn(apiService, 'get').mockResolvedValue(fleet([member()]))
    const asked = askMemberAccess(3, 7)
    await flushPromises()
    expect(useDeviceAccess().state.request?.transport).toBe('telnet')
    settleAccess({
      kind: 'telnet',
      username: 'admin',
      storage: 'vault',
      port: 23,
      https: false,
      viaProbeId: null,
      hasStoredSecret: true,
      sessionActive: false,
    })
    await expect(asked).resolves.toBe(true)
    expect(get).toHaveBeenCalledWith('/plugins/3/fleet')

    store.fleets[3] = fleet([member({ credentialsReady: true, missingAccess: null })])
    await expect(askMemberAccess(3, 7)).resolves.toBe(false)
    expect(useDeviceAccess().state.open).toBe(false)
  })
})
