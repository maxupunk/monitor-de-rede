import { DateTime } from 'luxon'
import type { DiscoveredHost } from './scanners/icmp_scanner.js'
import { IcmpScanner } from './scanners/icmp_scanner.js'
import { ArpScanner } from './scanners/arp_scanner.js'
import { PortScanner } from './scanners/port_scanner.js'
import { DiscoveryMerger } from './discovery_merger.js'
import DiscoveryRun from '#models/discovery_run'
import DiscoveryResult from '#models/discovery_result'
import { EventBus } from '#modules/events/event_bus'

export class DiscoveryService {
  private icmpScanner = new IcmpScanner()
  private arpScanner = new ArpScanner()
  private portScanner = new PortScanner()
  private merger = new DiscoveryMerger()
  private eventBus = EventBus.getInstance()

  async runDiscovery(cidr: string, networkId?: number, probeId?: number | null): Promise<DiscoveredHost[]> {
    let runRecord: DiscoveryRun | null = null

    if (networkId) {
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
      const [icmpRes, arpRes] = await Promise.all([
        this.icmpScanner.scanNetwork(cidr),
        this.arpScanner.scanNetwork(),
      ])

      const mergedBasic = this.merger.mergeResults([icmpRes, arpRes])
      const finalHosts = await this.portScanner.scanHosts(mergedBasic)

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
      const message = err instanceof Error ? err.message : String(err)
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
