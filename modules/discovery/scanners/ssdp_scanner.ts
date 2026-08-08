import dgram from 'node:dgram'
import type { DiscoveredHost } from './icmp_scanner.js'

const SSDP_MULTICAST_IP = '239.255.255.250'
const SSDP_PORT = 1900
const SCAN_TIMEOUT_MS = 2000

interface SsdpResponse {
  ipAddress: string
  server?: string
  location?: string
  usn?: string
  st?: string
}

/**
 * Scanner SSDP/UPnP. Envia M-SEARCH multicast e escuta respostas para descobrir
 * dispositivos na rede local (smart TVs, roteadores, câmeras, etc.).
 */
export class SsdpScanner {
  async scanSsdp(): Promise<DiscoveredHost[]> {
    return new Promise((resolve) => {
      const socket = dgram.createSocket({ type: 'udp4', reuseAddr: true })
      const responses = new Map<string, SsdpResponse>()
      let finished = false

      const finish = () => {
        if (finished) return
        finished = true
        try {
          socket.close()
        } catch {
          // socket pode já estar fechado
        }

        const discovered: DiscoveredHost[] = []
        for (const [, resp] of responses) {
          discovered.push({
            ipAddress: resp.ipAddress,
            vendor: resp.server ? this.extractVendor(resp.server) : undefined,
            confidence: 60,
            data: {
              source: 'ssdp',
              server: resp.server,
              location: resp.location,
              usn: resp.usn,
              st: resp.st,
            },
          })
        }
        resolve(discovered)
      }

      socket.on('error', () => finish())

      socket.on('message', (msg, rinfo) => {
        try {
          const text = msg.toString('utf-8')
          if (!text.startsWith('HTTP/1.1 200 OK')) return

          const headers = this.parseHeaders(text)
          const existing = responses.get(rinfo.address) || { ipAddress: rinfo.address }

          existing.server = headers.server || existing.server
          existing.location = headers.location || existing.location
          existing.usn = headers.usn || existing.usn
          existing.st = headers.st || existing.st

          responses.set(rinfo.address, existing)
        } catch {
          // Ignora pacotes malformados
        }
      })

      socket.bind(SSDP_PORT, () => {
        try {
          socket.addMembership(SSDP_MULTICAST_IP)
          socket.setBroadcast(true)

          const query = this.buildQuery()
          socket.send(query, 0, query.length, SSDP_PORT, SSDP_MULTICAST_IP, (err) => {
            if (err) finish()
          })
        } catch {
          finish()
        }
      })

      setTimeout(finish, SCAN_TIMEOUT_MS)
    })
  }

  private buildQuery(): Buffer {
    const query = [
      'M-SEARCH * HTTP/1.1',
      `HOST: ${SSDP_MULTICAST_IP}:${SSDP_PORT}`,
      'MAN: "ssdp:discover"',
      'MX: 2',
      'ST: ssdp:all',
      '',
      '',
    ].join('\r\n')
    return Buffer.from(query)
  }

  private parseHeaders(response: string): Record<string, string> {
    const headers: Record<string, string> = {}
    const lines = response.split('\r\n')
    for (const line of lines) {
      const separatorIndex = line.indexOf(':')
      if (separatorIndex === -1) continue
      const key = line.slice(0, separatorIndex).trim().toLowerCase()
      const value = line.slice(separatorIndex + 1).trim()
      headers[key] = value
    }
    return headers
  }

  private extractVendor(server?: string): string | undefined {
    if (!server) return undefined
    const known = [
      { key: 'linux', name: 'Linux/UPnP' },
      { key: 'microsoft', name: 'Microsoft' },
      { key: 'asus', name: 'ASUS' },
      { key: 'tp-link', name: 'TP-Link' },
      { key: 'netgear', name: 'NETGEAR' },
      { key: 'd-link', name: 'D-Link' },
      { key: 'synology', name: 'Synology' },
      { key: 'qnap', name: 'QNAP' },
    ]

    const lower = server.toLowerCase()
    for (const vendor of known) {
      if (lower.includes(vendor.key)) return vendor.name
    }

    return undefined
  }
}
