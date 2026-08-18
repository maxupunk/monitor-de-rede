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
