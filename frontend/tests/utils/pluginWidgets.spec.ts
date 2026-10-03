import { describe, expect, it } from 'vitest'
import { ruleHolds, visibleWhenOf, widgetOf, widgetRule } from '@/utils/pluginWidgets'

describe('componentes de campo', () => {
  it('cada componente confere o próprio valor com a mesma frase do backend', () => {
    const ip = widgetRule('ip')
    expect(ip?.('192.168.1.1')).toBe(true)
    expect(ip?.('fe80::1')).toBe(true)
    expect(ip?.('192.168.1.300')).toContain('endereço IP')
    expect(ip?.('')).toBe(true)
    expect(widgetRule('cidr')?.('10.0.0.0/8')).toBe(true)
    expect(widgetRule('cidr')?.('10.0.0.0/33')).toContain('192.168.1.0/24')
    expect(widgetRule('mac')?.('AA-BB-CC-DD-EE-FF')).toBe(true)
    expect(widgetRule('mac')?.('aa:bb:cc')).toContain('MAC')
    expect(widgetRule('port')?.(443)).toBe(true)
    expect(widgetRule('port')?.(0)).toContain('1 a 65535')
    expect(widgetRule('hostname')?.('ap-sala.lan')).toBe(true)
    expect(widgetRule('hostname')?.('-ruim')).toContain('nome de host')
    expect(widgetRule('url')?.('https://exemplo.com')).toBe(true)
    expect(widgetRule('url')?.('ftp://exemplo.com')).toContain('http')
    expect(widgetRule('textarea')).toBeNull()
    expect(widgetOf('mapa')).toBeUndefined()
  })

  it('a regra de visibilidade decide com equals, in e notIn', () => {
    const senha = visibleWhenOf({ field: 'encryption', notIn: ['none'] })
    expect(ruleHolds(senha, { encryption: 'psk2' })).toBe(true)
    expect(ruleHolds(senha, { encryption: 'none' })).toBe(false)
    const mesh = visibleWhenOf({ field: 'enabled', equals: true })
    expect(ruleHolds(mesh, { enabled: false })).toBe(false)
    expect(ruleHolds(visibleWhenOf({ field: 'band', in: ['5g'] }), { band: '5g' })).toBe(true)
    expect(ruleHolds(undefined, {})).toBe(true)
    expect(visibleWhenOf('nada')).toBeUndefined()
  })
})
