import { DateTime } from 'luxon'
import type { DiscoveredHost } from './scanners/icmp_scanner.js'
import { IcmpScanner } from './scanners/icmp_scanner.js'
import { ArpScanner } from './scanners/arp_scanner.js'
import { PortScanner } from './scanners/port_scanner.js'
import { MdnsScanner } from './scanners/mdns_scanner.js'
import { SsdpScanner } from './scanners/ssdp_scanner.js'
import { SnmpDiscoveryScanner } from './scanners/snmp_discovery_scanner.js'
import { DiscoveryMerger } from './discovery_merger.js'
import { expandCidr } from './cidr_range.js'
import DiscoveryRun from '#models/discovery_run'
import DiscoveryResult from '#models/discovery_result'
import { errorMessage } from '#modules/shared/errors'
import { EventBus } from '#modules/events/event_bus'

export interface DiscoveryCallbacks {
  onProgress?: (phase: string, current: number, total: number) => void
  onResult?: (host: DiscoveredHost) => void
}

function throwIfAborted(signal?: AbortSignal): void {
  if (signal?.aborted) {
    const error = new Error('Varredura cancelada.')
    error.name = 'AbortError'
    throw error
  }
}

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
    existingRun?: DiscoveryRun | null,
    callbacks?: DiscoveryCallbacks,
    signal?: AbortSignal
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

    /** Cache acumulativo de hosts por IP, usado para evitar duplicatas no stream. */
    const hostMap = new Map<string, DiscoveredHost>()

    const emitHost = (host: DiscoveredHost) => {
      hostMap.set(host.ipAddress, host)
      callbacks?.onResult?.(host)
    }

    try {
      throwIfAborted(signal)
      const totalHosts = this.estimateHostCount(cidr)

      // 1. ICMP: descobre quem responde ao ping na faixa.
      callbacks?.onProgress?.('icmp', 0, totalHosts)
      const icmpRes = await this.icmpScanner.scanNetwork(cidr, {
        onProgress: (current, total) => callbacks?.onProgress?.('icmp', current, total),
        onResult: (host) => emitHost(host),
        signal,
      })
      callbacks?.onProgress?.('icmp', icmpRes.length, icmpRes.length)

      throwIfAborted(signal)

      // 2. ARP/mDNS/SSDP: enriquece com MAC, nome e fabricante.
      callbacks?.onProgress?.('discovery', 0, icmpRes.length)
      const [arpRes, mdnsRes, ssdpRes] = await Promise.all([
        this.arpScanner.scanNetwork(icmpRes.map((h) => h.ipAddress)),
        this.mdnsScanner.scanMdns(),
        this.ssdpScanner.scanSsdp(),
      ])
      const discovered = this.merger.mergeResults([icmpRes, arpRes, mdnsRes, ssdpRes])
      for (const host of discovered) {
        emitHost(host)
      }
      callbacks?.onProgress?.('discovery', discovered.length, discovered.length)

      throwIfAborted(signal)

      // 3. Portas: verifica portas abertas em cada host conhecido.
      callbacks?.onProgress?.('ports', 0, discovered.length)
      const portScannedHosts: DiscoveredHost[] = []
      for (let i = 0; i < discovered.length; i++) {
        throwIfAborted(signal)
        const host = discovered[i]
        const openPorts = await this.portScanner.scanPortsForIp(host.ipAddress, signal)
        const updated: DiscoveredHost = {
          ...host,
          openPorts,
          confidence: openPorts.length > 0 ? Math.min(100, host.confidence + 20) : host.confidence,
        }
        portScannedHosts.push(updated)
        emitHost(updated)
        callbacks?.onProgress?.('ports', i + 1, discovered.length)
      }

      throwIfAborted(signal)

      // 4. SNMP: tenta coletar informações do sistema.
      callbacks?.onProgress?.('snmp', 0, portScannedHosts.length)
      const snmpHosts = await this.snmpDiscoveryScanner.scanHosts(portScannedHosts, signal)
      const finalHosts = this.merger.mergeResults([portScannedHosts, snmpHosts])
      for (let i = 0; i < finalHosts.length; i++) {
        emitHost(finalHosts[i])
        callbacks?.onProgress?.('snmp', i + 1, finalHosts.length)
      }

      if (runRecord) {
        const now = DateTime.now()

        // Discovery_results é apenas o cache do último scan: limpa resultados
        // anteriores antes de salvar os novos.
        await DiscoveryResult.query().delete()

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

  private estimateHostCount(cidr: string): number {
    try {
      return expandCidr(cidr).length
    } catch {
      return 100
    }
  }
}
