export interface TopologyNode {
  id: number
  name: string
  type: string
  status: 'online' | 'offline' | 'warning' | 'unknown'
  siteId?: number | null
  siteName?: string
  ipAddress?: string
  interfaceCount?: number
  activeAlertCount?: number
}

export interface TopologyEdge {
  id?: number | string
  source: number
  target: number
  sourceInterfaceId?: number | null
  sourceInterfaceName?: string
  targetInterfaceId?: number | null
  targetInterfaceName?: string
  linkType: string
  discoveryMethod: string
  confidence: number
  confirmed: boolean
  status: 'up' | 'down' | 'degraded'
}

export interface TopologyGraph {
  nodes: TopologyNode[]
  edges: TopologyEdge[]
}

export class TopologyBuilder {
  buildGraph(nodes: TopologyNode[], edges: TopologyEdge[]): TopologyGraph {
    return {
      nodes,
      edges,
    }
  }
}
