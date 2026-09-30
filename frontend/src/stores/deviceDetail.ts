import { defineStore } from 'pinia'
import { ref } from 'vue'
import { apiService } from '@/services/apiService'
import type { DeviceCapabilities } from '@/bindings/DeviceCapabilities'
import type { InterfaceCandidate } from '@/bindings/InterfaceCandidate'
import type { InterfaceSuggestions } from '@/bindings/InterfaceSuggestions'
import type { InterfaceSuggestionsInput } from '@/bindings/InterfaceSuggestionsInput'
import type { Device } from './devices'
import type { Monitor } from './monitors'

/**
 * Toda rota `/snmp/*` responde só depois de conversar com o equipamento: são
 * walks de `ifTable`/`ifXTable` sobre UDP, com 4 s de timeout por requisição e
 * retry. `apply-monitors` faz **dois** — um para decidir e outro no poll final
 * — mais uma escrita por interface.
 *
 * O padrão de 15 s do `apiService` é dimensionado para API que só lê banco.
 * Aplicado aqui, o `AbortController` cancelava a requisição no navegador com o
 * backend ainda escrevendo: a tela ficava carregando "sem acontecer nada" e o
 * operador não tinha como saber que a gravação seguiu do outro lado.
 */
export const SNMP_REQUEST_TIMEOUT_MS = 120_000

export interface DeviceInterface {
  id: number
  deviceId: number
  snmpIndex?: number
  ifIndex?: number
  name?: string
  ifName?: string
  ifType?: string
  description?: string | null
  alias?: string | null
  adminStatus?: string
  ifAdminStatus?: 'up' | 'down' | 'testing'
  operStatus?: string
  ifOperStatus?: 'up' | 'down' | 'testing'
  speed?: number
  ifSpeed?: number
  macAddress?: string
  ipAddress?: string
  inOctets?: number
  outOctets?: number
  inBps?: number
  outBps?: number
  /**
   * Há monitor habilitado coletando esta interface? Vem do backend
   * (`services::snmp::service::list_interfaces`) e é o que decide se a
   * interface gera métricas — não confundir com `adminStatus`, que o próprio
   * equipamento também preenche na primeira coleta.
   */
  isMonitored?: boolean
}

export interface DeviceMetric {
  id: number
  deviceId: number
  interfaceId?: number | null
  interfaceName?: string | null
  metricName: string
  metricValue: number
  unit?: string
  createdAt: string
}

/**
 * `GET /api/devices/:id/monitors` devolve o mesmo payload de `GET /api/monitors`
 * (ver `modules/monitoring/monitor_presenter.ts`), porque a aba "Monitores" do
 * equipamento usa o mesmo componente de listagem de `/monitors`.
 */
export type DeviceMonitor = Monitor

export interface DeviceEvent {
  id: number
  deviceId: number
  eventType: string
  severity: 'info' | 'warning' | 'error' | 'critical'
  message: string
  createdAt: string
}

export interface ScanInterfaceItem {
  ifIndex: number
  ifName: string
  ifDescr?: string | null
  ifAlias?: string | null
  macAddress?: string | null
  ifType?: number | null
  ifSpeed?: number | null
  ifAdminStatus?: string | null
  ifOperStatus?: string | null
  isMonitored: boolean
}

export interface ScanTrafficItem {
  ifIndex: number
  inOctets: number
  outOctets: number
}

/**
 * As interfaces do escaneamento no formato que o Laya lê: nome, alias,
 * descrição, tipo, velocidade, estado e o tráfego acumulado de cada uma.
 */
export function scanInterfaceCandidates(scan: ScanResult): InterfaceCandidate[] {
  const traffic = new Map((scan.traffic ?? []).map((item) => [item.ifIndex, item]))
  return scan.interfaces.map((iface) => ({
    ifIndex: iface.ifIndex,
    name: iface.ifName,
    alias: iface.ifAlias ?? null,
    descr: iface.ifDescr ?? null,
    ifType: iface.ifType ?? null,
    speed: iface.ifSpeed ?? null,
    operUp: iface.ifOperStatus == null ? null : iface.ifOperStatus === 'up',
    inOctets: traffic.get(iface.ifIndex)?.inOctets ?? null,
    outOctets: traffic.get(iface.ifIndex)?.outOctets ?? null,
  }))
}

/** Uma interface oferecida no seletor "Interface de entrada de link". */
export interface LinkInterfaceOption {
  id?: number
  ifIndex?: number
  name: string
  alias?: string | null
  description?: string | null
  speed?: number | null
  operStatus?: string | null
}

/** As opções do seletor no formato que o Laya lê. */
export function linkInterfaceCandidates(options: LinkInterfaceOption[]): InterfaceCandidate[] {
  return options.map((option, position) => ({
    // Sem ifIndex (cadastro antigo), a posição na lista serve de chave.
    ifIndex: option.ifIndex ?? -(position + 1),
    name: option.name,
    alias: option.alias ?? null,
    descr: option.description ?? null,
    ifType: null,
    speed: option.speed ?? null,
    operUp: option.operStatus == null ? null : option.operStatus.toLowerCase() === 'up',
    inOctets: null,
    outOctets: null,
  }))
}

function liveStatus(value: unknown): string | undefined {
  if (typeof value === 'number') return value === 1 ? 'up' : 'down'
  return typeof value === 'string' ? value : undefined
}

export interface SensorStateSpec {
  label: string
  color?: string
  icon?: string
}

export interface DiscoveredSensorItem {
  key: string
  label: string
  name?: string
  oid: string
  unit: string
  scale: number
  category: string
  dataType?: 'float' | 'integer' | 'boolean' | 'state'
  rawValue?: number | null
  value?: number | null
  formattedValue: string
  isMonitored: boolean
  icon?: string
  color?: string
  states?: Record<string, string | SensorStateSpec>
}

export interface MatchedProfileSummary {
  id: string
  name: string
  vendor: string
  category: string
  isBuiltin: boolean
}

/**
 * `POST /api/devices/:id/snmp/scan` (`services::snmp::service::SnmpScanResult`).
 *
 * Os campos opcionais chegam como `null` — e não ausentes — quando o
 * equipamento não expõe a MIB correspondente, porque o backend serializa
 * `Option` como `null`. Por isso as verificações usam `!= null` em vez de
 * comparar com `undefined`.
 */
export interface ScanResult {
  systemInfo: {
    sysName?: string | null
    sysDescr?: string | null
    sysObjectId?: string | null
    sysUpTime?: number | null
  }
  cpuInfo: {
    usagePercent?: number | null
    coresCount?: number | null
    load1min?: number | null
  }
  memoryInfo: {
    totalKb?: number | null
    usedKb?: number | null
    usedPercent?: number | null
  }
  interfaces: ScanInterfaceItem[]
  /** Contadores lidos junto com as interfaces (o backend sempre mandou). */
  traffic?: ScanTrafficItem[]
  collectorErrors: Record<string, string>
  hasCpuMonitor: boolean
  hasMemoryMonitor: boolean
  snmpResponded: boolean
  matchedProfile?: MatchedProfileSummary | null
  sensors?: DiscoveredSensorItem[]
}

/** Teto de amostras mantidas em memória na tela de detalhe */
const METRICS_LIMIT = 2000

/** Teto do histórico por monitor mantido em memória — acompanha o do backend */
const MONITOR_HISTORY_LIMIT = 100

export const useDeviceDetailStore = defineStore('deviceDetail', () => {
  const device = ref<Device | null>(null)
  const interfaces = ref<DeviceInterface[]>([])
  const metrics = ref<DeviceMetric[]>([])
  const monitors = ref<DeviceMonitor[]>([])
  const events = ref<DeviceEvent[]>([])
  const loading = ref(false)
  const pollingSnmp = ref(false)
  const scanningSnmp = ref(false)
  const updatingInterfaceId = ref<number | null>(null)
  const scanResult = ref<ScanResult | null>(null)
  const error = ref<string | null>(null)
  /**
   * O que este dispositivo de fato oferece, respondido pelo backend.
   *
   * A tela **não deduz** suporte a partir do nome, do ID ou de
   * `snmpEnabled`: quem sabe se houve comunicação SNMP bem-sucedida, se há
   * interfaces inventariadas ou se o equipamento é o próprio servidor é quem
   * tem os dados. `null` enquanto não carregou — e nesse estado nada é
   * afirmado, porque uma aba que aparece e some é pior que uma que demora.
   */
  const capabilities = ref<DeviceCapabilities | null>(null)

  /**
   * O backend serializa `enabled`; a UI (e o `MonitorsTable`) lê `isEnabled`.
   * Mesma normalização feita no store de monitores — sem ela o switch de
   * ativação abriria desligado em monitores ativos.
   */
  function normalizeMonitors(payload: unknown): DeviceMonitor[] {
    if (!Array.isArray(payload)) return []
    return payload.map((mon: any) => {
      const isEnabled = mon.isEnabled ?? mon.enabled ?? true
      return { ...mon, enabled: isEnabled, isEnabled }
    })
  }

  /** Recarrega apenas os monitores, após uma ação disparada pela listagem. */
  async function reloadMonitors(deviceId: number): Promise<void> {
    try {
      const data = await apiService.get<DeviceMonitor[]>(`/devices/${deviceId}/monitors`)
      monitors.value = normalizeMonitors(data)
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao recarregar os monitores'
    }
  }

  async function loadDeviceDetails(deviceId: number) {
    loading.value = true
    error.value = null
    // Nunca preserve telemetria do equipamento anterior enquanto a nova
    // seleção carrega ou quando apenas o endpoint de métricas falha.
    metrics.value = []
    try {
      const [devData, intfData, monData, metData, evtData, capData] = await Promise.allSettled([
        apiService.get<Device>(`/devices/${deviceId}`),
        apiService.get<DeviceInterface[]>(`/devices/${deviceId}/interfaces`),
        apiService.get<DeviceMonitor[]>(`/devices/${deviceId}/monitors`),
        apiService.get<DeviceMetric[]>(`/devices/${deviceId}/metrics`),
        apiService.get<DeviceEvent[]>(`/devices/${deviceId}/events`),
        apiService.get<DeviceCapabilities>(`/devices/${deviceId}/capabilities`),
      ])

      if (devData.status === 'fulfilled') device.value = devData.value
      if (intfData.status === 'fulfilled') {
        interfaces.value = Array.isArray(intfData.value) ? intfData.value : []
      }
      if (monData.status === 'fulfilled') {
        monitors.value = normalizeMonitors(monData.value)
      }
      if (metData.status === 'fulfilled') {
        metrics.value = Array.isArray(metData.value) ? metData.value : []
      }
      if (evtData.status === 'fulfilled') {
        events.value = Array.isArray(evtData.value) ? evtData.value : []
      }
      if (capData.status === 'fulfilled') capabilities.value = capData.value
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao carregar detalhes do dispositivo'
    } finally {
      loading.value = false
    }
  }

  async function triggerSnmpPoll(deviceId: number): Promise<boolean> {
    pollingSnmp.value = true
    try {
      await apiService.post(`/devices/${deviceId}/snmp/poll`, undefined, {
        timeoutMs: SNMP_REQUEST_TIMEOUT_MS,
      })
      await loadDeviceDetails(deviceId)
      return true
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao executar poll SNMP'
      return false
    } finally {
      pollingSnmp.value = false
    }
  }

  /** Palpite do Laya sobre as interfaces do último escaneamento. */
  const interfaceSuggestions = ref<InterfaceSuggestions | null>(null)
  const suggestingInterfaces = ref(false)

  /**
   * Pede ao Laya o uplink e as interfaces que valem monitorar, sem guardar —
   * para quem tem a própria lista (o formulário do dispositivo). Sem Laya, ou
   * se falhar, `null`: sugestão nunca atrapalha a tela.
   */
  async function fetchInterfaceSuggestions(
    input: InterfaceSuggestionsInput
  ): Promise<InterfaceSuggestions | null> {
    if (input.interfaces.length === 0) return null
    try {
      const res = await apiService.post<InterfaceSuggestions>('/snmp/interfaces/suggestions', input)
      return res.available ? res : null
    } catch {
      return null
    }
  }

  /** O mesmo, guardado para o modal de escaneamento desta tela. */
  async function suggestInterfaces(
    input: InterfaceSuggestionsInput
  ): Promise<InterfaceSuggestions | null> {
    interfaceSuggestions.value = null
    suggestingInterfaces.value = true
    try {
      interfaceSuggestions.value = await fetchInterfaceSuggestions(input)
      return interfaceSuggestions.value
    } finally {
      suggestingInterfaces.value = false
    }
  }

  /**
   * Interfaces para o seletor de link do formulário: as do cadastro (depois
   * de uma coleta ao vivo, se pedido) ou, sem cadastro, uma consulta SNMP
   * direta ao IP. Falhas viram lista vazia — o seletor aceita digitação.
   */
  async function loadLinkInterfaces(params: {
    deviceId?: number
    forceLive?: boolean
    host: string
    version: string
    community: string
  }): Promise<LinkInterfaceOption[]> {
    if (params.deviceId) {
      if (params.forceLive) {
        await apiService.post(`/devices/${params.deviceId}/snmp/poll`, {}).catch(() => undefined)
      }
      try {
        const rows = await apiService.get<DeviceInterface[]>(
          `/devices/${params.deviceId}/interfaces`
        )
        if (Array.isArray(rows) && rows.length > 0) {
          return rows.map((row) => ({
            id: row.id,
            ifIndex: row.ifIndex,
            name: row.name || row.ifName || '',
            alias: row.alias,
            description: row.description,
            speed: row.speed || row.ifSpeed,
            operStatus: row.operStatus || row.ifOperStatus,
          }))
        }
      } catch {
        // sem cadastro legível: tenta ao vivo abaixo
      }
    }
    if (!params.host) return []
    try {
      const live = await apiService.post<Array<Record<string, unknown>>>('/snmp/interfaces-query', {
        host: params.host,
        port: 161,
        version: params.version,
        community: params.community,
      })
      if (!Array.isArray(live)) return []
      return live.map((row) => ({
        id: typeof row.id === 'number' ? row.id : undefined,
        ifIndex: Number(row.ifIndex ?? row.if_index) || undefined,
        name: String(row.ifName ?? row.if_name ?? row.name ?? ''),
        alias: (row.ifAlias ?? row.if_alias ?? row.alias) as string | null,
        description: (row.ifDescr ?? row.if_descr ?? row.description) as string | null,
        speed: Number(row.ifSpeed ?? row.if_speed ?? row.speed) || null,
        operStatus: liveStatus(row.ifOperStatus ?? row.if_oper_status),
      }))
    } catch {
      return []
    }
  }

  async function scanDeviceSnmp(deviceId: number): Promise<ScanResult | null> {
    scanningSnmp.value = true
    error.value = null
    try {
      const res = await apiService.post<ScanResult>(`/devices/${deviceId}/snmp/scan`, undefined, {
        timeoutMs: SNMP_REQUEST_TIMEOUT_MS,
      })
      scanResult.value = res
      return res
    } catch (err: unknown) {
      error.value = err instanceof Error ? err.message : 'Erro ao escanear dispositivo via SNMP'
      return null
    } finally {
      scanningSnmp.value = false
    }
  }

  async function applySnmpMonitors(
    deviceId: number,
    options: {
      enableCpuMonitor?: boolean
      enableMemoryMonitor?: boolean
      monitoredIfIndexes?: number[]
      monitoredSensors?: string[]
      clearRemovedHistory?: boolean
    }
  ): Promise<boolean> {
    loading.value = true
    error.value = null
    try {
      await apiService.post(`/devices/${deviceId}/snmp/apply-monitors`, options, {
        timeoutMs: SNMP_REQUEST_TIMEOUT_MS,
      })
      await loadDeviceDetails(deviceId)
      return true
    } catch (err: unknown) {
      error.value =
        err instanceof Error ? err.message : 'Erro ao aplicar configurações de monitoramento'
      return false
    } finally {
      loading.value = false
    }
  }

  /**
   * Inclui/remove uma única interface do monitoramento, sem passar pela tela de
   * descoberta. Ao habilitar, o backend já executa uma coleta — por isso a
   * chamada demora e tem indicador próprio.
   */
  async function setInterfaceMonitoring(
    deviceId: number,
    interfaceId: number,
    enabled: boolean
  ): Promise<boolean> {
    updatingInterfaceId.value = interfaceId
    error.value = null
    try {
      await apiService.patch(
        `/devices/${deviceId}/interfaces/${interfaceId}/monitoring`,
        { enabled },
        { timeoutMs: SNMP_REQUEST_TIMEOUT_MS }
      )
      await loadDeviceDetails(deviceId)
      return true
    } catch (err: unknown) {
      error.value =
        err instanceof Error ? err.message : 'Erro ao alterar o monitoramento da interface'
      return false
    } finally {
      updatingInterfaceId.value = null
    }
  }

  // --- Aplicação dos eventos SSE na tela de detalhe aberta ---

  /** Só reage a eventos do dispositivo atualmente carregado */
  function isCurrentDevice(data: Record<string, unknown>): boolean {
    const id = Number(data.deviceId ?? data.id)
    return !!device.value && !!id && device.value.id === id
  }

  function applyDeviceStatus(data: Record<string, unknown>) {
    if (!isCurrentDevice(data) || !device.value) return
    if (data.status) device.value.status = data.status as Device['status']
  }

  function applyMonitorResult(data: Record<string, unknown>) {
    if (!isCurrentDevice(data)) return
    const monitorId = Number(data.monitorId ?? data.id)
    const monitor = monitors.value.find((m) => m.id === monitorId)
    if (!monitor) return

    const latencyMs = (data.latencyMs as number | null | undefined) ?? null
    const finishedAt = String(data.finishedAt ?? new Date().toISOString())

    if (data.status) monitor.status = data.status as DeviceMonitor['status']
    monitor.latencyMs = latencyMs
    monitor.lastLatencyMs = latencyMs ?? undefined
    monitor.lastCheckedAt = finishedAt

    // A listagem desta aba usa o mesmo componente de `/monitors`, que desenha a
    // linha do tempo a partir de `recentResults` — sem anexar a amostra aqui, a
    // barra ficaria congelada enquanto a tela estivesse aberta.
    monitor.recentResults = [
      ...(monitor.recentResults ?? []),
      {
        // Id sintético: o registro real só chega no próximo carregamento
        id: Date.now(),
        monitorId,
        status: data.status as DeviceMonitor['status'] as any,
        startedAt: String(data.startedAt ?? finishedAt),
        finishedAt,
        durationMs: Number(data.durationMs ?? 0),
        latencyMs,
        message: (data.message as string | null) ?? null,
      },
    ].slice(-MONITOR_HISTORY_LIMIT)
  }

  /**
   * Anexa as amostras do evento `metric:recorded` ao histórico já carregado,
   * mantendo gráficos de CPU/Memória e tráfego vivos sem recarregar a página.
   */
  function applyRecordedMetrics(data: Record<string, unknown>) {
    if (!isCurrentDevice(data)) return

    const samples = (data.metrics as Array<Record<string, unknown>>) || []
    if (samples.length === 0) return

    const deviceId = Number(data.deviceId)
    for (const sample of samples) {
      metrics.value.push({
        id: Date.now() + metrics.value.length,
        deviceId,
        interfaceId: (sample.interfaceId as number | null) ?? null,
        interfaceName: null,
        metricName: String(sample.name),
        metricValue: Number(sample.value),
        unit: String(sample.unit ?? ''),
        createdAt: String(sample.recordedAt),
      })
    }

    // Evita crescimento indefinido em telas deixadas abertas por horas
    if (metrics.value.length > METRICS_LIMIT) {
      metrics.value = metrics.value.slice(-METRICS_LIMIT)
    }
  }

  function applyInterfaceChange(type: string, data: Record<string, unknown>) {
    if (!isCurrentDevice(data)) return

    const iface = interfaces.value.find((i) => i.id === Number(data.interfaceId))
    if (iface) {
      if (type === 'interface:status_change' && data.currentStatus) {
        const status = String(data.currentStatus) as 'up' | 'down' | 'testing'
        iface.operStatus = status
        iface.ifOperStatus = status
      }
      if (type !== 'interface:status_change' && data.currentSpeed !== undefined) {
        iface.speed = Number(data.currentSpeed)
        iface.ifSpeed = Number(data.currentSpeed)
      }
    }

    // O evento também vira um registro na aba de eventos do dispositivo
    events.value.unshift({
      id: Number(data.alertEventId ?? Date.now()),
      deviceId: Number(data.deviceId),
      eventType: type,
      severity: type === 'interface:speed_change' ? 'info' : 'warning',
      message: String(data.message ?? 'Alteração detectada na interface'),
      createdAt: new Date().toISOString(),
    })

    if (events.value.length > 100) {
      events.value.length = 100
    }
  }

  function applyInterfaceTraffic(data: Record<string, unknown>) {
    if (!isCurrentDevice(data)) return

    const items = (data.interfaces as Array<Record<string, unknown>>) || []
    const deviceId = Number(data.deviceId)
    const now = new Date().toISOString()

    for (const item of items) {
      const ifId = Number(item.id)
      const iface = interfaces.value.find((i) => i.id === ifId)
      if (iface) {
        if (item.operStatus !== undefined) {
          const st = String(item.operStatus) as 'up' | 'down' | 'testing'
          iface.operStatus = st
          iface.ifOperStatus = st
        }
        if (item.speed !== undefined && item.speed !== null) {
          iface.speed = Number(item.speed)
          iface.ifSpeed = Number(item.speed)
        }
      }
      if (item.inBps !== undefined && item.inBps !== null) {
        metrics.value.push({
          id: Date.now() + metrics.value.length,
          deviceId,
          interfaceId: ifId,
          interfaceName: item.name ? String(item.name) : null,
          metricName: 'inBps',
          metricValue: Number(item.inBps),
          unit: 'bps',
          createdAt: now,
        })
      }
      if (item.outBps !== undefined && item.outBps !== null) {
        metrics.value.push({
          id: Date.now() + metrics.value.length,
          deviceId,
          interfaceId: ifId,
          interfaceName: item.name ? String(item.name) : null,
          metricName: 'outBps',
          metricValue: Number(item.outBps),
          unit: 'bps',
          createdAt: now,
        })
      }
    }

    if (metrics.value.length > METRICS_LIMIT) {
      metrics.value = metrics.value.slice(-METRICS_LIMIT)
    }
  }

  return {
    device,
    interfaces,
    interfaceSuggestions,
    suggestingInterfaces,
    suggestInterfaces,
    fetchInterfaceSuggestions,
    loadLinkInterfaces,
    metrics,
    monitors,
    events,
    loading,
    applyDeviceStatus,
    applyMonitorResult,
    applyRecordedMetrics,
    applyInterfaceChange,
    applyInterfaceTraffic,
    pollingSnmp,
    scanningSnmp,
    updatingInterfaceId,
    scanResult,
    error,
    capabilities,
    loadDeviceDetails,
    reloadMonitors,
    triggerSnmpPoll,
    scanDeviceSnmp,
    applySnmpMonitors,
    setInterfaceMonitoring,
  }
})
