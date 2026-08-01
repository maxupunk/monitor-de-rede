import type { DiscoveredHost } from './scanners/icmp_scanner.js'
import { DeviceIdentifier } from './device_identifier.js'

export class DiscoveryMerger {
  private identifier = new DeviceIdentifier()

  mergeResults(resultsList: DiscoveredHost[][]): DiscoveredHost[] {
    const ipMap = new Map<string, DiscoveredHost>()

    for (const list of resultsList) {
      for (const host of list) {
        if (!host.ipAddress) continue

        const existing = ipMap.get(host.ipAddress)

        if (existing) {
          const merged: DiscoveredHost = {
            ipAddress: host.ipAddress,
            macAddress: host.macAddress || existing.macAddress,
            hostname: host.hostname || existing.hostname,
            mdnsName: host.mdnsName || existing.mdnsName,
            vendor: host.vendor || existing.vendor,
            openPorts: Array.from(new Set([...(existing.openPorts || []), ...(host.openPorts || [])])),
            confidence: Math.max(existing.confidence, host.confidence),
            data: { ...(existing.data || {}), ...(host.data || {}) },
          }

          merged.deviceType = this.identifier.identifyType(merged)
          ipMap.set(host.ipAddress, merged)
        } else {
          const newHost = {
            ...host,
            deviceType: this.identifier.identifyType(host),
          }
          ipMap.set(host.ipAddress, newHost)
        }
      }
    }

    return Array.from(ipMap.values())
  }
}
