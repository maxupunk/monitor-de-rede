export interface DiscoveredHost {
  ipAddress: string
  macAddress?: string
  hostname?: string
  mdnsName?: string
  vendor?: string
  deviceType?: string
  confidence: number
}

export class IcmpScanner {
  async scanNetwork(_cidr: string): Promise<DiscoveredHost[]> {
    return []
  }
}
