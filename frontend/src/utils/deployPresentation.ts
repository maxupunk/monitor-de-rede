/**
 * Como o deploy contínuo aparece na tela (ADR 021): rótulo, cor e ícone de
 * cada estado e etapa, e o que cada estado permite fazer. Puro — testável
 * sem montar componente, e o mesmo em todas as telas.
 */
import type { DeployBuildView } from '@/bindings/DeployBuildView'
import type { DeployStage } from '@/bindings/DeployStage'
import type { DeployReadinessLevel } from '@/bindings/DeployReadinessLevel'
import type { DeployRunStatus } from '@/bindings/DeployRunStatus'
import type { DeployRunTrigger } from '@/bindings/DeployRunTrigger'
import type { DeployWorkflowState } from '@/bindings/DeployWorkflowState'
import type { BuildStage } from '@/bindings/BuildStage'
import type { DeployBuildStatus } from '@/bindings/DeployBuildStatus'
import type { DeployBuilder } from '@/bindings/DeployBuilder'
import type { DeployVulnerabilityPolicy } from '@/bindings/DeployVulnerabilityPolicy'

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
  { value: 'image', label: 'Conferir e enviar imagem', icon: 'mdi-shield-check' },
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

/** Como o `deploy.yml` do repositório está em relação ao gerado. */
export const WORKFLOW_STATE: Record<DeployWorkflowState, Omit<StatusMeta, 'live'>> = {
  current: { label: 'No repositório e atualizado', color: 'success', icon: 'mdi-check-circle' },
  different: {
    label: 'No repositório, diferente do gerado',
    color: 'warning',
    icon: 'mdi-file-compare',
  },
  missing: { label: 'Ainda não está no repositório', color: 'error', icon: 'mdi-file-hidden' },
  unknown: { label: 'Não deu para conferir', color: 'warning', icon: 'mdi-help-circle' },
}

/** O último build no GitHub Actions, em palavras e cor. */
export function buildMeta(build: Pick<DeployBuildView, 'status' | 'conclusion'>): StatusMeta {
  if (build.status !== 'completed') {
    return { label: 'Compilando', color: 'info', icon: 'mdi-progress-clock', live: true }
  }
  switch (build.conclusion) {
    case 'success':
      return { label: 'Passou', color: 'success', icon: 'mdi-check-circle', live: false }
    case 'failure':
    case 'timed_out':
    case 'startup_failure':
      return { label: 'Falhou', color: 'error', icon: 'mdi-alert-circle', live: false }
    case 'cancelled':
    case 'skipped':
      return { label: 'Cancelado', color: 'secondary', icon: 'mdi-cancel', live: false }
    default:
      return { label: 'Terminou', color: 'info', icon: 'mdi-information', live: false }
  }
}

// ── Compilação no servidor (ADR 023) ────────────────────────────────────

export const BUILD_STATUS: Record<DeployBuildStatus, StatusMeta> = {
  queued: { label: 'Na fila', color: 'info', icon: 'mdi-timer-sand', live: true },
  running: { label: 'Compilando', color: 'primary', icon: 'mdi-hammer-wrench', live: true },
  succeeded: { label: 'Compilada', color: 'success', icon: 'mdi-check-circle', live: false },
  failed: { label: 'Falhou', color: 'error', icon: 'mdi-alert-circle', live: false },
  cancelled: { label: 'Cancelada', color: 'secondary', icon: 'mdi-cancel', live: false },
  superseded: {
    label: 'Substituída',
    color: 'secondary',
    icon: 'mdi-skip-next-circle',
    live: false,
  },
}

export interface BuildStageMeta {
  value: BuildStage
  label: string
  icon: string
}

/** As etapas da compilação, na ordem do console. */
export const BUILD_STAGES: BuildStageMeta[] = [
  { value: 'prepare', label: 'Preparar o builder', icon: 'mdi-cog-play' },
  { value: 'clone', label: 'Buscar o commit', icon: 'mdi-source-branch' },
  { value: 'test', label: 'Testes', icon: 'mdi-test-tube' },
  { value: 'build', label: 'Compilar', icon: 'mdi-hammer-wrench' },
  { value: 'store', label: 'Guardar no registry', icon: 'mdi-package-variant-closed' },
  { value: 'scan', label: 'Vulnerabilidades', icon: 'mdi-shield-search' },
]

/** Estado de cada etapa da compilação (a de teste pula quando não há). */
export function buildStageStates(
  stage: BuildStage | null,
  status: DeployBuildStatus
): Record<BuildStage, StageState> {
  const order = BUILD_STAGES.map((item) => item.value)
  const result = Object.fromEntries(order.map((value) => [value, 'todo' as StageState])) as Record<
    BuildStage,
    StageState
  >
  if (status === 'succeeded') {
    for (const value of order) result[value] = 'done'
    return result
  }
  if (!stage) return result
  const current = order.indexOf(stage)
  order.forEach((value, index) => {
    if (index < current) result[value] = 'done'
    else if (index === current)
      result[value] = status === 'failed' || status === 'cancelled' ? 'failed' : 'current'
  })
  return result
}

export const BUILDER_OPTIONS: {
  value: DeployBuilder
  title: string
  subtitle: string
  icon: string
}[] = [
  {
    value: 'netmonitor',
    title: 'O NetMonitor compila',
    subtitle: 'O push chega, o servidor compila e sobe. Sem workflow e sem ghcr.io.',
    icon: 'mdi-server',
  },
  {
    value: 'github_actions',
    title: 'O GitHub Actions compila',
    subtitle: 'Um workflow no repositório publica no ghcr.io e avisa a central.',
    icon: 'mdi-github',
  },
]

export const POLICY_OPTIONS: { value: DeployVulnerabilityPolicy; title: string }[] = [
  { value: 'block_critical', title: 'Bloquear com vulnerabilidade crítica' },
  { value: 'block_high', title: 'Bloquear com vulnerabilidade alta ou crítica' },
  { value: 'warn', title: 'Só avisar' },
]

/** `refs/heads/main` → `main`; `refs/tags/v1` → `v1`. */
export function refName(reference: string | null | undefined): string {
  return (reference ?? '').replace(/^refs\/(heads|tags)\//, '')
}
