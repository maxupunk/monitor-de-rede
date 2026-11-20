import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import type { Pinia } from 'pinia'
import type { DatabaseUsage } from '@/bindings/DatabaseUsage'
import DatabaseUsageDialog from '@/components/settings/DatabaseUsageDialog.vue'
import { apiService } from '@/services/apiService'
import { bodyText, button, mountWith, setupDom, teardownDom } from './domHarness'

let pinia: Pinia

function usage(detailAvailable: boolean): DatabaseUsage {
  const bytes = (value: number) => (detailAvailable ? value : null)
  return {
    dbType: detailAvailable ? 'postgres' : 'sqlite',
    totalBytes: 3000,
    databases: [
      {
        kind: 'main',
        title: 'Banco principal',
        name: 'netmonitor',
        sizeBytes: 2048,
        otherBytes: detailAvailable ? 48 : null,
        detailAvailable,
        tables: [
          {
            table: 'metrics',
            label: 'Métricas dos dispositivos',
            area: 'core',
            areaTitle: 'Núcleo',
            totalBytes: bytes(1500),
            dataBytes: bytes(1000),
            indexBytes: bytes(500),
            rows: 1200,
            rowsEstimated: detailAvailable,
            retentionDays: 30,
          },
          {
            table: 'vpn_peers',
            label: 'Dispositivos da VPN',
            area: 'vpn',
            areaTitle: 'VPN WireGuard',
            totalBytes: bytes(500),
            dataBytes: bytes(400),
            indexBytes: bytes(100),
            rows: 3,
            rowsEstimated: false,
            retentionDays: null,
          },
        ],
      },
      {
        kind: 'logs',
        title: 'Banco de logs (Syslog)',
        name: 'netmonitor_logs',
        sizeBytes: 952,
        otherBytes: null,
        detailAvailable,
        tables: [],
      },
    ],
  }
}

describe('DatabaseUsageDialog', () => {
  beforeEach(() => {
    pinia = setupDom()
  })

  afterEach(teardownDom)

  it('ao abrir, mede e mostra cada tabela por área, com tamanho, fatia, linhas e retenção', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue(usage(true))
    mountWith(DatabaseUsageDialog, { modelValue: true }, pinia)
    await flushPromises()

    expect(get).toHaveBeenCalledWith('/settings/database-usage', { timeoutMs: 120_000 })
    const text = bodyText()
    expect(text).toContain('Banco principal')
    expect(text).toContain('Banco de logs (Syslog)')
    expect(text).toContain('Núcleo')
    expect(text).toContain('VPN WireGuard')
    expect(text).toContain('Métricas dos dispositivos')
    expect(text).toContain('≈ 1.200 linhas')
    expect(text).toContain('guarda 30 dias')
    expect(text).toContain('índices')
    expect(text).toContain('Outros')
    expect(text).not.toContain('dbstat')
  })

  it('SQLite sem dbstat: avisa e mostra as linhas, sem inventar tamanho', async () => {
    vi.spyOn(apiService, 'get').mockResolvedValue(usage(false))
    mountWith(DatabaseUsageDialog, { modelValue: true }, pinia)
    await flushPromises()

    const text = bodyText()
    expect(text).toContain('dbstat')
    expect(text).toContain('1.200 linhas')
    expect(text).not.toContain('índices')
  })

  it('atualizar mede de novo, e liberar espaço leva à limpeza que já existe', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue(usage(true))
    const wrapper = mountWith(DatabaseUsageDialog, { modelValue: true }, pinia)
    await flushPromises()

    button('Atualizar')?.click()
    await flushPromises()
    expect(get).toHaveBeenCalledTimes(2)

    button('Apagar Histórico')?.click()
    await flushPromises()
    expect(wrapper.emitted('clear-history')).toHaveLength(1)
  })

  it('fechado, não mede nada', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue(usage(true))
    mountWith(DatabaseUsageDialog, { modelValue: false }, pinia)
    await flushPromises()
    expect(get).not.toHaveBeenCalled()
  })
})
