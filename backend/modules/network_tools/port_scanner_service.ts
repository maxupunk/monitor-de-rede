import net from 'node:net'
import dgram from 'node:dgram'
import { UdpProbeRegistry } from './udp_probe_registry.js'

export type PortProtocol = 'tcp' | 'udp'
export type PortStatus = 'open' | 'closed' | 'open|filtered'

export interface PortScanItem {
  port: number
  protocol: PortProtocol
  status: PortStatus
  service?: string
  latencyMs: number
}

const TCP_SERVICE_NAMES: Record<number, string> = {
  21: 'FTP',
  22: 'SSH',
  23: 'Telnet',
  25: 'SMTP',
  53: 'DNS',
  80: 'HTTP',
  110: 'POP3',
  111: 'RPCBind',
  135: 'MS-RPC',
  139: 'NetBIOS',
  143: 'IMAP',
  161: 'SNMP',
  389: 'LDAP',
  443: 'HTTPS',
  445: 'SMB',
  465: 'SMTPS',
  587: 'SMTP (Submission)',
  993: 'IMAPS',
  995: 'POP3S',
  1433: 'MSSQL',
  1521: 'Oracle DB',
  2049: 'NFS',
  3306: 'MySQL',
  3389: 'RDP',
  5060: 'SIP',
  5432: 'PostgreSQL',
  5900: 'VNC',
  6379: 'Redis',
  8000: 'HTTP-Alt',
  8080: 'HTTP-Proxy',
  8443: 'HTTPS-Alt',
  9000: 'HTTP-Alt',
  27017: 'MongoDB',
}

const UDP_SERVICE_NAMES: Record<number, string> = {
  53: 'DNS',
  67: 'DHCP Server',
  68: 'DHCP Client',
  69: 'TFTP',
  123: 'NTP',
  137: 'NetBIOS-NS',
  138: 'NetBIOS-DGM',
  161: 'SNMP',
  162: 'SNMP Trap',
  500: 'IKE/IPSec',
  514: 'Syslog',
  520: 'RIP',
  1900: 'SSDP',
  4500: 'IPSec NAT-T',
  5353: 'mDNS',
}

// Muitos alvos são roteadores/equipamentos embarcados de baixa capacidade (ex: CPE, APs) —
// disparar dezenas de handshakes TCP simultâneos contra eles sobrecarrega a tabela de
// conntrack/firewall do próprio equipamento e gera falsos positivos/negativos (portas reais
// não respondem a tempo, ou o equipamento "confunde" o estado de conexões concorrentes).
// Concorrência mais baixa é mais lenta em varreduras grandes, mas os resultados são confiáveis.
const DEFAULT_CONCURRENCY = 16

export interface PortScanOptions {
  // Chamado assim que cada porta individual termina de ser verificada — permite ao
  // chamador transmitir o progresso em tempo real, em vez de esperar a varredura inteira.
  onResult?: (item: PortScanItem) => void
  // Quando abortado, os workers param de retirar novas portas da fila — o lote (até
  // DEFAULT_CONCURRENCY) que já estava em voo termina normalmente, mas nada novo é iniciado.
  signal?: AbortSignal
}

export class PortScannerService {
  async scan(
    host: string,
    ports: number[],
    protocol: PortProtocol,
    timeoutMs = 1500,
    options: PortScanOptions = {}
  ): Promise<PortScanItem[]> {
    const { onResult, signal } = options
    const results: PortScanItem[] = []
    const queue = [...ports]
    const scanPort = protocol === 'tcp' ? this.scanTcpPort.bind(this) : this.scanUdpPort.bind(this)

    const worker = async () => {
      while (queue.length > 0 && !signal?.aborted) {
        const port = queue.shift()
        if (port === undefined) break
        const item = await scanPort(host, port, timeoutMs)
        results.push(item)
        onResult?.(item)
      }
    }

    const workerCount = Math.min(DEFAULT_CONCURRENCY, ports.length)
    await Promise.all(Array.from({ length: workerCount }, () => worker()))

    return results.sort((a, b) => a.port - b.port)
  }

  private scanTcpPort(host: string, port: number, timeoutMs: number): Promise<PortScanItem> {
    return new Promise((resolve) => {
      const startedAt = Date.now()
      const socket = new net.Socket()
      socket.setTimeout(timeoutMs)

      const finish = (status: PortStatus) => {
        socket.destroy()
        resolve({
          port,
          protocol: 'tcp',
          status,
          service: TCP_SERVICE_NAMES[port],
          latencyMs: Date.now() - startedAt,
        })
      }

      socket.on('connect', () => finish('open'))
      socket.on('timeout', () => finish('closed'))
      socket.on('error', () => finish('closed'))

      socket.connect(port, host)
    })
  }

  private scanUdpPort(host: string, port: number, timeoutMs: number): Promise<PortScanItem> {
    return new Promise((resolve) => {
      const startedAt = Date.now()
      const socket = dgram.createSocket('udp4')
      let settled = false

      const finish = (status: PortStatus) => {
        if (settled) return
        settled = true
        clearTimeout(timer)
        try {
          socket.close()
        } catch {}
        resolve({
          port,
          protocol: 'udp',
          status,
          service: UDP_SERVICE_NAMES[port],
          latencyMs: Date.now() - startedAt,
        })
      }

      // Sem resposta dentro do prazo não significa porta aberta (UDP não confirma
      // recebimento) — só um ICMP "port unreachable" (reportado pelo SO como ECONNREFUSED
      // em socket conectado) confirma que está fechada; do contrário é "open|filtered".
      const timer = setTimeout(() => finish('open|filtered'), timeoutMs)

      socket.on('message', () => finish('open'))
      socket.on('error', (err: NodeJS.ErrnoException) => {
        finish(err.code === 'ECONNREFUSED' ? 'closed' : 'open|filtered')
      })

      socket.connect(port, host, () => {
        const probe = UdpProbeRegistry.getProbe(port)
        socket.send(probe, (err) => {
          if (err) finish('closed')
        })
      })
    })
  }
}
