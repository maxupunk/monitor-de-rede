import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import type { Pinia } from 'pinia'
import type { Area } from '@/bindings/Area'
import type { AreasSnapshot } from '@/bindings/AreasSnapshot'
import type { OnboardingStatus } from '@/bindings/OnboardingStatus'
import InitialSetupDialog from '@/components/InitialSetupDialog.vue'
import { setupSteps } from '@/components/setup/steps'
import { apiService } from '@/services/apiService'
import { useAreasStore } from '@/stores/areas'
import { useOnboardingStore } from '@/stores/onboarding'
import { useAuthStore } from '@/stores/auth'
import { bodyText, button, mountWith, setRole, setupDom, teardownDom } from './domHarness'

let pinia: Pinia

function snapshot(on: Partial<Record<Area, boolean>>): AreasSnapshot {
  const areas: Area[] = ['core', 'network', 'servers', 'vpn']
  return {
    areas: areas.map((area) => ({
      area,
      title: area,
      summary: '',
      icon: 'mdi-circle',
      enabled: area === 'core' || (on[area] ?? false),
      optional: area !== 'core',
      evidence: null,
    })),
  }
}

function status(overrides: Partial<OnboardingStatus> = {}): OnboardingStatus {
  return {
    completed: false,
    completedAt: null,
    needsOnboarding: true,
    intentChosen: false,
    sitesCount: 0,
    networksCount: 0,
    dnsServersCount: 0,
    vpnConfigured: false,
    detectedLanIp: null,
    detectedPublicIp: null,
    ...overrides,
  }
}

/** Só o status responde; o resto falha, e as stores seguem com o padrão. */
function mockGet(st: OnboardingStatus) {
  return vi.spyOn(apiService, 'get').mockImplementation((url: string) => {
    if (url === '/settings/onboarding') return Promise.resolve(st)
    return Promise.reject(new Error('fora do teste'))
  })
}

/** Os ids das etapas, na barra de etapas. */
function stepIds(): string[] {
  return [...document.body.querySelectorAll('[data-step]')].map(
    (el) => el.getAttribute('data-step') ?? ''
  )
}

function switchFor(label: string): HTMLInputElement {
  const input = document.body.querySelector<HTMLInputElement>(`input[aria-label="${label}"]`)
  if (!input) throw new Error(`switch ${label} não encontrado`)
  return input
}

async function open() {
  const wrapper = mountWith(InitialSetupDialog, { modelValue: true }, pinia)
  await flushPromises()
  return wrapper
}

describe('registro de etapas', () => {
  it('só Servidores: no máximo 3 etapas depois da escolha', () => {
    const only = (area: Area) => area === 'core' || area === 'servers'
    const steps = setupSteps(only, true)
    expect(steps[0]?.id).toBe('intent')
    expect(steps.slice(1).map((s) => s.id)).toEqual(['addresses', 'agents', 'notifications'])
  })

  it('Rede: as etapas de sempre, com Aplicativos e sem a etapa enxuta de notificações', () => {
    const ids = setupSteps((area) => area !== 'vpn', false).map((s) => s.id)
    expect(ids).toEqual(['site', 'addresses', 'network', 'dns', 'apps', 'agents', 'general'])
  })

  it('VPN só com a área VPN', () => {
    expect(setupSteps(() => true, false).map((s) => s.id)).toContain('vpn')
    expect(setupSteps((area) => area !== 'vpn', false).map((s) => s.id)).not.toContain('vpn')
  })
})

describe('Assistente de configuração inicial', () => {
  beforeEach(() => {
    pinia = setupDom()
  })

  afterEach(teardownDom)

  it('caminho só Servidores: grava a escolha e segue com 3 etapas', async () => {
    setRole('admin')
    useAreasStore().apply(snapshot({ network: true, servers: true, vpn: true }))
    mockGet(status())
    const post = vi
      .spyOn(apiService, 'post')
      .mockResolvedValue(snapshot({ network: false, servers: true, vpn: false }))
    await open()

    expect(bodyText()).toContain('O que você vai gerenciar?')
    switchFor('Gerenciar Rede').click()
    switchFor('Gerenciar VPN WireGuard').click()
    await flushPromises()
    button('Continuar')?.click()
    await flushPromises()

    expect(post).toHaveBeenCalledWith('/settings/onboarding/intent', {
      network: false,
      servers: true,
      vpn: false,
    })
    const ids = stepIds()
    expect(ids[0]).toBe('intent')
    expect(ids.slice(1)).toEqual(['addresses', 'agents', 'notifications'])
    expect(bodyText()).toContain('Etapa 2 de 4')
    expect(bodyText()).toContain('Endereços por onde este Servidor')
    expect(bodyText()).not.toContain('Túnel VPN (WireGuard)')
  })

  it('sem Rede nem Servidores: avisa e não chama a API', async () => {
    setRole('admin')
    useAreasStore().apply(snapshot({ network: true, servers: false, vpn: false }))
    mockGet(status())
    const post = vi.spyOn(apiService, 'post')
    await open()

    switchFor('Gerenciar Rede').click()
    await flushPromises()
    expect(bodyText()).toContain('para continuar')
    button('Continuar')?.click()
    await flushPromises()

    expect(post).not.toHaveBeenCalled()
    expect(bodyText()).toContain('Etapa 1 de')
  })

  it('mostra a recusa da API na escolha', async () => {
    setRole('admin')
    useAreasStore().apply(snapshot({ network: true, servers: true, vpn: false }))
    mockGet(status())
    vi.spyOn(apiService, 'post').mockRejectedValue(new Error('A área VPN tem túneis ativos.'))
    await open()

    button('Continuar')?.click()
    await flushPromises()
    expect(bodyText()).toContain('A área VPN tem túneis ativos.')
    expect(bodyText()).toContain('Etapa 1 de')
  })

  it('com Rede desligada não pede site nem sub-rede (operador, sem a escolha)', async () => {
    setRole('operator')
    useAreasStore().apply(snapshot({ network: false, servers: true, vpn: false }))
    mockGet(status({ intentChosen: true }))
    await open()

    const ids = stepIds()
    expect(ids).not.toContain('intent')
    expect(ids).not.toContain('site')
    expect(ids).not.toContain('network')
    expect(ids).not.toContain('dns')
    expect(bodyText()).not.toContain('Identificação do Site Local')
    expect(bodyText()).toContain('Endereços por onde este Servidor')
  })
})

describe('abertura automática', () => {
  beforeEach(() => {
    pinia = setupDom()
  })

  afterEach(teardownDom)

  it('abre quando o servidor diz que falta o assistente', async () => {
    setRole('operator')
    useAuthStore().token = 'x'
    mockGet(status({ intentChosen: false }))
    const store = useOnboardingStore()
    expect(await store.checkAndOpenIfNeeded()).toBe(true)
    expect(store.showWizard).toBe(true)
  })

  it('só Servidores escolhido e sem site/rede: não abre', async () => {
    setRole('admin')
    useAuthStore().token = 'x'
    mockGet(status({ intentChosen: true, needsOnboarding: false, sitesCount: 0 }))
    const store = useOnboardingStore()
    expect(await store.checkAndOpenIfNeeded()).toBe(false)
    expect(store.showWizard).toBe(false)
  })
})
