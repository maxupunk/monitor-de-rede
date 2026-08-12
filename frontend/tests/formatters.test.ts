import assert from 'node:assert/strict'
import test from 'node:test'
import { formatLatency, formatMeasuredValue } from '../src/utils/formatters.ts'

test('corta o lixo de ponto flutuante do RTT em uma casa decimal', () => {
  // O valor que apareceu na tela: `f64` cru vindo do backend.
  assert.equal(formatLatency(6.903808999999999), '6.9 ms')
  assert.equal(formatLatency(0.4321), '0.4 ms')
  assert.equal(formatLatency(12.05), '12.1 ms')
})

test('acima de 100 ms a casa decimal vira ruído e some', () => {
  assert.equal(formatLatency(250.4), '250 ms')
  assert.equal(formatLatency(99.94), '99.9 ms')
  assert.equal(formatLatency(1499.6), '1500 ms')
})

test('não deixa zero à direita', () => {
  assert.equal(formatLatency(7), '7 ms')
  assert.equal(formatLatency(7.04), '7 ms')
  assert.equal(formatLatency(0), '0 ms')
})

test('valor ausente devolve o rótulo combinado, não "NaN ms"', () => {
  assert.equal(formatLatency(null), 'N/A')
  assert.equal(formatLatency(undefined), 'N/A')
  assert.equal(formatLatency(Number.NaN), 'N/A')
  assert.equal(formatLatency(Number.POSITIVE_INFINITY), 'N/A')
  assert.equal(formatLatency(null, 'N/D'), 'N/D')
})

test('métrica que chega com unidade `ms` passa pelo mesmo arredondamento', () => {
  assert.equal(formatMeasuredValue(6.903808999999999, 'ms'), '6.9 ms')
  // As outras unidades seguem como antes.
  assert.equal(formatMeasuredValue(1536, 'bytes'), '1.5 KB')
  assert.equal(formatMeasuredValue(1_500_000, 'bps'), '1.5 Mbps')
})
