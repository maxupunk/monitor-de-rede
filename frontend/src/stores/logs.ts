import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { useInfiniteCursor } from '@/composables/useInfiniteCursor'
import { apiService } from '@/services/apiService'
import type { LogEntry } from '@/bindings/LogEntry'
import type { LogSourcesResponse } from '@/bindings/LogSourcesResponse'
import type { LogSourceEntry } from '@/bindings/LogSourceEntry'
import type { LogNatDiagnostics } from '@/bindings/LogNatDiagnostics'
import type { ProvisionHintsResponse } from '@/bindings/ProvisionHintsResponse'
import type { ProvisionLoggingResponse } from '@/bindings/ProvisionLoggingResponse'
import type { SetupGuide } from '@/bindings/SetupGuide'

export type {
  LogEntry,
  LogSourceEntry,
  LogNatDiagnostics,
  ProvisionHintsResponse,
  ProvisionLoggingResponse,
  SetupGuide,
}

export interface LogFilters {
  deviceId: number | null
  /**
   * Severidade numérica **máxima**. No syslog o número menor é o mais grave,
   * então `3` significa "erro e acima". `null` não filtra.
   */
  severity: number | null
  /** Janela em horas contadas para trás. `null` usa o padrão do backend (24 h). */
  hours: number | null
  search: string
}

/**
 * Opções do seletor de severidade, do mais grave para o menos.
 *
 * Os rótulos individuais vêm do backend em `severityLabel` — estes aqui
 * descrevem faixas ("erro e acima"), que é outra coisa e só existe na tela.
 */
export const SEVERITY_OPTIONS = [
  { value: 2, label: 'Crítico e acima' },
  { value: 3, label: 'Erro e acima' },
  { value: 4, label: 'Aviso e acima' },
  { value: 6, label: 'Informação e acima' },
  { value: 7, label: 'Tudo, inclusive depuração' },
] as const

/** Janelas oferecidas na tela. O backend recusa qualquer coisa além de 7 dias. */
export const WINDOW_OPTIONS = [
  { value: 1, label: 'Última hora' },
  { value: 6, label: 'Últimas 6 horas' },
  { value: 24, label: 'Últimas 24 horas' },
  { value: 24 * 7, label: 'Últimos 7 dias' },
] as const

export function defaultFilters(): LogFilters {
  return { deviceId: null, severity: null, hours: 24, search: '' }
}

export const useLogsStore = defineStore('logs', () => {
  const filters = ref<LogFilters>(defaultFilters())

  /**
   * O caminho é derivado dos filtros a cada chamada, e não guardado: é assim
   * que a mesma lista segue o filtro que o usuário acabou de mudar sem ser
   * recriada.
   */
  function endpoint(): string {
    const params = new URLSearchParams()
    if (filters.value.deviceId !== null) params.set('deviceId', String(filters.value.deviceId))
    if (filters.value.severity !== null) params.set('severity', String(filters.value.severity))
    if (filters.value.hours !== null) {
      const from = new Date(Date.now() - filters.value.hours * 3_600_000)
      params.set('from', from.toISOString())
    }
    const termo = filters.value.search.trim()
    if (termo) params.set('q', termo)
    const query = params.toString()
    return query ? `/logs?${query}` : '/logs'
  }

  const list = useInfiniteCursor<LogEntry>(endpoint, { label: 'os registros de log' })

  const isEmpty = computed(() => list.items.value.length === 0)

  /** Reinicia a lista. Chamado sempre que um filtro muda. */
  function applyFilters(next: Partial<LogFilters> = {}): void {
    filters.value = { ...filters.value, ...next }
    list.reset()
  }

  function clearFilters(): void {
    filters.value = defaultFilters()
    list.reset()
  }

  // --- live tail ------------------------------------------------------------

  const tailing = ref(false)
  let tailSource: EventSource | null = null

  /**
   * Liga o live tail.
   *
   * Stream **próprio**, não o `/api/events/stream` do painel: o barramento de
   * log é separado justamente para que um tail atrasado não derrube eventos de
   * domínio (ver `services/syslog/bus.rs` no backend).
   */
  function startTail(): void {
    if (tailSource) return
    const params = new URLSearchParams()
    if (filters.value.deviceId !== null) params.set('deviceId', String(filters.value.deviceId))
    if (filters.value.severity !== null) params.set('severity', String(filters.value.severity))
    const token = localStorage.getItem('auth_token')
    if (token) params.set('token', token)

    tailSource = new EventSource(`/api/logs/stream?${params.toString()}`)
    tailSource.onmessage = (msg) => {
      try {
        const bruto = JSON.parse(msg.data) as Record<string, unknown>
        // O primeiro quadro é só o handshake com o `retry`.
        if (bruto.type === 'stream:connected') return
        list.prepend([bruto as unknown as LogEntry], (entry) => entry.id)
      } catch {
        // Quadro ilegível não pode derrubar o tail.
      }
    }
    tailSource.onerror = () => {
      // O `EventSource` reconecta sozinho; a bandeira só reflete o estado.
      tailing.value = tailSource?.readyState !== EventSource.CLOSED
    }
    tailing.value = true
  }

  function stopTail(): void {
    tailSource?.close()
    tailSource = null
    tailing.value = false
  }

  function toggleTail(): void {
    if (tailing.value) stopTail()
    else startTail()
  }

  // --- fontes vistas --------------------------------------------------------

  const sources = ref<LogSourceEntry[]>([])
  const unknownCount = ref(0)
  const sourcesLoaded = ref(false)
  const nat = ref<LogNatDiagnostics | null>(null)

  async function fetchSources(): Promise<boolean> {
    try {
      const response = await apiService.get<LogSourcesResponse>('/logs/sources')
      sources.value = response.data ?? []
      unknownCount.value = response.unknownCount ?? 0
      nat.value = response.nat ?? null
      sourcesLoaded.value = true
      return true
    } catch {
      // A ingestão pode estar desligada; a tela de logs continua útil.
      sourcesLoaded.value = true
      return false
    }
  }

  /**
   * Se o mascaramento do Docker ainda está **atrapalhando**.
   *
   * Não é "existe mascaramento": é "existe origem mascarada sem vínculo". Os
   * roteadores chegam todos com o mesmo IP e só o hostname os separa — o aviso
   * existe para o operador não tentar resolver isso vinculando o endereço do
   * gateway, que atribuiria o parque inteiro a um dispositivo só.
   *
   * Vinculados todos, o mascaramento continua lá e deixou de custar alguma
   * coisa. Manter o aviso ali seria ensinar a ignorá-lo — e no dia em que um
   * equipamento novo aparecer sem vínculo, ele volta e volta significando algo.
   */
  const natMasking = computed(() => (nat.value?.unresolvedMaskedCount ?? 0) > 0)

  /**
   * `key` é o `bindKey` da origem, não o IP: atrás de NAT ele é `host:<nome>`.
   * Montar a chave na tela duplicaria a decisão de "isto está mascarado?", que
   * só o servidor sabe tomar.
   */
  /**
   * Vincula (ou desvincula) uma origem a um dispositivo.
   *
   * **Propaga o erro** em vez de devolver `false`: o servidor recusa o vínculo
   * do gateway do NAT com uma explicação inteira de por quê e do que fazer no
   * lugar, e engolir isso deixava o seletor voltando a vazio sem uma palavra —
   * exatamente o desfecho que a mensagem existe para evitar.
   *
   * A lista de logs **não** é recarregada: o vínculo vale para o que chegar
   * daqui em diante, e nenhuma linha já gravada muda. Recarregá-la só piscava a
   * tela atrás do diálogo.
   */
  async function bindSource(key: string, deviceId: number | null): Promise<void> {
    await apiService.post(`/logs/sources/${encodeURIComponent(key)}/bind`, { deviceId })
    await fetchSources()
  }

  // --- ativação automática --------------------------------------------------

  /**
   * O que o servidor consegue descobrir sozinho sobre o equipamento.
   *
   * Sonda as portas de acesso e consulta o SNMP, então demora alguns segundos
   * no pior caso — a tela abre antes e se preenche quando chega.
   */
  async function fetchProvisionHints(deviceId: number): Promise<ProvisionHintsResponse | null> {
    try {
      return await apiService.get<ProvisionHintsResponse>(
        `/logs/devices/${deviceId}/provision-hints`
      )
    } catch {
      // Palpite é conveniência: sem ele a tela ainda funciona, só exige que o
      // operador preencha à mão.
      return null
    }
  }

  /**
   * Manda o servidor entrar no equipamento e configurar o envio de syslog.
   *
   * A credencial vai no corpo e não é guardada em lugar nenhum — nem aqui, nem
   * no backend. O chamador é responsável por descartá-la depois.
   */
  async function provisionDevice(
    deviceId: number,
    entrada: {
      protocol: string
      port: number | null
      username: string
      password: string
      /** Id do catálogo de sistemas — ver `stores/operatingSystems`. */
      operatingSystem: string | null
      /**
       * Endereço deste servidor que o **equipamento** deve usar. Vem do campo
       * da tela, não do navegador: quem abre a interface em `localhost` mandava
       * `localhost` para dentro do roteador, e o aparelho passava a enviar o
       * log para si mesmo — sem erro e sem nada chegando aqui.
       */
      serverAddress: string | null
      macAddress: string | null
    }
  ): Promise<ProvisionLoggingResponse> {
    const resposta = await apiService.post<ProvisionLoggingResponse>(
      `/logs/devices/${deviceId}/provision`,
      entrada
    )
    // O que acabou de ser configurado muda a lista de origens e pode ter
    // vinculado o dispositivo — recarregar aqui evita a tela mostrar o estado
    // anterior logo depois de dizer "pronto".
    await fetchSources()
    list.reset()
    return resposta
  }

  // --- guia de configuração -------------------------------------------------

  const setupGuide = ref<SetupGuide | null>(null)

  /**
   * Gera os comandos com um endereço estampado.
   *
   * O endereço vem da lista "Endereços deste servidor", não do navegador. A
   * versão anterior mandava `window.location.hostname`, e quem abre a interface
   * em `http://localhost:3333` copiava um comando com `remote=localhost` — que
   * o roteador aceita e que o faz mandar o log para si mesmo.
   */
  async function fetchSetupGuide(address: string | null = null): Promise<void> {
    try {
      const query = address ? `?address=${encodeURIComponent(address)}` : ''
      setupGuide.value = await apiService.get<SetupGuide>(`/logs/setup-snippet${query}`)
    } catch {
      setupGuide.value = null
    }
  }

  return {
    filters,
    entries: list.items,
    scrollKey: list.scrollKey,
    window: list.window,
    error: list.error,
    isEmpty,
    load: list.load,
    reset: list.reset,
    prepend: list.prepend,
    applyFilters,
    clearFilters,
    tailing,
    startTail,
    stopTail,
    toggleTail,
    sources,
    unknownCount,
    sourcesLoaded,
    nat,
    natMasking,
    fetchSources,
    bindSource,
    fetchProvisionHints,
    provisionDevice,
    setupGuide,
    fetchSetupGuide,
  }
})

/**
 * Cor da severidade do syslog no tema do Vuetify.
 *
 * Mora aqui, e não no componente, porque a aba de logs do dispositivo (Fase 4)
 * mostra a mesma tabela: duas tabelas de cor divergiriam na primeira alteração.
 */
export function severityColor(severity: number | null): string {
  if (severity === null) return 'grey'
  if (severity <= 2) return 'error'
  if (severity === 3) return 'error'
  if (severity === 4) return 'warning'
  if (severity <= 6) return 'info'
  return 'grey'
}
