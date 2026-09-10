import { beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import DeviceLogsTab from '@/components/devices/tabs/DeviceLogsTab.vue'
import { useLogsStore } from '@/stores/logs'
import type { Device } from '@/stores/devices'

vi.mock('@/services/apiService', () => ({
  apiService: {
    get: vi.fn().mockResolvedValue([]),
    post: vi.fn(),
    download: vi.fn().mockResolvedValue(new Blob(['log data'])),
  },
  ApiError: class extends Error {},
  NetworkError: class extends Error {},
}))

const mockDevice: Device = {
  id: 2,
  siteId: 1,
  name: 'Roteador-Borda',
  ipAddress: '192.168.1.1',
  type: 'router',
  status: 'up',
  vendor: 'MikroTik',
  model: 'CCR1009',
  operatingSystem: 'routeros',
  effectiveOperatingSystem: 'routeros',
  isSystem: false,
  createdAt: '2026-01-01T00:00:00Z',
  updatedAt: '2026-01-01T00:00:00Z',
}

describe('DeviceLogsTab.vue: botão de download de logs', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  it('exibe o botão Baixar logs e aciona downloadExport com os filtros da tela', async () => {
    const logsStore = useLogsStore()
    const downloadSpy = vi.spyOn(logsStore, 'downloadExport').mockResolvedValue(true)

    const wrapper = mount(DeviceLogsTab, {
      props: {
        deviceId: 2,
        device: mockDevice,
      },
      global: {
        stubs: {
          'v-card': { template: '<div><slot /></div>' },
          'v-card-text': { template: '<div><slot /></div>' },
          'v-alert': { template: '<div><slot /></div>' },
          'v-select': { template: '<div class="stub-select" />' },
          'v-text-field': { template: '<div class="stub-text-field" />' },
          'v-spacer': { template: '<div />' },
          'v-btn': {
            template: '<button><slot /></button>',
          },
          'v-icon': { template: '<span />' },
          LogTable: { template: '<div class="stub-log-table" />' },
          SyslogAutoSetupDialog: { template: '<div />' },
          RouterLink: { template: '<a><slot /></a>' },
        },
      },
    })

    const buttons = wrapper.findAll('button')
    const downloadBtn = buttons.find((btn) => btn.text().includes('Baixar logs'))
    expect(downloadBtn).toBeDefined()

    await downloadBtn?.trigger('click')

    expect(downloadSpy).toHaveBeenCalledTimes(1)
    expect(downloadSpy).toHaveBeenCalledWith(
      expect.objectContaining({
        deviceId: 2,
        hours: 24,
      }),
      'Roteador-Borda'
    )
  })
})
