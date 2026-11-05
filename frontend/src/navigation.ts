/**
 * Registro do menu lateral (ADR 017).
 *
 * O layout não conhece item nenhum: ele chama {@link buildNavSections} e
 * desenha o resultado. Cada entrada declara a **área** a que pertence — é por
 * ela que o menu se adapta ao que a instalação usa — e as condições de
 * exibição (papel, plugin-apps, PWA) entram pelo {@link NavContext}. A função é
 * pura: mesmo contexto, mesmo menu, testável sem montar componente.
 *
 * As seções seguem o assunto, não o mecanismo: Docker e os servidores que o
 * hospedam ficam juntos em "Servidores"; as sondas, com a rede que medem.
 */

/** Áreas do produto (ADR 017). `core` nunca desliga. */
export type NavArea = 'core' | 'network' | 'servers' | 'vpn'

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

interface NavItemSpec extends NavItem {
  /** Só para administradores. */
  admin?: boolean
  /** Só quando o PWA pode ser instalado. */
  pwa?: boolean
}

interface NavSectionSpec {
  title: string
  items: (ctx: NavContext) => NavItemSpec[]
}

function appsGroup(apps: NavApp[]): NavItemSpec[] {
  if (apps.length === 0) return []
  return [
    {
      title: 'Aplicativos',
      icon: 'mdi-apps',
      area: 'network',
      children: apps.map((app) => ({
        title: app.title,
        icon: app.icon,
        area: 'network' as const,
        to: '/apps/' + app.id,
      })),
    },
  ]
}

/*
 * Lido de cima para baixo pela frequência de uso: o que se consulta o dia
 * todo primeiro; o que se configura uma vez e a administração, no fim.
 */
const SECTIONS: NavSectionSpec[] = [
  {
    title: 'Monitoramento',
    items: () => [
      { title: 'Dashboard', icon: 'mdi-view-dashboard', area: 'core', to: '/' },
      { title: 'Topologia', icon: 'mdi-sitemap', area: 'network', to: '/topology' },
      { title: 'Dispositivos', icon: 'mdi-devices', area: 'core', to: '/devices' },
      { title: 'Monitores', icon: 'mdi-heart-pulse', area: 'core', to: '/monitors' },
      { title: 'Alertas', icon: 'mdi-bell-outline', area: 'core', to: '/alerts' },
      { title: 'Eventos', icon: 'mdi-history', area: 'core', to: '/events' },
      { title: 'Logs', icon: 'mdi-text-box-search-outline', area: 'core', to: '/logs' },
    ],
  },
  {
    title: 'Rede',
    items: (ctx) => [
      { title: 'Descoberta', icon: 'mdi-radar', area: 'network', to: '/discovery' },
      { title: 'Redes IP', icon: 'mdi-ip-network-outline', area: 'network', to: '/networks' },
      { title: 'Sites', icon: 'mdi-domain', area: 'network', to: '/sites' },
      {
        title: 'Testes de velocidade',
        icon: 'mdi-speedometer',
        area: 'network',
        to: '/link-tests',
      },
      { title: 'Sinais Wi-Fi', icon: 'mdi-wifi', area: 'network', to: '/wifi' },
      { title: 'Sondas de rede', icon: 'mdi-access-point-network', area: 'network', to: '/probes' },
      {
        title: 'Diagnóstico',
        icon: 'mdi-stethoscope',
        area: 'network',
        children: [
          {
            title: 'Playbooks',
            icon: 'mdi-clipboard-play-outline',
            area: 'network',
            action: 'playbook',
          },
          { title: 'Traceroute ICMP', icon: 'mdi-routes', area: 'network', action: 'traceroute' },
          {
            // "Testes de velocidade" mede entre equipamentos; este mede do navegador.
            title: 'Velocidade do navegador',
            icon: 'mdi-speedometer-medium',
            area: 'network',
            action: 'speedTest',
          },
        ],
      },
      ...appsGroup(ctx.apps),
    ],
  },
  {
    title: 'Servidores',
    items: () => [
      { title: 'Docker', icon: 'mdi-docker', area: 'servers', to: '/docker' },
      // Os agentes servem às duas áreas (Docker e coleta remota): são do núcleo.
      { title: 'Servidores remotos', icon: 'mdi-server', area: 'core', to: '/remote-servers' },
    ],
  },
  {
    title: 'VPN',
    items: () => [
      { title: 'VPN WireGuard', icon: 'mdi-shield-lock-outline', area: 'vpn', to: '/vpn' },
    ],
  },
  {
    title: 'Operação',
    items: () => [
      { title: 'Manutenção', icon: 'mdi-wrench-clock', area: 'core', to: '/maintenance-windows' },
      {
        title: 'Tarefas agendadas',
        icon: 'mdi-calendar-clock',
        area: 'core',
        to: '/scheduled-tasks',
      },
    ],
  },
  {
    title: 'Administração',
    items: () => [
      {
        title: 'Usuários e acessos',
        icon: 'mdi-account-multiple-outline',
        area: 'core',
        to: '/users',
        admin: true,
      },
      {
        title: 'Trilha de auditoria',
        icon: 'mdi-shield-account-outline',
        area: 'core',
        to: '/audit',
        admin: true,
      },
      {
        title: 'Plugins de dispositivo',
        icon: 'mdi-puzzle-outline',
        area: 'network',
        to: '/plugins',
        admin: true,
      },
      { title: 'Backup', icon: 'mdi-backup-restore', area: 'core', to: '/backup', admin: true },
      { title: 'Configurações', icon: 'mdi-cog', area: 'core', to: '/settings' },
      {
        title: 'Instalar aplicativo',
        icon: 'mdi-cellphone-arrow-down',
        area: 'core',
        action: 'installPwa',
        pwa: true,
      },
    ],
  },
]

function areaOn(area: NavArea, ctx: NavContext): boolean {
  return area === 'core' || !ctx.enabledAreas || ctx.enabledAreas.has(area)
}

function visible(item: NavItemSpec, ctx: NavContext): boolean {
  if (item.admin && !ctx.isAdmin) return false
  if (item.pwa && !ctx.canInstallPwa) return false
  return areaOn(item.area, ctx)
}

/** O menu para este contexto. Seções e grupos vazios somem. */
export function buildNavSections(ctx: NavContext): NavSection[] {
  return SECTIONS.map((section) => ({
    title: section.title,
    items: section
      .items(ctx)
      .filter((item) => visible(item, ctx))
      .map(({ admin: _admin, pwa: _pwa, ...item }): NavItem => {
        if (!item.children) return item
        return { ...item, children: item.children.filter((child) => areaOn(child.area, ctx)) }
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
