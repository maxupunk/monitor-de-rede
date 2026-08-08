import { DateTime } from 'luxon'
import type { DiscoveredHost } from './scanners/icmp_scanner.js'
import { IcmpScanner } from './scanners/icmp_scanner.js'
import { ArpScanner } from './scanners/arp_scanner.js'
import { PortScanner } from './scanners/port_scanner.js'
import { MdnsScanner } from './scanners/mdns_scanner.js'
import { SsdpScanner } from './scanners/ssdp_scanner.js'
import { SnmpDiscoveryScanner } from './scanners/snmp_discovery_scanner.js'
import { DiscoveryMerger } from './discovery_merger.js'
import DiscoveryRun from '#models/discovery_run'
import DiscoveryResult from '#models/discovery_result'
import { errorMessage } from '#modules/shared/errors'
import { EventBus } from '#modules/events/event_bus'

export class DiscoveryService {
  private icmpScanner = new IcmpScanner()
  private arpScanner = new ArpScanner()
  private portScanner = new PortScanner()
  private mdnsScanner = new MdnsScanner()
  private ssdpScanner = new SsdpScanner()
  private snmpDiscoveryScanner = new SnmpDiscoveryScanner()
  private merger = new DiscoveryMerger()
  private eventBus = EventBus.getInstance()

  /**
   * Executa a varredura de uma faixa.
   *
   * `existingRun` permite retomar uma execução já enfileirada — é assim que o
   * scheduler processa as varreduras criadas por `POST /api/networks/:id/scan`
   * sem gerar um segundo registro no histórico.
   */
  async runDiscovery(
    cidr: string,
    networkId?: number,
    probeId?: number | null,
    existingRun?: DiscoveryRun | null
  ): Promise<DiscoveredHost[]> {
    let runRecord: DiscoveryRun | null = existingRun ?? null

    if (runRecord) {
      runRecord.status = 'running'
      runRecord.startedAt = DateTime.now()
      await runRecord.save()
    } else if (networkId) {
      runRecord = await DiscoveryRun.create({
        networkId,
        probeId: probeId || null,
        status: 'running',
        startedAt: DateTime.now(),
        configuration: { cidr },
      })
    }

    this.eventBus.emit('discovery:started', {
      runId: runRecord?.id ?? null,
      networkId: networkId ?? null,
      probeId: probeId ?? null,
      cidr,
      status: 'running',
    })

    try {
      const icmpRes = await this.icmpScanner.scanNetwork(cidr)
      const icmpIps = icmpRes.map((h) => h.ipAddress)

      const [arpRes, mdnsRes, ssdpRes] = await Promise.all([
        this.arpScanner.scanNetwork(icmpIps),
        this.mdnsScanner.scanMdns(),
        this.ssdpScanner.scanSsdp(),
      ])

      const mergedBasic = this.merger.mergeResults([icmpRes, arpRes, mdnsRes, ssdpRes])
      const portScannedHosts = await this.portScanner.scanHosts(mergedBasic)
      const snmpHosts = await this.snmpDiscoveryScanner.scanHosts(portScannedHosts)
      const finalHosts = this.merger.mergeResults([portScannedHosts, snmpHosts])

      if (runRecord) {
        const now = DateTime.now()
        for (const host of finalHosts) {
          await DiscoveryResult.create({
            discoveryRunId: runRecord.id,
            ipAddress: host.ipAddress,
            macAddress: host.macAddress || null,
            hostname: host.hostname || null,
            mdnsName: host.mdnsName || null,
            vendor: host.vendor || null,
            deviceType: host.deviceType || 'unknown',
            confidence: host.confidence || 50,
            status: 'pending',
            data: host.data || {},
            firstSeenAt: now,
            lastSeenAt: now,
          })
        }

        runRecord.status = 'completed'
        runRecord.finishedAt = now
        await runRecord.save()
      }

      this.eventBus.emit('discovery:completed', {
        runId: runRecord?.id ?? null,
        networkId: networkId ?? null,
        cidr,
        status: 'completed',
        hostsFound: finalHosts.length,
      })

      return finalHosts
    } catch (err: unknown) {
      const message = errorMessage(err)
      if (runRecord) {
        runRecord.status = 'failed'
        runRecord.finishedAt = DateTime.now()
        runRecord.error = message
        await runRecord.save()
      }

      this.eventBus.emit('discovery:failed', {
        runId: runRecord?.id ?? null,
        networkId: networkId ?? null,
        cidr,
        status: 'failed',
        error: message,
      })

      throw err
    }
  }
}
