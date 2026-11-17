import { describe, expect, it } from 'vitest'
import {
  accessOriginInfo,
  connectionSummary,
  DATABASE_ENGINES,
  databasesLabel,
  engineInfo,
  filterConnections,
  groupConnectionsByOrigin,
  restoredDatabaseName,
  routeLabel,
} from '@/utils/databasePresentation'

/** A mesma regra de `validate_database_name` no backend. */
const BACKEND_NAME = /^[A-Za-z_][A-Za-z0-9_-]{0,62}$/

describe('apresentação dos bancos', () => {
  it('o nome sugerido para restaurar é aceito pelo backend', () => {
    const date = new Date('2026-10-07T12:00:00Z')
    expect(restoredDatabaseName('vendas', date)).toBe('vendas_restaurado_20261007')
    for (const source of ['Estoque Central', '2026-dados', 'açaí', 'x'.repeat(80)]) {
      expect(restoredDatabaseName(source, date)).toMatch(BACKEND_NAME)
    }
  })

  it('MariaDB e MySQL são da mesma família; PostgreSQL não', () => {
    expect(engineInfo('mariadb').family).toBe(engineInfo('mysql').family)
    expect(engineInfo('postgres').family).not.toBe(engineInfo('mysql').family)
    expect(DATABASE_ENGINES.every((item) => item.color !== 'default')).toBe(true)
  })

  it('resume a lista de bancos', () => {
    expect(databasesLabel([])).toBe('Todos os bancos')
    expect(databasesLabel(['a', 'b'])).toBe('a, b')
    expect(databasesLabel(['a', 'b', 'c', 'd', 'e'])).toBe('a, b e mais 3')
  })
})

describe('rota da conexão', () => {
  const base = {
    id: 1,
    name: 'ERP',
    engine: 'postgres' as const,
    host: '10.0.0.20',
    port: 5432,
    username: 'postgres',
    passwordSet: true,
    sslMode: 'prefer' as const,
    databases: ['erp', 'rh'],
    storageDestinationId: null,
    storageDestinationName: null,
    viaProbeId: null,
    viaProbeName: null,
    backupEnabled: false,
    backupIntervalHours: 24,
    backupRetention: 7,
    lastBackupAt: null,
    lastBackupStatus: null,
    lastBackupError: null,
    nextBackupAt: null,
    createdAt: '',
    updatedAt: '',
  }

  it('conexão direta não fala de agente', () => {
    expect(routeLabel(base)).toBe('')
    expect(connectionSummary(base)).toBe('PostgreSQL em 10.0.0.20 · erp, rh')
  })

  it('conexão pela ponte diz por qual agente passa', () => {
    const viaAgent = { ...base, viaProbeId: 4, viaProbeName: 'filial-sp' }
    expect(connectionSummary(viaAgent)).toBe(
      'PostgreSQL em 10.0.0.20 pelo agente filial-sp · erp, rh'
    )
    expect(routeLabel({ viaProbeId: 9, viaProbeName: null })).toBe('pelo agente #9')
  })

  it('descreve, filtra e agrupa por "Acessar a partir de"', () => {
    const central = { ...base, id: 1, name: 'ERP Matriz', storageDestinationId: 2 }
    const filialA = {
      ...base,
      id: 2,
      name: 'Loja Sul',
      viaProbeId: 4,
      viaProbeName: 'srv-sul',
      lastBackupStatus: 'failed',
      storageDestinationId: 2,
    }
    const filialB = {
      ...base,
      id: 3,
      name: 'Loja Norte',
      viaProbeId: 5,
      viaProbeName: 'srv-norte',
      storageDestinationId: null,
    }

    expect(accessOriginInfo(central)).toMatchObject({
      key: 'central',
      label: 'Esta central (direto)',
      isAgent: false,
    })
    expect(accessOriginInfo(filialA)).toMatchObject({
      key: 'agent-4',
      label: 'Agente srv-sul',
      isAgent: true,
    })

    const groups = groupConnectionsByOrigin([filialB, central, filialA])
    expect(groups.map((g) => g.origin.key)).toEqual(['central', 'agent-5', 'agent-4'])

    expect(
      filterConnections([central, filialA, filialB], { originKey: 'agent-4' }).map((c) => c.id)
    ).toEqual([2])
    expect(
      filterConnections([central, filialA, filialB], { health: 'failed' }).map((c) => c.id)
    ).toEqual([2])
    expect(
      filterConnections([central, filialA, filialB], { health: 'unprotected' }).map((c) => c.id)
    ).toEqual([3])
    expect(
      filterConnections([central, filialA, filialB], { search: 'norte' }).map((c) => c.id)
    ).toEqual([3])
  })
})
