/**
 * Como os estados dos plugins de dispositivo aparecem na tela: rótulo, cor
 * semântica e ícone. Um lugar só para a aba do equipamento, a biblioteca e os
 * diálogos — e nenhum chip cinza para estado que importa.
 */
import type { Compat } from '@/bindings/Compat'
import type { DevicePluginItem } from '@/bindings/DevicePluginItem'
import type { DevicePluginsView } from '@/bindings/DevicePluginsView'
import type { Effect } from '@/bindings/Effect'
import type { PluginPackage } from '@/bindings/PluginPackage'
import type { Severity } from '@/bindings/Severity'
import type { MatchRule } from '@/bindings/MatchRule'
import type { Device } from '@/stores/devices'
import type { TranscriptEntry } from '@/bindings/TranscriptEntry'
import type { OutputFormat } from '@/bindings/OutputFormat'
import {
  formatBps,
  formatBytes,
  formatCompactCount,
  formatDateTime,
  formatElapsedMs,
  formatLatency,
  formatPercent,
  formatRelativeTime,
  formatTimeSpan,
} from '@/utils/formatters'

export interface Presentation {
  label: string
  color: string
  icon: string
}

const COMPAT: Record<Compat, Presentation> = {
  validated: { label: 'Validado', color: 'success', icon: 'mdi-check-decagram' },
  likely: { label: 'Compatível', color: 'info', icon: 'mdi-check-circle-outline' },
  possible: { label: 'Talvez compatível', color: 'warning', icon: 'mdi-help-circle-outline' },
  incompatible: { label: 'Incompatível', color: 'error', icon: 'mdi-close-circle-outline' },
}

const STATUS: Record<string, Presentation> = {
  active: { label: 'Ativo', color: 'success', icon: 'mdi-power-plug' },
  tested: { label: 'Testado', color: 'info', icon: 'mdi-test-tube' },
  draft: { label: 'Rascunho', color: 'warning', icon: 'mdi-pencil-outline' },
  quarantine: { label: 'Quarentena', color: 'error', icon: 'mdi-shield-alert-outline' },
  disabled: { label: 'Desativado', color: 'secondary', icon: 'mdi-power-plug-off-outline' },
}

const SOURCE: Record<string, Presentation> = {
  builtin: { label: 'Embutido', color: 'primary', icon: 'mdi-package-variant-closed' },
  user: { label: 'Próprio', color: 'secondary', icon: 'mdi-account-edit-outline' },
  ai: { label: 'Criado pela IA', color: 'info', icon: 'mdi-robot-outline' },
  imported: { label: 'Importado', color: 'warning', icon: 'mdi-file-import-outline' },
}

const EFFECT: Record<Effect, Presentation> = {
  read: { label: 'Leitura', color: 'info', icon: 'mdi-eye-outline' },
  write: { label: 'Altera o equipamento', color: 'error', icon: 'mdi-pencil-alert-outline' },
}

const SEVERITY: Record<Severity, Presentation> = {
  info: { label: 'Informativo', color: 'info', icon: 'mdi-information-outline' },
  low: { label: 'Baixo', color: 'success', icon: 'mdi-shield-check-outline' },
  medium: { label: 'Médio', color: 'warning', icon: 'mdi-shield-half-full' },
  high: { label: 'Alto', color: 'error', icon: 'mdi-shield-alert-outline' },
  critical: { label: 'Crítico', color: 'error', icon: 'mdi-skull-crossbones-outline' },
}

const RUN_STATUS: Record<string, Presentation> = {
  running: { label: 'Executando', color: 'primary', icon: 'mdi-progress-clock' },
  succeeded: { label: 'Concluído', color: 'success', icon: 'mdi-check' },
  failed: { label: 'Falhou', color: 'error', icon: 'mdi-alert-circle' },
  cancelled: { label: 'Cancelado', color: 'warning', icon: 'mdi-cancel' },
}

const ORIGIN: Record<string, string> = {
  user: 'Operador',
  ai: 'IA',
  validation: 'Validação',
}

const FALLBACK: Presentation = { label: '—', color: 'secondary', icon: 'mdi-help' }

export const compatPresentation = (value: Compat): Presentation => COMPAT[value] ?? FALLBACK
export const statusPresentation = (value: string): Presentation =>
  STATUS[value] ?? { ...FALLBACK, label: value }
export const sourcePresentation = (value: string): Presentation =>
  SOURCE[value] ?? { ...FALLBACK, label: value }
export const effectPresentation = (value: Effect): Presentation => EFFECT[value] ?? FALLBACK
export const severityPresentation = (value: Severity): Presentation => SEVERITY[value] ?? FALLBACK
export const runStatusPresentation = (value: string): Presentation =>
  RUN_STATUS[value] ?? { ...FALLBACK, label: value }
export const originLabel = (value: string): string => ORIGIN[value] ?? value

/** Ponto de partida do editor: o mínimo que passa na validação. */
export function packageTemplate(): PluginPackage {
  return {
    format: 1,
    manifest: {
      slug: 'meu-plugin',
      name: 'Meu plugin',
      version: '1.0.0',
      description: 'O que este plugin faz.',
      transports: ['ssh'],
      match: { platforms: [] },
      surfaces: ['device'],
      actions: [
        {
          id: 'detect',
          title: 'Detectar versão',
          effect: 'read',
          output: 'kv',
          safeToRetest: false,
        },
      ],
    },
    script:
      '// Cada ação é fn <id>(device, params). Veja a skill de autoria.\n' +
      'fn detect(device, params) {\n' +
      '    #{ firmware: trimmed(device.run("uname -r")) }\n' +
      '}\n',
    usage: '# Meu plugin\n\n## Detectar versão\nLê a versão do sistema. Não altera nada.\n',
    compatibility: [],
    tests: {
      unit: [
        {
          action: 'detect',
          params: {},
          fixtures: [
            {
              ssh: 'uname -r',
              regex: false,
              stdout: '6.1.0\n',
              stderr: '',
              exit: 0,
              status: 200,
              body: '',
            },
          ],
          expect: { firmware: '6.1.0' },
        },
      ],
      functional: [{ action: 'detect', params: {}, expectKeys: ['firmware'] }],
    },
  }
}

/** Lê um `.nmplugin.json` escolhido pelo usuário. */
export async function readPackageFile(file: File): Promise<PluginPackage> {
  const text = await file.text()
  const parsed: unknown = JSON.parse(text)
  if (
    typeof parsed !== 'object' ||
    parsed === null ||
    !('manifest' in parsed) ||
    !('script' in parsed)
  ) {
    throw new Error('O arquivo não é um pacote de plugin (.nmplugin.json).')
  }
  return parsed as PluginPackage
}

/** Valor da aba de um plugin instalado em `/devices/{id}` (também no `?tab=`). */
export const pluginTab = (pluginId: number): string => `plugin-${pluginId}`

export interface InstalledPluginTab {
  id: number
  title: string
  icon: string
  item: DevicePluginItem
}

/** Os plugins instalados no equipamento, como abas: título e ícone da tela própria. */
export function installedPluginTabs(
  view: DevicePluginsView | undefined | null
): InstalledPluginTab[] {
  return (view?.plugins ?? [])
    .filter((item) => item.installed && item.plugin.surfaces.includes('device'))
    .map((item) => ({
      id: item.plugin.id,
      title: item.plugin.list?.title ?? item.plugin.name,
      icon: item.plugin.list?.icon ?? 'mdi-puzzle',
      item,
    }))
}

const BATCH_STATUS: Record<string, Presentation> = {
  running: { label: 'Em andamento', color: 'primary', icon: 'mdi-progress-clock' },
  succeeded: { label: 'Concluído', color: 'success', icon: 'mdi-check-all' },
  partial: { label: 'Parcial', color: 'warning', icon: 'mdi-alert-outline' },
  failed: { label: 'Falhou', color: 'error', icon: 'mdi-alert-circle' },
  cancelled: { label: 'Cancelado', color: 'warning', icon: 'mdi-cancel' },
  pending: { label: 'Na fila', color: 'secondary', icon: 'mdi-timer-sand' },
  skipped: { label: 'Pulado', color: 'warning', icon: 'mdi-debug-step-over' },
}

export const batchStatusPresentation = (value: string): Presentation =>
  BATCH_STATUS[value] ?? RUN_STATUS[value] ?? { ...FALLBACK, label: value }

/**
 * Cor semântica de um estado que um plugin devolve (ex.: o estado de uma rede
 * Wi-Fi na grade da frota). Palavras conhecidas viram cor; o resto, `info`.
 */
const STATE_COLORS: Record<string, string> = {
  ok: 'success',
  sincronizado: 'success',
  ativo: 'success',
  online: 'success',
  divergente: 'warning',
  pendente: 'warning',
  sem_radio: 'warning',
  fora_do_ar: 'warning',
  no_ar: 'success',
  ativa: 'success',
  desativada: 'secondary',
  desligado: 'secondary',
  ausente: 'error',
  falhou: 'error',
  offline: 'error',
  desativado: 'secondary',
  criar: 'success',
  alterar: 'warning',
  remover: 'error',
}

export function stateColor(value: unknown): string {
  return STATE_COLORS[String(value).toLowerCase()] ?? 'info'
}

export function stateLabel(value: unknown): string {
  const text = String(value).replace(/_/g, ' ')
  return text.charAt(0).toUpperCase() + text.slice(1)
}

/** Títulos que o plugin declara para as chaves da saída (`labels`). */
export type OutputLabels = Record<string, string> | undefined

/**
 * Como a tela apresenta a saída de uma ação: os títulos (`labels`) e o
 * formato (`formats`) de cada chave. A própria ação (de dispositivo ou de
 * frota) serve — as duas declaram os dois campos.
 */
export type OutputPresentation =
  | { labels?: Record<string, string>; formats?: Partial<Record<string, OutputFormat>> }
  | null
  | undefined

/** O formato declarado para a chave, se há. */
export function outputFormat(
  key: string,
  presentation?: OutputPresentation
): OutputFormat | undefined {
  return presentation?.formats?.[key]
}

function numberOf(value: unknown): number {
  if (typeof value === 'number') return value
  if (typeof value === 'string' && value.trim() !== '') return Number(value)
  return Number.NaN
}

/** Data de um valor de saída: texto ISO ou segundos Unix. */
function dateOf(value: unknown): Date | null {
  const seconds = numberOf(value)
  if (Number.isFinite(seconds)) return new Date(seconds * 1000)
  if (typeof value !== 'string') return null
  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? null : date
}

/**
 * O valor escrito no formato declarado, com os formatadores do sistema. Valor
 * que não cabe no formato (texto onde se esperava número) sai como veio.
 */
export function formatOutputValue(value: unknown, format: OutputFormat): string {
  const number = numberOf(value)
  const numeric = Number.isFinite(number)
  switch (format) {
    case 'bytes':
      return numeric ? formatBytes(number) : String(value)
    case 'bps':
      return numeric ? formatBps(number) : String(value)
    case 'latency':
      return numeric ? formatLatency(number) : String(value)
    case 'percent':
      return numeric ? formatPercent(number) : String(value)
    case 'duration':
      return numeric ? formatElapsedMs(number) : String(value)
    case 'uptime':
      return numeric
        ? formatTimeSpan(new Date(Date.now() - number * 1000), new Date())
        : String(value)
    case 'count':
      return numeric ? formatCompactCount(number) : String(value)
    case 'datetime': {
      const date = dateOf(value)
      return date ? formatDateTime(date) : String(value)
    }
    case 'relative': {
      const date = dateOf(value)
      return date ? formatRelativeTime(date) : String(value)
    }
    case 'state':
      return stateLabel(value)
  }
}

/** Título de uma chave: o declarado pelo plugin ou a própria chave legível. */
export function outputLabel(key: string, labels?: OutputLabels): string {
  return labels?.[key] ?? keyLabel(key)
}

/** Rótulo legível de uma chave (`pending_changes` → "Pending changes"). */
export function keyLabel(key: string): string {
  const text = key.replace(/_/g, ' ')
  return text.charAt(0).toUpperCase() + text.slice(1)
}

/**
 * Sugestão de um consolidado (`reduce`): rodar a ação de frota `action` com
 * os parâmetros de cada equipamento (ex.: o canal que o plano escolheu).
 */
export interface FollowUp {
  action: string
  title: string
  devices: Record<number, Record<string, unknown>>
}

function isPlainRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

export function followUpOf(result: unknown): FollowUp | null {
  const next = isPlainRecord(result) ? result.next : null
  if (!isPlainRecord(next) || typeof next.action !== 'string' || !isPlainRecord(next.devices)) {
    return null
  }
  const devices: Record<number, Record<string, unknown>> = {}
  for (const [id, params] of Object.entries(next.devices)) {
    if (Number.isFinite(Number(id)) && isPlainRecord(params)) devices[Number(id)] = params
  }
  if (Object.keys(devices).length === 0) return null
  return {
    action: next.action,
    title: typeof next.title === 'string' ? next.title : 'Aplicar sugestão',
    devices,
  }
}

/** Sistemas que, no cadastro, já sugerem o tipo "roteador". */
const ROUTER_PLATFORMS = new Set(['openwrt', 'routeros'])

/**
 * O cadastro de um equipamento novo aberto de dentro de um plugin já vem com o
 * sistema que o plugin atende — o que também o deixa compatível e liga o
 * plugin no cadastro.
 */
export function devicePrefillFor(matcher: MatchRule): Partial<Device> {
  const platform = matcher.platforms?.[0]
  if (!platform) return { isMonitored: true }
  return {
    operatingSystem: platform,
    type: ROUTER_PLATFORMS.has(platform) ? 'router' : 'other',
    isMonitored: true,
  }
}

/** Prefixo do aviso "este acesso começou" (`plugin:run_step`). */
const STEP_PREFIX = 'step:'

/** Quanto do comando aparece no andamento (o resto vai para o histórico). */
const STEP_MAX = 70

/**
 * O último acesso de uma execução em andamento, para quem espera saber o que
 * está acontecendo ("SSH · iwinfo 'phy0-ap0' scan · 3,2 s").
 */
export function stepLabel(entry: TranscriptEntry | undefined): string {
  if (!entry) return 'Conectando ao equipamento…'
  const request =
    entry.request.length > STEP_MAX ? entry.request.slice(0, STEP_MAX) + '…' : entry.request
  // `step:ssh` = começou e ainda não respondeu; senão, já respondeu.
  const running = entry.kind.startsWith(STEP_PREFIX)
  const kind = (running ? entry.kind.slice(STEP_PREFIX.length) : entry.kind).toUpperCase()
  return running
    ? `${kind} · ${request} · executando…`
    : `${kind} · ${request} · respondeu em ${formatElapsedMs(entry.durationMs)}`
}
