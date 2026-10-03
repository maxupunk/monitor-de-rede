/**
 * A prévia do editor de plugins: a tela desenhada com as saídas dos testes
 * unitários (as respostas gravadas nas fixtures), sem tocar equipamento e sem
 * gravar nada. Funções puras sobre o relatório que `POST /plugins/preview`
 * devolve.
 */
import type { FleetAction } from '@/bindings/FleetAction'
import type { PluginAction } from '@/bindings/PluginAction'
import type { PluginManifest } from '@/bindings/PluginManifest'
import type { TestReport } from '@/bindings/TestReport'

type Output = Record<string, unknown>

/** Um teste que passou, com a saída que a tela vai mostrar. */
export interface PreviewOutput {
  name: string
  action: string
  output: unknown
}

/** As saídas dos testes que passaram, na ordem do pacote. */
export function previewOutputs(report: TestReport | null | undefined): PreviewOutput[] {
  return (report?.cases ?? [])
    .filter((item) => item.passed && item.output !== undefined && item.output !== null)
    .map((item) => ({ name: item.name, action: item.action, output: item.output }))
}

/** A primeira saída de uma ação (a lista do equipamento parte dela). */
export function firstOutputOf(
  report: TestReport | null | undefined,
  action: string | null | undefined
): unknown {
  return previewOutputs(report).find((item) => item.action === action)?.output ?? null
}

/**
 * A frota de exemplo: cada teste da ação de estado vira um "equipamento",
 * com o nome do teste — é como a página da frota vai juntar os itens.
 */
export function previewFleet(
  report: TestReport | null | undefined,
  statusAction: string | null | undefined
): { members: { deviceId: number; name: string }[]; outputs: Map<number, Output> } {
  const members: { deviceId: number; name: string }[] = []
  const outputs = new Map<number, Output>()
  previewOutputs(report)
    .filter((item) => item.action === statusAction)
    .forEach((item, index) => {
      const output = item.output
      if (typeof output !== 'object' || output === null || Array.isArray(output)) return
      members.push({ deviceId: index + 1, name: item.name })
      outputs.set(index + 1, output as Output)
    })
  return { members, outputs }
}

/**
 * As ações com formulário (algum parâmetro), para conferir cada um. A
 * pré-visualização de uma ação de frota tem o mesmo formulário dela e fica de
 * fora — o autor confere o da ação de verdade.
 */
export function actionsWithForm(manifest: PluginManifest | null | undefined): PluginAction[] {
  const previews = new Set((manifest?.fleet?.actions ?? []).map((action) => action.preview))
  return (manifest?.actions ?? []).filter((action) => {
    const properties = action.params?.properties
    return (
      !previews.has(action.id) &&
      typeof properties === 'object' &&
      properties !== null &&
      Object.keys(properties).length > 0
    )
  })
}

/**
 * A ação que um teste exercita, para o título e a apresentação do resultado:
 * a do plugin ou, num teste de `reduce`, a de frota.
 */
export function testedAction(
  manifest: PluginManifest | null | undefined,
  id: string
): PluginAction | FleetAction | null {
  return (
    manifest?.actions.find((action) => action.id === id) ??
    manifest?.fleet?.actions?.find((action) => action.id === id) ??
    null
  )
}
