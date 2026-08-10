/**
 * Expansão de faixas CIDR IPv4 para a varredura de descoberta.
 *
 * Vive fora dos scanners porque três lugares precisam da mesma resposta: o
 * scanner ICMP (quais IPs pingar), o endpoint que dispara a varredura de uma
 * rede (o CIDR cadastrado é utilizável?) e a UI (quantos hosts serão varridos).
 */

/** Teto de endereços varridos por execução — /22 completo. */
export const MAX_SCAN_HOSTS = 1024

export interface CidrRange {
  /** Endereço de rede normalizado, ex.: `192.168.1.0` */
  networkAddress: string
  prefix: number
  /** Total de endereços utilizáveis na faixa, antes de qualquer truncamento */
  usableHosts: number
  /** `true` quando `usableHosts` excede `MAX_SCAN_HOSTS` */
  truncated: boolean
}

export class InvalidCidrError extends Error {
  constructor(cidr: string, reason: string) {
    super(`Faixa CIDR inválida "${cidr}": ${reason}`)
    this.name = 'InvalidCidrError'
  }
}

function toNumber(ip: string): number | null {
  const parts = ip.split('.')
  if (parts.length !== 4) return null

  let value = 0
  for (const part of parts) {
    if (!/^\d{1,3}$/.test(part)) return null
    const octet = Number(part)
    if (octet > 255) return null
    // `<<` opera em 32 bits com sinal; a multiplicação evita o valor negativo
    // que apareceria no primeiro octeto acima de 127.
    value = value * 256 + octet
  }

  return value
}

function toAddress(value: number): string {
  return [
    Math.floor(value / 16_777_216) % 256,
    Math.floor(value / 65_536) % 256,
    Math.floor(value / 256) % 256,
    value % 256,
  ].join('.')
}

/**
 * Interpreta e valida uma faixa. Aceita host único (sem `/`) e prefixos de
 * /8 a /32 — abaixo de /8 a varredura deixa de fazer sentido para o alvo do
 * produto (redes residenciais e de pequenas empresas).
 */
export function parseCidrRange(cidr: string): CidrRange {
  const value = (cidr ?? '').trim()
  if (!value) throw new InvalidCidrError(cidr, 'valor vazio')

  const [address, prefixPart] = value.split('/')
  const base = toNumber(address.trim())
  if (base === null) throw new InvalidCidrError(cidr, 'endereço IPv4 malformado')

  // Host avulso: uma varredura de um endereço só é legítima (testar um alvo).
  const prefix = prefixPart === undefined ? 32 : Number(prefixPart)
  if (!Number.isInteger(prefix) || prefix < 8 || prefix > 32) {
    throw new InvalidCidrError(cidr, 'prefixo deve estar entre /8 e /32')
  }

  const size = 2 ** (32 - prefix)
  const networkNumber = Math.floor(base / size) * size

  // /31 e /32 não têm endereço de rede nem de broadcast reservados (RFC 3021)
  const usableHosts = prefix >= 31 ? size : size - 2

  return {
    networkAddress: toAddress(networkNumber),
    prefix,
    usableHosts,
    truncated: usableHosts > MAX_SCAN_HOSTS,
  }
}

/**
 * Endereços a varrer na faixa, já sem rede e broadcast.
 *
 * Faixas maiores que `MAX_SCAN_HOSTS` são truncadas no início do bloco — varrer
 * um /16 inteiro (65 mil pings) travaria o probe por horas. O truncamento é
 * visível em `parseCidrRange().truncated` para a UI poder avisar em vez de
 * silenciosamente varrer só um pedaço.
 */
export function expandCidr(cidr: string, maxHosts = MAX_SCAN_HOSTS): string[] {
  const range = parseCidrRange(cidr)
  const size = 2 ** (32 - range.prefix)
  const networkNumber = toNumber(range.networkAddress)!

  const first = range.prefix >= 31 ? networkNumber : networkNumber + 1
  const last = range.prefix >= 31 ? networkNumber + size - 1 : networkNumber + size - 2
  const limit = Math.min(last, first + maxHosts - 1)

  const addresses: string[] = []
  for (let current = first; current <= limit; current++) {
    addresses.push(toAddress(current))
  }

  return addresses
}

/** `true` se o CIDR é utilizável numa varredura. */
export function isScannableCidr(cidr: string): boolean {
  try {
    parseCidrRange(cidr)
    return true
  } catch {
    return false
  }
}
