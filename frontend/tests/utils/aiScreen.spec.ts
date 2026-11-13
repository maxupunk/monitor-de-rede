import { describe, expect, it } from 'vitest'
import {
  currentScreen,
  mergeSuggestions,
  screenAttachment,
  screenRequest,
  type ScreenRoute,
  withPreviousScreen,
} from '@/utils/aiScreen'
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

describe('sugestões do chat vazio', () => {
  const sugestao = (prompt: string) => ({ title: prompt, prompt, icon: 'x', color: 'primary' })

  it('as da tela vêm primeiro, sem repetir as gerais, e o painel corta', () => {
    const tela = [sugestao('logs'), sugestao('geral-1')]
    const gerais = [sugestao('geral-1'), sugestao('geral-2'), sugestao('geral-3')]
    expect(mergeSuggestions(tela, gerais).map((item) => item.prompt)).toEqual([
      'logs',
      'geral-1',
      'geral-2',
      'geral-3',
    ])
    expect(mergeSuggestions(tela, gerais, 3)).toHaveLength(3)
    expect(mergeSuggestions([], gerais)).toEqual(gerais)
  })
})

describe('troca de tela no meio da conversa', () => {
  const docker = {
    label: 'Docker',
    icon: 'mdi-docker',
    request: { screen: { id: 'docker', detail: null } },
  }

  it('leva a tela da pergunta anterior quando era outra', () => {
    const conversa = [
      { role: 'user', screen: { id: 'logs' } },
      { role: 'assistant', screen: null },
    ]
    expect(withPreviousScreen(docker, conversa).request.screen.previous).toBe('logs')
  })

  it('mesma tela, pergunta sem tela ou conversa nova: nada muda', () => {
    expect(withPreviousScreen(docker, [{ role: 'user', screen: { id: 'docker' } }])).toBe(docker)
    expect(withPreviousScreen(docker, [{ role: 'user', screen: null }])).toBe(docker)
    expect(withPreviousScreen(docker, [])).toBe(docker)
  })
})
