import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import type { Pinia } from 'pinia'
import type { AreaState } from '@/bindings/AreaState'
import AreaSuggestions from '@/components/areas/AreaSuggestions.vue'
import { areaSuggestions } from '@/components/areas/areaSuggestions'
import { apiService } from '@/services/apiService'
import { useAreasStore } from '@/stores/areas'
import { useOnboardingStore } from '@/stores/onboarding'
import { bodyText, button, mountWith, setRole, setupDom, teardownDom } from './domHarness'

let pinia: Pinia

function state(overrides: Partial<AreaState> & Pick<AreaState, 'area'>): AreaState {
  return {
    title: overrides.area,
    summary: '',
    icon: 'mdi-circle',
    enabled: true,
    optional: true,
    evidence: null,
    ...overrides,
  }
}

const vpnOffWithPeers = state({
  area: 'vpn',
  title: 'VPN',
  enabled: false,
  evidence: { present: true, reason: '3 peers cadastrados' },
})
const serversOnUnused = state({
  area: 'servers',
  title: 'Servidores',
  enabled: true,
  evidence: { present: false, reason: null },
})

/** `localStorage` de verdade (um Map), para o aviso dispensado sobreviver à remontagem. */
function memoryStorage() {
  const data = new Map<string, string>()
  vi.stubGlobal('localStorage', {
    getItem: (key: string) => data.get(key) ?? null,
    setItem: (key: string, value: string) => void data.set(key, value),
    removeItem: (key: string) => void data.delete(key),
    clear: () => data.clear(),
  })
  return data
}

function intentChosen(chosen: boolean) {
  useOnboardingStore().status = {
    completed: true,
    completedAt: null,
    needsOnboarding: false,
    intentChosen: chosen,
    sitesCount: 0,
    networksCount: 0,
    dnsServersCount: 0,
    vpnConfigured: false,
    detectedLanIp: null,
    detectedPublicIp: null,
  }
}

describe('areaSuggestions', () => {
  it('sugere mostrar a desligada com evidência e ocultar a ligada sem uso', () => {
    const list = areaSuggestions([vpnOffWithPeers, serversOnUnused], true)
    expect(list.map((s) => [s.area, s.direction])).toEqual([
      ['vpn', 'show'],
      ['servers', 'hide'],
    ])
    expect(list[0]?.message).toBe('Encontramos 3 peers cadastrados. Mostrar a área VPN?')
  })

  it('não sugere ocultar antes da escolha do assistente nem mexe no Núcleo', () => {
    const core = state({
      area: 'core',
      optional: false,
      evidence: { present: false, reason: null },
    })
    expect(areaSuggestions([serversOnUnused, core], false)).toEqual([])
  })
})

describe('AreaSuggestions', () => {
  beforeEach(() => {
    pinia = setupDom()
    memoryStorage()
  })

  afterEach(teardownDom)

  it('só para o administrador', async () => {
    setRole('operator')
    useAreasStore().apply({ areas: [vpnOffWithPeers] })
    mountWith(AreaSuggestions, {}, pinia)
    await flushPromises()
    expect(bodyText()).not.toContain('Mostrar a área')
  })

  it('"Mostrar" liga a área e o aviso não volta', async () => {
    setRole('admin')
    const areas = useAreasStore()
    areas.apply({ areas: [vpnOffWithPeers] })
    const put = vi
      .spyOn(apiService, 'put')
      .mockResolvedValue({ areas: [{ ...vpnOffWithPeers, enabled: true }] })
    const first = mountWith(AreaSuggestions, {}, pinia)
    await flushPromises()

    expect(bodyText()).toContain('Encontramos 3 peers cadastrados. Mostrar a área VPN?')
    button('Mostrar')?.click()
    await flushPromises()
    expect(put).toHaveBeenCalledWith('/areas/vpn', { enabled: true })

    // Mesmo que a área volte a ficar desligada, a sugestão já foi feita.
    first.unmount()
    areas.apply({ areas: [vpnOffWithPeers] })
    mountWith(AreaSuggestions, {}, pinia)
    await flushPromises()
    expect(bodyText()).not.toContain('Mostrar a área VPN')
  })

  it('dispensar esconde o aviso para sempre, sem mudar a área', async () => {
    setRole('admin')
    useAreasStore().apply({ areas: [vpnOffWithPeers] })
    const put = vi.spyOn(apiService, 'put')
    const first = mountWith(AreaSuggestions, {}, pinia)
    await flushPromises()

    const close = document.body.querySelector<HTMLButtonElement>('.v-alert__close button')
    close?.click()
    await flushPromises()
    expect(bodyText()).not.toContain('Mostrar a área VPN')
    expect(put).not.toHaveBeenCalled()

    first.unmount()
    mountWith(AreaSuggestions, {}, pinia)
    await flushPromises()
    expect(bodyText()).not.toContain('Mostrar a área VPN')
  })

  it('"Ocultar" pede confirmação com o impacto e respeita o bloqueio', async () => {
    setRole('admin')
    intentChosen(true)
    useAreasStore().apply({ areas: [serversOnUnused] })
    vi.spyOn(apiService, 'get').mockResolvedValue({
      area: 'servers',
      consequences: [{ message: 'Docker some do menu.' }],
      blocker: { message: 'Há um backup de banco agendado.' },
    })
    const put = vi.spyOn(apiService, 'put')
    mountWith(AreaSuggestions, {}, pinia)
    await flushPromises()

    expect(bodyText()).toContain('Nada na área Servidores está em uso. Ocultar do menu?')
    button('Ocultar')?.click()
    await flushPromises()

    expect(bodyText()).toContain('Docker some do menu.')
    expect(bodyText()).toContain('Há um backup de banco agendado.')
    expect(button('Ocultar área')?.disabled).toBe(true)
    expect(put).not.toHaveBeenCalled()
  })

  it('sem a escolha do assistente, não sugere ocultar', async () => {
    setRole('admin')
    intentChosen(false)
    useAreasStore().apply({ areas: [serversOnUnused] })
    mountWith(AreaSuggestions, {}, pinia)
    await flushPromises()
    expect(bodyText()).not.toContain('Ocultar do menu')
  })
})
