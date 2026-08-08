import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import net from 'node:net'
import type { DiscoveredHost } from './icmp_scanner.js'

const execFileAsync = promisify(execFile)

const ARP_BATCH_SIZE = 20
const ARP_PROBE_TIMEOUT_MS = 800

export class ArpScanner {
  /**
   * Lê a tabela ARP do sistema. Quando `targetIps` é fornecido, dispara probes
   * TCP (porta 80/443/22 fallback) contra cada IP antes da leitura para forçar
   * a resolução ARP ativa e aumentar a chance de encontrar MACs fora do cache.
   */
  async scanNetwork(targetIps?: string[]): Promise<DiscoveredHost[]> {
    if (targetIps && targetIps.length > 0) {
      await this.probeHosts(targetIps)
    }

    return this.readArpTable()
  }

  private async probeHosts(ips: string[]): Promise<void> {
    for (let i = 0; i < ips.length; i += ARP_BATCH_SIZE) {
      const batch = ips.slice(i, i + ARP_BATCH_SIZE)
      await Promise.all(batch.map((ip) => this.probeHost(ip)))
    }
  }

  private probeHost(ip: string): Promise<void> {
    return new Promise((resolve) => {
      const socket = new net.Socket()
      socket.setTimeout(ARP_PROBE_TIMEOUT_MS)

      socket.on('connect', () => {
        socket.destroy()
        resolve()
      })
      socket.on('timeout', () => {
        socket.destroy()
        resolve()
      })
      socket.on('error', () => {
        socket.destroy()
        resolve()
      })

      // Portas comuns: a conexão falha rapidamente se fechada, mas já gera o
      // pacote ARP necessário para popular o cache local.
      socket.connect(80, ip)
    })
  }

  private async readArpTable(): Promise<DiscoveredHost[]> {
    const discovered: DiscoveredHost[] = []

    try {
      const { stdout } = await execFileAsync('arp', ['-a'])
      const lines = stdout.split('\n')

      for (const line of lines) {
        const ipMacMatch = line.match(
          /(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})\s+([0-9a-fA-F:-]{11,17})/
        )
        if (ipMacMatch) {
          const ipAddress = ipMacMatch[1]
          let macAddress = ipMacMatch[2].toUpperCase().replace(/-/g, ':')

          if (
            macAddress !== 'FF:FF:FF:FF:FF:FF' &&
            !macAddress.startsWith('01:00:5E') &&
            !macAddress.startsWith('33:33') &&
            !ipAddress.startsWith('224.')
          ) {
            discovered.push({
              ipAddress,
              macAddress,
              confidence: 80,
            })
          }
        }
      }
    } catch {
      // Ignorar exceção do comando ARP caso o binário de sistema não permita execução
    }

    return discovered
  }
}
