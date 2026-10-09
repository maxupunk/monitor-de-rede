import { describe, expect, it } from 'vitest'
import type { AiToolCallState } from '@/utils/aiChatStream'
import {
  aiToolKind,
  aiToolMeta,
  isActiveTestTool,
  isDeviceAccessTool,
} from '@/components/ai/aiToolMeta'
import { describeLookups, groupToolActivity, isPassiveLookup } from '@/components/ai/toolActivity'
import { summarizeUsage } from '@/components/ai/aiUsageSummary'

function tool(name: string, extra: Partial<AiToolCallState> = {}): AiToolCallState {
  return { id: `${name}-${extra.id ?? '1'}`, name, arguments: {}, status: 'done', ...extra }
}

describe('natureza das ferramentas', () => {
  it('reaproveita os conjuntos de equipamento e teste ativo', () => {
    expect(aiToolKind('device_ssh_exec')).toBe('device')
    expect(isDeviceAccessTool('device_ssh_exec')).toBe(true)
    expect(aiToolKind('ping_host')).toBe('test')
    expect(isActiveTestTool('ping_host')).toBe(true)
    expect(aiToolKind('chart_monitor_latency')).toBe('chart')
    expect(aiToolKind('silence_alert')).toBe('action')
    expect(aiToolKind('get_system_summary')).toBe('lookup')
    expect(aiToolMeta('list_devices').kind).toBe('lookup')
  })

  it('ferramenta desconhecida aparece inteira e sem cinza', () => {
    expect(aiToolKind('ferramenta_nova')).toBe('action')
    expect(aiToolMeta('ferramenta_nova').color).not.toBe('grey')
  })
})

describe('agrupamento das consultas', () => {
  it('só consulta concluída e sem nada a mostrar é passiva', () => {
    expect(isPassiveLookup(tool('get_alerts'))).toBe(true)
    expect(isPassiveLookup(tool('get_alerts', { status: 'running' }))).toBe(false)
    expect(isPassiveLookup(tool('get_alerts', { status: 'error' }))).toBe(false)
    expect(isPassiveLookup(tool('get_alerts', { status: 'cancelled' }))).toBe(false)
    expect(isPassiveLookup(tool('get_alerts', { autoApproved: true }))).toBe(false)
    expect(isPassiveLookup(tool('get_alerts', { summary: 'Confirma?' }))).toBe(false)
    expect(isPassiveLookup(tool('ping_host'))).toBe(false)
    expect(isPassiveLookup(tool('create_monitor'))).toBe(false)
    expect(isPassiveLookup(tool('fingerprint_device'))).toBe(false)
  })

  it('consulta com gráfico continua com o cartão', () => {
    const chart = {} as NonNullable<AiToolCallState['chart']>
    expect(isPassiveLookup(tool('get_device_metrics', { chart }))).toBe(false)
  })

  it('junta as consultas na posição da primeira e preserva a ordem do resto', () => {
    const summary = tool('get_system_summary')
    const ping = tool('ping_host')
    const devices = tool('list_devices')
    const running = tool('get_alerts', { status: 'running' })
    const items = groupToolActivity([ping, summary, running, devices])
    expect(items).toEqual([
      { type: 'tool', tool: ping },
      { type: 'lookups', tools: [summary, devices] },
      { type: 'tool', tool: running },
    ])
  })

  it('sem consultas passivas, nada é agrupado', () => {
    const ping = tool('ping_host')
    expect(groupToolActivity([ping])).toEqual([{ type: 'tool', tool: ping }])
    expect(groupToolActivity([])).toEqual([])
  })

  it('descreve as consultas pelos rótulos, contando repetições', () => {
    expect(
      describeLookups([
        tool('get_system_summary'),
        tool('list_devices', { id: 'a' }),
        tool('get_alerts'),
        tool('list_devices', { id: 'b' }),
      ])
    ).toBe('Resumo da Infraestrutura, Consulta de Dispositivos (2×), Alertas')
  })
})

describe('rodapé de consumo', () => {
  it('sem medida não há rodapé', () => {
    expect(summarizeUsage(null)).toBeNull()
  })

  it('modelo curto à vista e o resto no detalhe', () => {
    const summary = summarizeUsage({
      model: 'meta-llama/llama-3.3-70b',
      promptTokens: 12_400,
      completionTokens: 320,
      cachedTokens: 8000,
      generationMs: 4000,
      durationMs: 6500,
      contextTokens: 12_720,
      contextWindow: 128_000,
      contextWindowReported: false,
    })
    expect(summary?.model).toBe('llama-3.3-70b')
    expect(summary?.details).toEqual([
      'Modelo: meta-llama/llama-3.3-70b',
      'Enviados ao provedor (↑): 12,4 mil tokens',
      'Servidos do cache do provedor: 8 mil tokens',
      'Gerados na resposta (↓): 320 tokens',
      'Velocidade de geração: 80 tok/s',
      'Tempo total da resposta: 6,5 s',
      'Contexto: 12,7 mil de 128 mil tokens (10%, janela estimada)',
    ])
  })
})
