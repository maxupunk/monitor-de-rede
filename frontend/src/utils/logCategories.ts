import type { LogTemplateInfo } from '@/bindings/LogTemplateInfo'

/**
 * Categorias de evento de log — espelho de
 * `backend/src/services/ai/laya/decisions/log_category.rs` (`LOG_CATEGORIES`).
 * Rótulo, ícone e cor ficam aqui, e não nos componentes.
 */
export const LOG_CATEGORY_OPTIONS = [
  { value: 'auth_failure', title: 'Falha de login', icon: 'mdi-account-lock', color: 'warning' },
  { value: 'link_change', title: 'Link / interface', icon: 'mdi-lan-disconnect', color: 'error' },
  { value: 'reboot', title: 'Reinício', icon: 'mdi-restart-alert', color: 'error' },
  { value: 'routing', title: 'Roteamento', icon: 'mdi-routes', color: 'info' },
  { value: 'config_change', title: 'Configuração', icon: 'mdi-cog-sync', color: 'secondary' },
  { value: 'dhcp', title: 'DHCP', icon: 'mdi-ip-network', color: 'info' },
  {
    value: 'resource_exhaustion',
    title: 'Recurso esgotado',
    icon: 'mdi-gauge-full',
    color: 'error',
  },
  { value: 'security', title: 'Segurança', icon: 'mdi-shield-alert', color: 'error' },
  { value: 'hardware', title: 'Hardware', icon: 'mdi-chip', color: 'warning' },
  { value: 'informational', title: 'Informativo', icon: 'mdi-information', color: 'primary' },
] as const

export type LogCategoryOption = (typeof LOG_CATEGORY_OPTIONS)[number]

export function logCategoryInfo(id?: string | null): LogCategoryOption | null {
  return LOG_CATEGORY_OPTIONS.find((option) => option.value === id) ?? null
}

/** O payload do evento `logs:template_classified` é um padrão completo? */
export function isLogTemplateInfo(value: unknown): value is LogTemplateInfo {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Record<string, unknown>
  return (
    typeof candidate.templateHash === 'string' &&
    typeof candidate.template === 'string' &&
    typeof candidate.confirmed === 'boolean'
  )
}
