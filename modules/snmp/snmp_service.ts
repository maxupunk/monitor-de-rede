import { SnmpSessionFactory } from './snmp_session_factory.js'
import type { SnmpConfig } from './clients/snmp_client.js'
import { SystemCollector } from './collectors/system_collector.js'
import { InterfaceCollector } from './collectors/interface_collector.js'
import { TrafficCollector } from './collectors/traffic_collector.js'
import { LldpCollector } from './collectors/lldp_collector.js'
import { CpuCollector } from './collectors/cpu_collector.js'
import { MemoryCollector } from './collectors/memory_collector.js'
import Device from '#models/device'
import DeviceInterface from '#models/device_interface'
import Metric from '#models/metric'
import Monitor from '#models/monitor'
import { DateTime } from 'luxon'
import { TopologyService } from '#modules/topology/topology_service'

export class SnmpService {
  private factory = new SnmpSessionFactory()
  private systemCollector = new SystemCollector()
  private interfaceCollector = new InterfaceCollector()
  private trafficCollector = new TrafficCollector()
  private lldpCollector = new LldpCollector()
  private cpuCollector = new CpuCollector()
  private memoryCollector = new MemoryCollector()
  private topologyService = new TopologyService()

  async scanDevice(device: Device, config: SnmpConfig) {
    const client = this.factory.createSession(config)
    const [systemInfo, discoveredIfaces, cpuInfo, memoryInfo] = await Promise.all([
      this.systemCollector.collect(client),
      this.interfaceCollector.collect(client),
      this.cpuCollector.collect(client),
      this.memoryCollector.collect(client),
    ])

    const existingIfaces = await DeviceInterface.query().where('deviceId', device.id)
    const existingMonitors = await Monitor.query().where('deviceId', device.id)

    const interfaces = discoveredIfaces.map((iface) => {
      const match = existingIfaces.find((e) => e.snmpIndex === iface.ifIndex)
      return {
        ifIndex: iface.ifIndex,
        ifName: iface.ifName || `if-${iface.ifIndex}`,
        ifDescr: iface.ifDescr,
        macAddress: iface.macAddress,
        ifSpeed: iface.ifSpeed,
        ifAdminStatus: iface.ifAdminStatus === 1 ? 'up' : 'down',
        ifOperStatus: iface.ifOperStatus === 1 ? 'up' : 'down',
        isMonitored: match ? match.adminStatus === 'up' : true,
      }
    })

    const hasCpuMonitor = existingMonitors.some(
      (m) => m.name.toLowerCase().includes('cpu') && m.enabled
    )
    const hasMemoryMonitor = existingMonitors.some(
      (m) =>
        (m.name.toLowerCase().includes('memoria') || m.name.toLowerCase().includes('memory')) &&
        m.enabled
    )

    return {
      systemInfo,
      cpuInfo,
      memoryInfo,
      interfaces,
      hasCpuMonitor,
      hasMemoryMonitor,
    }
  }

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

    // 4. CPU & Memory Metrics
    const cpuInfo = await this.cpuCollector.collect(client)
    if (cpuInfo.usagePercent !== undefined) {
      await Metric.create({
        deviceId: device.id,
        name: 'cpu_usage',
        value: cpuInfo.usagePercent,
        unit: '%',
        recordedAt: DateTime.now(),
      })
      metricCount++
    }
    if (cpuInfo.load1min !== undefined) {
      await Metric.create({
        deviceId: device.id,
        name: 'cpu_load_1min',
        value: cpuInfo.load1min,
        unit: 'load',
        recordedAt: DateTime.now(),
      })
      metricCount++
    }

    const memoryInfo = await this.memoryCollector.collect(client)
    if (memoryInfo.usedPercent !== undefined) {
      await Metric.create({
        deviceId: device.id,
        name: 'memory_usage',
        value: memoryInfo.usedPercent,
        unit: '%',
        recordedAt: DateTime.now(),
      })
      metricCount++
    }

    // 5. LLDP / CDP Neighbors
    const neighbors = await this.lldpCollector.collect(client)
    if (neighbors.length > 0) {
      await this.topologyService.resolveDiscoveredNeighbors(device, neighbors)
    }

    return {
      systemInfo,
      cpuInfo,
      memoryInfo,
      interfaceCount: savedIfaces.length,
      metricCount,
      neighborCount: neighbors.length,
    }
  }
}
