import type { NetworkLink } from './link_resolver.js'

export interface TopologyGraph {
  nodes: Array<{ id: string; name: string; type: string; status: string }>
  edges: NetworkLink[]
}

export class TopologyBuilder {
  buildGraph(nodes: Array<{ id: string; name: string; type: string; status: string }>, links: NetworkLink[]): TopologyGraph {
    return { nodes, edges: links }
  }
}
