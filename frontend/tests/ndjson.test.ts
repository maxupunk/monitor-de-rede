import assert from 'node:assert/strict'
import test from 'node:test'
import { drainNdjson } from '../src/services/ndjson.ts'

test('preserva todos os eventos de uma rajada NDJSON fragmentada', () => {
  const expected = Array.from({ length: 2_048 }, (_, port) => ({ type: 'result', port }))
  const wire = expected.map((event) => JSON.stringify(event)).join('\n') + '\n'
  const splitAt = wire.indexOf('1024') + 2

  const first = drainNdjson<{ type: string; port: number }>(wire.slice(0, splitAt))
  const second = drainNdjson<{ type: string; port: number }>(first.remainder + wire.slice(splitAt))
  const received = [...first.events, ...second.events]

  assert.equal(received.length, expected.length)
  assert.deepEqual(received[0], expected[0])
  assert.deepEqual(received.at(-1), expected.at(-1))
})

test('aceita a última linha mesmo sem quebra de linha', () => {
  const result = drainNdjson<{ type: string; port: number }>(
    '{"type":"result","port":443}',
    { final: true }
  )

  assert.deepEqual(result.events, [{ type: 'result', port: 443 }])
  assert.equal(result.remainder, '')
})
