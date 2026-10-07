import { describe, expect, it } from 'vitest'
import {
  DATABASE_ENGINES,
  databasesLabel,
  engineInfo,
  restoredDatabaseName,
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
