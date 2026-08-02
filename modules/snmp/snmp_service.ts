import { SnmpSessionFactory } from './snmp_session_factory.js'
import type { SnmpConfig } from './clients/snmp_client.js'
import { SystemCollector } from './collectors/system_collector.js'
import { InterfaceCollector } from './collectors/interface_collector.js'
import { TrafficCollector } from './collectors/traffic_collector.js'
import { LldpCollector } from './collectors/lldp_collector.js'
import Device from '#models/device'
import DeviceInterface from '#models/device_interface'
import Metric from '#models/metric'
import { DateTime } from 'luxon'
import { TopologyService } from '#modules/topology/topology_service'

export class SnmpService {
  private factory = new SnmpSessionFactory()
  private systemCollector = new SystemCollector()
  private interfaceCollector = new InterfaceCollector()
  private trafficCollector = new TrafficCollector()
  private lldpCollector = new LldpCollector()
  private topologyService = new TopologyService()

  async pollDevice(device: Device, config: SnmpConfig) {
    const client = this.factory.createSession(config)

    // 1. System Info
    const systemInfo = await this.systemCollector.collect(client)
    if (systemInfo.sysName) {
      device.name = device.name || systemInfo.sysName
    }
    if (systemInfo.sysDescr) {
      device.description = systemInfo.sysDescr
    }
    device.status = 'online'
    device.lastSeenAt = DateTime.now()
    await device.save()

    // 2. Interfaces
    const discoveredIfaces = await this.interfaceCollector.collect(client)
    const savedIfaces: DeviceInterface[] = []

    for (const ifaceData of discoveredIfaces) {
      let iface = await DeviceInterface.query()
        .where('deviceId', device.id)
        .where('snmpIndex', ifaceData.ifIndex)
        .first()

      if (!iface) {
        iface = new DeviceInterface()
        iface.deviceId = device.id
        iface.snmpIndex = ifaceData.ifIndex
      }

      iface.name = ifaceData.ifName || `if-${ifaceData.ifIndex}`
      iface.description = ifaceData.ifDescr || null
      iface.alias = ifaceData.ifAlias || null
      iface.macAddress = ifaceData.macAddress || null
      iface.speed = ifaceData.ifSpeed || null
      iface.adminStatus = ifaceData.ifAdminStatus === 1 ? 'up' : 'down'
      iface.operStatus = ifaceData.ifOperStatus === 1 ? 'up' : 'down'
      iface.lastSeenAt = DateTime.now()

      await iface.save()
      savedIfaces.push(iface)
    }

    // 3. Traffic Metrics
    const trafficList = await this.trafficCollector.collect(client)
    let metricCount = 0

    for (const traffic of trafficList) {
      const targetIface = savedIfaces.find((i) => i.snmpIndex === traffic.ifIndex)
      if (!targetIface) continue

      // Save raw inOctets & outOctets as metrics
      await Metric.create({
        deviceId: device.id,
        interfaceId: targetIface.id,
        name: 'ifHCInOctets',
        value: traffic.inOctets,
        unit: 'bytes',
        recordedAt: DateTime.fromJSDate(traffic.recordedAt),
      })

      await Metric.create({
        deviceId: device.id,
        interfaceId: targetIface.id,
        name: 'ifHCOutOctets',
        value: traffic.outOctets,
        unit: 'bytes',
        recordedAt: DateTime.fromJSDate(traffic.recordedAt),
      })

      metricCount += 2
    }

    // 4. LLDP / CDP Neighbors
    const neighbors = await this.lldpCollector.collect(client)
    if (neighbors.length > 0) {
      await this.topologyService.resolveDiscoveredNeighbors(device, neighbors)
    }

    return {
      systemInfo,
      interfaceCount: savedIfaces.length,
      metricCount,
      neighborCount: neighbors.length,
    }
  }
}
