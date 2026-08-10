import os from 'node:os'
import dns from 'node:dns/promises'
import { ipToLong, isValidIpv4 } from './cidr.js'

/**
 * Teste de pré-voo (§4.1): descobre o endereço público do servidor e diagnostica
 * se roteadores remotos conseguirão iniciar o túnel WireGuard.
 *
 * O CGNAT é a única condição realmente impeditiva — e ele é detectável pela
 * faixa 100.64.0.0/10 (RFC 6598).
 */

export type PreflightStatus = 'reachable' | 'port_forward_required' | 'cgnat' | 'unknown'

export interface PreflightResult {
  status: PreflightStatus
  level: 'success' | 'warning' | 'error'
  message: string
  recommendation: string
  publicIp: string | null
  resolvedIp: string | null
  port: number
  isCgnat: boolean
  behindNat: boolean
  /** Falso quando não houve confirmação externa real da porta UDP. */
  verified: boolean
}

/** Serviços públicos usados para descobrir o IP visto pela internet. */
const PUBLIC_IP_ENDPOINTS = ['https://api.ipify.org?format=json', 'https://ifconfig.co/json']

const PRIVATE_RANGES: Array<[string, string]> = [
  ['10.0.0.0', '10.255.255.255'],
  ['172.16.0.0', '172.31.255.255'],
  ['192.168.0.0', '192.168.255.255'],
  ['169.254.0.0', '169.254.255.255'],
  ['127.0.0.0', '127.255.255.255'],
]

/** Faixa reservada ao CGNAT (RFC 6598). */
const CGNAT_RANGE: [string, string] = ['100.64.0.0', '100.127.255.255']

function isInRange(ip: string, [start, end]: [string, string]): boolean {
  if (!isValidIpv4(ip)) return false
  const target = ipToLong(ip)
  return target >= ipToLong(start) && target <= ipToLong(end)
}

export function isCgnatAddress(ip: string): boolean {
  return isInRange(ip, CGNAT_RANGE)
}

export function isPrivateAddress(ip: string): boolean {
  return PRIVATE_RANGES.some((range) => isInRange(ip, range))
}

export class PreflightService {
  constructor(private timeoutMs = 5000) {}

  /** Endereços IPv4 atribuídos às interfaces locais do host. */
  private localAddresses(): string[] {
    return Object.values(os.networkInterfaces())
      .flatMap((entries) => entries ?? [])
      .filter((entry) => entry.family === 'IPv4' && !entry.internal)
      .map((entry) => entry.address)
  }

  /** IP público visto pela internet (auto-detecção do §4.1). */
  async detectPublicIp(): Promise<string | null> {
    for (const endpoint of PUBLIC_IP_ENDPOINTS) {
      try {
        const response = await fetch(endpoint, { signal: AbortSignal.timeout(this.timeoutMs) })
        if (!response.ok) continue

        const payload = (await response.json()) as { ip?: string }
        if (payload.ip && isValidIpv4(payload.ip)) {
          return payload.ip
        }
      } catch {
        // tenta o próximo provedor
      }
    }

    return null
  }

  private async resolveHost(host: string): Promise<string | null> {
    if (!host) return null
    if (isValidIpv4(host)) return host

    try {
      const { address } = await dns.lookup(host, { family: 4 })
      return address
    } catch {
      return null
    }
  }

  /**
   * Diagnostica a acessibilidade externa do endpoint configurado.
   *
   * Observação honesta: sem um verificador externo, o sistema não consegue
   * *provar* que a porta UDP está aberta de fora. O que ele faz é identificar as
   * condições que impedem a conexão (CGNAT) e as que exigem ação do usuário
   * (servidor atrás de NAT sem port-forward).
   */
  async run(endpointHost: string | null, port: number): Promise<PreflightResult> {
    const publicIp = await this.detectPublicIp()
    const resolvedIp = endpointHost ? await this.resolveHost(endpointHost) : publicIp
    const candidate = resolvedIp ?? publicIp
    const locals = this.localAddresses()

    if (!candidate) {
      return {
        status: 'unknown',
        level: 'warning',
        message: 'Não foi possível determinar o endereço público do servidor.',
        recommendation:
          'Verifique a conectividade de saída do servidor ou informe o endereço público (ou DDNS) manualmente.',
        publicIp,
        resolvedIp,
        port,
        isCgnat: false,
        behindNat: false,
        verified: false,
      }
    }

    if (isCgnatAddress(candidate)) {
      return {
        status: 'cgnat',
        level: 'error',
        message: `CGNAT detectado (IP ${candidate}). Seu provedor não permite conexões de entrada.`,
        recommendation:
          'Solicite um IP público ao provedor ou hospede um relay WireGuard em uma VPS de baixo custo.',
        publicIp,
        resolvedIp,
        port,
        isCgnat: true,
        behindNat: true,
        verified: true,
      }
    }

    const hasPublicIpLocally = locals.includes(candidate)
    if (hasPublicIpLocally) {
      return {
        status: 'reachable',
        level: 'success',
        message: `Porta UDP ${port} publicada em ${candidate}. Roteadores podem conectar.`,
        recommendation: 'Nenhuma ação necessária. Gere os scripts dos equipamentos.',
        publicIp,
        resolvedIp,
        port,
        isCgnat: false,
        behindNat: false,
        verified: false,
      }
    }

    const privateLocals = locals.filter((address) => isPrivateAddress(address))

    return {
      status: 'port_forward_required',
      level: 'warning',
      message: `O servidor está atrás de NAT (endereço público ${candidate}, endereços locais ${privateLocals.join(', ') || 'não identificados'}).`,
      recommendation: `Configure no seu roteador o redirecionamento da porta UDP ${port} para este servidor.`,
      publicIp,
      resolvedIp,
      port,
      isCgnat: false,
      behindNat: true,
      verified: false,
    }
  }
}
