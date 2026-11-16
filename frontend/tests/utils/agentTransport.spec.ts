import { describe, expect, it } from 'vitest'
import { isPrivateHost, transportNotice } from '@/utils/agentTransport'

describe('transportNotice', () => {
  it('avisa quando o canal é HTTP puro pela internet', () => {
    expect(transportNotice('http://monitor.exemplo.com:8080', 'domain')?.type).toBe('warning')
    expect(transportNotice('http://200.1.2.3:3333', 'public')?.type).toBe('warning')
    expect(transportNotice('http://200.1.2.3:3333', null)?.type).toBe('warning')
  })

  it('HTTPS e rede privada não pedem aviso', () => {
    expect(transportNotice('https://monitor.exemplo.com', 'domain')).toBeNull()
    expect(transportNotice('http://192.168.0.10:3333', 'lan')).toBeNull()
    expect(transportNotice('http://10.20.0.5:9000', 'custom')).toBeNull()
    expect(transportNotice('http://[fd00::5]:3333', 'lan')).toBeNull()
  })

  it('o endereço do túnel lembra que o servidor precisa já estar na VPN', () => {
    expect(transportNotice('http://10.8.0.1:3333', 'vpn')?.type).toBe('info')
  })
})

describe('isPrivateHost', () => {
  it('reconhece LAN, CGNAT e IPv6 local, sem confundir nomes', () => {
    for (const host of ['10.0.0.1', '172.16.5.4', '172.31.0.1', '100.64.0.1', 'fd00::5']) {
      expect(isPrivateHost(host), host).toBe(true)
    }
    for (const host of ['172.32.0.1', '100.128.0.1', '8.8.8.8', 'fdominio.com', 'fe80.com']) {
      expect(isPrivateHost(host), host).toBe(false)
    }
  })
})
