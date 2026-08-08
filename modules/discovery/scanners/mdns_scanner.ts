import dgram from 'node:dgram'
import type { DiscoveredHost } from './icmp_scanner.js'

const MDNS_MULTICAST_IP = '224.0.0.251'
const MDNS_PORT = 5353
const SCAN_TIMEOUT_MS = 2000

/**
 * Scanner mDNS/Bonjour. Envia uma query multicast e escuta anúncios na rede
 * local para descobrir hostnames `.local` e IPs de dispositivos.
 *
 * O parse é intencionalmente minimalista: extrai nomes `.local` e endereços
 * IPv4 dos pacotes de resposta, sem depender de bibliotecas externas.
 */
export class MdnsScanner {
  async scanMdns(): Promise<DiscoveredHost[]> {
    return new Promise((resolve) => {
      const socket = dgram.createSocket({ type: 'udp4', reuseAddr: true })
      const hostsByIp = new Map<string, { mdnsName?: string; confidence: number }>()
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
        for (const [ipAddress, info] of hostsByIp) {
          discovered.push({
            ipAddress,
            mdnsName: info.mdnsName,
            confidence: info.confidence,
            data: { source: 'mdns' },
          })
        }
        resolve(discovered)
      }

      socket.on('error', () => finish())

      socket.on('message', (msg, rinfo) => {
        try {
          const parsed = this.parseResponse(msg)
          const ipAddress = rinfo.address
          const existing = hostsByIp.get(ipAddress) || { confidence: 70 }

          if (parsed.name && parsed.name.endsWith('.local')) {
            existing.mdnsName = parsed.name.replace(/\.$/, '')
          }
          if (parsed.ip && !parsed.ip.startsWith('0.')) {
            hostsByIp.set(parsed.ip, existing)
          } else {
            hostsByIp.set(ipAddress, existing)
          }
        } catch {
          // Ignora pacotes malformados
        }
      })

      socket.bind(MDNS_PORT, () => {
        try {
          socket.addMembership(MDNS_MULTICAST_IP)
          socket.setBroadcast(true)

          const query = this.buildQuery()
          socket.send(query, 0, query.length, MDNS_PORT, MDNS_MULTICAST_IP, (err) => {
            if (err) finish()
          })
        } catch {
          finish()
        }
      })

      setTimeout(finish, SCAN_TIMEOUT_MS)
    })
  }

  /**
   * Monta um query packet mDNS PTR para `_services._dns-sd._udp.local`.
   */
  private buildQuery(): Buffer {
    const name = this.encodeDnsName('_services._dns-sd._udp.local')
    const header = Buffer.alloc(12)
    header.writeUInt16BE(0, 0) // transaction id
    header.writeUInt16BE(0x0000, 2) // flags: query
    header.writeUInt16BE(1, 4) // questions
    header.writeUInt16BE(0, 6) // answer RRs
    header.writeUInt16BE(0, 8) // authority RRs
    header.writeUInt16BE(0, 10) // additional RRs

    const questionType = Buffer.alloc(4)
    questionType.writeUInt16BE(12, 0) // PTR
    questionType.writeUInt16BE(0x0001, 2) // IN

    return Buffer.concat([header, name, questionType])
  }

  private encodeDnsName(name: string): Buffer {
    const parts = name.split('.')
    const buffers: Buffer[] = []
    for (const part of parts) {
      const len = Buffer.from([part.length])
      buffers.push(len, Buffer.from(part))
    }
    buffers.push(Buffer.from([0]))
    return Buffer.concat(buffers)
  }

  private parseResponse(msg: Buffer): { name?: string; ip?: string } {
    let offset = 12
    const result: { name?: string; ip?: string } = {}

    try {
      const questions = msg.readUInt16BE(4)
      const answers = msg.readUInt16BE(6)

      for (let i = 0; i < questions && offset < msg.length; i++) {
        this.skipName(msg, offset)
        offset += 4
      }

      for (let i = 0; i < answers && offset < msg.length; i++) {
        const { name, nextOffset } = this.readName(msg, offset)
        offset = nextOffset

        if (offset + 10 > msg.length) break
        const type = msg.readUInt16BE(offset)
        offset += 2
        // const classCode = msg.readUInt16BE(offset)
        offset += 2
        // const ttl = msg.readUInt32BE(offset)
        offset += 4
        const rdlength = msg.readUInt16BE(offset)
        offset += 2

        if (offset + rdlength > msg.length) break

        if (type === 1 && rdlength === 4) {
          // A record
          const ip = Array.from(msg.slice(offset, offset + 4)).join('.')
          result.ip = ip
          if (name?.endsWith('.local')) {
            result.name = name
          }
        } else if (type === 12 && name?.endsWith('.local')) {
          // PTR record
          const { name: targetName } = this.readName(msg, offset)
          result.name = targetName || result.name
        } else if (name?.endsWith('.local')) {
          result.name = name
        }

        offset += rdlength
      }
    } catch {
      // Pacote malformado — retorna o que conseguiu
    }

    return result
  }

  private skipName(msg: Buffer, offset: number): number {
    return this.readName(msg, offset).nextOffset
  }

  private readName(msg: Buffer, offset: number): { name: string; nextOffset: number } {
    const labels: string[] = []
    let jumped = false
    let originalOffset = offset

    while (offset < msg.length) {
      const len = msg[offset]
      if (len === 0) {
        offset++
        break
      }
      if ((len & 0xc0) === 0xc0) {
        if (!jumped) {
          originalOffset = offset + 2
        }
        offset = ((len & 0x3f) << 8) | msg[offset + 1]
        jumped = true
        continue
      }
      offset++
      labels.push(msg.slice(offset, offset + len).toString('utf-8'))
      offset += len
    }

    return { name: labels.join('.'), nextOffset: jumped ? originalOffset : offset }
  }
}
