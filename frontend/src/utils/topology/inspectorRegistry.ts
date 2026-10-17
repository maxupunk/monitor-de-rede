/**
 * Ponto de extensão do inspetor e do menu de contexto da topologia.
 *
 * Quem quer aparecer no mapa registra aqui — o inspetor e o menu só listam o
 * que o registro devolve, sem saber quem forneceu (aberto/fechado). Exemplos:
 *
 * ```ts
 * // Uma aba "Testes" no inspetor de enlace:
 * registerInspectorTab({
 *   id: 'link-tests', title: 'Testes', icon: 'mdi-speedometer',
 *   appliesTo: ['edge'], order: 30, component: LinkTestsTab,
 * })
 *
 * // Ações no nó, no enlace ou no fundo do mapa:
 * registerActionProvider('link-tests', (target, host) =>
 *   target.kind === 'edge' ? [{ id: 'x', title: '…', icon: 'mdi-…', group: 'test',
 *     requiresWrite: true, run: () => host.requestLinkTest({ kind: 'link', linkId: target.edge.id }) }] : [])
 * ```
 *
 * O componente de uma aba recebe as props `{ target, host }`.
 */
import { markRaw, shallowReactive, type Component } from 'vue'
import type { TopologyEdge, TopologyNode, TopologyPluginAction } from '@/stores/topology'
import type { LinkTestRequest } from '@/stores/linkTestRequests'
import type { ThemeColorToken } from './presentation'

export type TopologyTarget =
  | { kind: 'node'; node: TopologyNode }
  | { kind: 'edge'; edge: TopologyEdge }
  | { kind: 'background' }

export type TopologyTargetKind = TopologyTarget['kind']

/** Aba escolhida no inspetor, por tipo de alvo (id do `registerInspectorTab`). */
export type TabChoice = Partial<Record<'node' | 'edge', string>>

/**
 * O que a página oferece às ações. Ações nunca chamam a API direto: pedem à
 * página (que abre o diálogo certo, confirma, mostra o erro).
 */
export interface TopologyHost {
  readonly canWrite: boolean
  readonly editMode: boolean
  /** Nó selecionado agora (origem natural de "testar caminho até aqui"). */
  readonly selectedNodeId: number | null
  /** O servidor NetMonitor no mapa, quando o backend o identifica. */
  readonly systemNodeId: number | null
  openDevice(deviceId: number): void
  editNode(node: TopologyNode): void
  deleteNode(node: TopologyNode): void
  /** Abre o diálogo de enlace já com as pontas conhecidas. */
  connectFrom(sourceId: number, targetId?: number): void
  /** Liga o cabo no canvas a partir deste nó (exige modo de edição). */
  startCable(sourceId: number): void
  editEdge(edge: TopologyEdge): void
  deleteEdge(edge: TopologyEdge): void
  confirmEdge(edge: TopologyEdge): void
  focusNode(deviceId: number): void
  selectEdge(edgeId: number): void
  highlightPath(fromId: number, toId: number): void
  requestLinkTest(request: LinkTestRequest): void
  /**
   * Abre uma aba do inspetor pelo id (`registerInspectorTab`) para o item
   * selecionado — ex.: "Ver último teste" abre a aba "Testes".
   */
  openInspectorTab(tabId: string): void
  runPluginAction(node: TopologyNode, action: TopologyPluginAction): void
  addUnmanagedSwitch(): void
  discoverLinks(): void
  addLink(): void
  fit(): void
}

export type ActionGroup = 'navigate' | 'edit' | 'test' | 'plugin' | 'danger'

export const ACTION_GROUP_ORDER: Record<ActionGroup, number> = {
  navigate: 0,
  edit: 1,
  test: 2,
  plugin: 3,
  danger: 4,
}

export const ACTION_GROUP_LABEL: Record<ActionGroup, string> = {
  navigate: 'Navegar',
  edit: 'Editar',
  test: 'Testes',
  plugin: 'Plugins',
  danger: 'Remover',
}

export interface TopologyAction {
  id: string
  title: string
  subtitle?: string
  icon: string
  color?: ThemeColorToken
  group: ActionGroup
  order?: number
  /** Precisa de perfil com escrita (operador/admin). */
  requiresWrite?: boolean
  /** Precisa do "Editar mapa" ligado. */
  requiresEditMode?: boolean
  /** Atalho exibido ao lado (só rótulo). */
  shortcut?: string
  run: () => void | Promise<void>
}

/** Ação já resolvida para exibição: com o motivo de estar desabilitada. */
export interface ResolvedAction extends TopologyAction {
  disabledReason: string | null
}

export type TopologyActionProvider = (
  target: TopologyTarget,
  host: TopologyHost
) => TopologyAction[]

export interface InspectorTab {
  id: string
  title: string
  icon: string
  order?: number
  appliesTo: Array<'node' | 'edge'>
  visible?: (target: TopologyTarget) => boolean
  /** Recebe as props `{ target: TopologyTarget, host: TopologyHost }`. */
  component: Component
}

const providers = shallowReactive(new Map<string, TopologyActionProvider>())
const tabs = shallowReactive(new Map<string, InspectorTab>())

/** Registra (ou substitui, pelo id) um fornecedor de ações. Devolve o cancelamento. */
export function registerActionProvider(id: string, provider: TopologyActionProvider): () => void {
  providers.set(id, provider)
  return () => {
    if (providers.get(id) === provider) providers.delete(id)
  }
}

/** Registra (ou substitui, pelo id) uma aba do inspetor. Devolve o cancelamento. */
export function registerInspectorTab(tab: InspectorTab): () => void {
  const stored = { ...tab, component: markRaw(tab.component) }
  tabs.set(tab.id, stored)
  return () => {
    if (tabs.get(tab.id) === stored) tabs.delete(tab.id)
  }
}

export const READ_ONLY_REASON =
  'Seu perfil é somente leitura — peça a um operador ou administrador.'
export const EDIT_MODE_REASON = 'Ligue "Editar mapa" na barra para alterar o mapa.'

export function disabledReasonFor(action: TopologyAction, host: TopologyHost): string | null {
  if (action.requiresWrite && !host.canWrite) return READ_ONLY_REASON
  if (action.requiresEditMode && !host.editMode) return EDIT_MODE_REASON
  return null
}

/** Todas as ações para o alvo, ordenadas por grupo e ordem. */
export function resolveActions(target: TopologyTarget, host: TopologyHost): ResolvedAction[] {
  const result: ResolvedAction[] = []
  const seen = new Set<string>()
  for (const provider of providers.values()) {
    let actions: TopologyAction[] = []
    try {
      actions = provider(target, host)
    } catch {
      // Um fornecedor com defeito não derruba o menu dos outros.
      continue
    }
    for (const action of actions) {
      if (seen.has(action.id)) continue
      seen.add(action.id)
      result.push({ ...action, disabledReason: disabledReasonFor(action, host) })
    }
  }
  return result.sort(
    (a, b) =>
      ACTION_GROUP_ORDER[a.group] - ACTION_GROUP_ORDER[b.group] || (a.order ?? 50) - (b.order ?? 50)
  )
}

export function resolveTabs(target: TopologyTarget): InspectorTab[] {
  if (target.kind === 'background') return []
  return [...tabs.values()]
    .filter((tab) => tab.appliesTo.includes(target.kind as 'node' | 'edge'))
    .filter((tab) => !tab.visible || tab.visible(target))
    .sort((a, b) => (a.order ?? 50) - (b.order ?? 50))
}

/** Agrupa ações resolvidas para listas com subtítulo por grupo. */
export function groupActions(
  actions: ResolvedAction[]
): Array<{ group: ActionGroup; label: string; actions: ResolvedAction[] }> {
  const groups = new Map<ActionGroup, ResolvedAction[]>()
  for (const action of actions) {
    if (!groups.has(action.group)) groups.set(action.group, [])
    groups.get(action.group)!.push(action)
  }
  return [...groups.entries()]
    .sort(([a], [b]) => ACTION_GROUP_ORDER[a] - ACTION_GROUP_ORDER[b])
    .map(([group, list]) => ({ group, label: ACTION_GROUP_LABEL[group], actions: list }))
}
