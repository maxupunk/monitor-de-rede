import { Buffer } from 'node:buffer'

/**
 * Codificação/decodificação de mensagens DNS (RFC 1035) em wire format.
 *
 * O `node:dns` não permite escolher o transporte (UDP/TCP) nem medir apenas a
 * etapa de resolução — ele delega ao c-ares e ao resolvedor do sistema. Montando
 * a consulta nós mesmos conseguimos: usar o mesmo pacote nos três transportes
 * (UDP, TCP e DoH), cronometrar exatamente o intervalo pergunta→resposta e ler
 * o RCODE real devolvido pelo servidor.
 */

export const DNS_RECORD_TYPE_CODES = {
  A: 1,
  NS: 2,
  CNAME: 5,
  SOA: 6,
  PTR: 12,
  MX: 15,
  TXT: 16,
  AAAA: 28,
} as const

export type DnsRecordType = keyof typeof DNS_RECORD_TYPE_CODES

const CODE_TO_RECORD_TYPE = new Map<number, string>(
  Object.entries(DNS_RECORD_TYPE_CODES).map(([name, code]) => [code, name])
)

/** Códigos de resposta do cabeçalho DNS (RFC 1035 §4.1.1 e RFC 6895) */
export const DNS_RCODE_LABELS: Record<number, string> = {
  0: 'NOERROR',
  1: 'FORMERR',
  2: 'SERVFAIL',
  3: 'NXDOMAIN',
  4: 'NOTIMP',
  5: 'REFUSED',
}

/** Mensagens em português para os RCODEs de falha mais comuns */
export const DNS_RCODE_MESSAGES: Record<number, string> = {
  1: 'Servidor não entendeu a consulta (FORMERR)',
  2: 'Falha interna do servidor DNS (SERVFAIL)',
  3: 'Domínio inexistente (NXDOMAIN)',
  4: 'Consulta não implementada pelo servidor (NOTIMP)',
  5: 'Consulta recusada pelo servidor (REFUSED)',
}

export interface DnsAnswer {
  name: string
  type: string
  ttl: number
  value: string
}

export interface DnsMessage {
  id: number
  /** Resposta veio truncada e deveria ser repetida via TCP (flag TC) */
  truncated: boolean
  rcode: number
  rcodeLabel: string
  answers: DnsAnswer[]
  answerCount: number
}

export class DnsProtocolError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'DnsProtocolError'
  }
}

/**
 * Monta uma consulta padrão com recursão desejada (RD=1).
 *
 * O `id` fica em 0 por padrão porque o RFC 8484 recomenda esse valor em DoH
 * (melhora o cache HTTP); UDP e TCP passam um id aleatório para casar resposta
 * com pergunta.
 */
export function encodeDnsQuery(hostname: string, recordType: DnsRecordType, id = 0): Buffer {
  const name = hostname.trim().replace(/\.$/, '')
  if (!name) throw new DnsProtocolError('Hostname vazio')
  if (name.length > 253) throw new DnsProtocolError('Hostname excede 253 caracteres')

  const labels = name.split('.')
  const encodedLabels: Buffer[] = []

  for (const label of labels) {
    if (label.length === 0) throw new DnsProtocolError(`Hostname inválido: "${hostname}"`)
    if (label.length > 63) {
      throw new DnsProtocolError(`Rótulo "${label}" excede 63 caracteres`)
    }
    const labelBuffer = Buffer.from(label, 'ascii')
    encodedLabels.push(Buffer.from([labelBuffer.length]), labelBuffer)
  }
  encodedLabels.push(Buffer.from([0]))

  const question = Buffer.concat(encodedLabels)
  const header = Buffer.alloc(12)
  header.writeUInt16BE(id, 0)
  header.writeUInt16BE(0x0100, 2) // QR=0, OPCODE=0, RD=1
  header.writeUInt16BE(1, 4) // QDCOUNT
  // ANCOUNT / NSCOUNT / ARCOUNT permanecem zerados

  const tail = Buffer.alloc(4)
  tail.writeUInt16BE(DNS_RECORD_TYPE_CODES[recordType], 0) // QTYPE
  tail.writeUInt16BE(1, 2) // QCLASS = IN

  return Buffer.concat([header, question, tail])
}

/** Lê um nome tratando ponteiros de compressão (RFC 1035 §4.1.4) */
function readName(buffer: Buffer, offset: number): { name: string; offset: number } {
  const labels: string[] = []
  let position = offset
  let nextOffset = offset
  let followedPointer = false
  let guard = 0

  while (guard++ < 128) {
    if (position >= buffer.length) {
      throw new DnsProtocolError('Resposta DNS truncada ao ler um nome')
    }

    const length = buffer[position]!

    if (length === 0) {
      position += 1
      if (!followedPointer) nextOffset = position
      return { name: labels.join('.'), offset: nextOffset }
    }

    if ((length & 0xc0) === 0xc0) {
      if (position + 1 >= buffer.length) {
        throw new DnsProtocolError('Ponteiro de compressão incompleto na resposta DNS')
      }
      const pointer = ((length & 0x3f) << 8) | buffer[position + 1]!
      if (!followedPointer) nextOffset = position + 2
      if (pointer >= buffer.length) {
        throw new DnsProtocolError('Ponteiro de compressão aponta fora da mensagem')
      }
      followedPointer = true
      position = pointer
      continue
    }

    position += 1
    if (position + length > buffer.length) {
      throw new DnsProtocolError('Resposta DNS truncada ao ler um rótulo')
    }
    labels.push(buffer.subarray(position, position + length).toString('ascii'))
    position += length
    if (!followedPointer) nextOffset = position
  }

  throw new DnsProtocolError('Loop de ponteiros de compressão na resposta DNS')
}

/** Aplica a compressão `::` na maior sequência de grupos zerados (RFC 5952) */
function formatIpv6(groups: number[]): string {
  let bestStart = -1
  let bestLength = 0
  let currentStart = -1
  let currentLength = 0

  groups.forEach((group, index) => {
    if (group === 0) {
      if (currentStart === -1) currentStart = index
      currentLength += 1
      if (currentLength > bestLength) {
        bestStart = currentStart
        bestLength = currentLength
      }
    } else {
      currentStart = -1
      currentLength = 0
    }
  })

  const parts = groups.map((group) => group.toString(16))
  if (bestLength < 2) return parts.join(':')

  const head = parts.slice(0, bestStart).join(':')
  const tail = parts.slice(bestStart + bestLength).join(':')
  return `${head}::${tail}`
}

function readRdata(buffer: Buffer, type: number, offset: number, length: number): string {
  const end = offset + length
  if (end > buffer.length) {
    throw new DnsProtocolError('Resposta DNS truncada ao ler o conteúdo de um registro')
  }

  switch (type) {
    case DNS_RECORD_TYPE_CODES.A: {
      if (length !== 4) throw new DnsProtocolError('Registro A com tamanho inválido')
      return Array.from(buffer.subarray(offset, end)).join('.')
    }
    case DNS_RECORD_TYPE_CODES.AAAA: {
      if (length !== 16) throw new DnsProtocolError('Registro AAAA com tamanho inválido')
      const groups: number[] = []
      for (let i = 0; i < 16; i += 2) groups.push(buffer.readUInt16BE(offset + i))
      return formatIpv6(groups)
    }
    case DNS_RECORD_TYPE_CODES.MX: {
      const preference = buffer.readUInt16BE(offset)
      const { name } = readName(buffer, offset + 2)
      return `${preference} ${name}`
    }
    case DNS_RECORD_TYPE_CODES.TXT: {
      const chunks: string[] = []
      let position = offset
      while (position < end) {
        const chunkLength = buffer[position]!
        position += 1
        if (position + chunkLength > end) break
        chunks.push(buffer.subarray(position, position + chunkLength).toString('utf8'))
        position += chunkLength
      }
      return chunks.join('')
    }
    case DNS_RECORD_TYPE_CODES.CNAME:
    case DNS_RECORD_TYPE_CODES.NS:
    case DNS_RECORD_TYPE_CODES.PTR:
    case DNS_RECORD_TYPE_CODES.SOA: {
      const { name } = readName(buffer, offset)
      return name
    }
    default:
      return buffer.subarray(offset, end).toString('hex')
  }
}

export function decodeDnsMessage(buffer: Buffer): DnsMessage {
  if (buffer.length < 12) {
    throw new DnsProtocolError('Resposta DNS menor que o cabeçalho mínimo (12 bytes)')
  }

  const id = buffer.readUInt16BE(0)
  const flags = buffer.readUInt16BE(2)
  const rcode = flags & 0x0f
  const truncated = (flags & 0x0200) !== 0
  const questionCount = buffer.readUInt16BE(4)
  const answerCount = buffer.readUInt16BE(6)

  let offset = 12

  // Pula a seção de perguntas ecoada pelo servidor
  for (let i = 0; i < questionCount; i++) {
    const { offset: afterName } = readName(buffer, offset)
    offset = afterName + 4 // QTYPE + QCLASS
  }

  const answers: DnsAnswer[] = []
  for (let i = 0; i < answerCount; i++) {
    if (offset + 10 > buffer.length) break

    const { name, offset: afterName } = readName(buffer, offset)
    const type = buffer.readUInt16BE(afterName)
    const ttl = buffer.readUInt32BE(afterName + 4)
    const rdLength = buffer.readUInt16BE(afterName + 8)
    const rdOffset = afterName + 10

    answers.push({
      name,
      type: CODE_TO_RECORD_TYPE.get(type) ?? `TYPE${type}`,
      ttl,
      value: readRdata(buffer, type, rdOffset, rdLength),
    })

    offset = rdOffset + rdLength
  }

  return {
    id,
    truncated,
    rcode,
    rcodeLabel: DNS_RCODE_LABELS[rcode] ?? `RCODE${rcode}`,
    answers,
    answerCount,
  }
}
