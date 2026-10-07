import { describe, expect, it } from 'vitest'
import { isLoopbackHost, loopbackFieldHint } from '@/utils/hostHints'

describe('endereço local no campo Servidor', () => {
  it('reconhece localhost, 127.x e ::1', () => {
    for (const host of ['127.0.0.1', ' 127.0.1.1 ', 'localhost', 'LocalHost', '::1', '[::1]']) {
      expect(isLoopbackHost(host)).toBe(true)
    }
    for (const host of ['10.0.0.5', 'host.docker.internal', '127.0.0.1.nip.io', '']) {
      expect(isLoopbackHost(host)).toBe(false)
    }
  })

  it('só mostra a dica para endereço local', () => {
    expect(loopbackFieldHint('127.0.0.1').hint).toContain('host.docker.internal')
    expect(loopbackFieldHint('db.local')).toEqual({})
  })
})
