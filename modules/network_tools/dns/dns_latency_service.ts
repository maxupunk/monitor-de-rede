import dgram from 'node:dgram'
import net from 'node:net'
import dns from 'node:dns/promises'
import { randomInt } from 'node:crypto'
import { performance } from 'node:perf_hooks'
import { Buffer } from 'node:buffer'
import {
  DNS_RCODE_MESSAGES,
  DnsProtocolError,
  decodeDnsMessage,
  encodeDnsQuery,
  type DnsAnswer,
  type DnsRecordType,
} from './dns_wire.js'

/**
 * Medição de latência de resolução DNS.
 *
 * `udp` e `tcp` falam o protocolo direto com o servidor escolhido; `doh` usa
 * DNS over HTTPS (RFC 8484, wire format em POST); `system` delega ao resolvedor
 * do sistema operacional. Em todos os casos o cronômetro é o `performance.now()`
 * (resolução de microssegundos, relógio monotônico — imune a ajustes de hora do
 * sistema) e cobre exclusivamente a etapa de resolução do nome.
 */

export type DnsProtocol = 'udp' | 'tcp' | 'doh' | 'system'

export const DEFAULT_DNS_PORT = 53
export const DEFAULT_DNS_TIMEOUT_MS = 3000

/** Tipos de registro que devolvem um endereço final */
const ADDRESS_RECORD_TYPES = new Set(['A', 'AAAA'])

export interface DnsLookupOptions {
  hostname: string
  recordType?: DnsRecordType
  /** IP do servidor, aceita `ip:porta`. Ignorado nos protocolos `doh` e `system` */
  server?: string
  protocol?: DnsProtocol
  /** Endpoint DoH (ex.: https://cloudflare-dns.com/dns-query) */
  dohUrl?: string
  timeoutMs?: number
}

export interface DnsLookupSample {
  hostname: string
  recordType: string
  protocol: DnsProtocol
  /** Servidor efetivamente consultado (ou o endpoint DoH / "sistema") */
  server: string
  success: boolean
  /** Tempo da etapa de resolução, em milissegundos com casas decimais */
  lookupTimeMs: number
  addresses: string[]
  answers: DnsAnswer[]
  rcode: number | null
  rcodeLabel: string | null
  /** A resposta UDP veio truncada e foi refeita via TCP */
  usedTcpFallback: boolean
  error: string | null
}

export class DnsLatencyError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'DnsLatencyError'
  }
}

/** Separa `ip:porta` preservando IPv6 literal entre colchetes */
export function parseServerAddress(raw: string): { host: string; port: number } {
  const value = (raw || '').trim()
  if (!value) throw new DnsLatencyError('Servidor DNS não informado')

  const bracketMatch = value.match(/^\[(.+)\](?::(\d{1,5}))?$/)
  if (bracketMatch && bracketMatch[1]) {
    return { host: bracketMatch[1], port: Number(bracketMatch[2] ?? DEFAULT_DNS_PORT) }
  }

  // Só tratamos como `host:porta` quando há um único ':' (IPv6 puro tem vários)
  const parts = value.split(':')
  if (parts.length === 2 && parts[0] && /^\d{1,5}$/.test(parts[1]!)) {
    return { host: parts[0], port: Number(parts[1]) }
  }

  return { host: value, port: DEFAULT_DNS_PORT }
}

function extractAddresses(answers: DnsAnswer[]): string[] {
  return answers.filter((answer) => ADDRESS_RECORD_TYPES.has(answer.type)).map((a) => a.value)
}

/** Consulta via UDP — o transporte padrão do DNS */
function queryUdp(
  host: string,
  port: number,
  query: Buffer,
  expectedId: number,
  timeoutMs: number
): Promise<Buffer> {
  return new Promise<Buffer>((resolve, reject) => {
    const socket = dgram.createSocket(net.isIPv6(host) ? 'udp6' : 'udp4')
    let settled = false

    const finish = (error: Error | null, response?: Buffer) => {
      if (settled) return
      settled = true
      clearTimeout(timer)
      socket.removeAllListeners()
      try {
        socket.close()
      } catch {
        // Socket já fechado
      }
      if (error) reject(error)
      else resolve(response!)
    }

    const timer = setTimeout(
      () => finish(new DnsLatencyError(`Sem resposta do servidor DNS em ${timeoutMs}ms`)),
      timeoutMs
    )

    socket.on('message', (message) => {
      // Respostas de outra consulta (ou spoof) são descartadas sem encerrar a espera
      if (message.length >= 2 && message.readUInt16BE(0) !== expectedId) return
      finish(null, message)
    })

    socket.on('error', (err: Error) => finish(new DnsLatencyError(err.message)))

    socket.send(query, port, host, (err) => {
      if (err) finish(new DnsLatencyError(`Falha ao enviar consulta DNS: ${err.message}`))
    })
  })
}

/** Consulta via TCP — usada quando a resposta UDP vem truncada ou por escolha explícita */
function queryTcp(host: string, port: number, query: Buffer, timeoutMs: number): Promise<Buffer> {
  return new Promise<Buffer>((resolve, reject) => {
    const socket = new net.Socket()
    const chunks: Buffer[] = []
    let received = 0
    let expectedLength: number | null = null
    let settled = false

    const finish = (error: Error | null, response?: Buffer) => {
      if (settled) return
      settled = true
      socket.removeAllListeners()
      socket.destroy()
      if (error) reject(error)
      else resolve(response!)
    }

    socket.setTimeout(timeoutMs)

    socket.on('connect', () => {
      // DNS sobre TCP prefixa a mensagem com seu tamanho em 2 bytes (RFC 1035 §4.2.2)
      const framed = Buffer.alloc(2 + query.length)
      framed.writeUInt16BE(query.length, 0)
      query.copy(framed, 2)
      socket.write(framed)
    })

    socket.on('data', (chunk: Buffer) => {
      chunks.push(chunk)
      received += chunk.length

      const buffer = Buffer.concat(chunks, received)
      if (expectedLength === null && buffer.length >= 2) {
        expectedLength = buffer.readUInt16BE(0)
      }
      if (expectedLength !== null && buffer.length >= expectedLength + 2) {
        finish(null, buffer.subarray(2, expectedLength + 2))
      }
    })

    socket.on('timeout', () =>
      finish(new DnsLatencyError(`Timeout na consulta DNS via TCP (${timeoutMs}ms)`))
    )
    socket.on('error', (err: Error) => finish(new DnsLatencyError(err.message)))
    socket.on('close', () => {
      if (!settled) finish(new DnsLatencyError('Conexão TCP encerrada antes da resposta DNS'))
    })

    socket.connect(port, host)
  })
}

/** Consulta via DNS over HTTPS em wire format (RFC 8484) */
async function queryDoh(url: string, query: Buffer, timeoutMs: number): Promise<Buffer> {
  let endpoint: URL
  try {
    endpoint = new URL(url)
  } catch {
    throw new DnsLatencyError(`Endpoint DoH inválido: ${url}`)
  }
  if (endpoint.protocol !== 'https:') {
    throw new DnsLatencyError('O endpoint DoH precisa usar https://')
  }

  const response = await fetch(endpoint, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/dns-message',
      'Accept': 'application/dns-message',
    },
    body: new Uint8Array(query),
    signal: AbortSignal.timeout(timeoutMs),
  })

  if (!response.ok) {
    throw new DnsLatencyError(`Servidor DoH respondeu HTTP ${response.status}`)
  }

  return Buffer.from(await response.arrayBuffer())
}

/** Resolução pelo resolvedor do sistema, sem controle de transporte */
async function querySystem(
  hostname: string,
  recordType: DnsRecordType,
  server: string | undefined,
  timeoutMs: number
): Promise<DnsAnswer[]> {
  const resolver = server ? new dns.Resolver({ timeout: timeoutMs }) : dns
  if (server && resolver instanceof dns.Resolver) {
    const { host, port } = parseServerAddress(server)
    resolver.setServers([port === DEFAULT_DNS_PORT ? host : `${host}:${port}`])
  }

  const lookup = async (): Promise<DnsAnswer[]> => {
    switch (recordType) {
      case 'AAAA':
        return (await resolver.resolve6(hostname)).map((value) => ({
          name: hostname,
          type: 'AAAA',
          ttl: 0,
          value,
        }))
      case 'CNAME':
        return (await resolver.resolveCname(hostname)).map((value) => ({
          name: hostname,
          type: 'CNAME',
          ttl: 0,
          value,
        }))
      case 'MX':
        return (await resolver.resolveMx(hostname)).map((record) => ({
          name: hostname,
          type: 'MX',
          ttl: 0,
          value: `${record.priority} ${record.exchange}`,
        }))
      case 'TXT':
        return (await resolver.resolveTxt(hostname)).map((record) => ({
          name: hostname,
          type: 'TXT',
          ttl: 0,
          value: record.join(''),
        }))
      case 'NS':
        return (await resolver.resolveNs(hostname)).map((value) => ({
          name: hostname,
          type: 'NS',
          ttl: 0,
          value,
        }))
      default:
        return (await resolver.resolve4(hostname)).map((value) => ({
          name: hostname,
          type: 'A',
          ttl: 0,
          value,
        }))
    }
  }

  // O timeout do Resolver do Node não cobre todos os caminhos, então garantimos o teto aqui
  return Promise.race([
    lookup(),
    new Promise<DnsAnswer[]>((_, reject) =>
      setTimeout(
        () => reject(new DnsLatencyError(`Timeout de ${timeoutMs}ms na resolução DNS`)),
        timeoutMs
      )
    ),
  ])
}

/** Arredonda para 3 casas mantendo a precisão de microssegundos do performance.now() */
function round(value: number): number {
  return Number(value.toFixed(3))
}

/**
 * Mede o tempo de resolução de um hostname. Nunca lança: falhas viram um sample
 * com `success: false` e a mensagem em `error`, para que o chamador (checker ou
 * benchmark) some as tentativas sem se preocupar com try/catch por consulta.
 */
export async function measureDnsLookup(options: DnsLookupOptions): Promise<DnsLookupSample> {
  const hostname = (options.hostname || '').trim()
  const recordType = (options.recordType || 'A') as DnsRecordType
  const protocol = options.protocol || (options.server ? 'udp' : 'system')
  const timeoutMs = options.timeoutMs && options.timeoutMs > 0 ? options.timeoutMs : DEFAULT_DNS_TIMEOUT_MS

  const serverLabel =
    protocol === 'doh'
      ? options.dohUrl || ''
      : protocol === 'system'
        ? options.server || 'sistema'
        : options.server || ''

  const base: DnsLookupSample = {
    hostname,
    recordType,
    protocol,
    server: serverLabel,
    success: false,
    lookupTimeMs: 0,
    addresses: [],
    answers: [],
    rcode: null,
    rcodeLabel: null,
    usedTcpFallback: false,
    error: null,
  }

  if (!hostname) {
    return { ...base, error: 'Hostname não informado' }
  }
  if (protocol === 'doh' && !options.dohUrl) {
    return { ...base, error: 'Endpoint DoH não informado' }
  }
  if ((protocol === 'udp' || protocol === 'tcp') && !options.server) {
    return { ...base, error: 'Servidor DNS não informado' }
  }

  const startedAt = performance.now()

  try {
    if (protocol === 'system') {
      const answers = await querySystem(hostname, recordType, options.server, timeoutMs)
      const lookupTimeMs = round(performance.now() - startedAt)

      return {
        ...base,
        success: answers.length > 0,
        lookupTimeMs,
        answers,
        addresses: extractAddresses(answers),
        rcode: 0,
        rcodeLabel: 'NOERROR',
        error: answers.length > 0 ? null : 'Nenhum registro retornado',
      }
    }

    const queryId = protocol === 'doh' ? 0 : randomInt(1, 65535)
    const query = encodeDnsQuery(hostname, recordType, queryId)

    let responseBuffer: Buffer
    let usedTcpFallback = false

    if (protocol === 'doh') {
      responseBuffer = await queryDoh(options.dohUrl!, query, timeoutMs)
    } else {
      const { host, port } = parseServerAddress(options.server!)
      responseBuffer =
        protocol === 'tcp'
          ? await queryTcp(host, port, query, timeoutMs)
          : await queryUdp(host, port, query, queryId, timeoutMs)

      // Resposta truncada em UDP: o protocolo manda repetir via TCP
      if (protocol === 'udp') {
        const peek = decodeDnsMessage(responseBuffer)
        if (peek.truncated) {
          usedTcpFallback = true
          responseBuffer = await queryTcp(host, port, query, timeoutMs)
        }
      }
    }

    const lookupTimeMs = round(performance.now() - startedAt)
    const message = decodeDnsMessage(responseBuffer)
    const addresses = extractAddresses(message.answers)

    if (message.rcode !== 0) {
      return {
        ...base,
        lookupTimeMs,
        usedTcpFallback,
        rcode: message.rcode,
        rcodeLabel: message.rcodeLabel,
        answers: message.answers,
        addresses,
        error: DNS_RCODE_MESSAGES[message.rcode] ?? `Servidor respondeu ${message.rcodeLabel}`,
      }
    }

    return {
      ...base,
      success: message.answers.length > 0,
      lookupTimeMs,
      usedTcpFallback,
      rcode: message.rcode,
      rcodeLabel: message.rcodeLabel,
      answers: message.answers,
      addresses,
      error: message.answers.length > 0 ? null : 'Servidor respondeu sem registros para o nome',
    }
  } catch (error: unknown) {
    const lookupTimeMs = round(performance.now() - startedAt)
    const message =
      error instanceof DnsLatencyError || error instanceof DnsProtocolError
        ? error.message
        : error instanceof Error
          ? error.name === 'TimeoutError' || error.name === 'AbortError'
            ? `Timeout de ${timeoutMs}ms na consulta DNS`
            : error.message
          : String(error)

    return { ...base, lookupTimeMs, error: message }
  }
}

export interface DnsServerTarget {
  /** IP (`udp`/`tcp`) ou endpoint (`doh`). Vazio em `system` */
  server: string
  label?: string
  protocol?: DnsProtocol
}

export interface DnsBenchmarkOptions {
  servers: DnsServerTarget[]
  hostnames: string[]
  recordType?: DnsRecordType
  timeoutMs?: number
  /** Quantas medições por hostname (a menor é a mais representativa) */
  rounds?: number
}

export interface DnsServerRanking {
  server: string
  label: string
  protocol: DnsProtocol
  avgLookupTimeMs: number | null
  minLookupTimeMs: number | null
  maxLookupTimeMs: number | null
  /** Mediana — menos sensível a um outlier isolado que a média */
  medianLookupTimeMs: number | null
  successRate: number
  totalQueries: number
  failedQueries: number
  samples: DnsLookupSample[]
  error: string | null
}

/** Servidores públicos oferecidos como padrão na comparação */
export const DEFAULT_DNS_SERVERS: DnsServerTarget[] = [
  { server: '1.1.1.1', label: 'Cloudflare', protocol: 'udp' },
  { server: '8.8.8.8', label: 'Google', protocol: 'udp' },
  { server: '9.9.9.9', label: 'Quad9', protocol: 'udp' },
  { server: '208.67.222.222', label: 'OpenDNS', protocol: 'udp' },
  { server: '94.140.14.14', label: 'AdGuard', protocol: 'udp' },
]

export const DEFAULT_BENCHMARK_HOSTNAMES = ['google.com', 'cloudflare.com', 'globo.com']

function median(values: number[]): number | null {
  if (values.length === 0) return null
  const sorted = [...values].sort((a, b) => a - b)
  const middle = Math.floor(sorted.length / 2)
  return sorted.length % 2 === 0
    ? round((sorted[middle - 1]! + sorted[middle]!) / 2)
    : round(sorted[middle]!)
}

/**
 * Compara servidores DNS medindo os mesmos hostnames em cada um.
 *
 * As consultas rodam em série de propósito: medições simultâneas competem pelo
 * mesmo enlace e distorceriam a comparação entre os servidores.
 */
export async function benchmarkDnsServers(
  options: DnsBenchmarkOptions
): Promise<DnsServerRanking[]> {
  const hostnames = options.hostnames.length ? options.hostnames : DEFAULT_BENCHMARK_HOSTNAMES
  const rounds = Math.min(Math.max(options.rounds ?? 1, 1), 5)
  const rankings: DnsServerRanking[] = []

  for (const target of options.servers) {
    const protocol = target.protocol || (target.server.startsWith('http') ? 'doh' : 'udp')
    const samples: DnsLookupSample[] = []

    for (const hostname of hostnames) {
      for (let round_ = 0; round_ < rounds; round_++) {
        samples.push(
          await measureDnsLookup({
            hostname,
            recordType: options.recordType,
            protocol,
            server: protocol === 'doh' ? undefined : target.server,
            dohUrl: protocol === 'doh' ? target.server : undefined,
            timeoutMs: options.timeoutMs,
          })
        )
      }
    }

    const successful = samples.filter((sample) => sample.success)
    const times = successful.map((sample) => sample.lookupTimeMs)
    const firstError = samples.find((sample) => sample.error)?.error ?? null

    rankings.push({
      server: target.server,
      label: target.label || target.server,
      protocol,
      avgLookupTimeMs: times.length
        ? round(times.reduce((total, value) => total + value, 0) / times.length)
        : null,
      minLookupTimeMs: times.length ? round(Math.min(...times)) : null,
      maxLookupTimeMs: times.length ? round(Math.max(...times)) : null,
      medianLookupTimeMs: median(times),
      successRate: samples.length ? Number((successful.length / samples.length).toFixed(3)) : 0,
      totalQueries: samples.length,
      failedQueries: samples.length - successful.length,
      samples,
      error: successful.length === 0 ? firstError : null,
    })
  }

  return sortByLatency(rankings)
}

/** Menor latência primeiro; servidores sem nenhuma resposta vão para o fim */
export function sortByLatency<T extends { avgLookupTimeMs: number | null }>(items: T[]): T[] {
  return [...items].sort((a, b) => {
    if (a.avgLookupTimeMs === null && b.avgLookupTimeMs === null) return 0
    if (a.avgLookupTimeMs === null) return 1
    if (b.avgLookupTimeMs === null) return -1
    return a.avgLookupTimeMs - b.avgLookupTimeMs
  })
}
