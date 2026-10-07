import type { SystemBackupPlanResponse } from '@/bindings/SystemBackupPlanResponse'

/**
 * Planos de backup — o do NetMonitor e o de cada banco de dados têm a mesma
 * forma (destino, agenda, retenção, última execução), e tudo aqui serve aos
 * dois: frequência, retenção e o selo de "protegido".
 */

/** O que um plano tem em comum, venha do NetMonitor ou de um banco. */
export type BackupPlanView = Pick<
  SystemBackupPlanResponse,
  | 'storageDestinationId'
  | 'storageDestinationName'
  | 'backupEnabled'
  | 'backupIntervalHours'
  | 'backupRetention'
  | 'lastBackupAt'
  | 'lastBackupStatus'
  | 'lastBackupError'
  | 'nextBackupAt'
>

/** Frequências oferecidas nos formulários, em horas. */
export const BACKUP_INTERVALS = [
  { value: 6, title: 'A cada 6 horas' },
  { value: 12, title: 'A cada 12 horas' },
  { value: 24, title: 'Diariamente' },
  { value: 168, title: 'Semanalmente' },
]

/** "Diário", "a cada 6 h", "semanal" — o que cabe num chip. */
export function intervalLabel(hours: number): string {
  if (hours === 24) return 'Diário'
  if (hours === 168) return 'Semanal'
  if (hours % 24 === 0) return `A cada ${hours / 24} dias`
  return `A cada ${hours} h`
}

/** Itens do seletor, incluindo uma frequência gravada fora dos presets. */
export function intervalItems(current: number): Array<{ value: number; title: string }> {
  return BACKUP_INTERVALS.some((item) => item.value === current)
    ? BACKUP_INTERVALS
    : [...BACKUP_INTERVALS, { value: current, title: intervalLabel(current) }]
}

/** Regra do campo "Manter as últimas N cópias" (mesma faixa do backend). */
export function retentionRule(value: number | string): true | string {
  const n = Number(value)
  return (Number.isInteger(n) && n >= 1 && n <= 365) || 'Entre 1 e 365 cópias'
}

export interface BackupHealth {
  label: string
  color: 'success' | 'error' | 'warning' | 'info'
  icon: string
  /** Uma frase sobre o que fazer — vazia quando está tudo certo. */
  advice: string
}

/**
 * Está protegido? Responde a pergunta que o operador faz ao abrir a tela,
 * antes de ele precisar ler datas e agendas.
 */
export function backupHealth(plan: BackupPlanView): BackupHealth {
  if (plan.storageDestinationId == null) {
    return {
      label: 'Não protegido',
      color: 'warning',
      icon: 'mdi-shield-off-outline',
      advice: 'Escolha para onde vão as cópias.',
    }
  }
  if (plan.lastBackupStatus === 'failed') {
    return {
      label: 'Último backup falhou',
      color: 'error',
      icon: 'mdi-shield-alert-outline',
      advice: 'Veja o erro abaixo e tente de novo.',
    }
  }
  if (!plan.backupEnabled) {
    return {
      label: 'Só manual',
      color: 'info',
      icon: 'mdi-shield-half-full',
      advice: 'Ligue o backup automático para não depender de lembrar.',
    }
  }
  if (!plan.lastBackupAt) {
    return {
      label: 'Aguardando o primeiro backup',
      color: 'info',
      icon: 'mdi-shield-sync-outline',
      advice: '',
    }
  }
  return { label: 'Protegido', color: 'success', icon: 'mdi-shield-check', advice: '' }
}
