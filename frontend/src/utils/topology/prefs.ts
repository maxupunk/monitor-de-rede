/**
 * Preferências de quem olha o mapa (site escolhido, filtros, painel aberto,
 * modo de edição). Só conveniência por navegador: tudo em try/catch e com
 * padrão seguro quando o armazenamento não está disponível. Posições NÃO
 * moram aqui — são do servidor, por site.
 */
import { EMPTY_FILTERS, type TopologyFilters } from './filters'
import type { LinkKind, NodeStatusKey } from './presentation'

export interface TopologyPrefs {
  siteId: number | null
  filters: TopologyFilters
  legendOpen: boolean
  editMode: boolean
  groupBySite: boolean
  /** Mostra o tráfego de todos os enlaces, não só de perto. */
  alwaysDetail: boolean
}

export const DEFAULT_PREFS: TopologyPrefs = {
  siteId: null,
  filters: EMPTY_FILTERS,
  legendOpen: false,
  editMode: false,
  groupBySite: false,
  alwaysDetail: false,
}

const KEY = 'netmonitor.topology.prefs.v2'
/** Posições antigas (antes do layout por site no servidor). */
const LEGACY_POSITIONS_KEY = 'netmonitor_topology_positions_v1'

const STATUSES: NodeStatusKey[] = ['online', 'offline', 'warning', 'unknown']
const KINDS: LinkKind[] = ['ethernet', 'fiber', 'wireless', 'vpn', 'inferred', 'parent']

function stringList<T extends string>(value: unknown, allowed?: readonly T[]): T[] {
  if (!Array.isArray(value)) return []
  return value.filter(
    (item): item is T => typeof item === 'string' && (!allowed || allowed.includes(item as T))
  )
}

export function parsePrefs(raw: string | null): TopologyPrefs {
  if (!raw) return { ...DEFAULT_PREFS }
  try {
    const data: unknown = JSON.parse(raw)
    if (typeof data !== 'object' || data === null) return { ...DEFAULT_PREFS }
    const record = data as Record<string, unknown>
    const filters = (record.filters ?? {}) as Record<string, unknown>
    return {
      siteId: typeof record.siteId === 'number' ? record.siteId : null,
      filters: {
        statuses: stringList(filters.statuses, STATUSES),
        categories: stringList(filters.categories),
        linkKinds: stringList(filters.linkKinds, KINDS),
        problemsOnly: filters.problemsOnly === true,
        mode: filters.mode === 'hide' ? 'hide' : 'dim',
      },
      legendOpen: record.legendOpen === true,
      editMode: record.editMode === true,
      groupBySite: record.groupBySite === true,
      alwaysDetail: record.alwaysDetail === true,
    }
  } catch {
    return { ...DEFAULT_PREFS }
  }
}

export function loadPrefs(): TopologyPrefs {
  try {
    // A versão antiga guardava posições aqui; elas agora são do servidor.
    localStorage.removeItem(LEGACY_POSITIONS_KEY)
    return parsePrefs(localStorage.getItem(KEY))
  } catch {
    return { ...DEFAULT_PREFS }
  }
}

export function savePrefs(prefs: TopologyPrefs): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(prefs))
  } catch {
    // Sem armazenamento (aba privada, cota): a preferência vale só nesta visita.
  }
}
