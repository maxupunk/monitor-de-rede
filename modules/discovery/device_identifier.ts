import type { DiscoveredHost } from './scanners/icmp_scanner.js'

export class DeviceIdentifier {
  identifyType(host: Partial<DiscoveredHost>): string {
    if (host.hostname?.includes('router')) return 'router'
    if (host.hostname?.includes('switch')) return 'switch'
    if (host.hostname?.includes('printer')) return 'printer'
    return 'unknown'
  }
}
