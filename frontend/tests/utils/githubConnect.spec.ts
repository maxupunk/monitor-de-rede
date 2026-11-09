import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  centralUrlRule,
  looksPrivate,
  parseGithubCallback,
  rememberGithubReturn,
  storeCentralUrl,
  storedCentralUrl,
  submitManifest,
  takeGithubReturn,
} from '@/utils/githubConnect'
import { WORKFLOW_STATE, buildMeta } from '@/utils/deployPresentation'
import { applyDiscovery, emptyApplication, referenceChoices, slugify } from '@/utils/deployForms'
import { githubQuery } from '@/stores/deploys'
import { stubBrowserStorage } from '../helpers/storage'

beforeEach(() => stubBrowserStorage())
afterEach(() => vi.unstubAllGlobals())

describe('volta do GitHub', () => {
  it('reconhece a criação e a instalação do app', () => {
    expect(parseGithubCallback({ code: 'abc', state: 's1' })).toEqual({
      kind: 'created',
      code: 'abc',
      state: 's1',
    })
    expect(parseGithubCallback({ installation_id: '77', setup_action: 'install' })).toEqual({
      kind: 'installed',
      installationId: 77,
    })
    expect(parseGithubCallback({ run: '3' })).toBeNull()
    expect(parseGithubCallback({ code: 'abc' })).toBeNull()
    expect(parseGithubCallback({ installation_id: 'x' })).toBeNull()
  })

  it('retoma o assistente uma vez só', () => {
    rememberGithubReturn('setup')
    expect(takeGithubReturn()).toBe('setup')
    expect(takeGithubReturn()).toBeNull()
  })

  it('envia o manifest num formulário POST para o GitHub', () => {
    const submit = vi.spyOn(HTMLFormElement.prototype, 'submit').mockImplementation(() => {})
    submitManifest({
      actionUrl: 'https://github.com/settings/apps/new?state=s1',
      manifest: '{"name":"NetMonitor abc"}',
    })
    const form = document.body.querySelector('form')
    expect(form?.method).toBe('post')
    expect(form?.action).toBe('https://github.com/settings/apps/new?state=s1')
    const field = form?.querySelector<HTMLInputElement>('input[name="manifest"]')
    expect(field?.value).toBe('{"name":"NetMonitor abc"}')
    expect(submit).toHaveBeenCalledOnce()
    form?.remove()
    submit.mockRestore()
  })
})

describe('endereço público da central', () => {
  it('começa pelo endereço da aba e lembra o que a pessoa trocou', () => {
    expect(storedCentralUrl()).toBe(window.location.origin)
    storeCentralUrl('https://central.exemplo.com/')
    expect(storedCentralUrl()).toBe('https://central.exemplo.com')
    storeCentralUrl(window.location.origin)
    expect(storedCentralUrl()).toBe(window.location.origin)
  })

  it('aceita só o endereço, sem caminho', () => {
    expect(centralUrlRule('https://central.exemplo.com')).toBe(true)
    expect(centralUrlRule('https://central.exemplo.com/deploys')).not.toBe(true)
    expect(centralUrlRule('ftp://x')).not.toBe(true)
    expect(centralUrlRule('central')).not.toBe(true)
  })

  it('avisa quando o GitHub não alcança o endereço', () => {
    expect(looksPrivate('http://192.168.0.10:3333')).toBe(true)
    expect(looksPrivate('http://10.0.0.5')).toBe(true)
    expect(looksPrivate('http://localhost:5173')).toBe(true)
    expect(looksPrivate('https://central.exemplo.com')).toBe(false)
  })
})

describe('assistente preenchido pelo GitHub', () => {
  it('o compose preenche arquivos, imagens e a variável da tag', () => {
    const filled = applyDiscovery(emptyApplication(), {
      files: ['docker-compose.dev.yml', 'docker-compose.yml'],
      suggestedFiles: ['docker-compose.yml'],
      services: ['sgc-api'],
      images: [{ name: 'sgc-api', dockerfile: 'apps/api/Dockerfile', context: '.' }],
      imageTagVariable: 'SGC_TAG',
      notes: [],
    })
    expect(filled.composeFiles).toEqual(['docker-compose.yml'])
    expect(filled.images).toHaveLength(1)
    expect(filled.imageTagVariable).toBe('SGC_TAG')
  })

  it('compose sem nada a dizer não apaga o que já estava', () => {
    const before = { ...emptyApplication(), imageTagVariable: 'TAG' }
    const filled = applyDiscovery(before, {
      files: [],
      suggestedFiles: [],
      services: [],
      images: [],
      imageTagVariable: null,
      notes: ['Nenhum arquivo compose'],
    })
    expect(filled.composeFiles).toEqual(before.composeFiles)
    expect(filled.imageTagVariable).toBe('TAG')
  })

  it('as branches viram opções e as tags vN viram o padrão v*', () => {
    expect(
      referenceChoices({ defaultBranch: 'dev', branches: ['dev', 'stable'], tags: ['v1.2.0'] })
    ).toEqual(['dev', 'stable', 'v*'])
    expect(referenceChoices({ defaultBranch: 'main', branches: ['main'], tags: ['2024'] })).toEqual(
      ['main']
    )
    expect(referenceChoices(null)).toEqual([])
  })

  it('identificador a partir do nome do repositório', () => {
    expect(slugify('SGCv4')).toBe('sgcv4')
    expect(slugify('Meu App.Web')).toBe('meu-app-web')
  })

  it('a consulta leva só a origem escolhida', () => {
    expect(githubQuery({ connectionId: 3, credentialId: null }, { repository: 'a/b' })).toBe(
      'repository=a%2Fb&connectionId=3'
    )
    expect(githubQuery({ connectionId: null, credentialId: 5 })).toBe('credentialId=5')
  })
})

describe('repositório na tela', () => {
  it('build em andamento, que passou e que falhou', () => {
    expect(buildMeta({ status: 'in_progress', conclusion: null }).live).toBe(true)
    expect(buildMeta({ status: 'completed', conclusion: 'success' }).color).toBe('success')
    expect(buildMeta({ status: 'completed', conclusion: 'failure' }).color).toBe('error')
  })

  it('nenhum estado do workflow usa cor apagada', () => {
    for (const meta of Object.values(WORKFLOW_STATE)) {
      expect(['default', 'grey']).not.toContain(meta.color)
    }
  })
})
