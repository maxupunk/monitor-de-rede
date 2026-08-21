import { describe, it, expect } from 'vitest'
import { drainNdjson } from '../src/services/ndjson.ts'

describe('drainNdjson', () => {
  it('preserva todos os eventos de uma rajada NDJSON fragmentada', () => {
    const expected = Array.from({ length: 2_048 }, (_, port) => ({ type: 'result', port }))
    const wire = expected.map((event) => JSON.stringify(event)).join('\n') + '\n'
    const splitAt = wire.indexOf('1024') + 2

    const first = drainNdjson<{ type: string; port: number }>(wire.slice(0, splitAt))
    const second = drainNdjson<{ type: string; port: number }>(
      first.remainder + wire.slice(splitAt)
    )
    const received = [...first.events, ...second.events]

    expect(received.length).toBe(expected.length)
    expect(received[0]).toEqual(expected[0])
    expect(received.at(-1)).toEqual(expected.at(-1))
  })

  it('aceita a última linha mesmo sem quebra de linha', () => {
    const result = drainNdjson<{ type: string; port: number }>('{"type":"result","port":443}', {
      final: true,
    })

    expect(result.events).toEqual([{ type: 'result', port: 443 }])
    expect(result.remainder).toBe('')
  })
})
