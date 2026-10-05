import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { apiService } from '@/services/apiService'
import { macDigits, useVendorsStore } from '@/stores/vendors'

const espressif = {
  vendor: 'Espressif',
  organization: 'Espressif Inc.',
  source: 'ieee',
  locallyAdministered: false,
  hint: { deviceType: 'iot', confidence: 60, reasons: ['Fabricante menciona "espressif"'] },
}

describe('vendors store', () => {
  beforeEach(() => setActivePinia(createPinia()))
  afterEach(() => vi.restoreAllMocks())

  it('normaliza o MAC em dígitos', () => {
    expect(macDigits('5C:CF:7F-00.11 22')).toBe('5ccf7f001122')
  })

  it('não consulta MAC curto e só pergunta uma vez pelo mesmo endereço', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue(espressif)
    const vendors = useVendorsStore()

    expect(await vendors.lookup('5c:cf')).toBeNull()
    expect(get).not.toHaveBeenCalled()

    expect(await vendors.lookup('5C:CF:7F:00:11:22')).toEqual(espressif)
    expect(await vendors.lookup('5c-cf-7f-00-11-22')).toEqual(espressif)
    expect(get).toHaveBeenCalledOnce()
    expect(get).toHaveBeenCalledWith('/vendors/lookup?mac=5ccf7f001122')
  })

  it('atualizar limpa o cache e recarrega a situação', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue(espressif)
    vi.spyOn(apiService, 'post').mockResolvedValue({
      entries: 3,
      byRegistry: { 'MA-L': 3 },
      updatedAt: '2026-10-04T00:00:00Z',
    })
    const vendors = useVendorsStore()
    await vendors.lookup('5ccf7f001122')
    await vendors.refresh()
    await vendors.lookup('5ccf7f001122')
    // 1ª consulta, situação depois de atualizar, 2ª consulta (cache limpo).
    expect(get).toHaveBeenCalledTimes(3)
  })
})
