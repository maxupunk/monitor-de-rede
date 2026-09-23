import { describe, it, expect } from 'vitest'
import {
  formatBinaryBytes,
  formatDecimalBytes,
  formatLatency,
  formatMeasuredValue,
  formatPercent,
  formatTimeSpan,
  resolveAutoUnit,
  resolveSpeedUnit,
  convertSpeedFromMbps,
  formatSpeedByUnit,
  getSpeedUnitLabel,
} from '../src/utils/formatters.ts'

describe('formatters', () => {
  it('formata percentual com casas fixas no padrão brasileiro', () => {
    expect(formatPercent(42.456)).toBe('42,5%')
    expect(formatPercent(7, 0)).toBe('7%')
    expect(formatPercent(null)).toBe('—')
    expect(formatPercent(Number.NaN, 1, 'N/D')).toBe('N/D')
  })

  it('corta o lixo de ponto flutuante do RTT em uma casa decimal', () => {
    expect(formatLatency(6.903808999999999)).toBe('6.9 ms')
    expect(formatLatency(0.4321)).toBe('0.4 ms')
    expect(formatLatency(12.05)).toBe('12.1 ms')
  })

  it('acima de 100 ms a casa decimal vira ruído e some', () => {
    expect(formatLatency(250.4)).toBe('250 ms')
    expect(formatLatency(99.94)).toBe('99.9 ms')
    expect(formatLatency(1499.6)).toBe('1500 ms')
  })

  it('não deixa zero à direita', () => {
    expect(formatLatency(7)).toBe('7 ms')
    expect(formatLatency(7.04)).toBe('7 ms')
    expect(formatLatency(0)).toBe('0 ms')
  })

  it('valor ausente devolve o rótulo combinado, não "NaN ms"', () => {
    expect(formatLatency(null)).toBe('N/A')
    expect(formatLatency(undefined)).toBe('N/A')
    expect(formatLatency(Number.NaN)).toBe('N/A')
    expect(formatLatency(Number.POSITIVE_INFINITY)).toBe('N/A')
    expect(formatLatency(null, 'N/D')).toBe('N/D')
  })

  it('métrica que chega com unidade `ms` passa pelo mesmo arredondamento', () => {
    expect(formatMeasuredValue(6.903808999999999, 'ms')).toBe('6.9 ms')
    expect(formatMeasuredValue(1536, 'bytes')).toBe('1.5 KiB')
    expect(formatMeasuredValue(1_500_000, 'bps')).toBe('1.5 Mbps')
  })

  it('distingue bytes binários de contadores decimais', () => {
    expect(formatBinaryBytes(1_048_576)).toBe('1 MiB')
    expect(formatDecimalBytes(1_000_000)).toBe('1 MB')
  })

  it('formata o total de dias/horas de histórico entre duas datas', () => {
    const start = new Date('2026-08-01T10:00:00Z')
    const end31d6h = new Date('2026-09-01T16:00:00Z')
    expect(formatTimeSpan(start, end31d6h)).toBe('31 dias e 6 horas')

    const end1d1h = new Date('2026-08-02T11:00:00Z')
    expect(formatTimeSpan(start, end1d1h)).toBe('1 dia e 1 hora')

    const end1d = new Date('2026-08-02T10:00:00Z')
    expect(formatTimeSpan(start, end1d)).toBe('1 dia')

    const end5h = new Date('2026-08-01T15:00:00Z')
    expect(formatTimeSpan(start, end5h)).toBe('5 horas')

    const end5h30m = new Date('2026-08-01T15:30:00Z')
    expect(formatTimeSpan(start, end5h30m)).toBe('5 horas e 30 min')

    const end15m = new Date('2026-08-01T10:15:00Z')
    expect(formatTimeSpan(start, end15m)).toBe('15 minutos')

    const end0 = new Date('2026-08-01T10:00:00Z')
    expect(formatTimeSpan(start, end0)).toBe('menos de 1 minuto')

    expect(formatTimeSpan(null, null)).toBe('—')
    expect(formatTimeSpan(start, null)).toBe('—')
  })

  describe('resolução automática e formatação de unidades de velocidade', () => {
    it('determina unidade automaticamente conforme magnitude em Mbps', () => {
      expect(resolveAutoUnit(1500)).toBe('gbps')
      expect(resolveAutoUnit(1000)).toBe('gbps')
      expect(resolveAutoUnit(999.9)).toBe('mbps')
      expect(resolveAutoUnit(50)).toBe('mbps')
      expect(resolveAutoUnit(1)).toBe('mbps')
      expect(resolveAutoUnit(0.85)).toBe('kbps')
      expect(resolveAutoUnit(0.01)).toBe('kbps')
      expect(resolveAutoUnit(0)).toBe('mbps')
      expect(resolveAutoUnit(null)).toBe('mbps')
      expect(resolveAutoUnit(undefined)).toBe('mbps')
    })

    it('resolve a unidade respeitando modo auto vs modo manual', () => {
      expect(resolveSpeedUnit(0.85, 'auto')).toBe('kbps')
      expect(resolveSpeedUnit(0.85, 'mbps')).toBe('mbps')
      expect(resolveSpeedUnit(1200, 'auto')).toBe('gbps')
      expect(resolveSpeedUnit(1200, 'mbps')).toBe('mbps')
      expect(resolveSpeedUnit(50, 'kbps')).toBe('kbps')
    })

    it('converte valores a partir de Mbps para a unidade alvo', () => {
      expect(convertSpeedFromMbps(1.5, 'kbps')).toBe(1500)
      expect(convertSpeedFromMbps(1500, 'gbps')).toBe(1.5)
      expect(convertSpeedFromMbps(50, 'mbps')).toBe(50)
      expect(convertSpeedFromMbps(0.5, 'auto')).toBe(500)
    })

    it('formata valores e labels de velocidade corretamente com auto-identificação', () => {
      expect(formatSpeedByUnit(0.45, 'auto')).toBe('450')
      expect(getSpeedUnitLabel('auto', 0.45)).toBe('Kbps')

      expect(formatSpeedByUnit(85.4, 'auto')).toBe('85.4')
      expect(getSpeedUnitLabel('auto', 85.4)).toBe('Mbps')

      expect(formatSpeedByUnit(1250, 'auto')).toBe('1.25')
      expect(getSpeedUnitLabel('auto', 1250)).toBe('Gbps')

      expect(formatSpeedByUnit(null, 'auto')).toBe('--')
    })
  })
})
