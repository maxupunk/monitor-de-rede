import type { DiscoveredHost } from './icmp_scanner.js'

export class ArpScanner {
  async scanArpTable(): Promise<DiscoveredHost[]> {
    return []
  }
}
