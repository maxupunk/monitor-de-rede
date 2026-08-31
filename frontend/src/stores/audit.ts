import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { useInfiniteList } from '@/composables/useInfiniteList'
import { apiService } from '@/services/apiService'
import type { AuditLogResponse } from '@/bindings/AuditLogResponse'
import type { AuditLogListResponse } from '@/bindings/AuditLogListResponse'
import type { PaginationMeta } from '@/bindings/PaginationMeta'

export type { AuditLogResponse }

export interface AuditFilters {
  userId: number | null
  resourceType: string | null
  resourceId: number | null
  action: string | null
  from: string | null
  to: string | null
  search: string
}

export const ACTION_OPTIONS = [
  { value: 'create', label: 'Criação' },
  { value: 'update', label: 'Alteração' },
  { value: 'delete', label: 'Exclusão' },
  { value: 'login', label: 'Login' },
  { value: 'logout', label: 'Logout' },
] as const

export const RESOURCE_OPTIONS = [
  { value: 'device', label: 'Dispositivo' },
  { value: 'monitor', label: 'Monitor' },
  { value: 'site', label: 'Site' },
  { value: 'network', label: 'Rede' },
  { value: 'user', label: 'Usuário' },
  { value: 'probe', label: 'Probe' },
  { value: 'vpn_peer', label: 'Peer VPN' },
  { value: 'alert_rule', label: 'Regra de alerta' },
  { value: 'maintenance_window', label: 'Janela de manutenção' },
  { value: 'docker_container', label: 'Container Docker' },
  { value: 'docker_volume', label: 'Volume Docker' },
  { value: 'docker_network', label: 'Rede Docker' },
  { value: 'docker_image', label: 'Imagem Docker' },
] as const

export function defaultFilters(): AuditFilters {
  return {
    userId: null,
    resourceType: null,
    resourceId: null,
    action: null,
    from: null,
    to: null,
    search: '',
  }
}

export function actionLabel(action: string | null): string {
  if (!action) return '—'
  const found = ACTION_OPTIONS.find((opt) => opt.value === action)
  return found?.label ?? action
}

export function resourceLabel(type: string | null): string {
  if (!type) return '—'
  const found = RESOURCE_OPTIONS.find((opt) => opt.value === type)
  return found?.label ?? type
}

export function actionColor(action: string | null): string {
  switch (action) {
    case 'create':
      return 'success'
    case 'update':
      return 'warning'
    case 'delete':
      return 'error'
    case 'login':
      return 'primary'
    case 'logout':
      return 'grey'
    default:
      return 'grey'
  }
}

/**
 * Store da trilha de auditoria.
 *
 * A lista é paginada por número de página (`PaginationMeta`) porque a tabela de
 * auditoria não recebe inserções contínuas como a de syslog; portanto, o
 * deslocamento do `OFFSET` não é um problema e o total de registros é útil
 * para a navegação por páginas.
 */
export const useAuditStore = defineStore('audit', () => {
  const filters = ref<AuditFilters>(defaultFilters())
  const meta = ref<PaginationMeta | null>(null)
  const error = ref<string | null>(null)

  function endpoint(): string {
    const params = new URLSearchParams()
    if (filters.value.userId !== null) params.set('userId', String(filters.value.userId))
    if (filters.value.resourceType !== null) params.set('resourceType', filters.value.resourceType)
    if (filters.value.resourceId !== null)
      params.set('resourceId', String(filters.value.resourceId))
    if (filters.value.action !== null) params.set('action', filters.value.action)
    if (filters.value.from !== null) params.set('from', filters.value.from)
    if (filters.value.to !== null) params.set('to', filters.value.to)
    const termo = filters.value.search.trim()
    if (termo) params.set('q', termo)
    const query = params.toString()
    return query ? `/audit-logs?${query}` : '/audit-logs'
  }

  const list = useInfiniteList<AuditLogResponse>(endpoint, {
    limit: 20,
    label: 'os registros de auditoria',
  })

  const total = computed(() => meta.value?.total ?? 0)
  const isEmpty = computed(() => list.items.value.length === 0)

  async function fetchLogs(): Promise<boolean> {
    error.value = null
    try {
      const response = await apiService.get<AuditLogListResponse>(endpoint())
      list.items.value = response.data ?? []
      meta.value = response.meta ?? null
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Falha ao carregar a trilha de auditoria.'
      console.error('Erro ao carregar auditoria:', err)
      return false
    }
  }

  function applyFilters(next: Partial<AuditFilters> = {}): void {
    filters.value = { ...filters.value, ...next }
    list.reset()
    meta.value = null
    void fetchLogs()
  }

  function clearFilters(): void {
    filters.value = defaultFilters()
    list.reset()
    meta.value = null
    void fetchLogs()
  }

  function goToPage(page: number): void {
    if (!meta.value || page < 1 || page > meta.value.lastPage) return
    const params = new URLSearchParams(endpoint().split('?')[1] ?? '')
    params.set('page', String(page))
    const base = '/audit-logs'
    const query = params.toString()
    error.value = null
    void apiService
      .get<AuditLogListResponse>(query ? `${base}?${query}` : base)
      .then((response) => {
        list.items.value = response.data ?? []
        meta.value = response.meta ?? null
      })
      .catch((err: unknown) => {
        error.value =
          err instanceof Error ? err.message : 'Falha ao carregar a página de auditoria.'
        console.error('Erro ao carregar página de auditoria:', err)
      })
  }

  return {
    filters,
    entries: list.items,
    total,
    meta,
    error,
    isEmpty,
    scrollKey: list.scrollKey,
    applyFilters,
    clearFilters,
    fetchLogs,
    goToPage,
  }
})
