import { describe, expect, it } from 'vitest'
import { isLocalOllaya } from '@/utils/ollaya'

describe('isLocalOllaya', () => {
  it('reconhece o Ollaya desta máquina', () => {
    expect(isLocalOllaya('http://ollaya:11435')).toBe(true)
    expect(isLocalOllaya('http://localhost:11435/')).toBe(true)
    expect(isLocalOllaya('127.0.0.1:11435')).toBe(true)
    expect(isLocalOllaya('http://[::1]:11435')).toBe(true)
    expect(isLocalOllaya('http://host.docker.internal:11435')).toBe(true)
    expect(isLocalOllaya('')).toBe(true)
  })

  it('outra máquina não é local', () => {
    expect(isLocalOllaya('http://192.168.1.50:11435')).toBe(false)
    expect(isLocalOllaya('https://laya.exemplo.com.br')).toBe(false)
    expect(isLocalOllaya('http://ollaya-gpu:11435')).toBe(false)
  })
})
