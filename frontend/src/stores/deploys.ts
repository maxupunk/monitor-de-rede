import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { apiService } from '@/services/apiService'
import type { DeployApplicationInput } from '@/bindings/DeployApplicationInput'
import type { DeployCredentialInput } from '@/bindings/DeployCredentialInput'
import type { DeployAgentView } from '@/bindings/DeployAgentView'
import type { DeployBackupView } from '@/bindings/DeployBackupView'
import type { DeployChangesView } from '@/bindings/DeployChangesView'
import type { DeployCredentialView } from '@/bindings/DeployCredentialView'
import type { DeployEnvContentView } from '@/bindings/DeployEnvContentView'
import type { DeployEnvSaved } from '@/bindings/DeployEnvSaved'
import type { DeployEnvVersionView } from '@/bindings/DeployEnvVersionView'
import type { DeployInput } from '@/bindings/DeployInput'
import type { DeployOverview } from '@/bindings/DeployOverview'
import type { DeployReadinessView } from '@/bindings/DeployReadinessView'
import type { DeployRunLogView } from '@/bindings/DeployRunLogView'
import type { DeployRunView } from '@/bindings/DeployRunView'
import type { DeployTargetView } from '@/bindings/DeployTargetView'
import type { DeployWhen } from '@/bindings/DeployWhen'
import type { DeployWorkflowView } from '@/bindings/DeployWorkflowView'
import type { DeployEnvDiff } from '@/bindings/DeployEnvDiff'
import type { DeploySetupInput } from '@/bindings/DeploySetupInput'
import type { DeployTargetInput } from '@/bindings/DeployTargetInput'
import type { DeployComposeDiscovery } from '@/bindings/DeployComposeDiscovery'
import type { DeployGithubConnectionView } from '@/bindings/DeployGithubConnectionView'
import type { DeployGithubManifestInput } from '@/bindings/DeployGithubManifestInput'
import type { DeployGithubManifestView } from '@/bindings/DeployGithubManifestView'
import type { DeployGithubStatusView } from '@/bindings/DeployGithubStatusView'
import type { DeployPullRequestView } from '@/bindings/DeployPullRequestView'
import type { DeployReferencesView } from '@/bindings/DeployReferencesView'
import type { DeployRepositoryView } from '@/bindings/DeployRepositoryView'
import { isFinished, isPending } from '@/utils/deployPresentation'

/** O payload do SSE tem a forma de um deploy? */
export function isDeployRun(value: unknown): value is DeployRunView {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Record<string, unknown>
  return (
    typeof candidate.id === 'number' &&
    typeof candidate.targetId === 'number' &&
    typeof candidate.status === 'string' &&
    typeof candidate.commitSha === 'string'
  )
}

/** As linhas novas do console (`deploys:log`). */
export function isDeployLines(value: unknown): value is { runId: number; lines: string[] } {
  if (typeof value !== 'object' || value === null) return false
  const candidate = value as Record<string, unknown>
  return typeof candidate.runId === 'number' && Array.isArray(candidate.lines)
}

/** De onde ler o GitHub: a conexão (app) e/ou o token. */
export interface GithubSource {
  connectionId: number | null
  credentialId: number | null
}

/** A query das rotas de descoberta. */
export function githubQuery(source: GithubSource, extra: Record<string, string> = {}): string {
  const query = new URLSearchParams(extra)
  if (source.connectionId != null) query.set('connectionId', String(source.connectionId))
  if (source.credentialId != null) query.set('credentialId', String(source.credentialId))
  return query.toString()
}

/** Linhas do console guardadas por deploy — o fim, que é onde está o erro. */
const MAX_CONSOLE_LINES = 4000

/**
 * Deploy contínuo (ADR 021). A página carrega a visão geral uma vez; daí em
 * diante tudo chega pelo SSE: o estado de cada deploy (`deploys:run`), as
 * linhas do console (`deploys:log`) e o aviso de que o cadastro mudou
 * (`deploys:updated`, que relê a visão geral uma vez). Nenhuma consulta
 * periódica (AGENTS §9).
 */
export const useDeploysStore = defineStore('deploys', () => {
  const overview = ref<DeployOverview | null>(null)
  const loading = ref(false)
  const error = ref<string | null>(null)
  /** O estado mais novo de cada deploy visto (SSE ou resposta de ação). */
  const runs = ref<Record<number, DeployRunView>>({})
  /** O console de cada deploy aberto. */
  const consoles = ref<Record<number, string[]>>({})
  const agents = ref<DeployAgentView[]>([])

  const applications = computed(() => overview.value?.applications ?? [])
  const credentials = computed<DeployCredentialView[]>(() => overview.value?.credentials ?? [])
  const githubConnections = computed<DeployGithubConnectionView[]>(
    () => overview.value?.githubConnections ?? []
  )
  const targets = computed<DeployTargetView[]>(() =>
    applications.value.flatMap((application) => application.targets)
  )

  /** Os deploys recentes, já com o estado mais novo do SSE. */
  const recentRuns = computed<DeployRunView[]>(() => {
    const known = new Map<number, DeployRunView>()
    for (const run of overview.value?.recentRuns ?? []) known.set(run.id, run)
    for (const run of Object.values(runs.value)) known.set(run.id, run)
    return [...known.values()].sort((a, b) => b.id - a.id).slice(0, 30)
  })

  /** Deploys esperando um administrador. */
  const awaitingApproval = computed(() =>
    recentRuns.value.filter((run) => run.status === 'awaiting_approval')
  )

  function message(err: unknown, fallback: string): string {
    return err instanceof Error ? err.message : fallback
  }

  async function fetchOverview(): Promise<boolean> {
    loading.value = true
    error.value = null
    try {
      overview.value = await apiService.get<DeployOverview>('/deploys')
      return true
    } catch (err) {
      error.value = message(err, 'Erro ao carregar os deploys')
      return false
    } finally {
      loading.value = false
    }
  }

  /** O que o SSE chama: sem efeito para quem nunca abriu a página. */
  function refreshIfLoaded() {
    if (overview.value) void fetchOverview()
  }

  /** O deploy mais novo de um alvo, como a linha mostra. */
  function patchTarget(run: DeployRunView) {
    for (const application of overview.value?.applications ?? []) {
      const target = application.targets.find((item) => item.id === run.targetId)
      if (!target) continue
      if (isPending(run.status) || run.status === 'running') {
        if (!target.pending || target.pending.id <= run.id) target.pending = run
      } else if (target.pending?.id === run.id) {
        target.pending = null
      }
      if (run.status === 'succeeded' && (!target.live || target.live.id <= run.id)) {
        target.live = run
      }
    }
  }

  /** Snapshot de um deploy (SSE ou resposta de ação). */
  function applyRun(run: DeployRunView) {
    const current = runs.value[run.id]
    // Um evento atrasado de "rodando" não desfaz o fim que já chegou.
    if (current && isFinished(current.status) && !isFinished(run.status)) return
    runs.value = { ...runs.value, [run.id]: run }
    patchTarget(run)
  }

  /** Linhas novas do console. Só guarda as de quem tem o console aberto. */
  function applyLines(runId: number, lines: string[]) {
    const current = consoles.value[runId]
    if (!current) return
    const next = current.concat(lines)
    consoles.value = {
      ...consoles.value,
      [runId]: next.length > MAX_CONSOLE_LINES ? next.slice(-MAX_CONSOLE_LINES) : next,
    }
  }

  function runById(id: number): DeployRunView | null {
    return runs.value[id] ?? recentRuns.value.find((run) => run.id === id) ?? null
  }

  /** Abre o console: o log gravado; o resto chega pelo SSE. */
  async function openConsole(runId: number): Promise<void> {
    if (!consoles.value[runId]) consoles.value = { ...consoles.value, [runId]: [] }
    const [saved, run] = await Promise.all([
      apiService.get<DeployRunLogView>(`/deploys/runs/${runId}/log`),
      apiService.get<DeployRunView>(`/deploys/runs/${runId}`),
    ])
    const live = consoles.value[runId] ?? []
    const lines = saved.log === '' ? [] : saved.log.replace(/\n$/, '').split('\n')
    // Linhas que chegaram pelo SSE durante a leitura já estão no log gravado
    // ou vêm depois dele: o gravado é a base, o que veio a mais fica no fim.
    const extra = live.filter((line) => !lines.includes(line))
    consoles.value = { ...consoles.value, [runId]: [...lines, ...extra] }
    applyRun(run)
  }

  function closeConsole(runId: number) {
    const next = { ...consoles.value }
    delete next[runId]
    consoles.value = next
  }

  async function fetchAgents(): Promise<void> {
    try {
      agents.value = await apiService.get<DeployAgentView[]>('/deploys/agents')
    } catch {
      agents.value = []
    }
  }

  // ── Cadastro ──────────────────────────────────────────────────────────

  async function saveCredential(id: number | null, input: DeployCredentialInput) {
    const saved =
      id == null
        ? await apiService.post<DeployCredentialView>('/deploys/credentials', input)
        : await apiService.put<DeployCredentialView>(`/deploys/credentials/${id}`, input)
    await fetchOverview()
    return saved
  }

  async function removeCredential(id: number) {
    await apiService.delete(`/deploys/credentials/${id}`)
    await fetchOverview()
  }

  async function setup(input: DeploySetupInput) {
    const created = await apiService.post<{ applicationId: number; targetId: number }>(
      '/deploys/setup',
      input
    )
    await fetchOverview()
    return created
  }

  async function saveApplication(id: number | null, input: DeployApplicationInput) {
    const saved =
      id == null
        ? await apiService.post<{ id: number }>('/deploys/applications', input)
        : await apiService.put<{ id: number }>(`/deploys/applications/${id}`, input)
    await fetchOverview()
    return saved
  }

  async function removeApplication(id: number) {
    await apiService.delete(`/deploys/applications/${id}`)
    await fetchOverview()
  }

  async function saveTarget(applicationId: number, id: number | null, input: DeployTargetInput) {
    const saved =
      id == null
        ? await apiService.post<{ id: number }>(
            `/deploys/applications/${applicationId}/targets`,
            input
          )
        : await apiService.put<{ id: number }>(`/deploys/targets/${id}`, input)
    await fetchOverview()
    return saved
  }

  async function removeTarget(id: number) {
    await apiService.delete(`/deploys/targets/${id}`)
    await fetchOverview()
  }

  function workflow(applicationId: number, centralUrl: string) {
    const query = new URLSearchParams({ centralUrl })
    return apiService.get<DeployWorkflowView>(
      `/deploys/applications/${applicationId}/workflow?${query.toString()}`
    )
  }

  // ── GitHub ────────────────────────────────────────────────────────────

  function githubManifest(input: DeployGithubManifestInput) {
    return apiService.post<DeployGithubManifestView>('/deploys/github/manifest', input)
  }

  async function connectGithub(code: string, state: string) {
    const connection = await apiService.post<DeployGithubConnectionView>(
      '/deploys/github/connections',
      { code, state }
    )
    await fetchOverview()
    return connection
  }

  async function claimGithubInstallation(installationId: number) {
    const connection = await apiService.post<DeployGithubConnectionView>(
      '/deploys/github/installations',
      { installationId }
    )
    await fetchOverview()
    return connection
  }

  async function refreshGithub(id: number) {
    const connection = await apiService.post<DeployGithubConnectionView>(
      `/deploys/github/connections/${id}/refresh`,
      {}
    )
    await fetchOverview()
    return connection
  }

  async function removeGithub(id: number) {
    await apiService.delete(`/deploys/github/connections/${id}`)
    await fetchOverview()
  }

  function githubRepositories(source: GithubSource) {
    return apiService.get<DeployRepositoryView[]>(
      `/deploys/github/repositories?${githubQuery(source)}`
    )
  }

  function githubReferences(source: GithubSource, repository: string) {
    return apiService.get<DeployReferencesView>(
      `/deploys/github/references?${githubQuery(source, { repository })}`
    )
  }

  function githubCompose(source: GithubSource, repository: string) {
    return apiService.get<DeployComposeDiscovery>(
      `/deploys/github/compose?${githubQuery(source, { repository })}`
    )
  }

  function githubStatus(applicationId: number, centralUrl: string) {
    const query = new URLSearchParams({ centralUrl })
    return apiService.get<DeployGithubStatusView>(
      `/deploys/applications/${applicationId}/github?${query.toString()}`
    )
  }

  function githubPullRequest(applicationId: number, centralUrl: string) {
    return apiService.post<DeployPullRequestView>(
      `/deploys/applications/${applicationId}/github/pull-request`,
      { centralUrl }
    )
  }

  // ── Prontidão e histórico ─────────────────────────────────────────────

  function readinessFor(probeId: number, project: string) {
    const query = new URLSearchParams({ probeId: String(probeId), project })
    return apiService.get<DeployReadinessView>(`/deploys/readiness?${query.toString()}`)
  }

  function targetReadiness(targetId: number) {
    return apiService.get<DeployReadinessView>(`/deploys/targets/${targetId}/readiness`)
  }

  function history(targetId: number) {
    return apiService.get<DeployRunView[]>(`/deploys/targets/${targetId}/runs`)
  }

  function changes(runId: number) {
    return apiService.get<DeployChangesView>(`/deploys/runs/${runId}/changes`)
  }

  function backups(runId: number) {
    return apiService.get<DeployBackupView[]>(`/deploys/runs/${runId}/backups`)
  }

  // ── Ações sobre deploys ───────────────────────────────────────────────

  async function act(path: string, body: unknown): Promise<DeployRunView> {
    const run = await apiService.post<DeployRunView>(path, body)
    applyRun(run)
    return run
  }

  function deploy(targetId: number, input: DeployInput) {
    return act(`/deploys/targets/${targetId}/deploy`, input)
  }

  function approve(runId: number, when: DeployWhen) {
    return act(`/deploys/runs/${runId}/approve`, { when })
  }

  function reject(runId: number, reason?: string) {
    return act(`/deploys/runs/${runId}/reject`, { reason })
  }

  function cancel(runId: number) {
    return act(`/deploys/runs/${runId}/cancel`, {})
  }

  function restore(runId: number, when: DeployWhen) {
    return act(`/deploys/runs/${runId}/restore`, { when })
  }

  // ── .env ──────────────────────────────────────────────────────────────

  function envCurrent(targetId: number) {
    return apiService.get<DeployEnvContentView>(`/deploys/targets/${targetId}/env`)
  }

  function envVersions(targetId: number) {
    return apiService.get<DeployEnvVersionView[]>(`/deploys/targets/${targetId}/env/versions`)
  }

  function envVersion(versionId: number) {
    return apiService.get<DeployEnvContentView>(`/deploys/env-versions/${versionId}`)
  }

  function envPreview(targetId: number, content: string) {
    return apiService.post<DeployEnvDiff>(`/deploys/targets/${targetId}/env/preview`, { content })
  }

  async function envSave(targetId: number, content: string, redeploy: boolean, note?: string) {
    const saved = await apiService.put<DeployEnvSaved>(`/deploys/targets/${targetId}/env`, {
      content,
      redeploy,
      note,
    })
    if (saved.run) applyRun(saved.run)
    return saved
  }

  return {
    overview,
    loading,
    error,
    runs,
    consoles,
    agents,
    applications,
    credentials,
    githubConnections,
    targets,
    recentRuns,
    awaitingApproval,
    fetchOverview,
    refreshIfLoaded,
    applyRun,
    applyLines,
    runById,
    openConsole,
    closeConsole,
    fetchAgents,
    saveCredential,
    removeCredential,
    setup,
    saveApplication,
    removeApplication,
    saveTarget,
    removeTarget,
    workflow,
    githubManifest,
    connectGithub,
    claimGithubInstallation,
    refreshGithub,
    removeGithub,
    githubRepositories,
    githubReferences,
    githubCompose,
    githubStatus,
    githubPullRequest,
    readinessFor,
    targetReadiness,
    history,
    changes,
    backups,
    deploy,
    approve,
    reject,
    cancel,
    restore,
    envCurrent,
    envVersions,
    envVersion,
    envPreview,
    envSave,
  }
})
