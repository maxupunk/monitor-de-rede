import type { DiscoveredHost } from './icmp_scanner.js'

export class SsdpScanner {
  async scanSsdp(): Promise<DiscoveredHost[]> {
    return []
  }
}
