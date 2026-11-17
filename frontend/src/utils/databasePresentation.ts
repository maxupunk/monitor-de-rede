import type { DatabaseConnectionResponse } from '@/bindings/DatabaseConnectionResponse'
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

/** "pelo agente filial-sp" quando a conexão passa por uma ponte; vazio se direta. */
export function routeLabel(
  connection: Pick<DatabaseConnectionResponse, 'viaProbeId' | 'viaProbeName'>
): string {
  if (connection.viaProbeId == null) return ''
  return `pelo agente ${connection.viaProbeName ?? '#' + connection.viaProbeId}`
}

export interface AccessOriginInfo {
  key: string
  /** Rótulo completo para o campo "Acessar a partir de". */
  label: string
  /** Rótulo enxuto para chips e agrupamentos. */
  shortLabel: string
  icon: string
  color: 'primary' | 'info'
  isAgent: boolean
}

/** Como a tela identifica "Acessar a partir de" de cada banco de dados. */
export function accessOriginInfo(
  connection: Pick<DatabaseConnectionResponse, 'viaProbeId' | 'viaProbeName'>
): AccessOriginInfo {
  if (connection.viaProbeId == null) {
    return {
      key: 'central',
      label: 'Esta central (direto)',
      shortLabel: 'Esta central',
      icon: 'mdi-home-variant-outline',
      color: 'primary',
      isAgent: false,
    }
  }
  const agentName = connection.viaProbeName ?? `#${connection.viaProbeId}`
  return {
    key: `agent-${connection.viaProbeId}`,
    label: `Agente ${agentName}`,
    shortLabel: agentName,
    icon: 'mdi-transit-connection-variant',
    color: 'info',
    isAgent: true,
  }
}

export type ConnectionHealthFilter = 'all' | 'healthy' | 'failed' | 'unprotected'

export interface ConnectionFilters {
  search?: string
  originKey?: string | null
  health?: ConnectionHealthFilter
}

export function normalizeSearchText(value: string): string {
  return value
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .trim()
}

export function matchesConnectionHealth(
  connection: Pick<
    DatabaseConnectionResponse,
    'storageDestinationId' | 'lastBackupStatus' | 'backupEnabled' | 'lastBackupAt'
  >,
  health: ConnectionHealthFilter
): boolean {
  if (health === 'all') return true
  if (health === 'unprotected') return connection.storageDestinationId == null
  if (health === 'failed') return connection.lastBackupStatus === 'failed'
  return (
    connection.storageDestinationId != null &&
    connection.lastBackupStatus !== 'failed' &&
    connection.backupEnabled
  )
}

/** Filtra conexões de banco por busca textual, origem ("Acessar a partir de") e saúde. */
export function filterConnections(
  connections: DatabaseConnectionResponse[],
  filters: ConnectionFilters
): DatabaseConnectionResponse[] {
  const query = normalizeSearchText(filters.search ?? '')
  const originKey = filters.originKey ?? null
  const health = filters.health ?? 'all'

  return connections.filter((conn) => {
    if (originKey && accessOriginInfo(conn).key !== originKey) return false
    if (!matchesConnectionHealth(conn, health)) return false
    if (!query) return true

    const origin = accessOriginInfo(conn)
    const haystack = normalizeSearchText(
      [
        conn.name,
        conn.host,
        String(conn.port),
        conn.username,
        engineInfo(conn.engine).label,
        conn.storageDestinationName ?? '',
        origin.label,
        origin.shortLabel,
        ...conn.databases,
      ].join(' ')
    )
    return haystack.includes(query)
  })
}

export interface ConnectionOriginGroup {
  origin: AccessOriginInfo
  connections: DatabaseConnectionResponse[]
}

/** Agrupa conexões por "Acessar a partir de": central primeiro, depois cada agente por nome. */
export function groupConnectionsByOrigin(
  connections: DatabaseConnectionResponse[]
): ConnectionOriginGroup[] {
  const map = new Map<string, ConnectionOriginGroup>()
  for (const conn of connections) {
    const origin = accessOriginInfo(conn)
    const existing = map.get(origin.key)
    if (existing) {
      existing.connections.push(conn)
    } else {
      map.set(origin.key, { origin, connections: [conn] })
    }
  }
  return [...map.values()].sort((a, b) => {
    if (a.origin.isAgent !== b.origin.isAgent) return a.origin.isAgent ? 1 : -1
    return a.origin.shortLabel.localeCompare(b.origin.shortLabel)
  })
}

/** "PostgreSQL em 10.0.0.20:5432 · erp, rh" — o resumo curto do cartão. */
export function connectionSummary(connection: DatabaseConnectionResponse): string {
  const where = [connection.host, routeLabel(connection)].filter(Boolean).join(' ')
  return `${engineInfo(connection.engine).label} em ${where} · ${databasesLabel(connection.databases)}`
}

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
