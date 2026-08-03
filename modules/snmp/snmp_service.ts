import { SnmpSessionFactory } from './snmp_session_factory.js'
import type { SnmpConfig } from './clients/snmp_client.js'
import { SystemCollector } from './collectors/system_collector.js'
import { InterfaceCollector } from './collectors/interface_collector.js'
import { TrafficCollector, type InterfaceTraffic } from './collectors/traffic_collector.js'
import { LldpCollector } from './collectors/lldp_collector.js'
import { CpuCollector } from './collectors/cpu_collector.js'
import { MemoryCollector } from './collectors/memory_collector.js'
import Device from '#models/device'
import DeviceInterface from '#models/device_interface'
import Metric from '#models/metric'
import Monitor from '#models/monitor'
import { DateTime } from 'luxon'
import { TopologyService } from '#modules/topology/topology_service'
import { InterfaceMonitoringService } from '#modules/monitoring/interface_monitoring_service'

export class SnmpService {
  private factory = new SnmpSessionFactory()
  private systemCollector = new SystemCollector()
  private interfaceCollector = new InterfaceCollector()
  private trafficCollector = new TrafficCollector()
  private lldpCollector = new LldpCollector()
  private cpuCollector = new CpuCollector()
  private memoryCollector = new MemoryCollector()
  private topologyService = new TopologyService()
  private interfaceMonitoringService = new InterfaceMonitoringService()

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

    // 2. Interfaces (Preserva adminStatus configurado pelo usuário para monitoramento)
    const discoveredIfaces = await this.interfaceCollector.collect(client)
    const savedIfaces: DeviceInterface[] = []

    for (const ifaceData of discoveredIfaces) {
      let iface = await DeviceInterface.query()
        .where('deviceId', device.id)
        .where('snmpIndex', ifaceData.ifIndex)
        .first()

      const previousOperStatus = iface ? iface.operStatus : null
      const previousSpeed = iface ? iface.speed : null

      if (!iface) {
        iface = new DeviceInterface()
        iface.deviceId = device.id
        iface.snmpIndex = ifaceData.ifIndex
        iface.adminStatus = ifaceData.ifAdminStatus === 1 ? 'up' : 'down'
      }

      iface.name = ifaceData.ifName || `if-${ifaceData.ifIndex}`
      iface.description = ifaceData.ifDescr || null
      iface.alias = ifaceData.ifAlias || null
      iface.macAddress = ifaceData.macAddress || null
      iface.speed = ifaceData.ifSpeed || null
      iface.operStatus = ifaceData.ifOperStatus === 1 ? 'up' : 'down'
      iface.lastSeenAt = DateTime.now()

      await iface.save()
      savedIfaces.push(iface)

      if (iface.adminStatus === 'up') {
        await this.interfaceMonitoringService.evaluateInterfaceState(
          device,
          iface,
          previousOperStatus,
          previousSpeed
        )
      }
    }

    // Consulta monitores ativos para o dispositivo
    const activeMonitors = await Monitor.query()
      .where('deviceId', device.id)
      .where('enabled', true)

    const hasCpuMonitor = activeMonitors.some((m) => m.name.toLowerCase().includes('cpu'))
    const hasMemoryMonitor = activeMonitors.some((m) => {
      const name = m.name.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase()
      return name.includes('memoria') || name.includes('memory')
    })

    // 3. Traffic Metrics (Apenas para interfaces selecionadas/monitoradas)
    const trafficList = await this.trafficCollector.collect(client)
    let metricCount = 0

    for (const traffic of trafficList) {
      const targetIface = savedIfaces.find((i) => i.snmpIndex === traffic.ifIndex)
      if (!targetIface) continue

      // Verifica se a interface está configurada para monitoramento
      const isMonitored =
        targetIface.adminStatus === 'up' ||
        activeMonitors.some(
          (m) =>
            m.name.toLowerCase().includes(targetIface.name.toLowerCase()) ||
            ((m.configuration as Record<string, unknown>)?.ifIndex &&
              (m.configuration as Record<string, unknown>).ifIndex === targetIface.snmpIndex)
        )

      if (!isMonitored) {
        continue
      }

      const lastIn = await Metric.query()
        .where('deviceId', device.id)
        .where('interfaceId', targetIface.id)
        .whereIn('name', ['ifHCInOctets', 'ifInOctets'])
        .orderBy('recordedAt', 'desc')
        .first()

      const lastOut = await Metric.query()
        .where('deviceId', device.id)
        .where('interfaceId', targetIface.id)
        .whereIn('name', ['ifHCOutOctets', 'ifOutOctets'])
        .orderBy('recordedAt', 'desc')
        .first()

      let inBps = 0
      let outBps = 0

      if (lastIn && lastOut && lastIn.recordedAt) {
        try {
          const parseDate = (val: unknown): Date => {
            if (val instanceof Date) return val
            if (val && typeof (val as any).toJSDate === 'function') return (val as any).toJSDate()
            if (typeof val === 'string') return new Date(val)
            return new Date()
          }

          const prevTraffic: InterfaceTraffic = {
            ifIndex: traffic.ifIndex,
            inOctets: Number(lastIn.value) || 0,
            outOctets: Number(lastOut.value) || 0,
            inErrors: 0,
            outErrors: 0,
            recordedAt: parseDate(lastIn.recordedAt),
          }
          const rates = this.trafficCollector.calculateRates(prevTraffic, traffic)
          inBps = rates.inBps
          outBps = rates.outBps
        } catch {}
      }

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

      if (inBps >= 0) {
        await Metric.create({
          deviceId: device.id,
          interfaceId: targetIface.id,
          name: 'inBps',
          value: inBps,
          unit: 'bps',
          recordedAt: DateTime.fromJSDate(traffic.recordedAt),
        })
      }

      if (outBps >= 0) {
        await Metric.create({
          deviceId: device.id,
          interfaceId: targetIface.id,
          name: 'outBps',
          value: outBps,
          unit: 'bps',
          recordedAt: DateTime.fromJSDate(traffic.recordedAt),
        })
      }

      metricCount += 4
    }

    // 4. CPU & Memory Metrics (Apenas se o monitoramento de CPU / Memória estiver ativo)
    if (hasCpuMonitor || activeMonitors.length === 0) {
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
    }

    if (hasMemoryMonitor || activeMonitors.length === 0) {
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
    }

    // 5. LLDP / CDP Neighbors
    const neighbors = await this.lldpCollector.collect(client)
    if (neighbors.length > 0) {
      await this.topologyService.resolveDiscoveredNeighbors(device, neighbors)
    }

    return {
      systemInfo,
      interfaceCount: savedIfaces.filter((i) => i.adminStatus === 'up').length,
      metricCount,
      neighborCount: neighbors.length,
    }
  }
}
