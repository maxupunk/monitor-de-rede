export interface NetworkLink {
  id?: string
  sourceDeviceId: string
  targetDeviceId: string
  sourceInterfaceId?: string
  targetInterfaceId?: string
  linkType: 'manual' | 'lldp' | 'cdp' | 'snmp' | 'inferred' | 'traceroute'
  discoveryMethod: string
  confidence: number
  confirmed: boolean
}

export class LinkResolver {
  resolveLinks(rawLinks: NetworkLink[]): NetworkLink[] {
    return rawLinks
  }
}
