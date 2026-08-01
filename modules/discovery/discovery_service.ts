import type { DiscoveredHost } from './scanners/icmp_scanner.js'
import { DiscoveryMerger } from './discovery_merger.js'

export class DiscoveryService {
  private merger = new DiscoveryMerger()

  async runDiscovery(_cidr: string): Promise<DiscoveredHost[]> {
    return this.merger.mergeResults([])
  }
}
