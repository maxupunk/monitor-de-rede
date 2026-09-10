import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import DatabaseInfoCard from '@/components/settings/DatabaseInfoCard.vue'
import { apiService } from '@/services/apiService'

vi.mock('@/services/apiService', () => ({
  apiService: {
    get: vi.fn(),
    post: vi.fn(),
  },
}))

describe('DatabaseInfoCard.vue', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  it('carrega e renderiza o tamanho, tipo e datas limites do banco de dados', async () => {
    vi.mocked(apiService.get).mockResolvedValueOnce({
      dbType: 'sqlite',
      sizeBytes: 10485760n,
      earliestRecord: '2026-01-15T10:00:00Z',
      latestRecord: '2026-09-10T16:00:00Z',
    })

    const wrapper = mount(DatabaseInfoCard, {
      global: {
        mocks: {
          $vuetify: {
            display: { xs: false },
          },
        },
        stubs: {
          'v-card': { template: '<div><slot /></div>' },
          'v-card-title': { template: '<div><slot /></div>' },
          'v-card-text': { template: '<div><slot /></div>' },
          'v-card-actions': { template: '<div><slot /></div>' },
          'v-row': { template: '<div><slot /></div>' },
          'v-col': { template: '<div><slot /></div>' },
          'v-alert': { template: '<div class="stub-alert"><slot /></div>' },
          'v-btn': { template: '<button><slot /></button>' },
          'v-icon': { template: '<span />' },
          'v-dialog': { template: '<div class="stub-dialog"><slot /></div>' },
        },
      },
    })

    await flushPromises()

    expect(apiService.get).toHaveBeenCalledWith('/settings/database-size')
    expect(wrapper.text()).toContain('10 MiB')
    expect(wrapper.text()).toContain('SQLite')
    expect(wrapper.text()).toContain('Primeiro Registro')
    expect(wrapper.text()).toContain('Último Registro')
  })

  it('aciona clearHistory e exibe feedback de sucesso', async () => {
    vi.mocked(apiService.get)
      .mockResolvedValueOnce({
        dbType: 'sqlite',
        sizeBytes: 10485760n,
        earliestRecord: '2026-01-15T10:00:00Z',
        latestRecord: '2026-09-10T16:00:00Z',
      })
      .mockResolvedValueOnce({
        dbType: 'sqlite',
        sizeBytes: 4096000n,
        earliestRecord: null,
        latestRecord: null,
      })

    vi.mocked(apiService.post).mockResolvedValueOnce({
      metricsDeleted: 1500n,
      resultsDeleted: 500n,
      logsDeleted: 200n,
      alertsDeleted: 10n,
      totalDeleted: 2210n,
    })

    const wrapper = mount(DatabaseInfoCard, {
      global: {
        mocks: {
          $vuetify: {
            display: { xs: false },
          },
        },
        stubs: {
          'v-card': { template: '<div><slot /></div>' },
          'v-card-title': { template: '<div><slot /></div>' },
          'v-card-text': { template: '<div><slot /></div>' },
          'v-card-actions': { template: '<div><slot /></div>' },
          'v-row': { template: '<div><slot /></div>' },
          'v-col': { template: '<div><slot /></div>' },
          'v-alert': { template: '<div class="stub-alert"><slot /></div>' },
          'v-btn': { template: '<button><slot /></button>' },
          'v-icon': { template: '<span />' },
          'v-dialog': { template: '<div class="stub-dialog"><slot /></div>' },
        },
      },
    })

    await flushPromises()

    const buttons = wrapper.findAll('button')
    const confirmBtn = buttons.find((btn) => btn.text().includes('Confirmar e Apagar'))
    expect(confirmBtn).toBeDefined()

    await confirmBtn?.trigger('click')
    await flushPromises()

    expect(apiService.post).toHaveBeenCalledWith('/settings/clear-history', {})
    expect(apiService.get).toHaveBeenCalledTimes(2)
    expect(wrapper.text()).toContain('Histórico apagado com sucesso!')
    expect(wrapper.text()).toContain('2.210')
  })
})
