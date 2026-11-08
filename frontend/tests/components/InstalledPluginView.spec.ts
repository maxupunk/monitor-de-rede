import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import type { Pinia } from 'pinia'
import type { DevicePluginItem } from '@/bindings/DevicePluginItem'
import type { PluginDetail } from '@/bindings/PluginDetail'
import InstalledPluginView from '@/components/plugins/device/InstalledPluginView.vue'
import { usePluginsStore } from '@/stores/plugins'
import { bodyText, button, mountWith, setRole, setupDom, teardownDom } from './domHarness'

function installed(status: string): DevicePluginItem {
  return {
    plugin: {
      id: 3,
      slug: 'openwrt-wifi',
      name: 'Rede Wi-Fi (OpenWrt)',
      version: '2.7.0',
      description: 'Gerencia as redes Wi-Fi.',
      scope: 'model',
      deviceId: null,
      source: 'builtin',
      status,
      transports: ['ssh'],
      actions: [
        { id: 'detect', title: 'Detectar', effect: 'read', output: 'kv', safeToRetest: false },
      ],
      matcher: {},
      list: null,
      surfaces: ['device'],
      settings: null,
      fleet: null,
      wifi: true,
      risk: null,
      lastTestAt: null,
      lastTestOk: null,
      validatedCount: 0,
      updatedAt: '2026-10-01T00:00:00Z',
    },
    compat: 'likely',
    reasons: [],
    installed: true,
    installedAt: '2026-10-01T00:00:00Z',
    deviceSettings: null,
  }
}

let pinia: Pinia

beforeEach(() => {
  pinia = setupDom()
  setRole('admin')
})

afterEach(teardownDom)

describe('plugin instalado no equipamento', () => {
  it('ativo mostra as ações', () => {
    mountWith(
      InstalledPluginView,
      { item: installed('active'), deviceId: 2, deviceName: 'R1' },
      pinia
    )
    expect(button('Executar')).toBeDefined()
    expect(button('Ativar plugin')).toBeUndefined()
  })

  it('desativado esconde tudo do plugin e oferece ativar', async () => {
    const store = usePluginsStore()
    const lifecycle = vi.spyOn(store, 'lifecycle').mockResolvedValue({} as PluginDetail)
    mountWith(
      InstalledPluginView,
      { item: installed('disabled'), deviceId: 2, deviceName: 'R1' },
      pinia
    )

    expect(bodyText()).toContain('Plugin desativado')
    expect(button('Executar')).toBeUndefined()
    expect(button('Validar neste equipamento')).toBeUndefined()
    expect(button('Desinstalar')).toBeDefined()

    button('Ativar plugin')?.click()
    await flushPromises()
    expect(lifecycle).toHaveBeenCalledWith(3, 'enable', 2)
  })

  it('em quarentena pede a revisão em vez de ativar', () => {
    const wrapper = mountWith(
      InstalledPluginView,
      { item: installed('quarantine'), deviceId: 2, deviceName: 'R1' },
      pinia
    )
    expect(button('Executar')).toBeUndefined()
    expect(button('Ativar plugin')).toBeUndefined()
    button('Revisar segurança')?.click()
    expect(wrapper.emitted('review')).toEqual([[3]])
  })
})
