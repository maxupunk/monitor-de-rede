/**
 * Menu lateral (ADR 017).
 *
 * O layout não conhece item nenhum: ele chama {@link buildNavSections} com as
 * entradas que as áreas declaram (`src/areas/*`) e desenha o resultado. Aqui
 * ficam só as seções — título e ordem —; cada entrada diz em que seção entra,
 * em que posição e de que **área** é — é por ela que o menu se adapta ao que a
 * instalação usa. As condições de exibição (papel, plugin-apps, PWA) entram
 * pelo {@link NavContext}. A função é pura: mesmo contexto e mesmas entradas,
 * mesmo menu, testável sem montar componente.
 *
 * As seções seguem o assunto, não o mecanismo: Docker e os servidores que o
 * hospedam ficam juntos em "Servidores"; as sondas, com a rede que medem.
 */
import type { Area } from '@/bindings/Area'

/** Áreas do produto (ADR 017). `core` nunca desliga. */
export type NavArea = Area

/** Ações que o layout sabe executar — o registro só as nomeia. */
export type NavAction = 'playbook' | 'traceroute' | 'speedTest' | 'installPwa'

export interface NavLeaf {
  title: string
  icon: string
  area: NavArea
  to?: string
  action?: NavAction
}

export interface NavItem extends NavLeaf {
  children?: NavLeaf[]
}

export interface NavSection {
  title: string
  items: NavItem[]
}

/** Um plugin com página de frota. */
export interface NavApp {
  id: number
  title: string
  icon: string
}

export interface NavContext {
  isAdmin: boolean
  apps: NavApp[]
  canInstallPwa: boolean
  /** Áreas ligadas; ausente, todas. O núcleo é sempre considerado ligado. */
  enabledAreas?: ReadonlySet<NavArea>
}

/** Item como a área o declara: a área vem da entrada. */
export interface NavItemSpec extends Omit<NavLeaf, 'area'> {
  children?: Omit<NavLeaf, 'area'>[]
  /** Só para administradores. */
  admin?: boolean
  /** Só quando o PWA pode ser instalado. */
  pwa?: boolean
}

/*
 * Lido de cima para baixo pela frequência de uso: o que se consulta o dia
 * todo primeiro; o que se configura uma vez e a administração, no fim.
 */
const SECTIONS = [
  { id: 'monitoring', title: 'Monitoramento' },
  { id: 'network', title: 'Rede' },
  { id: 'servers', title: 'Servidores' },
  { id: 'vpn', title: 'VPN' },
  { id: 'operations', title: 'Operação' },
  { id: 'admin', title: 'Administração' },
] as const

export type NavSectionId = (typeof SECTIONS)[number]['id']

/** Uma contribuição de área para o menu. */
export interface NavEntry {
  area: NavArea
  section: NavSectionId
  /** Posição dentro da seção (menor primeiro). */
  order: number
  /** Item fixo, ou gerado pelo contexto (`null` quando não se aplica). */
  item: NavItemSpec | ((ctx: NavContext) => NavItemSpec | null)
}

function areaOn(area: NavArea, ctx: NavContext): boolean {
  return area === 'core' || !ctx.enabledAreas || ctx.enabledAreas.has(area)
}

function visible(item: NavItemSpec, area: NavArea, ctx: NavContext): boolean {
  if (item.admin && !ctx.isAdmin) return false
  if (item.pwa && !ctx.canInstallPwa) return false
  return areaOn(area, ctx)
}

function toNavItem(
  { admin: _admin, pwa: _pwa, children, ...item }: NavItemSpec,
  area: NavArea
): NavItem {
  if (!children) return { ...item, area }
  return { ...item, area, children: children.map((child) => ({ ...child, area })) }
}

/** O menu para este contexto. Seções e grupos vazios somem. */
export function buildNavSections(ctx: NavContext, entries: readonly NavEntry[]): NavSection[] {
  return SECTIONS.map((section) => ({
    title: section.title,
    items: entries
      .filter((entry) => entry.section === section.id)
      .sort((a, b) => a.order - b.order)
      .flatMap((entry) => {
        const spec = typeof entry.item === 'function' ? entry.item(ctx) : entry.item
        return spec && visible(spec, entry.area, ctx) ? [toNavItem(spec, entry.area)] : []
      })
      .filter((item) => !item.children || item.children.length > 0),
  })).filter((section) => section.items.length > 0)
}

/** O grupo do menu que contém a rota — para abri-lo ao navegar. */
export function groupOfPath(sections: NavSection[], path: string): string | undefined {
  return sections
    .flatMap((section) => section.items)
    .find((item) =>
      item.children?.some(
        (child) => child.to && (path === child.to || path.startsWith(child.to + '/'))
      )
    )?.title
}
