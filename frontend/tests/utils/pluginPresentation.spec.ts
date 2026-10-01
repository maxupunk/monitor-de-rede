import { describe, expect, it } from 'vitest'
import type { DevicePluginItem } from '@/bindings/DevicePluginItem'
import type { DevicePluginsView } from '@/bindings/DevicePluginsView'
import { installedPluginTabs, pluginTab } from '@/utils/pluginPresentation'

function item(id: number, installed: boolean, panelTitle?: string): DevicePluginItem {
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
  }
}

describe('abas de plugins instalados', () => {
  it('só os instalados viram aba, com o título e o ícone da tela própria', () => {
    const view: DevicePluginsView = {
      deviceId: 2,
      platform: 'openwrt',
      firmware: null,
      plugins: [item(1, true, 'Gerenciador de pacotes'), item(2, false), item(3, true)],
      credentials: [],
      runs: [],
      agents: [],
    }
    const tabs = installedPluginTabs(view)
    expect(tabs.map((tab) => [tab.id, tab.title, tab.icon])).toEqual([
      [1, 'Gerenciador de pacotes', 'mdi-package-variant-closed'],
      [3, 'Plugin 3', 'mdi-puzzle'],
    ])
    expect(pluginTab(3)).toBe('plugin-3')
  })

  it('sem visão carregada não há abas', () => {
    expect(installedPluginTabs(undefined)).toEqual([])
  })
})
