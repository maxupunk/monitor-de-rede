/**
 * O teste de enlace entra na topologia só pelos registros (ADR 015 §7): a
 * aba "Testes" do inspetor e as ações "Repetir último teste" / "Ver último
 * teste". As ações de iniciar ("Testar velocidade do enlace"…) são nativas
 * (`utils/topology/builtinActions.ts`).
 */
import { useLinkTestsStore } from '@/stores/linkTests'
import type { LinkTest } from '@/types/linkTests'
import { formatRelativeTime } from '@/utils/formatters'
import { TEST_KINDS, testStatusPresentation } from '@/utils/linkTests/presentation'
import { repeatRequest } from '@/utils/linkTests/resultView'
import { specSummary } from '@/utils/linkTests/spec'
import {
  registerActionProvider,
  registerInspectorTab,
  type TopologyAction,
  type TopologyActionProvider,
} from '@/utils/topology/inspectorRegistry'
import { isUnmanagedSwitch } from '@/utils/topology/presentation'
import LinkTestsTab from './LinkTestsTab.vue'

/** Ações sobre o último teste do alvo (repetir com os mesmos parâmetros, abrir). */
export function linkTestHistoryActions(
  latest: (target: Parameters<TopologyActionProvider>[0]) => LinkTest | null
): TopologyActionProvider {
  return (target, host) => {
    if (target.kind === 'background') return []
    const test = latest(target)
    if (!test) return []
    const store = useLinkTestsStore()
    const status = testStatusPresentation(test.status)
    const actions: TopologyAction[] = [
      {
        id: 'test.view-last',
        title: 'Ver último teste',
        subtitle: `${status.label} · ${formatRelativeTime(test.createdAt)}`,
        icon: 'mdi-history',
        color: status.color,
        group: 'test',
        order: 40,
        run: () => {
          host.openInspectorTab(LINK_TESTS_TAB)
          store.focus(test.id)
        },
      },
      {
        id: 'test.repeat',
        title: 'Repetir último teste',
        subtitle: `${TEST_KINDS[test.kind].label} · ${specSummary(test.spec)}`,
        icon: 'mdi-repeat',
        color: 'info',
        group: 'test',
        order: 30,
        requiresWrite: true,
        run: () => host.requestLinkTest(repeatRequest(test)),
      },
    ]
    return actions
  }
}

/** Id da aba "Testes" do inspetor. */
export const LINK_TESTS_TAB = 'link-tests'

let registered = false

export function registerLinkTestTopology() {
  if (registered) return
  registered = true
  registerInspectorTab({
    id: LINK_TESTS_TAB,
    title: 'Testes',
    icon: 'mdi-speedometer',
    appliesTo: ['node', 'edge'],
    order: 40,
    visible: (target) => {
      if (target.kind === 'edge') return true
      if (target.kind === 'node') return !isUnmanagedSwitch(target.node)
      return false
    },
    component: LinkTestsTab,
  })
  registerActionProvider(
    'link-tests.history',
    linkTestHistoryActions((target) => {
      const store = useLinkTestsStore()
      if (target.kind === 'edge') return store.latestForEdge(target.edge.id)
      if (target.kind === 'node') return store.latestForDevice(target.node.id)
      return null
    })
  )
}
