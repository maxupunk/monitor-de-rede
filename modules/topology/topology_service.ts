import { TopologyBuilder } from './topology_builder.js'
import { LinkResolver } from './link_resolver.js'

export class TopologyService {
  private builder = new TopologyBuilder()
  private linkResolver = new LinkResolver()

  async getTopology(_siteId?: string) {
    const links = this.linkResolver.resolveLinks([])
    return this.builder.buildGraph([], links)
  }
}
