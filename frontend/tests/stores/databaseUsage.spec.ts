import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { DatabaseUsage } from '@/bindings/DatabaseUsage'
import type { DatabaseUsageSection } from '@/bindings/DatabaseUsageSection'
import type { TableUsage } from '@/bindings/TableUsage'
import { useDatabaseUsageStore } from '@/stores/databaseUsage'
import { groupByArea, shareOf, usageRows } from '@/utils/databaseUsage'

const getMock = vi.fn()
vi.mock('@/services/apiService', () => ({
  apiService: { get: (...args: unknown[]) => getMock(...args) },
}))

function table(overrides: Partial<TableUsage>): TableUsage {
  return {
    table: 'metrics',
    label: 'Métricas dos dispositivos',
    area: 'core',
    areaTitle: 'Núcleo',
    totalBytes: 600,
    dataBytes: 400,
    indexBytes: 200,
    rows: 10,
    rowsEstimated: false,
    retentionDays: 30,
    ...overrides,
  }
}

function section(overrides: Partial<DatabaseUsageSection> = {}): DatabaseUsageSection {
  return {
    kind: 'main',
    title: 'Banco principal',
    name: 'netmonitor',
    sizeBytes: 1000,
    otherBytes: 100,
    detailAvailable: true,
    tables: [
      table({}),
      table({ table: 'vpn_peers', area: 'vpn', areaTitle: 'VPN WireGuard', totalBytes: 200 }),
      table({ table: 'devices', label: 'Dispositivos', totalBytes: 100, retentionDays: null }),
    ],
    ...overrides,
  }
}

describe('databaseUsage store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    getMock.mockReset()
  })

  it('mede sob demanda, com prazo longo, e guarda a resposta', async () => {
    const usage: DatabaseUsage = { dbType: 'postgres', totalBytes: 1000, databases: [section()] }
    getMock.mockResolvedValueOnce(usage)
    const store = useDatabaseUsageStore()

    expect(getMock).not.toHaveBeenCalled()
    await store.fetchUsage()

    expect(getMock).toHaveBeenCalledWith('/settings/database-usage', { timeoutMs: 120_000 })
    expect(store.usage).toEqual(usage)
    expect(store.loading).toBe(false)
    expect(store.error).toBeNull()
  })

  it('mostra a falha sem perder a medição anterior', async () => {
    const usage: DatabaseUsage = { dbType: 'sqlite', totalBytes: 1000, databases: [section()] }
    getMock.mockResolvedValueOnce(usage).mockRejectedValueOnce(new Error('banco fora do ar'))
    const store = useDatabaseUsageStore()
    await store.fetchUsage()
    await store.fetchUsage()

    expect(store.error).toBe('banco fora do ar')
    expect(store.usage).toEqual(usage)
  })
})

describe('agrupamento do espaço ocupado', () => {
  it('fatia de cada tabela pelo tamanho do banco', () => {
    const rows = usageRows(section())
    expect(rows.map((row) => row.share)).toEqual([60, 20, 10])
  })

  it('agrupa por área, da que mais ocupa para a que menos ocupa', () => {
    const groups = groupByArea(section())
    expect(groups.map((g) => [g.area, g.totalBytes, g.share])).toEqual([
      ['core', 700, 70],
      ['vpn', 200, 20],
    ])
    expect(groups[0]?.tables.map((t) => t.table)).toEqual(['metrics', 'devices'])
  })

  it('sem o detalhe por tabela (SQLite sem dbstat), a fatia é pelas linhas', () => {
    const semDetalhe = section({
      detailAvailable: false,
      otherBytes: null,
      tables: [
        table({ totalBytes: null, dataBytes: null, indexBytes: null, rows: 30 }),
        table({ table: 'sites', totalBytes: null, dataBytes: null, indexBytes: null, rows: 10 }),
      ],
    })
    expect(usageRows(semDetalhe).map((row) => row.share)).toEqual([75, 25])
    const [grupo] = groupByArea(semDetalhe)
    expect(grupo?.totalBytes).toBeNull()
    expect(grupo?.rows).toBe(40)
    expect(shareOf({ totalBytes: null, rows: 0 }, section({ tables: [] }))).toBeNull()
  })
})
