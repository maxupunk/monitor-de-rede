import { describe, expect, it } from 'vitest'
import {
  chooseScreen,
  currentScreen,
  mergeSuggestions,
  screenAttachment,
  screenRequest,
  screenToConfirm,
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

describe('pergunta vaga depois de trocar de tela', () => {
  const docker = {
    label: 'Docker',
    icon: 'mdi-docker',
    request: { screen: { id: 'docker', detail: 'postgres parado' } },
  }
  const logs = { id: 'logs', label: 'Logs', icon: 'mdi-text' }
  const conversa = [
    { role: 'user', screen: logs },
    { role: 'assistant', screen: null },
  ]

  it('pergunta de qual tela fala, a aberta primeiro', () => {
    expect(screenToConfirm(docker, conversa, 'e isso, é grave?', false)).toEqual([
      { id: 'docker', label: 'Docker', icon: 'mdi-docker' },
      logs,
    ])
    expect(screenToConfirm(docker, conversa, 'Esses erros continuam?', false)).not.toBeNull()
  })

  it('não pergunta quando a pergunta já diz do que fala', () => {
    // "aqui" é a tela aberta; @ é o alvo; sem demonstrativo não há "isso".
    expect(screenToConfirm(docker, conversa, 'e isso aqui, é grave?', false)).toBeNull()
    expect(screenToConfirm(docker, conversa, 'e isso, é grave?', true)).toBeNull()
    expect(screenToConfirm(docker, conversa, 'o postgres parado é grave?', false)).toBeNull()
    // "processo" contém "esso", mas não é "esso" sozinho.
    expect(screenToConfirm(docker, conversa, 'o processo travou?', false)).toBeNull()
  })

  it('não pergunta sem troca de tela', () => {
    const mesma = [{ role: 'user', screen: { ...logs, id: 'docker' } }]
    expect(screenToConfirm(docker, mesma, 'e isso?', false)).toBeNull()
    expect(screenToConfirm(docker, [], 'e isso?', false)).toBeNull()
  })

  it('a escolha decide a tela, sem a anterior junto', () => {
    const aberta = chooseScreen(docker, { id: 'docker', label: 'Docker', icon: 'mdi-docker' })
    expect(aberta.request.screen).toEqual({ id: 'docker', detail: 'postgres parado' })
    expect(withPreviousScreen(aberta, conversa).request.screen.previous).toBeUndefined()

    const anterior = chooseScreen(docker, logs)
    expect(anterior).toEqual({
      label: 'Logs',
      icon: 'mdi-text',
      request: { screen: { id: 'logs', detail: null } },
      chosen: true,
    })
    expect(withPreviousScreen(anterior, conversa)).toBe(anterior)
  })
})
