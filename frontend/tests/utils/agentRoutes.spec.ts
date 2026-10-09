import { describe, expect, it } from 'vitest'
import { agentRouteItems } from '@/utils/agentRoutes'

describe('opções de "Acessar a partir de"', () => {
  const agents = [
    { id: 1, name: 'pronto', connected: true, allowed: true },
    { id: 2, name: 'fora', connected: false, allowed: true },
    { id: 3, name: 'sem-permissao', connected: true, allowed: false },
  ]

  it('a central vem primeiro e sempre serve', () => {
    const [central] = agentRouteItems(agents, 'database', 'direto')
    expect(central).toEqual({
      title: 'Central (este servidor)',
      value: null,
      props: { disabled: false, subtitle: 'direto' },
    })
  })

  it('agente sem a permissão fica desabilitado e diz o motivo', () => {
    const items = agentRouteItems(agents, 'database', '').slice(1)
    expect(items.map((item) => item.props.disabled)).toEqual([false, false, true])
    expect(items[1]?.props.subtitle).toBe('desconectado agora')
    expect(items[2]?.props.subtitle).toContain("'database'")
  })
})
