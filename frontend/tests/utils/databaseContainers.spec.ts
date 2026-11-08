import { describe, expect, it } from 'vitest'
import type { CandidateAddress } from '@/bindings/CandidateAddress'
import type { DatabaseContainer } from '@/bindings/DatabaseContainer'
import type { DatabaseContainerResolution } from '@/bindings/DatabaseContainerResolution'
import {
  addressToUse,
  agentTargetHost,
  containersFor,
  fallbackAddress,
  resolutionSummary,
} from '@/utils/databaseContainers'

const dns: CandidateAddress = {
  kind: 'dns',
  host: 'projetox_postgres',
  port: 5432,
  network: 'projetox_default',
}
const ip: CandidateAddress = {
  kind: 'container_ip',
  host: '172.20.0.5',
  port: 5432,
  network: 'projetox_default',
}
const published: CandidateAddress = {
  kind: 'published_port',
  host: '172.20.0.1',
  port: 15432,
  network: 'projetox_default',
}

function container(overrides: Partial<DatabaseContainer> = {}): DatabaseContainer {
  return {
    hostKey: 'agent-1',
    hostName: 'central',
    local: true,
    viaProbeId: null,
    routeAllowed: true,
    containerId: 'abc',
    containerName: 'projetox_postgres',
    project: 'projetox',
    service: 'postgres',
    image: 'postgres:16',
    engine: 'postgres',
    running: true,
    status: 'Up 3 hours',
    port: 5432,
    ...overrides,
  }
}

function resolution(
  chosen: CandidateAddress | null,
  overrides: Partial<DatabaseContainer> = {},
  addresses: CandidateAddress[] = [dns, ip, published]
): DatabaseContainerResolution {
  return {
    container: container(overrides),
    username: 'erp',
    database: 'vendas',
    addresses,
    chosen,
    attempts: addresses.map((address) => ({
      address,
      reachable: address === chosen,
      message: address === chosen ? null : 'Não respondeu a tempo',
    })),
  }
}

describe('bancos em contêineres', () => {
  it('filtra pelo SGBD escolhido', () => {
    const items = [container(), container({ containerId: 'm', engine: 'mysql' })]
    expect(containersFor(items, 'mysql').map((item) => item.containerId)).toEqual(['m'])
    expect(containersFor(items, null)).toHaveLength(2)
  })

  it('usa o endereço que respondeu', () => {
    expect(addressToUse(resolution(ip))).toEqual(ip)
  })

  it('sem resposta, direto fica com o nome e pelo agente com o IP', () => {
    expect(fallbackAddress(resolution(null))).toEqual(dns)
    expect(fallbackAddress(resolution(null, { local: false, viaProbeId: 7 }))).toEqual(ip)
    expect(addressToUse(resolution(null, { local: false, viaProbeId: 7 }))).toEqual(ip)
  })

  it('a linha AGENT_DATABASE_TARGETS troca o nome pelo IP do mesmo contêiner', () => {
    expect(agentTargetHost('projetox_postgres', resolution(dns))).toBe('172.20.0.5')
    expect(agentTargetHost('10.0.0.9', resolution(dns))).toBe('10.0.0.9')
    expect(agentTargetHost('db.empresa.local', null)).toBe('db.empresa.local')
  })

  it('explica por qual endereço chegou', () => {
    expect(resolutionSummary(resolution(dns))).toMatchObject({
      tone: 'success',
      title: 'Alcançado pelo nome na rede Docker a partir da central',
    })
    const byIp = resolutionSummary(
      resolution(ip, { local: false, viaProbeId: 7, hostName: 'filial' })
    )
    expect(byIp.title).toBe('Alcançado pelo IP do contêiner pelo agente filial')
    expect(byIp.text).toContain('O nome projetox_postgres não respondeu daqui')
  })

  it('sem resposta, diz o que fazer', () => {
    const direct = resolutionSummary(resolution(null))
    expect(direct.tone).toBe('warning')
    expect(direct.text).toContain('Publique a porta 5432')
    expect(direct.text).toContain('projetox_default')

    const blocked = resolutionSummary(
      resolution(null, { local: false, viaProbeId: 7, routeAllowed: false, hostName: 'filial' })
    )
    expect(blocked.title).toBe('O agente filial ainda não libera a ponte de banco')

    expect(resolutionSummary(resolution(null, {}, [])).title).toBe(
      'O contêiner não tem endereço de rede'
    )
  })
})
