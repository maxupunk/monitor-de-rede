/**
 * Como as chamadas de ferramenta de uma resposta aparecem no chat: consultas
 * passivas já concluídas viram uma linha só ("Consultou: …"); o que pede
 * atenção (em andamento, aguardando, falha, gráfico, teste, ação, equipamento)
 * continua com o cartão inteiro.
 */
import type { AiToolCallState } from '@/utils/aiChatStream'
import { aiToolMeta } from './aiToolMeta'

export type ToolActivityItem =
  { type: 'tool'; tool: AiToolCallState } | { type: 'lookups'; tools: AiToolCallState[] }

/** Consulta concluída sem nada que o usuário precise ver de imediato. */
export function isPassiveLookup(tool: AiToolCallState): boolean {
  return (
    tool.status === 'done' &&
    !tool.chart &&
    !tool.autoApproved &&
    !tool.summary &&
    aiToolMeta(tool.name).kind === 'lookup'
  )
}

/**
 * Agrupa as consultas passivas num único item, no lugar da primeira delas;
 * os demais cartões mantêm a ordem em que a IA os chamou.
 */
export function groupToolActivity(tools: AiToolCallState[]): ToolActivityItem[] {
  const items: ToolActivityItem[] = []
  let lookups: AiToolCallState[] | null = null
  for (const tool of tools) {
    if (!isPassiveLookup(tool)) {
      items.push({ type: 'tool', tool })
    } else if (lookups) {
      lookups.push(tool)
    } else {
      lookups = [tool]
      items.push({ type: 'lookups', tools: lookups })
    }
  }
  return items
}

/** "Alertas, Consulta de Dispositivos (2×)": rótulos na ordem, repetições contadas. */
export function describeLookups(tools: AiToolCallState[]): string {
  const counts = new Map<string, number>()
  for (const tool of tools) {
    const label = aiToolMeta(tool.name).label
    counts.set(label, (counts.get(label) ?? 0) + 1)
  }
  return [...counts]
    .map(([label, count]) => (count > 1 ? `${label} (${count}×)` : label))
    .join(', ')
}
