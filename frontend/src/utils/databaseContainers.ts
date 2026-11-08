import type { AddressKind } from '@/bindings/AddressKind'
import type { CandidateAddress } from '@/bindings/CandidateAddress'
import type { DatabaseContainer } from '@/bindings/DatabaseContainer'
import type { DatabaseContainerResolution } from '@/bindings/DatabaseContainerResolution'
import type { DatabaseEngine } from '@/bindings/DatabaseEngine'

/**
 * Bancos em contêineres dos agentes: como a tela fala deles. O backend
 * (`databases::discovery`) acha os contêineres, monta os endereços e testa;
 * aqui mora só o texto, a ordem e o que preencher no formulário.
 */

export const ADDRESS_KINDS: Record<AddressKind, { label: string; icon: string }> = {
  dns: { label: 'Nome na rede Docker', icon: 'mdi-dns-outline' },
  container_ip: { label: 'IP do contêiner', icon: 'mdi-ip-network-outline' },
  published_port: { label: 'Porta publicada no host', icon: 'mdi-export' },
  loopback: { label: 'Rede do próprio host', icon: 'mdi-home-import-outline' },
}

/** `projetox_postgres:5432`. */
export function addressText(address: Pick<CandidateAddress, 'host' | 'port'>): string {
  return `${address.host}:${address.port}`
}

/** Os contêineres do SGBD escolhido (ou todos, sem escolha). */
export function containersFor(
  containers: DatabaseContainer[],
  engine: DatabaseEngine | null
): DatabaseContainer[] {
  return engine ? containers.filter((item) => item.engine === engine) : containers
}

/** "projetox · postgres:16 · Up 3 hours" — a segunda linha do item. */
export function containerSubtitle(container: DatabaseContainer): string {
  const host = container.local ? null : container.hostName
  return [host, container.project, container.image, container.status].filter(Boolean).join(' · ')
}

/** A chave de um contêiner na lista: `agent-1/<id>`. */
export function containerKey(container: Pick<DatabaseContainer, 'hostKey' | 'containerId'>) {
  return `${container.hostKey}/${container.containerId}`
}

/**
 * O endereço que vai para o formulário quando nenhum respondeu: pelo agente,
 * o primeiro IP (a lista `AGENT_DATABASE_TARGETS` do agente aceita IP, não
 * nome); direto, o primeiro da ordem recomendada.
 */
export function fallbackAddress(
  resolution: Pick<DatabaseContainerResolution, 'container' | 'addresses'>
): CandidateAddress | null {
  const { container, addresses } = resolution
  if (!container.local) {
    const byIp = addresses.find((address) => address.kind !== 'dns')
    if (byIp) return byIp
  }
  return addresses[0] ?? null
}

/** O endereço que o formulário recebe: o que respondeu ou o melhor palpite. */
export function addressToUse(resolution: DatabaseContainerResolution): CandidateAddress | null {
  return resolution.chosen ?? fallbackAddress(resolution)
}

/**
 * O IP do destino para a linha `AGENT_DATABASE_TARGETS`, que só aceita IP ou
 * faixa: o nome escolhido vira o IP do mesmo contêiner.
 */
export function agentTargetHost(host: string, resolution: DatabaseContainerResolution | null) {
  const isIp = /^[\d.]+$/.test(host) || host.includes(':')
  if (isIp || !resolution) return host
  const sameContainer = resolution.addresses.some((address) => address.host === host)
  const ip = resolution.addresses.find((address) => address.kind === 'container_ip')
  return sameContainer && ip ? ip.host : host
}

export interface ResolutionSummary {
  tone: 'success' | 'warning'
  title: string
  text: string
}

/** O que aconteceu ao testar o contêiner, em uma frase para o operador. */
export function resolutionSummary(resolution: DatabaseContainerResolution): ResolutionSummary {
  const { container, chosen, attempts, addresses } = resolution
  const route = container.local ? 'a partir da central' : `pelo agente ${container.hostName}`
  const name = addresses.find((address) => address.kind === 'dns')
  const network = addresses.find((address) => address.network)?.network

  if (!chosen) {
    if (!addresses.length) {
      return {
        tone: 'warning',
        title: 'O contêiner não tem endereço de rede',
        text: 'Ele está parado ou sem rede. Inicie o contêiner e procure de novo.',
      }
    }
    if (!container.local && !container.routeAllowed) {
      return {
        tone: 'warning',
        title: `O agente ${container.hostName} ainda não libera a ponte de banco`,
        text: 'Preenchemos o endereço do contêiner. Libere a ponte no host do agente (linhas abaixo) e clique em "Testar e listar bancos".',
      }
    }
    return {
      tone: 'warning',
      title: `Nenhum endereço respondeu ${route}`,
      text: container.local
        ? `Tentamos ${attempts.length} endereços. Publique a porta ${container.port} do contêiner no compose (ports: "${container.port}:${container.port}") ou coloque a central na mesma rede dele${network ? ` (${network})` : ''}.`
        : `Tentamos ${attempts.length} endereços. Confira se o destino está em AGENT_DATABASE_TARGETS no host do agente e se o contêiner está rodando.`,
    }
  }

  const where = addressText(chosen)
  switch (chosen.kind) {
    case 'dns':
      return {
        tone: 'success',
        title: `Alcançado pelo nome na rede Docker ${route}`,
        text: `${where}${chosen.network ? ` (rede ${chosen.network})` : ''} — o endereço continua valendo mesmo se o contêiner for recriado.`,
      }
    case 'container_ip':
      return {
        tone: 'success',
        title: `Alcançado pelo IP do contêiner ${route}`,
        text: `${name ? `O nome ${name.host} não respondeu daqui; usando ` : 'Usando '}${where}. Se o contêiner for recriado, o IP pode mudar.`,
      }
    case 'published_port':
      return {
        tone: 'success',
        title: `Alcançado pela porta publicada no host ${route}`,
        text: `Nome e IP do contêiner não responderam daqui; usando ${where}.`,
      }
    case 'loopback':
      return {
        tone: 'success',
        title: `Alcançado na rede do host ${route}`,
        text: `O contêiner usa a rede do host; usando ${where}.`,
      }
  }
}
