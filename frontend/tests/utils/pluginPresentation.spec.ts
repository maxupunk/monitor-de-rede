import { describe, expect, it } from 'vitest'
import type { DevicePluginItem } from '@/bindings/DevicePluginItem'
import type { DevicePluginsView } from '@/bindings/DevicePluginsView'
import type { Surface } from '@/bindings/Surface'
import {
  devicePrefillFor,
  followUpOf,
  installedPluginTabs,
  pluginTab,
  stepLabel,
} from '@/utils/pluginPresentation'

function item(
  id: number,
  installed: boolean,
  panelTitle?: string,
  surfaces: Surface[] = ['device']
): DevicePluginItem {
  return {
    plugin: {
      id,
      slug: `p${id}`,
      name: `Plugin ${id}`,
      version: '1.0.0',
      description: null,
      scope: 'model',
      deviceId: null,
      source: 'builtin',
      status: 'active',
      transports: ['ssh'],
      actions: [],
      matcher: {},
      panel: panelTitle
        ? {
            title: panelTitle,
            icon: 'mdi-package-variant-closed',
            listAction: 'x',
            keyColumn: 'name',
          }
        : null,
      surfaces,
      settings: null,
      fleet: null,
      risk: null,
      lastTestAt: null,
      lastTestOk: null,
      validatedCount: 0,
      updatedAt: '2026-10-01T00:00:00Z',
    },
    compat: 'likely',
    reasons: [],
    installed,
    installedAt: installed ? '2026-10-01T00:00:00Z' : null,
    deviceSettings: null,
  }
}

describe('abas de plugins instalados', () => {
  it('só os instalados com tela de dispositivo viram aba, com título e ícone próprios', () => {
    const view: DevicePluginsView = {
      deviceId: 2,
      platform: 'openwrt',
      firmware: null,
      plugins: [
        item(1, true, 'Gerenciador de pacotes'),
        item(2, false),
        item(3, true),
        item(4, true, undefined, ['fleet']),
        item(5, true, undefined, ['device', 'fleet']),
      ],
      credentials: [],
      runs: [],
      agents: [],
    }
    const tabs = installedPluginTabs(view)
    expect(tabs.map((tab) => [tab.id, tab.title, tab.icon])).toEqual([
      [1, 'Gerenciador de pacotes', 'mdi-package-variant-closed'],
      [3, 'Plugin 3', 'mdi-puzzle'],
      [5, 'Plugin 5', 'mdi-puzzle'],
    ])
    expect(pluginTab(3)).toBe('plugin-3')
  })

  it('sem visão carregada não há abas', () => {
    expect(installedPluginTabs(undefined)).toEqual([])
  })
})

describe('sugestão de um consolidado', () => {
  it('lê a ação e os parâmetros de cada equipamento', () => {
    const next = followUpOf({
      summary: 'x',
      next: {
        action: 'radios',
        title: 'Aplicar o plano de canais',
        devices: { '7': { radio_2g_channel: '6' }, lixo: 1 },
      },
    })
    expect(next).toEqual({
      action: 'radios',
      title: 'Aplicar o plano de canais',
      devices: { 7: { radio_2g_channel: '6' } },
    })
  })

  it('sem sugestão, ou malformada, não há o que aceitar', () => {
    expect(followUpOf({ summary: 'ok' })).toBeNull()
    expect(followUpOf({ next: { action: 'radios', devices: {} } })).toBeNull()
    expect(followUpOf(null)).toBeNull()
  })
})

describe('cadastro aberto de dentro de um plugin', () => {
  it('já vem com o sistema que o plugin atende', () => {
    expect(devicePrefillFor({ platforms: ['openwrt'] })).toEqual({
      operatingSystem: 'openwrt',
      type: 'router',
      isMonitored: true,
    })
    expect(devicePrefillFor({ platforms: ['linux', 'openwrt'] }).type).toBe('other')
  })

  it('plugin sem regra de sistema não escolhe por você', () => {
    expect(devicePrefillFor({})).toEqual({ isMonitored: true })
  })
})

describe('andamento de uma execução', () => {
  const entry = (kind: string, durationMs: number) => ({
    seq: 0,
    kind,
    request: "iwinfo 'phy0-ap0' scan",
    status: undefined,
    output: '',
    durationMs,
    origin: 'central',
    at: '2026-10-02T09:00:00Z',
  })

  it('mostra o que está rodando agora e o que já respondeu', () => {
    expect(stepLabel(undefined)).toBe('Conectando ao equipamento…')
    expect(stepLabel(entry('step:ssh', 0))).toBe("SSH · iwinfo 'phy0-ap0' scan · executando…")
    expect(stepLabel(entry('ssh', 3200))).toBe("SSH · iwinfo 'phy0-ap0' scan · respondeu em 3,2 s")
  })
})
