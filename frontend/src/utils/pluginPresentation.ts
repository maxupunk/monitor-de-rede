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
    .filter((item) => item.installed)
    .map((item) => ({
      id: item.plugin.id,
      title: item.plugin.panel?.title ?? item.plugin.name,
      icon: item.plugin.panel?.icon ?? 'mdi-puzzle',
      item,
    }))
}
