import { ConfidenceCalculator } from './confidence_calculator.js'
import DeviceLink from '#models/device_link'
import { DateTime } from 'luxon'

export interface NetworkLink {
  id?: number
  sourceDeviceId: number
  targetDeviceId: number
  sourceInterfaceId?: number | null
  targetInterfaceId?: number | null
  linkType: 'manual' | 'lldp' | 'cdp' | 'snmp' | 'inferred' | 'traceroute'
  discoveryMethod: string
  confidence?: number
  confirmed?: boolean
}

export class LinkResolver {
  private confidenceCalc = new ConfidenceCalculator()

  resolveLinks(rawLinks: NetworkLink[]): NetworkLink[] {
    const linkMap = new Map<string, NetworkLink>()

    for (const raw of rawLinks) {
      if (raw.sourceDeviceId === raw.targetDeviceId) continue // Ignore self-loop

      const confidence = raw.confidence ?? this.confidenceCalc.calculateConfidence(raw.linkType)

      // Key is ordered pair of device IDs to deduplicate bidirectional links
      const [minId, maxId] = [raw.sourceDeviceId, raw.targetDeviceId].sort((a, b) => a - b)
      const pairKey = `${minId}:${maxId}`

      const existing = linkMap.get(pairKey)
      if (!existing || (existing.confidence ?? 0) < confidence) {
        linkMap.set(pairKey, {
          ...raw,
          confidence,
          confirmed: raw.confirmed ?? (raw.linkType === 'manual'),
        })
      }
    }

    return Array.from(linkMap.values())
  }

  async persistResolvedLinks(links: NetworkLink[]): Promise<DeviceLink[]> {
    const resolved = this.resolveLinks(links)
    const savedLinks: DeviceLink[] = []

    for (const linkData of resolved) {
      let link = await DeviceLink.query()
        .where('sourceDeviceId', linkData.sourceDeviceId)
        .where('targetDeviceId', linkData.targetDeviceId)
        .first()

      if (!link) {
        link = await DeviceLink.query()
          .where('sourceDeviceId', linkData.targetDeviceId)
          .where('targetDeviceId', linkData.sourceDeviceId)
          .first()
      }

      if (!link) {
        link = new DeviceLink()
        link.sourceDeviceId = linkData.sourceDeviceId
        link.targetDeviceId = linkData.targetDeviceId
      }

      link.sourceInterfaceId = linkData.sourceInterfaceId ?? null
      link.targetInterfaceId = linkData.targetInterfaceId ?? null
      link.linkType = linkData.linkType
      link.discoveryMethod = linkData.discoveryMethod
      link.confidence = linkData.confidence ?? 100
      link.confirmed = linkData.confirmed ?? false
      link.lastSeenAt = DateTime.now()

      await link.save()
      savedLinks.push(link)
    }

    return savedLinks
  }
}
