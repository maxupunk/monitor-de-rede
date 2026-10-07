import type { DatabaseEngine } from '@/bindings/DatabaseEngine'
import type { SslMode } from '@/bindings/SslMode'

export interface DatabaseEngineInfo {
  value: DatabaseEngine
  label: string
  hint: string
  icon: string
  color: string
  defaultPort: number
  /** Protocolo: um backup só restaura num SGBD da mesma família. */
  family: 'postgres' | 'mysql'
}

/** Os SGBDs suportados, na ordem da escolha. Único lugar com ícone e cor. */
export const DATABASE_ENGINES: DatabaseEngineInfo[] = [
  {
    value: 'postgres',
    label: 'PostgreSQL',
    hint: 'Versão 12 ou mais nova',
    icon: 'mdi-elephant',
    color: 'info',
    defaultPort: 5432,
    family: 'postgres',
  },
  {
    value: 'mysql',
    label: 'MySQL',
    hint: 'Versão 5.7 ou mais nova',
    icon: 'mdi-dolphin',
    color: 'warning',
    defaultPort: 3306,
    family: 'mysql',
  },
  {
    value: 'mariadb',
    label: 'MariaDB',
    hint: 'Versão 10.3 ou mais nova',
    icon: 'mdi-database-outline',
    color: 'secondary',
    defaultPort: 3306,
    family: 'mysql',
  },
]

export function engineInfo(engine: DatabaseEngine): DatabaseEngineInfo {
  return DATABASE_ENGINES.find((item) => item.value === engine) ?? DATABASE_ENGINES[0]!
}

export const SSL_MODES: Array<{ value: SslMode; title: string; subtitle: string }> = [
  { value: 'prefer', title: 'Usar se disponível', subtitle: 'TLS quando o servidor oferece' },
  { value: 'require', title: 'Obrigatório', subtitle: 'Falha se o servidor não tiver TLS' },
  { value: 'disable', title: 'Desligado', subtitle: 'Só em rede interna confiável' },
]

/** Os bancos de uma conexão num texto curto. */
export function databasesLabel(databases: string[]): string {
  if (databases.length === 0) return 'Todos os bancos'
  if (databases.length <= 3) return databases.join(', ')
  return `${databases.slice(0, 2).join(', ')} e mais ${databases.length - 2}`
}

/**
 * Nome sugerido para restaurar sem sobrescrever: `vendas_restaurado_20261007`.
 * Só letras, números e `_`, que é o que o backend aceita num nome novo.
 */
export function restoredDatabaseName(source: string, date = new Date()): string {
  const base = source.replace(/[^A-Za-z0-9_]/g, '_').replace(/^[^A-Za-z_]+/, '') || 'banco'
  const stamp = date.toISOString().slice(0, 10).replace(/-/g, '')
  return `${base}_restaurado_${stamp}`.slice(0, 63)
}
