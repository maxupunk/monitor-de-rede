import type { TopologyEdge, TopologyNode } from '@/stores/topology'

export function node(id: number, patch: Partial<TopologyNode> = {}): TopologyNode {
  return {
    id,
    name: `Dev-${id}`,
    type: 'workstation',
    status: 'online',
    interfaceCount: 1,
    ...patch,
  }
}

export function edge(
  id: number,
  source: number,
  target: number,
  patch: Partial<TopologyEdge> = {}
): TopologyEdge {
  return {
    id,
    source,
    target,
    sourceDeviceId: source,
    targetDeviceId: target,
    linkType: 'manual',
    discoveryMethod: 'user_defined',
    status: 'up',
    ...patch,
  }
}
