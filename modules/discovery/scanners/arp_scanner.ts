import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import type { DiscoveredHost } from './icmp_scanner.js'

const execFileAsync = promisify(execFile)

export class ArpScanner {
  async scanNetwork(): Promise<DiscoveredHost[]> {
    const discovered: DiscoveredHost[] = []

    try {
      const { stdout } = await execFileAsync('arp', ['-a'])
      const lines = stdout.split('\n')

      for (const line of lines) {
        const ipMacMatch = line.match(/(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})\s+([0-9a-fA-F:-]{11,17})/)
        if (ipMacMatch) {
          const ipAddress = ipMacMatch[1]
          let macAddress = ipMacMatch[2].toUpperCase().replace(/-/g, ':')

          if (macAddress !== 'FF:FF:FF:FF:FF:FF' && !macAddress.startsWith('224.') && !ipAddress.startsWith('224.')) {
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
