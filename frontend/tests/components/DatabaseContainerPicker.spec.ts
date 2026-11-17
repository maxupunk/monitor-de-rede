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
    protectedConnectionId: null,
    protectedConnectionName: null,
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

  it('filtra pela busca e marca os contêineres que já estão na lista de backup', async () => {
    const store = useDatabasesStore()
    store.containers = {
      containers: [
        container({
          containerId: 'erp',
          containerName: 'erp_postgres',
          protectedConnectionId: 42,
          protectedConnectionName: 'ERP Matriz',
        }),
        container({
          containerId: 'crm',
          containerName: 'crm_postgres',
          project: 'vendas',
        }),
      ],
      issues: [],
    }
    const wrapper = mountWith(DatabaseContainerPicker, { engine: 'postgres' }, pinia)

    expect(bodyText()).toContain('1 já no backup')
    expect(bodyText()).toContain('Já no backup · ERP Matriz')

    // Filtra no campo de busca
    const input = document.body.querySelector<HTMLInputElement>(
      '[data-test="container-search"] input'
    )
    expect(input).not.toBeNull()
    input!.value = 'vendas'
    input!.dispatchEvent(new Event('input', { bubbles: true }))
    await flushPromises()
    expect(bodyText()).toContain('crm_postgres')
    expect(bodyText()).not.toContain('erp_postgres')

    // Limpa busca e clica em Editar no banco já protegido
    input!.value = ''
    input!.dispatchEvent(new Event('input', { bubbles: true }))
    await flushPromises()
    button('Editar')?.click()
    await flushPromises()
    expect(wrapper.emitted('editConnection')).toEqual([[42]])
  })
})
