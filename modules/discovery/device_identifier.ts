import type { DiscoveredHost } from './scanners/icmp_scanner.js'

export class DeviceIdentifier {
  identifyType(host: Partial<DiscoveredHost>): string {
    const hostname = (host.hostname || host.mdnsName || '').toLowerCase()
    const ports = host.openPorts || []

    if (
      hostname.includes('router') ||
      hostname.includes('gateway') ||
      hostname.includes('mikrotik') ||
      ports.includes(8291)
    ) {
      return 'router'
    }
    if (hostname.includes('switch')) {
      return 'switch'
    }
    if (hostname.includes('ap') || hostname.includes('accesspoint') || hostname.includes('unifi')) {
      return 'access_point'
    }
    if (
      hostname.includes('print') ||
      hostname.includes('epson') ||
      hostname.includes('hp') ||
      ports.includes(9100)
    ) {
      return 'printer'
    }
    if (hostname.includes('camera') || hostname.includes('cam') || ports.includes(554)) {
      return 'camera'
    }
    if (
      hostname.includes('server') ||
      hostname.includes('nas') ||
      ports.includes(445) ||
      ports.includes(22)
    ) {
      return 'server'
    }
    if (ports.includes(80) || ports.includes(443)) {
      return 'web_device'
    }

    return 'unknown'
  }
}
