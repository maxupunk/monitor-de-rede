import type { Area } from '@/bindings/Area'
import type { DatabaseUsageSection } from '@/bindings/DatabaseUsageSection'
import type { TableUsage } from '@/bindings/TableUsage'

/** Uma tabela com a fatia que ela ocupa do banco. */
export interface UsageRow extends TableUsage {
  /** Percentual do tamanho do banco (ou das linhas, sem o detalhe por tabela). */
  share: number | null
}

/** As tabelas de uma área (ADR 017), somadas. */
export interface UsageGroup {
  area: Area
  title: string
  totalBytes: number | null
  rows: number
  share: number | null
  tables: UsageRow[]
}

function totalRows(section: DatabaseUsageSection): number {
  return section.tables.reduce((sum, table) => sum + table.rows, 0)
}

/**
 * A fatia da tabela no banco: pelo tamanho quando o banco o informa; sem ele
 * (SQLite sem `dbstat`), pelas linhas — o melhor indício que sobra.
 */
export function shareOf(
  table: Pick<TableUsage, 'totalBytes' | 'rows'>,
  section: DatabaseUsageSection
): number | null {
  if (section.detailAvailable) {
    if (table.totalBytes == null || section.sizeBytes <= 0) return null
    return (table.totalBytes / section.sizeBytes) * 100
  }
  const rows = totalRows(section)
  return rows > 0 ? (table.rows / rows) * 100 : null
}

/** As tabelas da seção com a fatia de cada uma, na ordem do servidor (maior primeiro). */
export function usageRows(section: DatabaseUsageSection): UsageRow[] {
  return section.tables.map((table) => ({ ...table, share: shareOf(table, section) }))
}

/** As tabelas agrupadas pela área dona, da área que mais ocupa para a que menos ocupa. */
export function groupByArea(section: DatabaseUsageSection): UsageGroup[] {
  const groups = new Map<Area, UsageGroup>()
  for (const row of usageRows(section)) {
    const group = groups.get(row.area) ?? {
      area: row.area,
      title: row.areaTitle,
      totalBytes: section.detailAvailable ? 0 : null,
      rows: 0,
      share: null,
      tables: [],
    }
    group.tables.push(row)
    group.rows += row.rows
    if (group.totalBytes != null) group.totalBytes += row.totalBytes ?? 0
    groups.set(row.area, group)
  }
  const list = [...groups.values()]
  for (const group of list) {
    group.share = shareOf({ totalBytes: group.totalBytes, rows: group.rows }, section)
  }
  return list.sort((a, b) =>
    a.totalBytes != null && b.totalBytes != null ? b.totalBytes - a.totalBytes : b.rows - a.rows
  )
}

/**
 * Cor de cada área: as opcionais com a mesma da escolha das áreas no
 * assistente inicial; o Núcleo, que não aparece lá, com `secondary`.
 */
export const AREA_COLORS: Record<Area, string> = {
  core: 'secondary',
  network: 'primary',
  servers: 'info',
  vpn: 'deep-purple',
}
