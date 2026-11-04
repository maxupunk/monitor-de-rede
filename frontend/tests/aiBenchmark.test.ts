import { describe, expect, it } from 'vitest'
import type { ChatBenchmarkRow } from '@/bindings/ChatBenchmarkRow'
import type { ChatBenchmarkSplitView } from '@/bindings/ChatBenchmarkSplitView'
import {
  chatActiveSplit,
  chatDomainScoreOf,
  chatSpeedFact,
  chatTierFact,
  chatVerdictFact,
  formatConfidenceInterval,
  formatDownloadSizeMb,
  formatSecondsPt,
  formatTokensPerSecond,
  resolveBenchmarkTab,
} from '@/utils/aiBenchmark'

const SAMPLE_SPLIT: ChatBenchmarkSplitView = {
  run: '2026-11-02-chat-blind-v3-quants',
  cases: 45,
  overall: 0.9743,
  overallLow: 0.9563,
  overallHigh: 0.9922,
  tools: 0.9889,
  answer: 0.9407,
  safety: 1.0,
  perfect: 0.8889,
  errors: 0,
  turnP50Ms: 6547.8,
  turnP95Ms: 13773.4,
  tokensPerS: 13.8,
  fit: true,
  domains: [
    { domain: 'diagnostico', score: 1.0 },
    { domain: 'redes_equipamentos', score: 0.9611 },
  ],
}

const SAMPLE_ROW: ChatBenchmarkRow = {
  model: 'lfm3-netmonitor:latest',
  label: 'LFM3 NetMonitor (Q8_0)',
  family: 'Liquid LFM3 MoE (NetMonitor v3)',
  quantization: 'Q8_0',
  parameterSize: '2.6B MoE (472M ativos)',
  downloadMb: 1846,
  recommended: true,
  bundled: true,
  netmonitorTrained: true,
  fit: true,
  bench: { ...SAMPLE_SPLIT, cases: 26, overall: 0.9375 },
  blind: SAMPLE_SPLIT,
}

describe('aiBenchmark utilities', () => {
  it('resolveBenchmarkTab normaliza valores de query para chat ou decider', () => {
    expect(resolveBenchmarkTab(undefined)).toBe('chat')
    expect(resolveBenchmarkTab('chat')).toBe('chat')
    expect(resolveBenchmarkTab('DECIDER')).toBe('decider')
    expect(resolveBenchmarkTab('invalido')).toBe('chat')
  })

  it('formata intervalos, segundos, tok/s e tamanho em MB/GB em português', () => {
    expect(formatConfidenceInterval(0.8971, 0.9779)).toBe('90%–98%')
    expect(formatSecondsPt(6547.8)).toBe('6,5 s')
    expect(formatTokensPerSecond(18.1)).toBe('18,1 tok/s')
    expect(formatDownloadSizeMb(1846)).toBe('1,85 GB')
    expect(formatDownloadSizeMb(510)).toBe('510 MB')
    expect(formatDownloadSizeMb(null)).toBe('—')
  })

  it('seleciona a suíte correta (blind ou bench) e extrai notas por domínio', () => {
    expect(chatActiveSplit(SAMPLE_ROW, 'blind')?.cases).toBe(45)
    expect(chatActiveSplit(SAMPLE_ROW, 'bench')?.cases).toBe(26)
    const withoutBlind: ChatBenchmarkRow = { ...SAMPLE_ROW, blind: undefined }
    expect(chatActiveSplit(withoutBlind, 'blind')).toBeNull()
    expect(chatDomainScoreOf(SAMPLE_SPLIT, 'diagnostico')).toBe(1.0)
    expect(chatDomainScoreOf(SAMPLE_SPLIT, 'inexistente')).toBeNull()
  })

  it('classifica o perfil das 3 quantizações e modelos de referência', () => {
    expect(chatTierFact(SAMPLE_ROW).label).toContain('Máxima precisão')
    expect(
      chatTierFact({ ...SAMPLE_ROW, recommended: false, quantization: 'Q5_K_M' }).label
    ).toContain('Equilibrado')
    expect(
      chatTierFact({ ...SAMPLE_ROW, recommended: false, quantization: 'Q4_K_M' }).label
    ).toContain('Ultraleve')
    expect(
      chatTierFact({ ...SAMPLE_ROW, recommended: false, bundled: false }).label
    ).toContain('Referência')
  })

  it('avalia velocidade e veredito do portão com motivos detalhados', () => {
    const gates = { minScore: 0.8, minTools: 0.8, minSafety: 0.95, timeoutMs: 30000 }
    expect(chatSpeedFact(SAMPLE_SPLIT, 30000).color).toBe('info')
    const okVerdict = chatVerdictFact(SAMPLE_ROW, SAMPLE_SPLIT, gates)
    expect(okVerdict.passed).toBe(true)

    const failedSplit: ChatBenchmarkSplitView = {
      ...SAMPLE_SPLIT,
      overall: 0.668,
      tools: 0.596,
      safety: 0.923,
      fit: false,
    }
    const failVerdict = chatVerdictFact({ ...SAMPLE_ROW, fit: false }, failedSplit, gates)
    expect(failVerdict.passed).toBe(false)
    expect(failVerdict.reason).toContain('ferramentas')
    expect(failVerdict.reason).toContain('segurança')
  })
})

