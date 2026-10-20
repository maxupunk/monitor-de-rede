/**
 * Ações nativas do mapa, registradas no mesmo registro que as extensões usam
 * (`inspectorRegistry.ts`). Inspetor e menu de contexto mostram exatamente
 * esta lista — uma ação nunca existe só num dos dois.
 */
import type { LinkTestRequest } from '@/stores/linkTestRequests'
import type { TopologyEdge, TopologyNode, TopologyPluginActionMap } from '@/stores/topology'
import { transparentEdgeOrigin } from '@/utils/linkTests/targets'
import {
  registerActionProvider,
  type TopologyAction,
  type TopologyActionProvider,
} from './inspectorRegistry'
import { canConfirmEdge, edgeEndpoints, isUnmanagedSwitch, isVirtualEdge } from './presentation'

export const nodeActions: TopologyActionProvider = (target, host) => {
  if (target.kind !== 'node') return []
  const node = target.node
  const unmanaged = isUnmanagedSwitch(node)
  const actions: TopologyAction[] = [
    {
      id: 'node.focus',
      title: 'Centralizar no mapa',
      icon: 'mdi-crosshairs-gps',
      color: 'primary',
      group: 'navigate',
      order: 10,
      run: () => host.focusNode(node.id),
    },
  ]
  if (!unmanaged) {
    actions.push({
      id: 'node.open',
      title: 'Abrir ficha do dispositivo',
      icon: 'mdi-open-in-new',
      color: 'primary',
      group: 'navigate',
      order: 20,
      run: () => host.openDevice(node.id),
    })
  }
  if (host.systemNodeId !== null && host.systemNodeId !== node.id) {
    const system = host.systemNodeId
    actions.push({
      id: 'node.path-from-server',
      title: 'Mostrar caminho do servidor até aqui',
      icon: 'mdi-map-marker-path',
      color: 'info',
      group: 'navigate',
      order: 30,
      run: () => host.highlightPath(system, node.id),
    })
  }
  actions.push(
    {
      id: 'node.connect',
      title: 'Conectar a outro dispositivo…',
      icon: 'mdi-vector-polyline-plus',
      color: 'primary',
      group: 'edit',
      order: 10,
      requiresWrite: true,
      run: () => host.connectFrom(node.id),
    },
    {
      id: 'node.cable',
      title: 'Puxar cabo daqui no mapa',
      subtitle: 'Clique no destino em seguida',
      icon: 'mdi-cable-data',
      color: 'secondary',
      group: 'edit',
      order: 20,
      requiresWrite: true,
      requiresEditMode: true,
      run: () => host.startCable(node.id),
    },
    {
      id: 'node.edit',
      title: unmanaged ? 'Editar switch' : 'Editar dispositivo',
      icon: 'mdi-pencil',
      color: 'primary',
      group: 'edit',
      order: 30,
      requiresWrite: true,
      run: () => host.editNode(node),
    },
    {
      id: 'node.delete',
      title: unmanaged ? 'Excluir switch' : 'Excluir dispositivo',
      icon: 'mdi-trash-can',
      color: 'error',
      group: 'danger',
      order: 10,
      requiresWrite: true,
      shortcut: 'Del',
      run: () => host.deleteNode(node),
    }
  )
  return actions
}

export const edgeActions: TopologyActionProvider = (target, host) => {
  if (target.kind !== 'edge') return []
  const edge = target.edge
  const { source, target: to } = edgeEndpoints(edge)
  const virtual = isVirtualEdge(edge)
  const actions: TopologyAction[] = [
    {
      id: 'edge.source',
      title: `Ir para ${edge.sourceDeviceName || 'origem'}`,
      icon: 'mdi-ray-start-arrow',
      color: 'primary',
      group: 'navigate',
      order: 10,
      run: () => host.focusNode(source),
    },
    {
      id: 'edge.target',
      title: `Ir para ${edge.targetDeviceName || 'destino'}`,
      icon: 'mdi-ray-end-arrow',
      color: 'primary',
      group: 'navigate',
      order: 20,
      run: () => host.focusNode(to),
    },
  ]
  if (canConfirmEdge(edge)) {
    actions.push({
      id: 'edge.confirm',
      title: 'Confirmar enlace',
      subtitle: 'Passa a valer como definido pelo operador',
      icon: 'mdi-check-decagram',
      color: 'success',
      group: 'edit',
      order: 5,
      requiresWrite: true,
      run: () => host.confirmEdge(edge),
    })
  }
  actions.push({
    id: 'edge.edit',
    title: virtual ? 'Transformar em enlace físico…' : 'Editar enlace…',
    icon: 'mdi-pencil',
    color: 'primary',
    group: 'edit',
    order: 10,
    requiresWrite: true,
    run: () => host.editEdge(edge),
  })
  actions.push({
    id: 'edge.delete',
    title: virtual ? 'Desvincular hierarquia' : 'Remover enlace',
    icon: virtual ? 'mdi-link-variant-off' : 'mdi-trash-can',
    color: 'error',
    group: 'danger',
    order: 10,
    requiresWrite: true,
    shortcut: 'Del',
    run: () => host.deleteEdge(edge),
  })
  return actions
}

export const backgroundActions: TopologyActionProvider = (target, host) => {
  if (target.kind !== 'background') return []
  return [
    {
      id: 'map.fit',
      title: 'Enquadrar o mapa',
      icon: 'mdi-fit-to-screen',
      color: 'primary',
      group: 'navigate',
      order: 10,
      shortcut: 'F',
      run: () => host.fit(),
    },
    {
      id: 'map.add-link',
      title: 'Adicionar enlace…',
      icon: 'mdi-vector-polyline-plus',
      color: 'primary',
      group: 'edit',
      order: 10,
      requiresWrite: true,
      run: () => host.addLink(),
    },
    {
      id: 'map.add-switch',
      title: 'Adicionar switch não gerenciável…',
      icon: 'mdi-hub',
      color: 'secondary',
      group: 'edit',
      order: 20,
      requiresWrite: true,
      run: () => host.addUnmanagedSwitch(),
    },
    {
      id: 'map.discover',
      title: 'Descobrir enlaces (LLDP / sub-rede)',
      icon: 'mdi-radar',
      color: 'secondary',
      group: 'edit',
      order: 30,
      requiresWrite: true,
      run: () => host.discoverLinks(),
    },
  ]
}

/**
 * Pedido de teste de velocidade de um enlace (`null` se o enlace não é
 * testável). A hierarquia declarada (id negativo) também se mede, como no
 * backend. Uma ponta sem IP (switch não gerenciável) não mede: o pedido
 * abre o assistente na ponta com IP, para escolher o destino depois do
 * switch — o teste passa por ele.
 */
export function edgeLinkTestRequest(
  edge: TopologyEdge,
  nodeOf: (deviceId: number) => TopologyNode | undefined = () => undefined
): LinkTestRequest | null {
  const { source, target } = edgeEndpoints(edge)
  const through = transparentEdgeOrigin(edge, nodeOf)
  if (through.direct) {
    return { kind: 'link', linkId: edge.id, sourceDeviceId: source, targetDeviceId: target }
  }
  return through.origin !== null ? { kind: 'link', sourceDeviceId: through.origin } : null
}

/** O pedido do enlace só escolhe a origem: o destino fica além de um switch sem IP. */
export function choosesTargetBeyond(request: LinkTestRequest): boolean {
  return request.kind === 'link' && request.linkId === undefined
}

/** Entradas do teste de velocidade (ADR 015) — o mapa só registra o pedido. */
export const linkTestActions: TopologyActionProvider = (target, host) => {
  if (target.kind === 'edge') {
    const request = edgeLinkTestRequest(target.edge, (id) => host.nodeById(id))
    if (!request) return []
    return [
      {
        id: 'test.link',
        title: 'Testar velocidade do enlace',
        ...(choosesTargetBeyond(request)
          ? { subtitle: 'Passa pelo switch sem IP — escolha o destino' }
          : {}),
        icon: 'mdi-speedometer',
        color: 'info',
        group: 'test',
        order: 10,
        requiresWrite: true,
        run: () => host.requestLinkTest(request),
      },
    ]
  }
  if (target.kind !== 'node') return []
  const node = target.node
  const actions: TopologyAction[] = []
  // De qualquer equipamento: o assistente abre com ele em "De" e oferece os
  // vizinhos (enlace) e o resto da rede (ponta a ponta).
  actions.push({
    id: 'test.from',
    title: 'Testar daqui até…',
    subtitle: 'Um enlace ou ponta a ponta até qualquer equipamento',
    icon: 'mdi-ray-start-arrow',
    color: 'info',
    group: 'test',
    order: 10,
    requiresWrite: true,
    run: () => host.requestLinkTest({ kind: 'end_to_end', sourceDeviceId: node.id }),
  })
  if (host.systemNodeId !== node.id) {
    actions.push({
      id: 'test.server',
      title: 'Testar do servidor até aqui',
      icon: 'mdi-server-network',
      color: 'info',
      group: 'test',
      order: 20,
      requiresWrite: true,
      run: () =>
        host.requestLinkTest({
          kind: 'server',
          targetDeviceId: node.id,
          ...(host.systemNodeId !== null ? { sourceDeviceId: host.systemNodeId } : {}),
        }),
    })
  }
  return actions
}

/** Ações de plugin do nó (`GET /api/plugins/topology-actions`). */
export function pluginActionsProvider(
  getMap: () => TopologyPluginActionMap
): TopologyActionProvider {
  return (target, host) => {
    if (target.kind !== 'node') return []
    const node = target.node
    return (getMap()[node.id] ?? []).map((item, index) => ({
      id: `plugin.${item.pluginId}.${item.action}`,
      title: item.hasParams ? `${item.title}…` : item.title,
      subtitle: item.pluginName,
      icon: item.effect === 'read' ? 'mdi-puzzle-outline' : 'mdi-puzzle-edit',
      color: item.effect === 'read' ? 'secondary' : 'warning',
      group: 'plugin',
      order: index,
      requiresWrite: item.effect !== 'read',
      run: () => host.runPluginAction(node, item),
    }))
  }
}

let registered = false

/** Registra as ações nativas uma vez por aplicação. */
export function registerBuiltinTopologyActions(getPluginMap: () => TopologyPluginActionMap) {
  registerActionProvider('builtin.plugins', pluginActionsProvider(getPluginMap))
  if (registered) return
  registered = true
  registerActionProvider('builtin.node', nodeActions)
  registerActionProvider('builtin.edge', edgeActions)
  registerActionProvider('builtin.background', backgroundActions)
  registerActionProvider('builtin.link-tests', linkTestActions)
}
