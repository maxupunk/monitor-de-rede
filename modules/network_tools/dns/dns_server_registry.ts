import DnsServer from '#models/dns_server'
import type { DnsProtocol, DnsServerTarget } from './dns_latency_service.js'

/**
 * Lista semeada no primeiro acesso para o usuário já encontrar a comparação do
 * dashboard funcionando. A partir daí o cadastro é totalmente editável.
 */
const SEED_SERVERS: Array<{
  name: string
  address: string
  protocol: DnsProtocol
  description: string
}> = [
  { name: 'Cloudflare', address: '1.1.1.1', protocol: 'udp', description: 'Resolvedor público' },
  { name: 'Google', address: '8.8.8.8', protocol: 'udp', description: 'Resolvedor público' },
  {
    name: 'Quad9',
    address: '9.9.9.9',
    protocol: 'udp',
    description: 'Resolvedor público com filtro de segurança',
  },
  {
    name: 'OpenDNS',
    address: '208.67.222.222',
    protocol: 'udp',
    description: 'Resolvedor público',
  },
  {
    name: 'AdGuard',
    address: '94.140.14.14',
    protocol: 'udp',
    description: 'Resolvedor público com bloqueio de anúncios',
  },
]

export class DnsServerRegistry {
  /** Semeia os resolvedores públicos apenas quando o cadastro está vazio */
  async ensureDefaults(): Promise<void> {
    const existing = await DnsServer.query().count('* as total')
    const total = Number(
      (existing[0] as unknown as { $extras: Record<string, unknown> })?.$extras?.total ?? 0
    )
    if (total > 0) return

    await DnsServer.createMany(
      SEED_SERVERS.map((server) => ({
        name: server.name,
        address: server.address,
        protocol: server.protocol as 'udp' | 'tcp' | 'doh',
        description: server.description,
        isDefault: true,
      }))
    )
  }

  async list(): Promise<DnsServer[]> {
    await this.ensureDefaults()
    return DnsServer.query().orderBy('isDefault', 'desc').orderBy('name', 'asc')
  }

  /**
   * Servidores marcados para participar da comparação do dashboard. Se nenhum
   * estiver marcado, compara todos os cadastrados — melhor que devolver uma
   * lista vazia ou reintroduzir os resolvedores públicos por conta própria.
   */
  async benchmarkTargets(): Promise<DnsServerTarget[]> {
    const servers = await this.list()
    const selected = servers.filter((server) => server.isDefault)
    const targets = selected.length > 0 ? selected : servers

    return targets.map((server) => ({
      server: server.address,
      label: server.name,
      protocol: server.protocol,
    }))
  }
}
