/**
 * Como o deploy contínuo aparece na tela (ADR 021): rótulo, cor e ícone de
 * cada estado e etapa, e o que cada estado permite fazer. Puro — testável
 * sem montar componente, e o mesmo em todas as telas.
 */
import type { DeployStage } from '@/bindings/DeployStage'
import type { DeployReadinessLevel } from '@/bindings/DeployReadinessLevel'
import type { DeployRunStatus } from '@/bindings/DeployRunStatus'
import type { DeployRunTrigger } from '@/bindings/DeployRunTrigger'

export interface StatusMeta {
  label: string
  color: string
  icon: string
  /** Ainda vai mudar sozinho (a tela mostra o indicador de "ao vivo"). */
  live: boolean
}

export const RUN_STATUS: Record<DeployRunStatus, StatusMeta> = {
  awaiting_approval: {
    label: 'Aguarda aprovação',
    color: 'warning',
    icon: 'mdi-account-clock',
    live: false,
  },
  scheduled: { label: 'Agendado', color: 'info', icon: 'mdi-calendar-clock', live: false },
  queued: { label: 'Na fila', color: 'info', icon: 'mdi-timer-sand', live: true },
  running: { label: 'Publicando', color: 'primary', icon: 'mdi-rocket-launch', live: true },
  succeeded: { label: 'No ar', color: 'success', icon: 'mdi-check-circle', live: false },
  failed: { label: 'Falhou', color: 'error', icon: 'mdi-alert-circle', live: false },
  rolled_back: { label: 'Revertido', color: 'warning', icon: 'mdi-backup-restore', live: false },
  superseded: {
    label: 'Substituído',
    color: 'secondary',
    icon: 'mdi-skip-next-circle',
    live: false,
  },
  cancelled: { label: 'Cancelado', color: 'secondary', icon: 'mdi-cancel', live: false },
  rejected: { label: 'Recusado', color: 'error', icon: 'mdi-close-circle', live: false },
}

export function runStatusMeta(status: DeployRunStatus): StatusMeta {
  return RUN_STATUS[status]
}

/** Ainda não começou: dá para cancelar. */
export function isPending(status: DeployRunStatus): boolean {
  return status === 'awaiting_approval' || status === 'scheduled' || status === 'queued'
}

/** Terminou (bem ou mal). */
export function isFinished(status: DeployRunStatus): boolean {
  return !isPending(status) && status !== 'running'
}

export const TRIGGER_LABEL: Record<DeployRunTrigger, string> = {
  webhook: 'GitHub',
  manual: 'Manual',
  rollback: 'Restauração',
}

export interface StageMeta {
  value: DeployStage
  label: string
  icon: string
}

/** As etapas na ordem em que acontecem (o rollback só aparece quando há). */
export const DEPLOY_STAGES: StageMeta[] = [
  { value: 'backup', label: 'Backup do banco', icon: 'mdi-database-export' },
  { value: 'sync', label: 'Repositório', icon: 'mdi-source-branch-sync' },
  { value: 'env', label: 'Variáveis', icon: 'mdi-file-key' },
  { value: 'pull', label: 'Baixar imagens', icon: 'mdi-download' },
  { value: 'start', label: 'Subir e conferir saúde', icon: 'mdi-heart-pulse' },
  { value: 'prune', label: 'Limpar imagens antigas', icon: 'mdi-broom' },
  { value: 'rollback', label: 'Voltar versão anterior', icon: 'mdi-backup-restore' },
]

export type StageState = 'done' | 'current' | 'todo' | 'failed'

/**
 * Estado de cada etapa a partir da etapa em curso e do fim do deploy. Etapas
 * puladas (sem backup configurado, `.env` mantido) aparecem como feitas: o
 * que importa para quem olha é "passou daqui".
 */
export function stageStates(
  stage: DeployStage | null,
  status: DeployRunStatus
): Record<DeployStage, StageState> {
  const order = DEPLOY_STAGES.filter((item) => item.value !== 'rollback').map((item) => item.value)
  const result = Object.fromEntries(
    DEPLOY_STAGES.map((item) => [item.value, 'todo' as StageState])
  ) as Record<DeployStage, StageState>
  if (status === 'succeeded') {
    for (const value of order) result[value] = 'done'
    return result
  }
  if (status === 'rolled_back') {
    for (const value of order) result[value] = 'done'
    result.start = 'failed'
    result.prune = 'todo'
    result.rollback = 'done'
    return result
  }
  if (!stage) return result
  if (stage === 'rollback') {
    for (const value of order) result[value] = 'done'
    result.start = 'failed'
    result.prune = 'todo'
    result.rollback = status === 'failed' ? 'failed' : 'current'
    return result
  }
  const current = order.indexOf(stage)
  order.forEach((value, index) => {
    if (index < current) result[value] = 'done'
    else if (index === current) result[value] = status === 'failed' ? 'failed' : 'current'
  })
  return result
}

export const READINESS_META: Record<DeployReadinessLevel, { color: string; icon: string }> = {
  ok: { color: 'success', icon: 'mdi-check-circle' },
  warning: { color: 'warning', icon: 'mdi-alert' },
  error: { color: 'error', icon: 'mdi-close-circle' },
}

/** `https://github.com/dono/repo/commit/sha`. */
export function commitUrl(repository: string, sha: string): string {
  return `https://github.com/${repository}/commit/${sha}`
}

/** A primeira linha da mensagem do commit. */
export function commitTitle(message: string | null | undefined): string {
  return (message ?? '').split('\n')[0]?.trim() ?? ''
}

/** "23:00–05:00" ou vazio quando não há janela. */
export function windowLabel(start: string | null, end: string | null): string {
  return start && end ? `${start}–${end}` : ''
}
