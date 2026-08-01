import type { DiscoveredHost } from './icmp_scanner.js'

export class MdnsScanner {
  async scanMdns(): Promise<DiscoveredHost[]> {
    return []
  }
}
