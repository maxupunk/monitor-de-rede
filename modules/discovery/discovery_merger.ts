import type { DiscoveredHost } from './scanners/icmp_scanner.js'

export class DiscoveryMerger {
  mergeResults(resultsList: DiscoveredHost[][]): DiscoveredHost[] {
    const map = new Map<string, DiscoveredHost>()
    for (const list of resultsList) {
      for (const host of list) {
        const existing = map.get(host.ipAddress)
        if (existing) {
          map.set(host.ipAddress, { ...existing, ...host })
        } else {
          map.set(host.ipAddress, host)
        }
      }
    }
    return Array.from(map.values())
  }
}
