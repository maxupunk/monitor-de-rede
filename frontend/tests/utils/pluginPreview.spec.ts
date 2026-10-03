import { describe, expect, it } from 'vitest'
import type { PluginAction } from '@/bindings/PluginAction'
import type { PluginManifest } from '@/bindings/PluginManifest'
import type { TestReport } from '@/bindings/TestReport'
import {
  actionsWithForm,
  firstOutputOf,
  previewFleet,
  previewOutputs,
  testedAction,
} from '@/utils/pluginPreview'

function report(): TestReport {
  const passed = (name: string, action: string, output: unknown) => ({
    name,
    action,
    passed: true,
    output,
    transcript: [],
  })
  return {
    passed: false,
    total: 4,
    failed: 1,
    problems: [],
    hints: [],
    cases: [
      passed('AP com duas redes', 'status', { networks: [{ ssid: 'Loja' }, { ssid: 'OpenWrt' }] }),
      passed('AP com uma rede', 'status', { networks: [{ ssid: 'Loja' }] }),
      passed('lista de pacotes', 'list_packages', [{ name: 'usteer' }]),
      { name: 'quebrado', action: 'status', passed: false, message: 'x', transcript: [] },
    ],
  }
}

describe('prévia do editor', () => {
  it('só os testes que passaram viram tela, na ordem do pacote', () => {
    expect(previewOutputs(report()).map((item) => item.name)).toEqual([
      'AP com duas redes',
      'AP com uma rede',
      'lista de pacotes',
    ])
    expect(firstOutputOf(report(), 'list_packages')).toEqual([{ name: 'usteer' }])
    expect(firstOutputOf(report(), 'inexistente')).toBeNull()
  })

  it('cada teste da ação de estado vira um equipamento de exemplo', () => {
    const fleet = previewFleet(report(), 'status')
    expect(fleet.members).toEqual([
      { deviceId: 1, name: 'AP com duas redes' },
      { deviceId: 2, name: 'AP com uma rede' },
    ])
    expect(fleet.outputs.get(2)).toEqual({ networks: [{ ssid: 'Loja' }] })
  })

  it('só as ações com algum parâmetro têm formulário — sem repetir o da pré-visualização', () => {
    const action = (id: string, params?: Record<string, unknown>): PluginAction => ({
      id,
      title: id,
      effect: 'read',
      output: 'kv',
      safeToRetest: false,
      params,
    })
    const form = { type: 'object', properties: { address: { type: 'string' } } }
    const manifest: PluginManifest = {
      slug: 'x',
      name: 'X',
      version: '1.0.0',
      transports: ['ssh'],
      match: {},
      surfaces: ['device', 'fleet'],
      actions: [
        action('detect'),
        action('vazio', { type: 'object', properties: {} }),
        action('preview_lan', form),
        action('set_lan', form),
      ],
      fleet: {
        title: 'LAN',
        actions: [
          { id: 'lan', title: 'Configurar LAN', action: 'set_lan', preview: 'preview_lan' },
        ],
      },
    }
    expect(actionsWithForm(manifest).map((item) => item.id)).toEqual(['set_lan'])
    expect(testedAction(manifest, 'lan')?.title).toBe('Configurar LAN')
    expect(testedAction(manifest, 'detect')?.title).toBe('detect')
    expect(testedAction(manifest, 'nada')).toBeNull()
  })
})
