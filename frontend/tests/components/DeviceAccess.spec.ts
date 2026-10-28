import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import type { Pinia } from 'pinia'
import type { CredentialView } from '@/bindings/CredentialView'
import type { DeviceAccessView } from '@/bindings/DeviceAccessView'
import DeviceAccessHost from '@/components/devices/access/DeviceAccessHost.vue'
import { confirm } from '@/composables/useConfirm'
import {
  ensureAccess,
  settleAccess,
  useDeviceAccess,
  withAccess,
} from '@/composables/useDeviceAccess'
import { ApiError, apiService } from '@/services/apiService'
import { bodyText, button, mountWith, setRole, setupDom, teardownDom, type } from './domHarness'

vi.mock('@/composables/useConfirm', () => ({
  confirm: vi.fn(async () => true),
  prompt: vi.fn(async () => null),
}))

let pinia: Pinia

function access(credentials: CredentialView[] = []): DeviceAccessView {
  return { deviceId: 7, deviceName: 'AP-terraço', credentials, agents: [] }
}

function saved(): CredentialView {
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

function missing(transport = 'ssh'): ApiError {
  return new ApiError('Cadastre a credencial SSH deste equipamento para usar o plugin.', 409, {
    message: 'Cadastre a credencial SSH deste equipamento para usar o plugin.',
    code: 'missing_credential',
    deviceId: 7,
    transport,
  })
}

/** Preenche a senha do formulário aberto e salva. */
async function fillAndSave() {
  await flushPromises()
  const password = document.body.querySelector<HTMLInputElement>('input[type="password"]')
  expect(password).not.toBeNull()
  type(password as HTMLInputElement, 'segredo')
  await flushPromises()
  button('Salvar acesso')?.click()
  await flushPromises()
}

describe('acesso do equipamento pedido ali mesmo', () => {
  beforeEach(() => {
    pinia = setupDom()
    vi.mocked(confirm).mockClear()
  })

  afterEach(() => {
    settleAccess(null)
    teardownDom()
  })

  it('leitura sem acesso: o formulário abre no transporte que falta, salva e a ação roda de novo', async () => {
    setRole('admin')
    mountWith(DeviceAccessHost, {}, pinia)
    vi.spyOn(apiService, 'get').mockResolvedValue(access())
    const put = vi.spyOn(apiService, 'put').mockResolvedValue(saved())
    const action = vi.fn().mockRejectedValueOnce(missing()).mockResolvedValueOnce(41)

    const result = withAccess(action, { retry: 'auto', what: 'Ler rádios' })
    await flushPromises()
    expect(bodyText()).toContain('Acesso SSH')
    expect(bodyText()).toContain('AP-terraço')
    // O motivo do backend vai no topo; o tipo vem fixo (não há seletor).
    expect(bodyText()).toContain('Cadastre a credencial SSH')
    expect(bodyText()).not.toContain('Tipo de acesso')

    await fillAndSave()
    expect(put).toHaveBeenCalledWith(
      '/devices/7/credentials',
      expect.objectContaining({
        kind: 'ssh',
        username: 'root',
        secret: 'segredo',
        storage: 'ask',
      })
    )
    await expect(result).resolves.toBe(41)
    expect(action).toHaveBeenCalledTimes(2)
    expect(confirm).not.toHaveBeenCalled()
  })

  it('escrita: depois de salvar, pergunta antes de repetir; "não" devolve null', async () => {
    setRole('admin')
    vi.mocked(confirm).mockResolvedValueOnce(false)
    const action = vi.fn().mockRejectedValue(missing('telnet'))
    const result = withAccess(action, { retry: 'confirm', what: 'Reiniciar' })
    await flushPromises()
    expect(useDeviceAccess().state.request).toEqual(
      expect.objectContaining({ deviceId: 7, transport: 'telnet' })
    )
    settleAccess({ ...saved(), kind: 'telnet' })
    await expect(result).resolves.toBeNull()
    expect(confirm).toHaveBeenCalledWith(
      expect.objectContaining({ message: expect.stringContaining('«Reiniciar»') })
    )
    expect(action).toHaveBeenCalledTimes(1)
  })

  it('"Agora não" e outro erro sobem como vieram; perfil sem permissão não abre nada', async () => {
    setRole('admin')
    const err = missing()
    const cancelled = withAccess(() => Promise.reject(err), { retry: 'auto', what: 'x' })
    await flushPromises()
    expect(useDeviceAccess().state.open).toBe(true)
    settleAccess(null)
    await expect(cancelled).rejects.toBe(err)

    const other = new ApiError('Equipamento sem IP', 400, { message: 'Equipamento sem IP' })
    await expect(
      withAccess(() => Promise.reject(other), { retry: 'auto', what: 'x' })
    ).rejects.toBe(other)
    expect(useDeviceAccess().state.open).toBe(false)

    setRole('operator')
    await expect(withAccess(() => Promise.reject(err), { retry: 'auto', what: 'x' })).rejects.toBe(
      err
    )
    expect(useDeviceAccess().state.open).toBe(false)
  })

  it('ensureAccess: pronto segue sem perguntar; faltando, pede', async () => {
    setRole('admin')
    const get = vi.spyOn(apiService, 'get').mockResolvedValueOnce(access([saved()]))
    await expect(ensureAccess(7, 'ssh')).resolves.toBe(true)
    expect(get).toHaveBeenCalledWith('/devices/7/access')
    expect(useDeviceAccess().state.open).toBe(false)

    get.mockResolvedValueOnce(access())
    const asked = ensureAccess(7, 'ssh', 'Para medir, o NetMonitor precisa do SSH.')
    await flushPromises()
    expect(useDeviceAccess().state.request?.reason).toContain('precisa do SSH')
    settleAccess(saved())
    await expect(asked).resolves.toBe(true)
  })
})
