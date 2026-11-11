import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import type { Pinia } from 'pinia'
import type { DeployGuideStep } from '@/bindings/DeployGuideStep'
import type { DeployGuideView } from '@/bindings/DeployGuideView'
import DeployGuide from '@/components/deploys/DeployGuide.vue'
import { useAiStore } from '@/stores/ai'
import { guideProgress, guideQuestion } from '@/utils/deployPresentation'
import { bodyText, button, mountWith, setupDom, teardownDom } from './domHarness'

let pinia: Pinia

/** O guia começa fechado: abre pelo botão, como o usuário faz. */
async function open() {
  await flushPromises()
  button('Ver passo a passo')?.click()
  await flushPromises()
}

function step(partial: Partial<DeployGuideStep> & Pick<DeployGuideStep, 'key'>): DeployGuideStep {
  return {
    status: 'todo',
    title: partial.key,
    how: `como fazer ${partial.key}`,
    detail: `estado de ${partial.key}`,
    action: null,
    ...partial,
  }
}

/** GitHub pronto, agente é a atual, o resto depois. */
function midway(): DeployGuideView {
  return {
    done: false,
    steps: [
      step({
        key: 'github',
        status: 'done',
        title: 'Conectar o GitHub',
        detail: 'Conectado como acme.',
      }),
      step({
        key: 'agent',
        status: 'current',
        title: 'Ter um servidor com o agente',
        detail: 'Nenhum servidor com o agente ainda.',
        action: { kind: 'open_agents' },
      }),
      step({ key: 'application', title: 'Cadastrar a aplicação', action: { kind: 'setup' } }),
      step({ key: 'target', title: 'Dizer onde ela roda' }),
      step({ key: 'env', status: 'optional', title: 'Gravar o .env do servidor' }),
      step({ key: 'first_deploy', title: 'Primeira versão no ar' }),
    ],
  }
}

describe('guia do deploy (puro)', () => {
  it('conta só as etapas obrigatórias', () => {
    expect(guideProgress(midway().steps)).toEqual({ done: 1, total: 5 })
  })

  it('a pergunta à IA leva a etapa e pede o estado real', () => {
    const question = guideQuestion(midway().steps[1]!)
    expect(question).toContain('Ter um servidor com o agente')
    expect(question).toContain('Nenhum servidor com o agente ainda.')
    expect(question).toContain('get_deploy_guide')
    expect(guideQuestion(null)).toContain('get_deploy_guide')
  })
})

describe('DeployGuide', () => {
  beforeEach(() => {
    pinia = setupDom()
    // IA ligada, sem ir à rede.
    const ai = useAiStore()
    ai.settings = { enabled: true } as never
  })

  afterEach(teardownDom)

  it('abre na etapa atual, com o estado e o botão que a resolve', async () => {
    const wrapper = mountWith(DeployGuide, { guide: midway(), canAdmin: true }, pinia)
    await open()
    expect(bodyText()).toContain('1 de 5 etapas')
    const panel = document.querySelector('[data-test="guide-panel"]')
    expect(panel?.textContent).toContain('Nenhum servidor com o agente ainda.')
    expect(panel?.textContent).toContain('como fazer agent')
    expect(
      document.querySelector('[data-test="guide-step-agent"]')?.getAttribute('aria-current')
    ).toBe('step')

    button('Abrir Servidores remotos')?.click()
    expect(wrapper.emitted('action')?.[0]).toEqual([{ kind: 'open_agents' }])
  })

  it('escolher outra etapa mostra como fazê-la', async () => {
    mountWith(DeployGuide, { guide: midway(), canAdmin: true }, pinia)
    await open()
    ;(document.querySelector('[data-test="guide-step-application"]') as HTMLElement).click()
    await flushPromises()
    const panel = document.querySelector('[data-test="guide-panel"]')
    expect(panel?.textContent).toContain('como fazer application')
    expect(button('Automatizar deploy')).toBeTruthy()
  })

  it('sem ser administrador, a ação de cadastro vira aviso', async () => {
    const guide = midway()
    guide.steps[1] = { ...guide.steps[1]!, status: 'done' }
    guide.steps[2] = { ...guide.steps[2]!, status: 'current' }
    mountWith(DeployGuide, { guide, canAdmin: false }, pinia)
    await open()
    expect(button('Automatizar deploy')).toBeUndefined()
    expect(bodyText()).toContain('Esta etapa é com um administrador de Servidores.')
  })

  it('"Perguntar à IA sobre esta etapa" abre o chat com a etapa', async () => {
    const ask = vi.spyOn(useAiStore(), 'askAbout').mockReturnValue(true)
    mountWith(DeployGuide, { guide: midway(), canAdmin: true }, pinia)
    await open()
    button('Perguntar à IA sobre esta etapa')?.click()
    expect(ask).toHaveBeenCalledTimes(1)
    expect(ask.mock.calls[0]?.[0]).toContain('Ter um servidor com o agente')
  })

  it('a etapa atual andou (SSE): o painel acompanha', async () => {
    const wrapper = mountWith(DeployGuide, { guide: midway(), canAdmin: true }, pinia)
    await open()
    const next = midway()
    next.steps[1] = { ...next.steps[1]!, status: 'done', detail: 'srv-01 conectado.' }
    next.steps[2] = {
      ...next.steps[2]!,
      status: 'current',
      detail: 'Nenhuma aplicação cadastrada.',
    }
    await wrapper.setProps({ guide: next })
    await flushPromises()
    expect(document.querySelector('[data-test="guide-panel"]')?.textContent).toContain(
      'Nenhuma aplicação cadastrada.'
    )
  })

  it('começa fechado: uma linha com a próxima etapa, o passo a passo no botão', async () => {
    mountWith(DeployGuide, { guide: midway(), canAdmin: true }, pinia)
    await flushPromises()
    expect(document.querySelector('[data-test="guide-summary"]')?.textContent).toContain(
      'próxima: Ter um servidor com o agente'
    )
    expect(document.querySelector('[data-test="guide-panel"]')).toBeNull()
    expect(button('Perguntar à IA')).toBeUndefined()
    await open()
    expect(document.querySelector('[data-test="guide-panel"]')).not.toBeNull()
  })

  it('concluído, o resumo diz que está tudo pronto', async () => {
    const guide = midway()
    guide.done = true
    guide.steps = guide.steps.map((item) => ({ ...item, status: 'done' as const }))
    mountWith(DeployGuide, { guide, canAdmin: true }, pinia)
    await flushPromises()
    expect(bodyText()).toContain('Tudo pronto: cada push publica sozinho')
    expect(document.querySelector('[data-test="guide-panel"]')).toBeNull()
  })
})
