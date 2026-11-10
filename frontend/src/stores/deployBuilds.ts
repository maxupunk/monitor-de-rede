import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { apiService } from '@/services/apiService'
import type { BuilderReadiness } from '@/bindings/BuilderReadiness'
import type { DeployBuildInput } from '@/bindings/DeployBuildInput'
import type { DeployBuildSecretView } from '@/bindings/DeployBuildSecretView'
import type { DeployServerBuildView } from '@/bindings/DeployServerBuildView'
import { BUILD_STATUS } from '@/utils/deployPresentation'

/** O payload do SSE tem a forma de uma compilação? */
export function isServerBuild(value: unknown): value is DeployServerBuildView {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Record<string, unknown>
  return (
    typeof candidate.id === 'number' &&
    typeof candidate.applicationId === 'number' &&
    typeof candidate.status === 'string' &&
    typeof candidate.commitSha === 'string'
  )
}

/** As linhas novas do console de uma compilação (`deploys:build-log`). */
export function isBuildLines(value: unknown): value is { buildId: number; lines: string[] } {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Record<string, unknown>
  return typeof candidate.buildId === 'number' && Array.isArray(candidate.lines)
}

const MAX_CONSOLE_LINES = 4000

/**
 * Compilação no servidor (ADR 023). A lista vem uma vez ao abrir a página;
 * daí em diante o estado de cada compilação (`deploys:build`) e as linhas do
 * console (`deploys:build-log`) chegam pelo SSE. Nenhuma consulta periódica
 * (AGENTS §9).
 */
export const useDeployBuildsStore = defineStore('deployBuilds', () => {
  const builds = ref<Record<number, DeployServerBuildView>>({})
  const consoles = ref<Record<number, string[]>>({})
  const loaded = ref(false)

  /** As mais recentes primeiro. */
  const recent = computed(() =>
    Object.values(builds.value)
      .sort((a, b) => b.id - a.id)
      .slice(0, 30)
  )

  /** A compilação mais nova de cada aplicação. */
  function latestOf(applicationId: number): DeployServerBuildView | null {
    return recent.value.find((build) => build.applicationId === applicationId) ?? null
  }

  async function fetchRecent(): Promise<void> {
    const list = await apiService.get<DeployServerBuildView[]>('/deploys/builds')
    const next: Record<number, DeployServerBuildView> = { ...builds.value }
    for (const build of list) next[build.id] = build
    builds.value = next
    loaded.value = true
  }

  /** Um estado novo — um "em curso" atrasado não desfaz um fim. */
  function apply(build: DeployServerBuildView) {
    const known = builds.value[build.id]
    if (known && !BUILD_STATUS[known.status].live && BUILD_STATUS[build.status].live) return
    builds.value = { ...builds.value, [build.id]: build }
  }

  function applyLines(buildId: number, lines: string[]) {
    const current = consoles.value[buildId]
    if (!current) return
    const next = [...current, ...lines]
    consoles.value = {
      ...consoles.value,
      [buildId]: next.length > MAX_CONSOLE_LINES ? next.slice(-MAX_CONSOLE_LINES) : next,
    }
  }

  /** Abre o console: o que já foi gravado, e o resto chega pelo SSE. */
  async function openConsole(buildId: number): Promise<void> {
    const { log } = await apiService.get<{ log: string }>(`/deploys/builds/${buildId}/log`)
    consoles.value = { ...consoles.value, [buildId]: log ? log.split('\n') : [] }
  }

  function closeConsole(buildId: number) {
    const next = { ...consoles.value }
    delete next[buildId]
    consoles.value = next
  }

  async function start(applicationId: number, input: DeployBuildInput) {
    const build = await apiService.post<DeployServerBuildView>(
      `/deploys/applications/${applicationId}/builds`,
      input
    )
    apply(build)
    return build
  }

  async function rebuild(buildId: number) {
    const build = await apiService.post<DeployServerBuildView>(
      `/deploys/builds/${buildId}/rebuild`,
      {}
    )
    apply(build)
    return build
  }

  async function cancel(buildId: number) {
    await apiService.post(`/deploys/builds/${buildId}/cancel`, {})
  }

  function secrets(applicationId: number) {
    return apiService.get<DeployBuildSecretView[]>(
      `/deploys/applications/${applicationId}/build-secrets`
    )
  }

  function saveSecret(applicationId: number, name: string, value: string) {
    return apiService.post<DeployBuildSecretView[]>(
      `/deploys/applications/${applicationId}/build-secrets`,
      { name, value }
    )
  }

  function removeSecret(applicationId: number, name: string) {
    return apiService.delete(
      `/deploys/applications/${applicationId}/build-secrets/${encodeURIComponent(name)}`
    )
  }

  function builderReadiness(probeId: number) {
    return apiService.get<BuilderReadiness>(`/deploys/agents/${probeId}/builder`)
  }

  return {
    builds,
    consoles,
    loaded,
    recent,
    latestOf,
    fetchRecent,
    apply,
    applyLines,
    openConsole,
    closeConsole,
    start,
    rebuild,
    cancel,
    secrets,
    saveSecret,
    removeSecret,
    builderReadiness,
  }
})
