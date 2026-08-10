import type { DiscoveredHost } from './scanners/icmp_scanner.js'

export class DeviceIdentifier {
  identifyType(host: Partial<DiscoveredHost>): string {
    const hostname = (host.hostname || host.mdnsName || '').toLowerCase()
    const vendor = (host.vendor || '').toLowerCase()
    const ports = host.openPorts || []

    if (
      hostname.includes('router') ||
      hostname.includes('gateway') ||
      hostname.includes('mikrotik') ||
      vendor.includes('mikrotik') ||
      vendor.includes('cisco') ||
      vendor.includes('juniper') ||
      vendor.includes('fortinet') ||
      vendor.includes('sonicwall') ||
      ports.includes(8291)
    ) {
      return 'router'
    }
    if (hostname.includes('switch') || vendor.includes('switch')) {
      return 'switch'
    }
    if (
      hostname.includes('ap') ||
      hostname.includes('accesspoint') ||
      hostname.includes('unifi') ||
      vendor.includes('ubiquiti') ||
      vendor.includes('aruba') ||
      vendor.includes('ruckus')
    ) {
      return 'access_point'
    }
    if (
      hostname.includes('print') ||
      hostname.includes('epson') ||
      (hostname.includes('hp') && ports.includes(9100)) ||
      vendor.includes('hp') ||
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
      vendor.includes('synology') ||
      vendor.includes('qnap') ||
      vendor.includes('dell') ||
      vendor.includes('supermicro') ||
      vendor.includes('vmware') ||
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
