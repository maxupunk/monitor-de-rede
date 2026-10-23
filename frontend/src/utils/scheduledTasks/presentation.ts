/** Cores, ícones e rótulos das tarefas agendadas (cores semânticas, nunca cinza). */
import type { SchedulableToolKind } from '@/bindings/SchedulableToolKind'
import type { ScheduledTaskOrigin } from '@/bindings/ScheduledTaskOrigin'
import type { ScheduledTaskStatus } from '@/bindings/ScheduledTaskStatus'
import type { ScheduledTaskTrigger } from '@/bindings/ScheduledTaskTrigger'
import type { StatusPresentation } from '@/utils/topology/presentation'

export type { StatusPresentation }

export const RUN_STATUS: Record<ScheduledTaskStatus, StatusPresentation> = {
  running: { label: 'Rodando', color: 'primary', icon: 'mdi-progress-clock' },
  success: { label: 'Concluída', color: 'success', icon: 'mdi-check-circle' },
  failed: { label: 'Falhou', color: 'error', icon: 'mdi-close-circle' },
  skipped: { label: 'Não rodou', color: 'warning', icon: 'mdi-debug-step-over' },
}

export function runStatusPresentation(status: string): StatusPresentation {
  return RUN_STATUS[status as ScheduledTaskStatus] ?? RUN_STATUS.failed
}

export const TOOL_KIND: Record<SchedulableToolKind, StatusPresentation> = {
  query: { label: 'Consulta', color: 'info', icon: 'mdi-database-search-outline' },
  test: { label: 'Teste na rede', color: 'primary', icon: 'mdi-access-point-network' },
  action: { label: 'Ação', color: 'warning', icon: 'mdi-flash-outline' },
  container: { label: 'Container', color: 'error', icon: 'mdi-docker' },
}

export const TRIGGER_LABEL: Record<ScheduledTaskTrigger, string> = {
  schedule: 'No horário',
  manual: 'Rodar agora',
}

export const ORIGIN_LABEL: Record<ScheduledTaskOrigin, string> = {
  user: 'Criada na tela',
  ai: 'Criada pelo Assistente IA',
}

/** Ponto de partida do estado vazio: um clique abre o formulário preenchido. */
export interface ScheduledTaskTemplate {
  title: string
  icon: string
  name: string
  tool: string
  arguments: Record<string, unknown>
  cron: string
}

export const TASK_TEMPLATES: readonly ScheduledTaskTemplate[] = [
  {
    title: 'Resumo da rede toda manhã',
    icon: 'mdi-chart-box-outline',
    name: 'Resumo da rede',
    tool: 'get_system_summary',
    arguments: {},
    cron: '0 7 * * 1-5',
  },
  {
    title: 'Ping no gateway a cada 15 min',
    icon: 'mdi-access-point-network',
    name: 'Ping no gateway',
    tool: 'ping_host',
    arguments: {},
    cron: '*/15 * * * *',
  },
  {
    title: 'Reiniciar um container de madrugada',
    icon: 'mdi-docker',
    name: 'Reiniciar container',
    tool: 'docker_container_action',
    arguments: { action: 'restart' },
    cron: '0 4 * * *',
  },
]
