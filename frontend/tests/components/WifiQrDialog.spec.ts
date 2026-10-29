import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import type { Pinia } from 'pinia'
import type { WifiQr } from '@/bindings/WifiQr'
import WifiQrDialog from '@/components/plugins/items/WifiQrDialog.vue'
import { apiService } from '@/services/apiService'
import { bodyText, mountWith, setRole, setupDom, teardownDom } from './domHarness'

let pinia: Pinia

const spec: WifiQr = {
  ssid: 'ssid',
  encryption: 'encryption',
  hidden: 'hidden',
  secret: 'key',
  reveal: { action: 'reveal_key', params: { ssid: 'ssid' } },
}

const devices = [
  { deviceId: 7, name: 'AP-terraço' },
  { deviceId: 8, name: 'AP-sala' },
]

function open(item: Record<string, unknown>) {
  return mountWith(WifiQrDialog, { modelValue: true, pluginId: 3, spec, item, devices }, pinia)
}

describe('QR code do Wi-Fi', () => {
  beforeEach(() => {
    pinia = setupDom()
    setRole('admin')
  })

  afterEach(teardownDom)

  it('lê a senha do primeiro roteador que responde e desenha o código', async () => {
    const post = vi
      .spyOn(apiService, 'post')
      .mockRejectedValueOnce(new Error('sem resposta'))
      .mockResolvedValueOnce({ output: { key: 'segredo123' } })
    open({ ssid: 'Loja', encryption: 'sae-mixed', hidden: true })
    await flushPromises()

    expect(post).toHaveBeenNthCalledWith(
      1,
      '/devices/7/plugins/3/reveal/reveal_key',
      { params: { ssid: 'Loja' } },
      { timeoutMs: 30_000 }
    )
    expect(post.mock.calls[1]?.[0]).toBe('/devices/8/plugins/3/reveal/reveal_key')
    expect(document.querySelector('svg[aria-label="QR code da rede Loja"] path')).not.toBeNull()
    expect(bodyText()).toContain('Senha lida agora de AP-sala')
    expect(bodyText()).toContain('Rede oculta')
    expect(bodyText()).not.toContain('segredo123')
  })

  it('rede sem senha não lê nada no roteador', async () => {
    const post = vi.spyOn(apiService, 'post')
    open({ ssid: 'Visitantes', encryption: 'owe' })
    await flushPromises()
    expect(post).not.toHaveBeenCalled()
    expect(bodyText()).toContain('Aberta protegida, sem senha')
    expect(document.querySelector('svg path')).not.toBeNull()
  })

  it('nenhum roteador respondeu: diz por quê e oferece tentar de novo', async () => {
    vi.spyOn(apiService, 'post').mockRejectedValue(new Error('Tempo esgotado'))
    open({ ssid: 'Loja', encryption: 'psk2' })
    await flushPromises()
    expect(bodyText()).toContain('Tempo esgotado')
    expect(bodyText()).toContain('Tentar de novo')
  })
})
