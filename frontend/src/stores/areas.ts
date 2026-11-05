import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { Area } from '@/bindings/Area'
import type { AreaImpact } from '@/bindings/AreaImpact'
import type { AreasSnapshot } from '@/bindings/AreasSnapshot'
import type { AreaState } from '@/bindings/AreaState'
import type { OnboardingIntentInput } from '@/bindings/OnboardingIntentInput'

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** O payload do `areas:updated` (e do `GET /api/areas`) tem a forma esperada? */
export function isAreasSnapshot(value: unknown): value is AreasSnapshot {
  return (
    isRecord(value) &&
    Array.isArray(value.areas) &&
    value.areas.every(
      (item) => isRecord(item) && typeof item.area === 'string' && typeof item.enabled === 'boolean'
    )
  )
}

/**
 * Áreas do produto (ADR 017): Núcleo, Rede, Servidores e VPN.
 *
 * O estado chega **só pelo SSE** (`areas:updated`, também no snapshot de quem
 * conecta) — sem polling nem hidratação por HTTP (AGENTS §9). Enquanto o
 * snapshot não chega, toda área conta como ligada: é o comportamento de antes
 * das áreas, e nada some da tela por engano.
 *
 * As ações (ligar/desligar, impacto, escolha do assistente) são pedidos
 * pontuais do administrador; a resposta já atualiza o estado, e o SSE leva a
 * mudança às outras abas.
 */
export const useAreasStore = defineStore('areas', () => {
  const snapshot = ref<AreasSnapshot | null>(null)
  const saving = ref<Area | null>(null)

  const states = computed<AreaState[]>(() => snapshot.value?.areas ?? [])

  /** Áreas ligadas. Sem snapshot, todas. */
  const enabledAreas = computed<ReadonlySet<Area>>(() => {
    if (!snapshot.value) return new Set<Area>(['core', 'network', 'servers', 'vpn'])
    return new Set(snapshot.value.areas.filter((state) => state.enabled).map((state) => state.area))
  })

  function isEnabled(area: Area): boolean {
    return area === 'core' || enabledAreas.value.has(area)
  }

  function stateOf(area: Area): AreaState | undefined {
    return states.value.find((state) => state.area === area)
  }

  /** Aplica o estado vindo do SSE ou da resposta de uma ação. */
  function apply(value: unknown) {
    if (isAreasSnapshot(value)) snapshot.value = value
  }

  /** O que acontece ao desligar a área (para a confirmação). Só admin. */
  async function fetchImpact(area: Area): Promise<AreaImpact> {
    return apiService.get<AreaImpact>(`/areas/${area}/impact`)
  }

  /** Liga ou desliga a área. Só admin; recusa (409) quando a área bloqueia. */
  async function setEnabled(area: Area, enabled: boolean): Promise<void> {
    saving.value = area
    try {
      apply(await apiService.put<AreasSnapshot>(`/areas/${area}`, { enabled }))
    } finally {
      saving.value = null
    }
  }

  /** A primeira etapa do assistente: o que a instalação vai gerenciar. */
  async function applyIntent(intent: OnboardingIntentInput): Promise<void> {
    apply(await apiService.post<AreasSnapshot>('/settings/onboarding/intent', intent))
  }

  return {
    snapshot,
    states,
    saving,
    enabledAreas,
    isEnabled,
    stateOf,
    apply,
    fetchImpact,
    setEnabled,
    applyIntent,
  }
})
