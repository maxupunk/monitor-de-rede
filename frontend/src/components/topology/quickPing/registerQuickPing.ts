/**
 * O diagnóstico rápido por ping entra no mapa só pelo registro de ações
 * (`inspectorRegistry`): inspetor e menu de contexto mostram a mesma ação.
 * Vale para qualquer nó com IP — inclusive equipamento desconhecido, que não
 * roda o teste de velocidade completo.
 */
import type { TopologyNode } from '@/stores/topology'
import { canQuickPing, QUICK_PING_TITLE } from '@/utils/diagnostics/quickPing'
import {
  registerActionProvider,
  type TopologyActionProvider,
} from '@/utils/topology/inspectorRegistry'

export const QUICK_PING_PROVIDER = 'diagnostics.quick-ping'

export function quickPingActions(open: (node: TopologyNode) => void): TopologyActionProvider {
  return (target, host) => {
    if (target.kind !== 'node') return []
    const node = target.node
    // A central não se mede a si mesma.
    if (node.id === host.systemNodeId || !canQuickPing(node)) return []
    return [
      {
        id: 'test.quick-ping',
        title: QUICK_PING_TITLE,
        subtitle: 'Latência, perda e fila em ~4 s — estimativa de baixa precisão',
        icon: 'mdi-pulse',
        color: 'info',
        group: 'test',
        order: 25,
        requiresWrite: true,
        run: () => open(node),
      },
    ]
  }
}

/** Registra a ação; devolve o cancelamento (a camada desmonta com a página). */
export function registerQuickPing(open: (node: TopologyNode) => void): () => void {
  return registerActionProvider(QUICK_PING_PROVIDER, quickPingActions(open))
}
