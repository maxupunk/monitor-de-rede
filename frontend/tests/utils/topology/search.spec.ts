import { describe, expect, it } from 'vitest'
import { searchNodes, termScore } from '@/utils/topology/search'
import { node } from './fixtures'

const nodes = [
  node(1, { name: 'Switch Core', type: 'switch', ipAddress: '10.0.1.1', vendor: 'Cisco' }),
  node(2, { name: 'SW-Acesso-Térreo', type: 'switch', ipAddress: '10.0.10.1', vendor: 'Aruba' }),
  node(3, { name: 'Core-RT01', type: 'router', ipAddress: '10.0.0.2', vendor: 'MikroTik' }),
  node(4, { name: 'PC-001', type: 'workstation', ipAddress: '10.0.11.101', model: 'ThinkCentre' }),
]

describe('busca do mapa', () => {
  it('nome exato > prefixo > palavra > trecho > subsequência', () => {
    expect(termScore('core', 'core')).toBeGreaterThan(termScore('core', 'core-rt01'))
    expect(termScore('core', 'core-rt01')).toBeGreaterThan(termScore('core', 'switch core'))
    expect(termScore('rt0', 'core-rt01')).toBeGreaterThan(termScore('crt', 'core-rt01'))
    expect(termScore('xyz', 'core-rt01')).toBe(0)
  })

  it('ranqueia pelo nome antes do fabricante', () => {
    const hits = searchNodes(nodes, 'core')
    expect(hits.map((hit) => hit.node.id)).toEqual([3, 1])
  })

  it('acha por IP (prefixo) e diz por qual campo casou', () => {
    const hits = searchNodes(nodes, '10.0.10')
    expect(hits[0].node.id).toBe(2)
    expect(hits[0].matchedField).toBe('ip')
  })

  it('ignora acentos e aceita vários termos', () => {
    expect(searchNodes(nodes, 'terreo')[0].node.id).toBe(2)
    expect(searchNodes(nodes, 'sw cor')[0].node.id).toBe(1)
  })

  it('acha por fabricante e modelo; vazio não lista nada', () => {
    expect(searchNodes(nodes, 'aruba')[0].node.id).toBe(2)
    expect(searchNodes(nodes, 'thinkcentre')[0].node.id).toBe(4)
    expect(searchNodes(nodes, '   ')).toEqual([])
  })
})
