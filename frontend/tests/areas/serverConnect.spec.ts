import { describe, expect, it } from 'vitest'
import type { Area } from '@/bindings/Area'
import { AREAS } from '@/areas'
import { buildServerConnectPaths, defaultServerConnectPath } from '@/areas/serverConnect'

const all = (): boolean => true
const only =
  (...areas: Area[]) =>
  (area: Area): boolean =>
    area === 'core' || areas.includes(area)

describe('buildServerConnectPaths', () => {
  it('oferece rede local, VPN e internet, com a VPN recomendada', () => {
    const paths = buildServerConnectPaths(AREAS, all)
    expect(paths.map((path) => path.id)).toEqual(['lan', 'vpn', 'internet'])
    expect(defaultServerConnectPath(paths)?.id).toBe('vpn')
  })

  it('com a VPN desligada, sobram só os caminhos diretos do núcleo', () => {
    const paths = buildServerConnectPaths(AREAS, only('network', 'servers'))
    expect(paths.map((path) => path.id)).toEqual(['lan', 'internet'])
    expect(defaultServerConnectPath(paths)?.id).toBe('lan')
    expect(paths.some((path) => path.moveExisting)).toBe(false)
  })

  it('caminho direto declara os tipos de endereço; o da VPN traz o fluxo próprio', () => {
    const byId = Object.fromEntries(buildServerConnectPaths(AREAS, all).map((p) => [p.id, p]))
    expect(byId.lan?.addressKinds).toEqual(['lan'])
    expect(byId.internet?.addressKinds?.[0]).toBe('domain')
    expect(byId.lan?.component).toBeUndefined()
    expect(byId.vpn?.component).toBeDefined()
    expect(byId.vpn?.moveExisting?.label).toBe('Mover para a VPN')
  })

  it('sem nenhum caminho não há padrão', () => {
    expect(defaultServerConnectPath([])).toBeNull()
  })
})
