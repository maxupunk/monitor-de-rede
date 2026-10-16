import { describe, expect, it } from 'vitest'
import {
  canConfirmEdge,
  confidencePercent,
  discoveryLabel,
  edgeSpeed,
  edgeStatusKey,
  edgeTrafficLabel,
  edgeUtilization,
  isVirtualEdge,
  linkKind,
  linkKindPresentation,
  nodeCategory,
  nodeIcon,
  nodeIconKey,
  nodeStatusKey,
  nodeTypeLabel,
  speedTier,
  utilizationLevel,
} from '@/utils/topology/presentation'
import { edge, node } from './fixtures'

describe('apresentação da topologia', () => {
  it('normaliza o estado do nó (inclusive sinônimos)', () => {
    expect(nodeStatusKey('online')).toBe('online')
    expect(nodeStatusKey('DOWN')).toBe('offline')
    expect(nodeStatusKey('degraded')).toBe('warning')
    expect(nodeStatusKey(undefined)).toBe('unknown')
  })

  it('ícone e rótulo do nó: sistema, switch não gerenciável e tipos do cadastro', () => {
    expect(nodeIconKey(node(1, { isSystem: true }))).toBe('system')
    expect(nodeTypeLabel(node(1, { isSystem: true }))).toBe('Servidor NetMonitor')
    expect(nodeIconKey(node(1, { type: 'hub' }))).toBe('unmanaged_switch')
    expect(nodeIcon(node(1, { type: 'unmanaged_switch' }))).toBe('mdi-hub')
    expect(nodeTypeLabel(node(1, { type: 'router' }))).toBe('Roteador')
    expect(nodeCategory(node(1, { type: 'gateway' }))).toBe('router')
    expect(nodeIconKey(node(1, { type: 'firewall' }))).toBe('firewall')
  })

  it('tipo de enlace: o protocolo de descoberta é cabo; inferido e hierarquia à parte', () => {
    expect(linkKind('lldp')).toBe('ethernet')
    expect(linkKind('snmp')).toBe('ethernet')
    expect(linkKind('manual')).toBe('ethernet')
    expect(linkKind('inferred')).toBe('inferred')
    expect(linkKind('parent')).toBe('parent')
    expect(linkKind('fiber')).toBe('fiber')
    expect(linkKindPresentation('inferred').label).toBe('Inferido (sub-rede)')
    // Nunca mais "Cabo Ethernet" para um enlace inferido.
    expect(linkKindPresentation('inferred').label).not.toContain('Cabo')
  })

  it('método de descoberta em português', () => {
    expect(discoveryLabel('lldp')).toBe('Descoberto por LLDP')
    expect(discoveryLabel('subnet_inference')).toBe('Inferido pela sub-rede')
    expect(discoveryLabel('user_defined')).toBe('Definido manualmente')
    expect(discoveryLabel('parent_hierarchy')).toBe('Hierarquia (dispositivo pai)')
  })

  it('velocidade, faixa e uso do enlace', () => {
    const e = edge(1, 1, 2, {
      sourceInterfaceSpeed: 1_000_000_000,
      targetInterfaceSpeed: 100_000_000,
      inBps: 75_000_000,
      outBps: 10_000_000,
    })
    expect(edgeSpeed(e)).toBe(100_000_000)
    expect(speedTier(edgeSpeed(e))).toBe('slow')
    expect(speedTier(1_000_000_000)).toBe('gigabit')
    expect(speedTier(10_000_000_000)).toBe('fast')
    expect(speedTier(null)).toBe('unknown')
    expect(edgeUtilization(e)).toBeCloseTo(0.75)
    expect(utilizationLevel(0.75)).toBe('high')
    expect(utilizationLevel(0.95)).toBe('critical')
    expect(utilizationLevel(0.2)).toBe('normal')
    expect(utilizationLevel(null)).toBeNull()
    expect(edgeUtilization({ ...e, utilization: 0.4 })).toBe(0.4)
    expect(edgeTrafficLabel(e)).toBe('↓ 75 Mbps ↑ 10 Mbps')
    expect(edgeTrafficLabel(edge(2, 1, 2))).toBeNull()
  })

  it('estado do enlace: porta fora derruba o enlace', () => {
    expect(edgeStatusKey(edge(1, 1, 2, { status: 'up', targetInterfaceStatus: 'down' }))).toBe(
      'down'
    )
    expect(edgeStatusKey(edge(1, 1, 2, { status: undefined }))).toBe('unknown')
  })

  it('só enlace descoberto/inferido e não confirmado pode ser confirmado', () => {
    expect(canConfirmEdge(edge(1, 1, 2, { discoveryMethod: 'lldp', confirmed: false }))).toBe(true)
    expect(canConfirmEdge(edge(1, 1, 2, { discoveryMethod: 'lldp', confirmed: true }))).toBe(false)
    expect(canConfirmEdge(edge(1, 1, 2, { discoveryMethod: 'user_defined' }))).toBe(false)
    expect(isVirtualEdge(edge(-5, 1, 2, { linkType: 'parent' }))).toBe(true)
    expect(canConfirmEdge(edge(-5, 1, 2, { linkType: 'parent', confirmed: false }))).toBe(false)
  })

  it('confiança aceita 0–100 e 0..1', () => {
    expect(confidencePercent(95)).toBe(95)
    expect(confidencePercent(0.6)).toBe(60)
    expect(confidencePercent(null)).toBeNull()
  })
})
