/**
 * Utilitários de cálculo IPv4/CIDR usados pelo IPAM da VPN.
 * Sem dependências externas e sem I/O — puramente funcional e testável.
 */

export interface CidrRange {
  /** Endereço de rede (ex.: 10.8.0.0). */
  networkAddress: string
  /** Endereço de broadcast (ex.: 10.8.0.255). */
  broadcastAddress: string
  /** Máscara em bits (ex.: 24). */
  prefixLength: number
  /** Máscara em notação decimal (ex.: 255.255.255.0). */
  netmask: string
  /** Quantidade de endereços utilizáveis por hosts. */
  usableHosts: number
}

const IPV4_PATTERN = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/

export function isValidIpv4(ip: string): boolean {
  const match = IPV4_PATTERN.exec(ip ?? '')
  if (!match) return false
  return match.slice(1).every((octet) => Number(octet) >= 0 && Number(octet) <= 255)
}

export function ipToLong(ip: string): number {
  if (!isValidIpv4(ip)) {
    throw new Error(`Endereço IPv4 inválido: ${ip}`)
  }
  return ip.split('.').reduce((acc, octet) => acc * 256 + Number(octet), 0)
}

export function longToIp(value: number): string {
  return [24, 16, 8, 0].map((shift) => (value >>> shift) & 255).join('.')
}

export function parseCidr(cidr: string): CidrRange {
  const [address, prefix] = String(cidr ?? '').split('/')
  const prefixLength = Number(prefix)

  if (
    !isValidIpv4(address) ||
    !Number.isInteger(prefixLength) ||
    prefixLength < 0 ||
    prefixLength > 32
  ) {
    throw new Error(`CIDR inválido: ${cidr}`)
  }

  const mask = prefixLength === 0 ? 0 : (0xffffffff << (32 - prefixLength)) >>> 0
  const networkLong = (ipToLong(address) & mask) >>> 0
  const broadcastLong = (networkLong | (~mask >>> 0)) >>> 0
  const totalAddresses = 2 ** (32 - prefixLength)

  return {
    networkAddress: longToIp(networkLong),
    broadcastAddress: longToIp(broadcastLong),
    prefixLength,
    netmask: longToIp(mask),
    usableHosts: Math.max(totalAddresses - 2, 0),
  }
}

/** Retorna o primeiro endereço utilizável da faixa (convenção: gateway/servidor). */
export function firstUsableAddress(cidr: string): string {
  const range = parseCidr(cidr)
  return longToIp(ipToLong(range.networkAddress) + 1)
}

export function isIpInCidr(ip: string, cidr: string): boolean {
  if (!isValidIpv4(ip)) return false
  const range = parseCidr(cidr)
  const target = ipToLong(ip)
  return target >= ipToLong(range.networkAddress) && target <= ipToLong(range.broadcastAddress)
}

/**
 * Itera os endereços utilizáveis da faixa (exclui rede e broadcast).
 * Generator para não materializar faixas grandes em memória.
 */
export function* iterateUsableAddresses(cidr: string): Generator<string> {
  const range = parseCidr(cidr)
  const start = ipToLong(range.networkAddress) + 1
  const end = ipToLong(range.broadcastAddress) - 1

  for (let current = start; current <= end; current++) {
    yield longToIp(current)
  }
}
