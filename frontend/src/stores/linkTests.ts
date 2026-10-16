import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { ApiError, apiService } from '@/services/apiService'
import type {
  LinkTest,
  LinkTestFlow,
  LinkTestKind,
  LinkTestPage,
  LinkTestPlan,
  LinkTestRequest,
  LinkTestSample,
  LinkTestSampleEvent,
  LinkTestSchedule,
  LinkTestScheduleInput,
  LinkTestStatus,
} from '@/types/linkTests'
import { isActiveStatus } from '@/utils/linkTests/presentation'

/**
 * Teste de enlace (ADR 015 §5): toda a conversa HTTP da funcionalidade mora
 * aqui (plano, início, cancelamento, histórico, detalhe e agendamentos), e o
 * estado ao vivo chega só pelo SSE — `link_test:updated` (snapshot completo,
 * inclusive o inicial dos testes ativos), `link_test:sample` (amostra
 * efêmera) e `link_test_schedule:updated`. Nada aqui consulta a API sozinho.
 */

/** Amostras guardadas por perna e sentido (2 min a uma por segundo). */
export const SAMPLE_LIMIT = 120
/** De quantos testes as amostras ficam em memória. */
const SAMPLE_TESTS_LIMIT = 12
/** Testes mantidos no cache (os ativos nunca saem). */
const CACHE_LIMIT = 200
/** Itens por página do histórico. */
export const HISTORY_PER_PAGE = 20
/** Testes recentes mostrados na aba da topologia. */
const RECENT_PER_TARGET = 5

export interface LinkTestFilters {
  page?: number
  perPage?: number
  deviceId?: number | null
  linkId?: number | null
  status?: LinkTestStatus | null
  kind?: LinkTestKind | null
  /** ISO — testes criados a partir de. */
  since?: string | null
}

export interface LinkTestStarted {
  id: number
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

/** Validação mínima do snapshot vindo do SSE (o resto é o contrato). */
export function isLinkTest(value: unknown): value is LinkTest {
  return (
    isRecord(value) &&
    typeof value.id === 'number' &&
    typeof value.kind === 'string' &&
    typeof value.status === 'string' &&
    Array.isArray(value.legs)
  )
}

function isFlow(value: unknown): value is LinkTestFlow {
  return value === 'forward' || value === 'reverse'
}

export function isSampleEvent(value: unknown): value is LinkTestSampleEvent {
  return (
    isRecord(value) &&
    typeof value.testId === 'number' &&
    typeof value.legSeq === 'number' &&
    isFlow(value.flow) &&
    typeof value.bps === 'number' &&
    Number.isFinite(value.bps) &&
    typeof value.tMs === 'number'
  )
}

export function sampleKey(testId: number, legSeq: number, flow: LinkTestFlow): string {
  return `${testId}:${legSeq}:${flow}`
}

/** Teste envolve o dispositivo (pedido ou ponta de alguma perna)? */
export function involvesDevice(test: LinkTest, deviceId: number): boolean {
  return (
    test.sourceDeviceId === deviceId ||
    test.targetDeviceId === deviceId ||
    test.legs.some((leg) => leg.a.deviceId === deviceId || leg.b.deviceId === deviceId)
  )
}

/** Teste mede o enlace (pedido ou aresta de alguma perna)? */
export function involvesEdge(test: LinkTest, edgeId: number): boolean {
  return test.linkId === edgeId || test.legs.some((leg) => leg.edgeId === edgeId)
}

/** O teste entra na página filtrada? (Para inserir o que chega pelo SSE.) */
export function matchesFilters(test: LinkTest, filters: LinkTestFilters): boolean {
  if (filters.status && test.status !== filters.status) return false
  if (filters.kind && test.kind !== filters.kind) return false
  if (filters.deviceId && !involvesDevice(test, filters.deviceId)) return false
  if (filters.linkId && !involvesEdge(test, filters.linkId)) return false
  if (filters.since && Date.parse(test.createdAt) < Date.parse(filters.since)) return false
  return true
}

function byNewest(a: LinkTest, b: LinkTest): number {
  return Date.parse(b.createdAt) - Date.parse(a.createdAt) || b.id - a.id
}

function describe(err: unknown, fallback: string): string {
  return err instanceof Error && err.message ? err.message : fallback
}

function queryOf(filters: LinkTestFilters): string {
  const params = new URLSearchParams()
  params.set('page', String(filters.page ?? 1))
  params.set('perPage', String(filters.perPage ?? HISTORY_PER_PAGE))
  if (filters.deviceId) params.set('deviceId', String(filters.deviceId))
  if (filters.linkId) params.set('linkId', String(filters.linkId))
  if (filters.status) params.set('status', filters.status)
  if (filters.kind) params.set('kind', filters.kind)
  if (filters.since) params.set('since', filters.since)
  return params.toString()
}

/** O snapshot do SSE não traz amostras; o detalhe traz. Não perde as que já temos. */
function withoutSamples(test: LinkTest): LinkTest {
  if (!test.legs.some((leg) => leg.samples !== undefined)) return test
  return {
    ...test,
    legs: test.legs.map((leg) => {
      const copy = { ...leg }
      delete copy.samples
      return copy
    }),
  }
}

export const useLinkTestsStore = defineStore('linkTests', () => {
  /** Todos os testes conhecidos (SSE, histórico, detalhe), por id. */
  const tests = ref<Record<number, LinkTest>>({})
  /** Amostras ao vivo por `testId:legSeq:flow` (anel de `SAMPLE_LIMIT`). */
  const samples = ref<Record<string, LinkTestSample[]>>({})
  /** Momento (ms) da última notícia de cada teste — o "agora" do painel. */
  const lastActivity = ref<Record<number, number>>({})

  const page = ref<LinkTestPage | null>(null)
  const pageFilters = ref<LinkTestFilters>({})
  const pageLoading = ref(false)
  const pageError = ref<string | null>(null)

  const schedules = ref<LinkTestSchedule[]>([])
  const schedulesLoaded = ref(false)
  const schedulesLoading = ref(false)
  const schedulesError = ref<string | null>(null)

  /**
   * Teste acompanhado na tela (painel aberto, sobreposição no mapa). Estado
   * de apresentação compartilhado entre a aba da topologia, o cartão
   * flutuante e o histórico — quem fecha o painel limpa.
   */
  const focusedId = ref<number | null>(null)

  const all = computed(() => Object.values(tests.value).sort(byNewest))
  const active = computed(() => all.value.filter((test) => isActiveStatus(test.status)))
  const focused = computed(() =>
    focusedId.value !== null ? (tests.value[focusedId.value] ?? null) : null
  )

  function byId(id: number | null | undefined): LinkTest | null {
    return id === null || id === undefined ? null : (tests.value[id] ?? null)
  }

  function testsForEdge(edgeId: number): LinkTest[] {
    return all.value.filter((test) => involvesEdge(test, edgeId))
  }

  function testsForDevice(deviceId: number): LinkTest[] {
    return all.value.filter((test) => involvesDevice(test, deviceId))
  }

  function latestForEdge(edgeId: number): LinkTest | null {
    return testsForEdge(edgeId)[0] ?? null
  }

  function latestForDevice(deviceId: number): LinkTest | null {
    return testsForDevice(deviceId)[0] ?? null
  }

  function samplesFor(testId: number, legSeq: number, flow: LinkTestFlow): LinkTestSample[] {
    return samples.value[sampleKey(testId, legSeq, flow)] ?? []
  }

  /** Último valor ao vivo do sentido, ou `null`. */
  function liveBps(testId: number, legSeq: number, flow: LinkTestFlow): number | null {
    const list = samplesFor(testId, legSeq, flow)
    return list.length > 0 ? list[list.length - 1].bps : null
  }

  function lastActivityAt(testId: number): number | null {
    return lastActivity.value[testId] ?? null
  }

  // --- Cache ------------------------------------------------------------------

  function touch(id: number) {
    lastActivity.value = { ...lastActivity.value, [id]: Date.now() }
  }

  function prune(next: Record<number, LinkTest>) {
    const ids = Object.values(next)
      .sort(byNewest)
      .filter((test) => !isActiveStatus(test.status) && test.id !== focusedId.value)
      .map((test) => test.id)
    const excess = Object.keys(next).length - CACHE_LIMIT
    if (excess <= 0) return
    for (const id of ids.slice(-excess)) delete next[id]
  }

  /** Uma leitura atrasada (ou notícia fora de ordem) não desfaz o fim de um teste. */
  function shouldReplace(previous: LinkTest | undefined, incoming: LinkTest): boolean {
    return !(previous && !isActiveStatus(previous.status) && isActiveStatus(incoming.status))
  }

  function upsertMany(list: LinkTest[]) {
    const next = { ...tests.value }
    for (const test of list) {
      if (shouldReplace(next[test.id], test)) next[test.id] = withoutSamples(test)
    }
    prune(next)
    tests.value = next
  }

  function upsert(test: LinkTest) {
    upsertMany([test])
  }

  /** Semeia as amostras com as do detalhe (o GET traz as gravadas). */
  function seedSamples(test: LinkTest) {
    const next = { ...samples.value }
    for (const leg of test.legs) {
      if (!leg.samples) continue
      for (const flow of ['forward', 'reverse'] as const) {
        const list = leg.samples
          .filter((sample) => sample.flow === flow)
          .sort((a, b) => a.tMs - b.tMs)
        if (list.length > 0) next[sampleKey(test.id, leg.seq, flow)] = list.slice(-SAMPLE_LIMIT)
      }
    }
    samples.value = limitSampleTests(next)
  }

  function limitSampleTests(next: Record<string, LinkTestSample[]>) {
    const testIds = [...new Set(Object.keys(next).map((key) => Number(key.split(':')[0])))].sort(
      (a, b) => a - b
    )
    const keep = new Set(testIds.slice(-SAMPLE_TESTS_LIMIT))
    if (focusedId.value !== null) keep.add(focusedId.value)
    for (const key of Object.keys(next)) {
      if (!keep.has(Number(key.split(':')[0]))) delete next[key]
    }
    return next
  }

  /** A página do histórico acompanha o SSE: troca o item ou insere o novo. */
  function patchPage(test: LinkTest) {
    const current = page.value
    if (!current) return
    const index = current.items.findIndex((item) => item.id === test.id)
    if (index >= 0) {
      const items = [...current.items]
      items.splice(index, 1, withoutSamples(test))
      page.value = { ...current, items }
      return
    }
    if (current.page !== 1 || !matchesFilters(test, pageFilters.value)) return
    page.value = {
      ...current,
      items: [withoutSamples(test), ...current.items].slice(0, current.perPage),
      total: current.total + 1,
    }
  }

  // --- HTTP (ações explícitas do usuário) ----------------------------------------

  /** `POST /api/link-tests/plan` — o que vai acontecer, sem rodar nada. */
  function plan(request: LinkTestRequest): Promise<LinkTestPlan> {
    const body = { ...request }
    delete body.confirm
    return apiService.post<LinkTestPlan>('/link-tests/plan', body, { timeoutMs: 30_000 })
  }

  /** `POST /api/link-tests` com `confirm: true` — devolve o id do teste. */
  async function start(request: LinkTestRequest): Promise<number> {
    const started = await apiService.post<LinkTestStarted | LinkTest>(
      '/link-tests',
      { ...request, confirm: true },
      { timeoutMs: 30_000 }
    )
    if (isLinkTest(started)) {
      upsert(started)
      touch(started.id)
    }
    return started.id
  }

  async function cancel(id: number): Promise<void> {
    try {
      await apiService.post(`/link-tests/${id}/cancel`, {})
    } catch (err: unknown) {
      // 404/409: o teste já tinha terminado — o SSE traz o estado final.
      if (!(err instanceof ApiError && (err.status === 404 || err.status === 409))) throw err
    }
  }

  async function fetchPage(filters: LinkTestFilters = {}): Promise<void> {
    pageLoading.value = true
    pageError.value = null
    pageFilters.value = { ...filters }
    try {
      const result = await apiService.get<LinkTestPage>(`/link-tests?${queryOf(filters)}`)
      page.value = result
      upsertMany(result.items)
    } catch (err: unknown) {
      pageError.value = describe(err, 'Não foi possível carregar o histórico de testes.')
    } finally {
      pageLoading.value = false
    }
  }

  /** Testes recentes de um enlace ou dispositivo (aba "Testes" da topologia). */
  async function fetchRecent(target: { linkId?: number; deviceId?: number }): Promise<void> {
    const result = await apiService.get<LinkTestPage>(
      `/link-tests?${queryOf({ ...target, page: 1, perPage: RECENT_PER_TARGET })}`
    )
    upsertMany(result.items)
  }

  /** `GET /api/link-tests/{id}` — o teste com as amostras gravadas. */
  async function fetchDetail(id: number): Promise<LinkTest> {
    const test = await apiService.get<LinkTest>(`/link-tests/${id}`)
    seedSamples(test)
    upsert(test)
    return test
  }

  // --- Agendamentos -----------------------------------------------------------

  async function fetchSchedules(): Promise<void> {
    schedulesLoading.value = true
    schedulesError.value = null
    try {
      schedules.value = await apiService.get<LinkTestSchedule[]>('/link-test-schedules')
      schedulesLoaded.value = true
    } catch (err: unknown) {
      schedulesError.value = describe(err, 'Não foi possível carregar os agendamentos.')
    } finally {
      schedulesLoading.value = false
    }
  }

  function replaceSchedule(saved: LinkTestSchedule) {
    const index = schedules.value.findIndex((item) => item.id === saved.id)
    if (index >= 0) schedules.value.splice(index, 1, saved)
    else schedules.value = [...schedules.value, saved]
  }

  async function createSchedule(input: LinkTestScheduleInput): Promise<LinkTestSchedule> {
    const saved = await apiService.post<LinkTestSchedule>('/link-test-schedules', input)
    replaceSchedule(saved)
    return saved
  }

  async function updateSchedule(
    id: number,
    input: LinkTestScheduleInput
  ): Promise<LinkTestSchedule> {
    const saved = await apiService.put<LinkTestSchedule>(`/link-test-schedules/${id}`, input)
    replaceSchedule(saved)
    return saved
  }

  async function deleteSchedule(id: number): Promise<void> {
    await apiService.delete(`/link-test-schedules/${id}`)
    schedules.value = schedules.value.filter((item) => item.id !== id)
  }

  /**
   * "Rodar agora": `POST /api/link-test-schedules/{id}/run` (o teste nasce
   * com `origin: schedule`). Sem a rota no backend (404/405), roda o mesmo
   * alvo e parâmetros como um teste avulso.
   */
  async function runScheduleNow(id: number): Promise<number> {
    try {
      const started = await apiService.post<LinkTestStarted>(`/link-test-schedules/${id}/run`, {})
      return started.id
    } catch (err: unknown) {
      const schedule = schedules.value.find((item) => item.id === id)
      if (!(err instanceof ApiError && (err.status === 404 || err.status === 405)) || !schedule) {
        throw err
      }
      return start({
        kind: schedule.kind,
        ...(schedule.linkId !== null ? { linkId: schedule.linkId } : {}),
        ...(schedule.sourceDeviceId !== null ? { sourceDeviceId: schedule.sourceDeviceId } : {}),
        ...(schedule.targetDeviceId !== null ? { targetDeviceId: schedule.targetDeviceId } : {}),
        spec: schedule.spec,
      })
    }
  }

  // --- Foco (painel aberto / mapa) ----------------------------------------------

  function focus(id: number | null) {
    focusedId.value = id
  }

  // --- Tempo real (SSE) ---------------------------------------------------------

  /** `link_test:updated` — snapshot completo do teste, aplicado em memória. */
  function applyUpdated(data: unknown) {
    if (!isLinkTest(data)) return
    upsert(data)
    touch(data.id)
    patchPage(data)
  }

  /** `link_test:sample` — amostra efêmera, guardada num anel por perna e sentido. */
  function applySample(data: unknown) {
    if (!isSampleEvent(data)) return
    const key = sampleKey(data.testId, data.legSeq, data.flow)
    const list = samples.value[key] ?? []
    const last = list[list.length - 1]
    // Repetida (reconexão) ou fora de ordem: ignora.
    if (last && data.tMs <= last.tMs) return
    const next = {
      ...samples.value,
      [key]: [...list, { flow: data.flow, bps: data.bps, tMs: data.tMs }].slice(-SAMPLE_LIMIT),
    }
    samples.value = limitSampleTests(next)
    touch(data.testId)
  }

  /** `link_test_schedule:updated` — só relê se a lista estiver aberta. */
  function applyScheduleUpdated(_data: unknown) {
    if (!schedulesLoaded.value) return
    void fetchSchedules()
  }

  return {
    tests,
    samples,
    page,
    pageFilters,
    pageLoading,
    pageError,
    schedules,
    schedulesLoaded,
    schedulesLoading,
    schedulesError,
    focusedId,
    focused,
    all,
    active,
    byId,
    testsForEdge,
    testsForDevice,
    latestForEdge,
    latestForDevice,
    samplesFor,
    liveBps,
    lastActivityAt,
    plan,
    start,
    cancel,
    fetchPage,
    fetchRecent,
    fetchDetail,
    fetchSchedules,
    createSchedule,
    updateSchedule,
    deleteSchedule,
    runScheduleNow,
    focus,
    applyUpdated,
    applySample,
    applyScheduleUpdated,
  }
})
