import { describe, it, expect } from 'vitest'
import {
  buildNavSections as build,
  groupOfPath,
  type NavContext,
  type NavSection,
} from '@/navigation'
import { NAV_ENTRIES } from '@/areas'

const buildNavSections = (ctx: NavContext) => build(ctx, NAV_ENTRIES)

const BASE: NavContext = { isAdmin: false, apps: [], canInstallPwa: false }

function leaves(sections: NavSection[]): string[] {
  return sections.flatMap((section) =>
    section.items.flatMap((item) => (item.children ? item.children : [item]).map((l) => l.title))
  )
}

function titles(sections: NavSection[]): string[] {
  return sections.map((section) => section.title)
}

describe('buildNavSections', () => {
  it('organiza por assunto: Docker e servidores remotos juntos, sondas na Rede', () => {
    const sections = buildNavSections(BASE)
    const servidores = sections.find((s) => s.title === 'Servidores')
    const rede = sections.find((s) => s.title === 'Rede')
    expect(servidores?.items.map((i) => i.title)).toEqual(['Docker', 'Servidores remotos'])
    expect(rede?.items.map((i) => i.title)).toContain('Sondas de rede')
  })

  it('Docker e VPN são uma entrada só; IA e benchmark saíram do menu', () => {
    const all = leaves(buildNavSections({ ...BASE, isAdmin: true }))
    expect(all.filter((t) => t === 'Docker')).toHaveLength(1)
    expect(all).toContain('VPN WireGuard')
    expect(all).not.toContain('Containers')
    expect(all).not.toContain('Dispositivos VPN')
    expect(all).not.toContain('Assistente IA')
    expect(all).not.toContain('Benchmark dos modelos')
  })

  it('perfil completo, para admin, tem 26 entradas (eram ~36)', () => {
    // Sem plugin-apps nem PWA, que variam por instalação.
    expect(leaves(buildNavSections({ ...BASE, isAdmin: true }))).toHaveLength(26)
  })

  it('itens de administração só para admin', () => {
    const viewer = leaves(buildNavSections(BASE))
    const admin = leaves(buildNavSections({ ...BASE, isAdmin: true }))
    expect(viewer).not.toContain('Usuários e acessos')
    expect(admin).toContain('Usuários e acessos')
    expect(admin).toContain('Backup')
  })

  it('grupo Aplicativos só existe com plugin-apps', () => {
    expect(leaves(buildNavSections(BASE))).not.toContain('Wi-Fi da frota')
    const sections = buildNavSections({
      ...BASE,
      apps: [{ id: 7, title: 'Wi-Fi da frota', icon: 'mdi-wifi' }],
    })
    const apps = sections.flatMap((s) => s.items).find((i) => i.title === 'Aplicativos')
    expect(apps?.children?.[0]).toMatchObject({ title: 'Wi-Fi da frota', to: '/apps/7' })
  })

  it('instalar aplicativo só quando o PWA pode ser instalado', () => {
    expect(leaves(buildNavSections(BASE))).not.toContain('Instalar aplicativo')
    expect(leaves(buildNavSections({ ...BASE, canInstallPwa: true }))).toContain(
      'Instalar aplicativo'
    )
  })

  it('área desligada some do menu, e seção vazia some junto', () => {
    const sections = buildNavSections({ ...BASE, enabledAreas: new Set(['servers']) })
    expect(titles(sections)).not.toContain('Rede')
    expect(titles(sections)).not.toContain('VPN')
    expect(leaves(sections)).toContain('Docker')
    expect(leaves(sections)).not.toContain('Topologia')
    // O núcleo nunca desliga.
    expect(leaves(sections)).toContain('Servidores remotos')
    expect(leaves(sections)).toContain('Monitores')
  })
})

describe('groupOfPath', () => {
  it('abre o grupo que contém a rota', () => {
    const sections = buildNavSections({
      ...BASE,
      apps: [{ id: 3, title: 'Firmware', icon: 'mdi-chip' }],
    })
    expect(groupOfPath(sections, '/apps/3')).toBe('Aplicativos')
    expect(groupOfPath(sections, '/monitors')).toBeUndefined()
  })
})
