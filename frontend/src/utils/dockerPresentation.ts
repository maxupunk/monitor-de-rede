import type { DockerContainerSummary } from '@/bindings/DockerContainerSummary'
import type { DockerContainerMetrics } from '@/bindings/DockerContainerMetrics'

/**
 * Como as telas `/docker` falam dos objetos da Engine: nome, estado, cor,
 * agrupamento e o que dá para fazer. Um lugar só — as abas só desenham.
 * Nada aqui é cinza: estado parado é aviso, morto é erro (AGENTS.md §12).
 */

export interface Presentation {
  label: string
  color: string
  icon: string
}

const CONTAINER_STATES: Record<string, Presentation> = {
  running: { label: 'Executando', color: 'success', icon: 'mdi-play-circle' },
  exited: { label: 'Parado', color: 'warning', icon: 'mdi-stop-circle' },
  created: { label: 'Criado', color: 'info', icon: 'mdi-plus-circle' },
  paused: { label: 'Pausado', color: 'warning', icon: 'mdi-pause-circle' },
  restarting: { label: 'Reiniciando', color: 'info', icon: 'mdi-restart' },
  removing: { label: 'Removendo', color: 'error', icon: 'mdi-delete-circle' },
  dead: { label: 'Inativo', color: 'error', icon: 'mdi-alert-circle' },
}

export function containerState(state: string): Presentation {
  return (
    CONTAINER_STATES[state] ?? {
      label: state || 'Desconhecido',
      color: 'warning',
      icon: 'mdi-help-circle',
    }
  )
}

/** Os 12 primeiros caracteres do id, sem o `sha256:`. */
export function shortId(id: string): string {
  return id.replace(/^sha256:/, '').slice(0, 12)
}

export function containerName(container: Pick<DockerContainerSummary, 'names' | 'id'>): string {
  return container.names[0]?.replace(/^\//, '') || shortId(container.id)
}

/** O projeto compose de um objeto pelas labels; `null` quando avulso. */
export function composeProject(labels: Record<string, string>): string | null {
  return labels['com.docker.compose.project']?.trim() || null
}

// ---------------------------------------------------------------------------
// Filtro e agrupamento da aba Containers
// ---------------------------------------------------------------------------

export type ContainerStateFilter = 'all' | 'running' | 'stopped'

export const CONTAINER_STATE_FILTERS: Array<{
  value: ContainerStateFilter
  label: string
  color: string
}> = [
  { value: 'all', label: 'Todos', color: 'primary' },
  { value: 'running', label: 'Em execução', color: 'success' },
  { value: 'stopped', label: 'Parados', color: 'warning' },
]

export function isContainerStateFilter(value: unknown): value is ContainerStateFilter {
  return value === 'all' || value === 'running' || value === 'stopped'
}

function matchesState(container: DockerContainerSummary, filter: ContainerStateFilter): boolean {
  if (filter === 'running') return container.state === 'running'
  if (filter === 'stopped') return container.state !== 'running'
  return true
}

/** Quantos contêineres cada filtro mostraria — o número no chip. */
export function stateFilterCounts(
  containers: DockerContainerSummary[]
): Record<ContainerStateFilter, number> {
  const running = containers.filter((item) => item.state === 'running').length
  return { all: containers.length, running, stopped: containers.length - running }
}

export function filterContainers(
  containers: DockerContainerSummary[],
  search: string,
  state: ContainerStateFilter
): DockerContainerSummary[] {
  const term = search.trim().toLocaleLowerCase()
  return containers.filter(
    (container) =>
      matchesState(container, state) &&
      (!term ||
        [containerName(container), container.image, container.projectName || '']
          .join(' ')
          .toLocaleLowerCase()
          .includes(term))
  )
}

export interface ContainerGroup {
  key: string
  label: string
  containers: DockerContainerSummary[]
  running: number
  cpuPercent: number
  memoryBytes: number
  standalone: boolean
}

/** Por projeto compose, avulsos no fim; dentro do grupo, pelo nome. */
export function groupContainers(
  containers: DockerContainerSummary[],
  metricFor: (id: string) => DockerContainerMetrics | undefined
): ContainerGroup[] {
  const groups = new Map<string, ContainerGroup>()
  for (const container of containers) {
    const project = container.projectName?.trim() || ''
    const key = project ? project.toLocaleLowerCase() : '__standalone__'
    const metric = metricFor(container.id)
    const group = groups.get(key) ?? {
      key,
      label: project || 'Containers avulsos',
      containers: [],
      running: 0,
      cpuPercent: 0,
      memoryBytes: 0,
      standalone: !project,
    }
    group.containers.push(container)
    group.running += container.state === 'running' ? 1 : 0
    group.cpuPercent += metric?.cpu.usagePercent ?? 0
    group.memoryBytes += metric?.memory.usageBytes ?? 0
    groups.set(key, group)
  }
  return [...groups.values()]
    .map((group) => ({
      ...group,
      containers: [...group.containers].sort((a, b) =>
        containerName(a).localeCompare(containerName(b))
      ),
    }))
    .sort((a, b) =>
      a.standalone !== b.standalone ? (a.standalone ? 1 : -1) : a.label.localeCompare(b.label)
    )
}

// ---------------------------------------------------------------------------
// Ações sobre um contêiner
// ---------------------------------------------------------------------------

export type ContainerActionKey = 'start' | 'stop' | 'restart' | 'update' | 'remove'

export interface ContainerActionMeta extends Presentation {
  key: ContainerActionKey
  /** O que acontece, em uma frase (tooltip e confirmação). */
  hint: string
}

export const CONTAINER_ACTIONS: Record<ContainerActionKey, ContainerActionMeta> = {
  start: {
    key: 'start',
    label: 'Iniciar',
    color: 'success',
    icon: 'mdi-play',
    hint: 'Liga o container parado com a mesma configuração.',
  },
  stop: {
    key: 'stop',
    label: 'Parar',
    color: 'warning',
    icon: 'mdi-stop',
    hint: 'Desliga o container sem removê-lo.',
  },
  restart: {
    key: 'restart',
    label: 'Reiniciar',
    color: 'info',
    icon: 'mdi-restart',
    hint: 'Para e liga de novo, sem recriar.',
  },
  update: {
    key: 'update',
    label: 'Atualizar imagem',
    color: 'primary',
    icon: 'mdi-update',
    hint: 'Baixa a versão atual da imagem e recria o container com a mesma configuração.',
  },
  remove: {
    key: 'remove',
    label: 'Remover',
    color: 'error',
    icon: 'mdi-delete-outline',
    hint: 'Apaga o container. Os volumes ficam.',
  },
}

export interface ContainerActionAccess {
  /** `lifecycle` liberado no host e usuário administrador. */
  lifecycle: boolean
  /** `update` liberado no host e usuário administrador. */
  update: boolean
}

/**
 * O que dá para fazer com o container agora. O primeiro é a ação principal
 * (ligar ou desligar); container protegido (a própria central) só liga.
 */
export function containerActions(
  container: Pick<DockerContainerSummary, 'state' | 'protected'>,
  access: ContainerActionAccess
): ContainerActionMeta[] {
  const keys: ContainerActionKey[] = []
  const running = container.state === 'running'
  if (access.lifecycle && !running) keys.push('start')
  if (access.lifecycle && running && !container.protected) keys.push('stop')
  if (access.lifecycle && !container.protected) keys.push('restart')
  if (access.update && !container.protected) keys.push('update')
  if (access.lifecycle && !container.protected) keys.push('remove')
  return keys.map((key) => CONTAINER_ACTIONS[key])
}
