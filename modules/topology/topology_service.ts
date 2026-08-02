import { TopologyBuilder, type TopologyGraph, type TopologyNode, type TopologyEdge } from './topology_builder.js'
import { LinkResolver, type NetworkLink } from './link_resolver.js'
import Device from '#models/device'
import DeviceLink from '#models/device_link'
import DeviceInterface from '#models/device_interface'
import type { LldpNeighbor } from '#modules/snmp/collectors/lldp_collector'

export class TopologyService {
  private builder = new TopologyBuilder()
  private linkResolver = new LinkResolver()

  async getTopology(siteId?: number): Promise<TopologyGraph> {
    const query = Device.query().preload('site').preload('interfaces')
    if (siteId) {
      query.where('siteId', siteId)
    }
    const devices = await query

    const deviceIds = devices.map((d) => d.id)

    const dbLinks = await DeviceLink.query()
      .whereIn('sourceDeviceId', deviceIds)
      .orWhereIn('targetDeviceId', deviceIds)
      .preload('sourceInterface')
      .preload('targetInterface')

    const nodes: TopologyNode[] = devices.map((d) => ({
      id: d.id,
      name: d.name,
      type: d.type || 'generic',
      status: d.status || 'unknown',
      siteId: d.siteId,
      siteName: d.site ? d.site.name : undefined,
      interfaceCount: d.interfaces ? d.interfaces.length : 0,
    }))

    const edges: TopologyEdge[] = dbLinks.map((l) => ({
      id: l.id,
      source: l.sourceDeviceId,
      target: l.targetDeviceId,
      sourceInterfaceId: l.sourceInterfaceId,
      sourceInterfaceName: l.sourceInterface ? l.sourceInterface.name : undefined,
      targetInterfaceId: l.targetInterfaceId,
      targetInterfaceName: l.targetInterface ? l.targetInterface.name : undefined,
      linkType: l.linkType,
      discoveryMethod: l.discoveryMethod,
      confidence: l.confidence,
      confirmed: l.confirmed,
      status: 'up',
    }))

    return this.builder.buildGraph(nodes, edges)
  }

  async resolveDiscoveredNeighbors(sourceDevice: Device, neighbors: LldpNeighbor[]): Promise<DeviceLink[]> {
    const rawLinks: NetworkLink[] = []

    for (const n of neighbors) {
      // Find target device by remoteSysName or mgmt IP
      let targetDevice: Device | null = null

      if (n.remoteSysName && n.remoteSysName !== 'unknown') {
        targetDevice = await Device.query().whereILike('name', `%${n.remoteSysName}%`).first()
      }

      if (!targetDevice) continue

      // Find local interface if matches
      const localIface = await DeviceInterface.query()
        .where('deviceId', sourceDevice.id)
        .where((q) => {
          q.where('name', n.localPort).orWhere('snmpIndex', Number(n.localPort) || 0)
        })
        .first()

      // Find remote interface if matches
      const remoteIface = await DeviceInterface.query()
        .where('deviceId', targetDevice.id)
        .where((q) => {
          q.where('name', n.remotePort).orWhere('snmpIndex', Number(n.remotePort) || 0)
        })
        .first()

      rawLinks.push({
        sourceDeviceId: sourceDevice.id,
        targetDeviceId: targetDevice.id,
        sourceInterfaceId: localIface ? localIface.id : null,
        targetInterfaceId: remoteIface ? remoteIface.id : null,
        linkType: n.protocol === 'cdp' ? 'cdp' : 'lldp',
        discoveryMethod: `snmp_${n.protocol}`,
        confidence: n.protocol === 'cdp' ? 90 : 95,
        confirmed: false,
      })
    }

    return this.linkResolver.persistResolvedLinks(rawLinks)
  }

  async inferSubnetLinks(): Promise<DeviceLink[]> {
    const devices = await Device.query().whereNotNull('networkId')
    const rawLinks: NetworkLink[] = []

    const bySubnet = new Map<number, Device[]>()
    for (const d of devices) {
      if (!d.networkId) continue
      const list = bySubnet.get(d.networkId) || []
      list.push(d)
      bySubnet.set(d.networkId, list)
    }

    for (const [, netDevices] of bySubnet.entries()) {
      // Connect routers/switches to end devices in same subnet
      const infrastructure = netDevices.filter((d) => ['router', 'switch', 'firewall'].includes(d.type))
      const endDevices = netDevices.filter((d) => !['router', 'switch', 'firewall'].includes(d.type))

      for (const infra of infrastructure) {
        for (const endDev of endDevices) {
          rawLinks.push({
            sourceDeviceId: infra.id,
            targetDeviceId: endDev.id,
            linkType: 'inferred',
            discoveryMethod: 'subnet_inference',
            confidence: 60,
            confirmed: false,
          })
        }
      }
    }

    return this.linkResolver.persistResolvedLinks(rawLinks)
  }

  async createManualLink(
    sourceDeviceId: number,
    targetDeviceId: number,
    sourceInterfaceId?: number,
    targetInterfaceId?: number
  ): Promise<DeviceLink> {
    const [saved] = await this.linkResolver.persistResolvedLinks([
      {
        sourceDeviceId,
        targetDeviceId,
        sourceInterfaceId,
        targetInterfaceId,
        linkType: 'manual',
        discoveryMethod: 'user_defined',
        confidence: 100,
        confirmed: true,
      },
    ])
    return saved
  }

  async deleteLink(linkId: number): Promise<boolean> {
    const link = await DeviceLink.find(linkId)
    if (!link) return false
    await link.delete()
    return true
  }
}
