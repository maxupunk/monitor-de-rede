import type { Area } from '@/bindings/Area'

export type UserRole = 'admin' | 'operator' | 'viewer'

export interface RoleOption {
  title: string
  value: UserRole
  description: string
  icon: string
}

export const ROLE_OPTIONS: RoleOption[] = [
  {
    title: 'Administrador',
    value: 'admin',
    description: 'Acesso total, incluindo usuários e perfis.',
    icon: 'mdi-shield-crown-outline',
  },
  {
    title: 'Operador',
    value: 'operator',
    description: 'Visualiza e altera o monitoramento, sem gerenciar usuários.',
    icon: 'mdi-account-cog-outline',
  },
  {
    title: 'Visualizador',
    value: 'viewer',
    description: 'Consulta dados e eventos sem realizar alterações.',
    icon: 'mdi-eye-outline',
  },
]

export function roleLabel(role?: string): string {
  return ROLE_OPTIONS.find((item) => item.value === role)?.title ?? 'Perfil desconhecido'
}

/** Papel atribuído por área (ADR 019). Área ausente: sem acesso. */
export type AreaRoles = Partial<Record<Area, UserRole>>

/**
 * O papel numa área — a mesma regra do backend (`AreaAccess::role_in`): o
 * administrador vale em todas; o Núcleo segue o papel geral; nas demais, o
 * papel atribuído a ela ou `null` (sem acesso).
 */
export function roleInArea(
  role: UserRole | undefined,
  areaRoles: AreaRoles | undefined,
  area: Area
): UserRole | null {
  if (role === 'admin') return 'admin'
  if (area === 'core') return role ?? null
  const assigned = areaRoles?.[area]
  return assigned === 'operator' || assigned === 'viewer' ? assigned : null
}

/** O papel muda algo? */
export function roleCanWrite(role: UserRole | null): boolean {
  return role === 'admin' || role === 'operator'
}

export function isUserRole(value: unknown): value is UserRole {
  return value === 'admin' || value === 'operator' || value === 'viewer'
}

const OPTIONAL_AREAS: readonly Area[] = ['network', 'servers', 'vpn']

/** Os papéis por área vindos da API ou do SSE, só com o que é válido. */
export function parseAreaRoles(value: unknown): AreaRoles {
  const result: AreaRoles = {}
  if (typeof value !== 'object' || value === null) return result
  for (const area of OPTIONAL_AREAS) {
    const role: unknown = Reflect.get(value, area)
    if (role === 'operator' || role === 'viewer') result[area] = role
  }
  return result
}
