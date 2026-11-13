import { describe, expect, it } from 'vitest'
import { currentScreen, screenAttachment, screenRequest, type ScreenRoute } from '@/utils/aiScreen'
import { AREA_ROUTES } from '@/areas'
import type { RouteRecordRaw } from 'vue-router'

function rota(meta: ScreenRoute['meta'], params: ScreenRoute['params'] = {}): ScreenRoute {
  return { meta, params }
}

describe('tela aberta no chat', () => {
  it('rota sem aiScreen não vira tela', () => {
    expect(currentScreen(rota({}))).toBeNull()
  })

  it('tela de lista leva o id e o filtro', () => {
    const logs = currentScreen(rota({ aiScreen: { id: 'logs', label: 'Logs', icon: 'mdi-text' } }))
    expect(logs?.key).toBe('logs')
    expect(screenRequest(logs!, '  erro e acima ')).toEqual({
      screen: { id: 'logs', detail: 'erro e acima' },
    })
    expect(screenRequest(logs!, '   ').screen.detail).toBeNull()
  })

  it('tela de detalhe leva o equipamento ou o monitor da rota', () => {
    const device = currentScreen(
      rota(
        { aiScreen: { id: 'device-detail', label: 'Equipamento', icon: 'x', entity: 'device' } },
        { id: '12' }
      )
    )
    expect(device?.key).toBe('device-detail:12')
    expect(screenAttachment(device!, null)).toEqual({
      label: 'Equipamento',
      icon: 'x',
      request: { screen: { id: 'device-detail', detail: null }, deviceId: 12 },
    })

    const monitor = currentScreen(
      rota(
        { aiScreen: { id: 'monitor-detail', label: 'M', icon: 'x', entity: 'monitor' } },
        {
          id: 'abc',
        }
      )
    )
    expect(monitor?.entityId).toBeNull()
    expect(screenRequest(monitor!, null).monitorId).toBeUndefined()
  })

  it('as telas pedidas existem nas rotas das áreas', () => {
    const ids = new Set<string>()
    const visit = (routes: RouteRecordRaw[]) => {
      for (const route of routes) {
        if (route.meta?.aiScreen) ids.add(route.meta.aiScreen.id)
        if (route.children) visit(route.children)
      }
    }
    visit(AREA_ROUTES)
    for (const id of ['logs', 'docker', 'wifi', 'alerts', 'device-detail', 'vpn']) {
      expect(ids.has(id), id).toBe(true)
    }
  })
})
