import type { DnsServerPayload } from '@/stores/dnsServers'
import { defaultPreferences, type Preferences } from '@/stores/preferences'

/** Estado dos formulários do assistente, uma fatia por etapa. */
export interface SiteForm {
  enabled: boolean
  name: string
  location: string
  description: string
}

export interface AddressesForm {
  lan: string
  public: string
  vpn: string
  domain: string
  domainHttps: boolean
}

export interface NetworkForm {
  enabled: boolean
  name: string
  cidr: string
  gateway: string
  vlanId: number | undefined
  scanEnabled: boolean
  scanNow: boolean
}

export interface VpnForm {
  enabled: boolean
  publicEndpoint: string
  listenPort: number
  cidr: string
}

export interface SetupForm {
  site: SiteForm
  addresses: AddressesForm
  network: NetworkForm
  dns: DnsServerPayload[]
  vpn: VpnForm
  preferences: Preferences
  /** id do plugin de frota → ligado. */
  apps: Record<number, boolean>
}

/** Valores sugeridos ao abrir o assistente. */
export function createSetupForm(): SetupForm {
  return {
    site: {
      enabled: true,
      name: 'Matriz - Servidor Central',
      location: 'São Paulo, SP - Sala de Servidores',
      description: 'Site principal onde o servidor NetMonitor está hospedado',
    },
    addresses: {
      lan: '',
      public: '',
      vpn: '10.8.0.1',
      domain: '',
      domainHttps: false,
    },
    network: {
      enabled: true,
      name: 'Rede Local Principal (LAN)',
      cidr: '192.168.1.0/24',
      gateway: '192.168.1.1',
      vlanId: undefined,
      scanEnabled: true,
      scanNow: true,
    },
    dns: [],
    vpn: {
      enabled: false,
      publicEndpoint: '',
      listenPort: 51820,
      cidr: '10.8.0.0/24',
    },
    preferences: defaultPreferences(),
    apps: {},
  }
}

/** Resolvedores públicos oferecidos na etapa de DNS (os dois primeiros vêm marcados). */
export const POPULAR_DNS_PRESETS: readonly DnsServerPayload[] = [
  {
    name: 'Cloudflare DNS',
    address: '1.1.1.1',
    protocol: 'udp',
    isDefault: true,
    description: 'Ultra rápido e privado (1.1.1.1)',
  },
  {
    name: 'Google Public DNS',
    address: '8.8.8.8',
    protocol: 'udp',
    isDefault: true,
    description: 'Alta confiabilidade global (8.8.8.8)',
  },
  {
    name: 'Quad9 Security',
    address: '9.9.9.9',
    protocol: 'udp',
    isDefault: false,
    description: 'Bloqueio integrado de malware (9.9.9.9)',
  },
  {
    name: 'OpenDNS Umbrella',
    address: '208.67.222.222',
    protocol: 'udp',
    isDefault: false,
    description: 'Cisco OpenDNS (208.67.222.222)',
  },
  {
    name: 'Cloudflare DoH',
    address: 'https://cloudflare-dns.com/dns-query',
    protocol: 'doh',
    isDefault: false,
    description: 'DNS sobre HTTPS criptografado',
  },
]

/** A faixa /24 e o gateway `.1` sugeridos a partir do IP da LAN. */
export function suggestNetworkFromIp(ip: string): { cidr: string; gateway: string } | null {
  const parts = ip.split('.')
  if (parts.length !== 4) return null
  const prefix = `${parts[0]}.${parts[1]}.${parts[2]}`
  return { cidr: `${prefix}.0/24`, gateway: `${prefix}.1` }
}
