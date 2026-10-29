import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import type { Pinia } from 'pinia'
import type { BatchDevice } from '@/bindings/BatchDevice'
import type { CredentialView } from '@/bindings/CredentialView'
import type { FleetMember } from '@/bindings/FleetMember'
import type { FleetView } from '@/bindings/FleetView'
import type { PluginAction } from '@/bindings/PluginAction'
import type { PluginBatchView } from '@/bindings/PluginBatchView'
import type { PluginSummary } from '@/bindings/PluginSummary'
import DeviceAccessHost from '@/components/devices/access/DeviceAccessHost.vue'
import FleetBatchCard from '@/components/plugins/fleet/FleetBatchCard.vue'
import FleetMembers from '@/components/plugins/fleet/FleetMembers.vue'
import { confirm } from '@/composables/useConfirm'
import { settleAccess, useDeviceAccess } from '@/composables/useDeviceAccess'
import { askFleetAccess, askMemberAccess, rerunAfterAccess } from '@/composables/useFleetAccess'
import { apiService } from '@/services/apiService'
import { usePluginAppsStore } from '@/stores/pluginApps'
import { button, mountWith, setRole, setupDom, teardownDom } from './domHarness'

vi.mock('@/composables/useConfirm', () => ({
  confirm: vi.fn(async () => true),
  prompt: vi.fn(async () => null),
}))

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
    wifi: false,
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
      storage: 'ask',
      port: 23,
      https: false,
      viaProbeId: null,
      hasStoredSecret: false,
      sessionActive: true,
    })
    await expect(asked).resolves.toBe(true)
    expect(get).toHaveBeenCalledWith('/plugins/3/fleet')

    store.fleets[3] = fleet([member({ credentialsReady: true, missingAccess: null })])
    await expect(askMemberAccess(3, 7)).resolves.toBe(false)
    expect(useDeviceAccess().state.open).toBe(false)
  })

  it('nenhum com acesso: a janela pede um por vez e "Agora não" encerra a fila', async () => {
    setRole('admin')
    const store = usePluginAppsStore()
    const outro = member({ deviceId: 8, name: 'AP-sala' })
    store.fleets[3] = fleet([member(), outro])
    const get = vi
      .spyOn(apiService, 'get')
      .mockResolvedValue(fleet([member({ credentialsReady: true, missingAccess: null }), outro]))
    const asked = askFleetAccess(3)
    await flushPromises()
    const { state } = useDeviceAccess()
    expect(state.request).toMatchObject({
      deviceId: 7,
      reason: 'Para gerenciar AP-terraço o NetMonitor precisa do acesso SSH dele. (1 de 2)',
    })
    settleAccess(sshSaved())
    await flushPromises()
    expect(state.request).toMatchObject({ deviceId: 8 })
    settleAccess(null)
    await expect(asked).resolves.toBe(true)
    expect(get).toHaveBeenCalledWith('/plugins/3/fleet')
  })

  it('"Agora não" no primeiro: nada é relido e a ação não abre', async () => {
    setRole('admin')
    usePluginAppsStore().fleets[3] = fleet([member()])
    const get = vi.spyOn(apiService, 'get')
    const asked = askFleetAccess(3)
    await flushPromises()
    settleAccess(null)
    await expect(asked).resolves.toBe(false)
    expect(get).not.toHaveBeenCalled()
  })
})

function failedDevice(): BatchDevice {
  return {
    deviceId: 7,
    deviceName: 'AP-terraço',
    runId: 90,
    status: 'failed',
    error: 'Cadastre a credencial SSH deste equipamento para usar o plugin.',
    output: null,
    missingAccess: 'ssh',
  }
}

function failedBatch(id = 30, action = 'set_wifi'): PluginBatchView {
  return {
    id,
    pluginId: 3,
    action,
    title: 'Trocar a senha do Wi-Fi',
    status: 'failed',
    devices: [failedDevice()],
    result: null,
    hasPatch: false,
    error: null,
    createdAt: '2026-10-17T10:00:00Z',
    finishedAt: '2026-10-17T10:00:05Z',
  }
}

function deviceAction(patch: Partial<PluginAction> = {}): PluginAction {
  return {
    id: 'set_wifi',
    title: 'Trocar a senha do Wi-Fi',
    effect: 'write',
    params: {
      type: 'object',
      properties: { ssid: { type: 'string' }, key: { type: 'string' } },
    },
    output: 'json',
    safeToRetest: false,
    ...patch,
  }
}

function sshSaved(): CredentialView {
  return {
    kind: 'ssh',
    username: 'root',
    storage: 'ask',
    port: 22,
    https: false,
    viaProbeId: null,
    hasStoredSecret: false,
    sessionActive: true,
  }
}

describe('lote de frota: rodar de novo depois de salvar o acesso', () => {
  beforeEach(() => {
    pinia = setupDom()
    vi.mocked(confirm).mockClear()
  })

  afterEach(() => {
    settleAccess(null)
    teardownDom()
  })

  it('da linha do equipamento, salvar o acesso avisa a página com o lote e o equipamento', async () => {
    setRole('admin')
    mountWith(DeviceAccessHost, {}, pinia)
    vi.spyOn(apiService, 'get').mockResolvedValue({
      deviceId: 7,
      deviceName: 'AP-terraço',
      credentials: [],
      agents: [],
    })
    const card = mountWith(
      FleetBatchCard,
      { batch: failedBatch(), canWrite: true, expandAll: true },
      pinia
    )
    await flushPromises()
    button('Cadastrar o acesso (SSH)')?.click()
    await flushPromises()
    expect(useDeviceAccess().state.request).toEqual(
      expect.objectContaining({ deviceId: 7, transport: 'ssh' })
    )
    settleAccess(sshSaved())
    await flushPromises()
    expect(card.emitted('accessSaved')).toEqual([[failedBatch(), failedDevice()]])
  })

  it('lançado daqui, uma leitura com parâmetros roda de novo sozinha com os mesmos valores', async () => {
    setRole('admin')
    const post = vi
      .spyOn(apiService, 'post')
      .mockResolvedValueOnce({ batchId: 30 })
      .mockResolvedValueOnce({ batchId: 31 })
    const store = usePluginAppsStore()
    await store.runAction(3, 'scan', [7], false, { band: '5g' }, { 7: { radio: 'phy1' } })

    const outcome = await rerunAfterAccess(
      failedBatch(30, 'scan'),
      failedDevice(),
      deviceAction({ id: 'scan', effect: 'read' })
    )
    expect(outcome).toBe('started')
    expect(confirm).not.toHaveBeenCalled()
    expect(post).toHaveBeenLastCalledWith('/plugins/3/fleet/actions/scan', {
      deviceIds: [7],
      params: { band: '5g' },
      confirmWrite: false,
      deviceParams: { 7: { radio: 'phy1' } },
    })
    // Rodou de novo: o lote antigo não guarda mais os valores dele.
    expect(store.canRerun(30, 7)).toBe(false)
  })

  it('lançado daqui, uma escrita pergunta antes; recusar não roda nada', async () => {
    setRole('admin')
    const post = vi
      .spyOn(apiService, 'post')
      .mockResolvedValueOnce({ batchId: 30 })
      .mockResolvedValueOnce({ batchId: 31 })
    const store = usePluginAppsStore()
    await store.runAction(3, 'set_wifi', [7], true, { ssid: 'Loja', key: 'segredo' })

    vi.mocked(confirm).mockResolvedValueOnce(false)
    await expect(rerunAfterAccess(failedBatch(), failedDevice(), deviceAction())).resolves.toBe(
      'declined'
    )
    expect(post).toHaveBeenCalledTimes(1)

    await expect(rerunAfterAccess(failedBatch(), failedDevice(), deviceAction())).resolves.toBe(
      'started'
    )
    expect(confirm).toHaveBeenLastCalledWith(
      expect.objectContaining({ message: expect.stringContaining('Isso altera o equipamento') })
    )
    expect(post).toHaveBeenLastCalledWith('/plugins/3/fleet/actions/set_wifi', {
      deviceIds: [7],
      params: { ssid: 'Loja', key: 'segredo' },
      confirmWrite: true,
      deviceParams: undefined,
    })
  })

  it('sem os valores do lote: nada roda, e o cartão oferece o formulário com o equipamento', async () => {
    setRole('admin')
    const post = vi.spyOn(apiService, 'post')
    await expect(rerunAfterAccess(failedBatch(), failedDevice(), deviceAction())).resolves.toBe(
      'needsForm'
    )
    expect(post).not.toHaveBeenCalled()
    expect(confirm).not.toHaveBeenCalled()

    const card = mountWith(
      FleetBatchCard,
      { batch: failedBatch(), canWrite: true, expandAll: true, needsForm: [7] },
      pinia
    )
    await flushPromises()
    expect(card.text()).toContain('não ficaram neste navegador')
    expect(button('Cadastrar o acesso')).toBeUndefined()
    button('Abrir o formulário')?.click()
    expect(card.emitted('openForm')).toEqual([[failedBatch(), failedDevice()]])
  })

  it('sem parâmetros, roda de novo mesmo sem os valores guardados', async () => {
    setRole('admin')
    const post = vi.spyOn(apiService, 'post').mockResolvedValue({ batchId: 31 })
    const action = deviceAction({ id: 'status', effect: 'read', params: undefined })
    await expect(rerunAfterAccess(failedBatch(30, 'status'), failedDevice(), action)).resolves.toBe(
      'started'
    )
    expect(post).toHaveBeenCalledWith('/plugins/3/fleet/actions/status', {
      deviceIds: [7],
      params: {},
      confirmWrite: false,
      deviceParams: undefined,
    })
  })
})
