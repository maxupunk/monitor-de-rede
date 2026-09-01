import { describe, it, expect } from 'vitest'
import {
  formatBinaryBytes,
  formatDecimalBytes,
  formatLatency,
  formatMeasuredValue,
} from '../src/utils/formatters.ts'

describe('formatters', () => {
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
})
