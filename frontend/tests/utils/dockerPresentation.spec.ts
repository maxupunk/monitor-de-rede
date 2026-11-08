import { describe, expect, it } from 'vitest'
import type { DockerContainerMetrics } from '@/bindings/DockerContainerMetrics'
import type { DockerContainerSummary } from '@/bindings/DockerContainerSummary'
import {
  composeProject,
  containerActions,
  containerName,
  containerState,
  filterContainers,
  groupContainers,
  shortId,
  stateFilterCounts,
} from '@/utils/dockerPresentation'

function container(overrides: Partial<DockerContainerSummary>): DockerContainerSummary {
  return {
    id: 'a1b2c3d4e5f6a7b8c9',
    names: ['/web'],
    image: 'nginx:1.27',
    imageId: 'sha256:abc',
    state: 'running',
    status: 'Up 1 hour',
    labels: {},
    ports: [],
    created: 0,
    projectName: null,
    protected: false,
    ...overrides,
  }
}

describe('apresentação do Docker', () => {
  it('nome, id curto e projeto', () => {
    expect(containerName(container({}))).toBe('web')
    expect(containerName(container({ names: [] }))).toBe('a1b2c3d4e5f6')
    expect(shortId('sha256:0123456789abcdef')).toBe('0123456789ab')
    expect(composeProject({ 'com.docker.compose.project': 'erp' })).toBe('erp')
    expect(composeProject({})).toBeNull()
  })

  it('estado nunca é cinza', () => {
    for (const state of ['running', 'exited', 'created', 'paused', 'restarting', 'dead', 'x']) {
      expect(containerState(state).color).not.toMatch(/grey|gray|default/)
    }
    expect(containerState('exited')).toMatchObject({ label: 'Parado', color: 'warning' })
  })

  it('filtra por estado e busca, e conta cada filtro', () => {
    const items = [
      container({ id: '1', names: ['/api'], projectName: 'erp' }),
      container({ id: '2', names: ['/db'], state: 'exited', projectName: 'erp' }),
      container({ id: '3', names: ['/cache'], image: 'redis:7' }),
    ]
    expect(stateFilterCounts(items)).toEqual({ all: 3, running: 2, stopped: 1 })
    expect(filterContainers(items, '', 'stopped').map((item) => item.id)).toEqual(['2'])
    expect(filterContainers(items, 'redis', 'all').map((item) => item.id)).toEqual(['3'])
    expect(filterContainers(items, 'ERP', 'running').map((item) => item.id)).toEqual(['1'])
  })

  it('agrupa por projeto, avulsos no fim, com o consumo somado', () => {
    const items = [
      container({ id: '3', names: ['/cache'] }),
      container({ id: '2', names: ['/db'], projectName: 'erp', state: 'exited' }),
      container({ id: '1', names: ['/api'], projectName: 'erp' }),
    ]
    const metric = (cpu: number) =>
      ({ cpu: { usagePercent: cpu }, memory: { usageBytes: 100 } }) as DockerContainerMetrics
    const groups = groupContainers(items, (id) => (id === '3' ? undefined : metric(10)))
    expect(groups.map((group) => group.label)).toEqual(['erp', 'Containers avulsos'])
    expect(groups[0]?.containers.map(containerName)).toEqual(['api', 'db'])
    expect(groups[0]).toMatchObject({ running: 1, cpuPercent: 20, memoryBytes: 200 })
  })

  it('ações: a principal primeiro, nada destrutivo no container protegido', () => {
    const access = { lifecycle: true, update: true }
    const keys = (overrides: Partial<DockerContainerSummary>) =>
      containerActions(container(overrides), access).map((action) => action.key)
    expect(keys({})).toEqual(['stop', 'restart', 'update', 'remove'])
    expect(keys({ state: 'exited' })).toEqual(['start', 'restart', 'update', 'remove'])
    expect(keys({ protected: true })).toEqual([])
    expect(keys({ protected: true, state: 'exited' })).toEqual(['start'])
    expect(
      containerActions(container({}), { lifecycle: false, update: false }).map((a) => a.key)
    ).toEqual([])
  })
})
