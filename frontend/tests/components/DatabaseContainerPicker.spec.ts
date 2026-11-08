import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import type { Pinia } from 'pinia'
import type { DatabaseContainer } from '@/bindings/DatabaseContainer'
import type { DatabaseContainerResolution } from '@/bindings/DatabaseContainerResolution'
import DatabaseContainerPicker from '@/components/databases/DatabaseContainerPicker.vue'
import { apiService } from '@/services/apiService'
import { useDatabasesStore } from '@/stores/databases'
import { bodyText, button, mountWith, setRole, setupDom, teardownDom } from './domHarness'

function container(overrides: Partial<DatabaseContainer>): DatabaseContainer {
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

let pinia: Pinia

beforeEach(() => {
  pinia = setupDom()
  setRole('admin')
})

afterEach(() => {
  vi.restoreAllMocks()
  teardownDom()
})

describe('bancos em contêineres no "Nova conexão"', () => {
  it('mostra só os do SGBD escolhido e testa o escolhido', async () => {
    const store = useDatabasesStore()
    store.containers = {
      containers: [
        container({}),
        container({ containerId: 'm', containerName: 'loja_mysql', engine: 'mysql' }),
      ],
      issues: [{ hostName: 'filial', reason: 'O agente não respondeu' }],
    }
    const resolution: DatabaseContainerResolution = {
      container: container({}),
      username: 'postgres',
      database: null,
      addresses: [],
      chosen: null,
      attempts: [],
    }
    const post = vi.spyOn(apiService, 'post').mockResolvedValue(resolution)
    const wrapper = mountWith(DatabaseContainerPicker, { engine: 'postgres' }, pinia)

    expect(bodyText()).toContain('projetox_postgres')
    expect(bodyText()).not.toContain('loja_mysql')
    expect(bodyText()).toContain('filial: O agente não respondeu')

    button('Usar')?.click()
    await flushPromises()
    expect(post).toHaveBeenCalledWith('/databases/containers/resolve', {
      hostKey: 'agent-1',
      containerId: 'abc',
    })
    expect(wrapper.emitted('picked')).toEqual([[resolution]])
  })

  it('parados ficam escondidos até o operador pedir', async () => {
    const store = useDatabasesStore()
    store.containers = {
      containers: [
        container({}),
        container({ containerId: 'old', containerName: 'legado_pg', running: false }),
      ],
      issues: [],
    }
    mountWith(DatabaseContainerPicker, { engine: 'postgres' }, pinia)
    expect(bodyText()).not.toContain('legado_pg')
    button('Mostrar os parados (1)')?.click()
    await flushPromises()
    expect(bodyText()).toContain('legado_pg')
  })

  it('sem contêiner do SGBD, manda preencher à mão', () => {
    mountWith(DatabaseContainerPicker, { engine: 'mariadb' }, pinia)
    expect(bodyText()).toContain('Nenhum contêiner de MariaDB nos agentes conectados')
  })
})
