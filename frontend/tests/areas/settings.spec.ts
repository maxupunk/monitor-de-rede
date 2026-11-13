import { describe, expect, it } from 'vitest'
import { defineComponent } from 'vue'
import type { Area } from '@/bindings/Area'
import { AREAS } from '@/areas'
import { defineArea } from '@/areas/defineArea'
import { buildSettingsTabs } from '@/areas/settings'

const Card = defineComponent({ template: '<div />' })
const all = (): boolean => true
const only =
  (...areas: Area[]) =>
  (area: Area): boolean =>
    area === 'core' || areas.includes(area)

describe('buildSettingsTabs', () => {
  it('com a Rede desligada, Configurações não mostra nada só da Rede', () => {
    const values = buildSettingsTabs(AREAS, only()).map((tab) => tab.value)
    expect(values).not.toContain('rede')
    expect(values).not.toContain('wifi')
    expect(values).toEqual(['geral', 'conectividade', 'notificacoes', 'ia', 'sistema'])
  })

  it('com a Rede ligada, a aba dela entra na ordem declarada', () => {
    const values = buildSettingsTabs(AREAS, all).map((tab) => tab.value)
    expect(values).toEqual([
      'geral',
      'conectividade',
      'rede',
      'wifi',
      'notificacoes',
      'ia',
      'sistema',
    ])
  })

  it('cartão de outra área entra na aba declarada e some com a área', () => {
    const core = defineArea({
      id: 'core',
      settingsTabs: [{ value: 'geral', title: 'Geral', icon: 'mdi-tune', order: 10 }],
      settingsCards: [{ tab: 'geral', order: 20, component: Card }],
    })
    const vpn = defineArea({
      id: 'vpn',
      settingsCards: [{ tab: 'geral', order: 10, component: Card }],
    })
    expect(buildSettingsTabs([core, vpn], all)[0]?.cards.map((card) => card.order)).toEqual([
      10, 20,
    ])
    expect(buildSettingsTabs([core, vpn], only())[0]?.cards).toHaveLength(1)
  })

  it('aba sem nenhum cartão visível não aparece', () => {
    const core = defineArea({
      id: 'core',
      settingsTabs: [{ value: 'vazia', title: 'Vazia', icon: 'mdi-tune', order: 10 }],
    })
    const network = defineArea({
      id: 'network',
      settingsCards: [{ tab: 'vazia', order: 10, component: Card }],
    })
    expect(buildSettingsTabs([core, network], only())).toEqual([])
    expect(buildSettingsTabs([core, network], all)).toHaveLength(1)
  })
})
