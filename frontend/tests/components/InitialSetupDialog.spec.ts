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

const finishRestoreSpy = vi.hoisted(() => vi.fn())
vi.mock('@/stores/backup', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/stores/backup')>()),
  finishRestore: finishRestoreSpy,
}))

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

/** O administrador começa escolhendo: restaurar um backup ou configurar do zero. */
async function choose(option: 'restore' | 'fresh') {
  document.body.querySelector<HTMLElement>(`[data-choice="${option}"]`)?.click()
  await flushPromises()
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
    finishRestoreSpy.mockReset()
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
    await choose('fresh')

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
    await choose('fresh')

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
    await choose('fresh')

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
  it('administrador escolhe antes das etapas; do zero segue o assistente de sempre', async () => {
    setRole('admin')
    useAreasStore().apply(snapshot({ network: true, servers: true, vpn: false }))
    mockGet(status())
    await open()

    expect(bodyText()).toContain('Como você quer começar?')
    expect(bodyText()).toContain('Restaurar um backup')
    expect(bodyText()).toContain('Configurar do zero')
    expect(stepIds()).toEqual([])

    await choose('fresh')
    expect(bodyText()).toContain('O que você vai gerenciar?')
    expect(stepIds()[0]).toBe('intent')

    // Voltar da primeira etapa devolve à escolha.
    button('Voltar')?.click()
    await flushPromises()
    expect(bodyText()).toContain('Como você quer começar?')
  })

  it('restaurar: confere o arquivo, aplica pela rota de backup e conclui o assistente', async () => {
    setRole('admin')
    useAreasStore().apply(snapshot({ network: true, servers: true, vpn: false }))
    mockGet(status())
    const counts = {
      scope: 'full',
      tables: [{ table: 'users', label: 'Usuários', rows: 2 }],
      totalRows: 2,
    }
    const upload = vi
      .spyOn(apiService, 'upload')
      .mockResolvedValueOnce(counts)
      .mockResolvedValueOnce({ counts, signInAgain: false })
    const post = vi.spyOn(apiService, 'post').mockResolvedValue({})
    await open()
    await choose('restore')

    expect(bodyText()).toContain('O que acontece depois')
    const input = document.body.querySelector<HTMLInputElement>('input[type="file"]')
    expect(input).not.toBeNull()
    const file = new File(['{}'], 'netmonitor-backup-completo.json.gz', {
      type: 'application/gzip',
    })
    Object.defineProperty(input, 'files', { value: [file] })
    input?.dispatchEvent(new Event('change'))
    await flushPromises()

    expect(upload).toHaveBeenCalledWith('/backup/preview', file, expect.any(Function))
    expect(bodyText()).toContain('Backup completo')
    expect(bodyText()).toContain('Usuários: 2')

    button('Restaurar esta cópia')?.click()
    await flushPromises()

    expect(upload).toHaveBeenLastCalledWith('/backup/restore', file, expect.any(Function))
    expect(post).toHaveBeenCalledWith('/settings/onboarding/complete')
    expect(finishRestoreSpy).toHaveBeenCalledWith({ counts, signInAgain: false })
  })

  it('restaurar: a recusa do arquivo aparece e nada é aplicado', async () => {
    setRole('admin')
    useAreasStore().apply(snapshot({ network: true, servers: true, vpn: false }))
    mockGet(status())
    const upload = vi
      .spyOn(apiService, 'upload')
      .mockRejectedValueOnce(new Error('O arquivo não é um backup do NetMonitor'))
    await open()
    await choose('restore')

    const input = document.body.querySelector<HTMLInputElement>('input[type="file"]')
    Object.defineProperty(input, 'files', { value: [new File(['x'], 'qualquer.json')] })
    input?.dispatchEvent(new Event('change'))
    await flushPromises()

    expect(upload).toHaveBeenCalledTimes(1)
    expect(bodyText()).toContain('O arquivo não é um backup do NetMonitor')
    expect(button('Restaurar esta cópia')).toBeUndefined()
    expect(finishRestoreSpy).not.toHaveBeenCalled()
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
