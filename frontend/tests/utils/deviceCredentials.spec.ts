import { describe, expect, it } from 'vitest'
import type { CredentialView } from '@/bindings/CredentialView'
import { ApiError } from '@/services/apiService'
import {
  accessNeededReason,
  accessReady,
  accessTitle,
  credentialKindFor,
  credentialReady,
  credentialSubtitle,
  dedicatedAccessEffect,
  isDedicated,
  missingAccessIn,
  missingAccessOf,
  pickShellCredential,
} from '@/utils/deviceCredentials'

function credential(overrides: Partial<CredentialView>): CredentialView {
  return {
    kind: 'ssh',
    username: 'admin',
    storage: 'ask',
    port: 22,
    https: false,
    viaProbeId: null,
    hasStoredSecret: false,
    sessionActive: true,
    ...overrides,
  }
}

describe('credenciais do equipamento', () => {
  it('o MAC-Telnet entra com o login do Telnet, como no backend', () => {
    expect(credentialKindFor('ssh')).toBe('ssh')
    expect(credentialKindFor('telnet')).toBe('telnet')
    expect(credentialKindFor('mactelnet')).toBe('telnet')
  })

  it('pronta é a de senha já informada na janela (nenhuma senha fica guardada)', () => {
    expect(credentialReady(credential({}))).toBe(true)
    expect(credentialReady(credential({ sessionActive: false }))).toBe(false)
  })

  it('o acesso dedicado vale pela chave guardada, sem sessão', () => {
    const dedicated = credential({
      storage: 'key',
      username: 'netmonitor',
      hasStoredSecret: true,
      sessionActive: false,
      dedicated: {
        platform: 'mikrotik',
        publicKey: 'ssh-ed25519 AAAA netmonitor-0a1b',
        keyFingerprint: 'SHA256:chave',
        keyComment: 'netmonitor-0a1b',
        hostKey: 'SHA256:host',
        provisionedAt: '2026-10-28T12:00:00Z',
      },
    })
    expect(credentialReady(dedicated)).toBe(true)
    expect(isDedicated(dedicated)).toBe(true)
    expect(isDedicated(credential({}))).toBe(false)
    expect(credentialReady(credential({ storage: 'key', hasStoredSecret: false }))).toBe(false)
    expect(credentialSubtitle(dedicated)).toBe(
      'porta 22 · acesso dedicado por chave · a partir da central'
    )
  })

  it('a tela diz o que o acesso dedicado cria em cada sistema', () => {
    expect(dedicatedAccessEffect('mikrotik')).toContain('usuário netmonitor')
    expect(dedicatedAccessEffect('openwrt')).toContain('OpenWrt')
    expect(dedicatedAccessEffect(null)).toContain('MikroTik')
  })

  it('o subtítulo diz porta, senha e por onde sai', () => {
    expect(credentialSubtitle(credential({ viaProbeId: 3 }))).toBe(
      'porta 22 · senha informada (vale 30 min) · via agente #3'
    )
  })

  it('prefere a do próprio tipo, e uma pronta vence uma que pede senha', () => {
    const ssh = credential({ kind: 'ssh' })
    const telnet = credential({ kind: 'telnet', port: 23 })
    const telnetPendente = credential({ kind: 'telnet', sessionActive: false })

    expect(pickShellCredential([telnet, ssh], 'ssh')).toBe(ssh)
    expect(pickShellCredential([ssh, telnet], 'telnet')).toBe(telnet)
    expect(pickShellCredential([ssh, telnetPendente], 'telnet')).toBe(ssh)
    expect(pickShellCredential([telnetPendente], 'mactelnet')).toBe(telnetPendente)
  })

  it('HTTP não abre shell e não serve para ativar o log', () => {
    expect(pickShellCredential([credential({ kind: 'http', port: 80 })], 'ssh')).toBeNull()
  })

  it('acesso pronto pelo tipo: HTTP sem login vale; SSH/Telnet precisam de senha', () => {
    expect(accessReady([], 'http')).toBe(true)
    expect(accessReady([], 'ssh')).toBe(false)
    expect(accessReady([credential({})], 'ssh')).toBe(true)
    expect(accessReady([credential({ sessionActive: false })], 'ssh')).toBe(false)
    expect(accessReady([credential({ kind: 'telnet' })], 'ssh')).toBe(false)
    expect(accessTitle('http')).toBe('Acesso HTTP (interface web)')
    expect(accessNeededReason('AP-terraço', 'ssh')).toBe(
      'Para gerenciar AP-terraço o NetMonitor precisa do acesso SSH dele.'
    )
  })

  it('reconhece "falta o acesso" só pelo código do backend', () => {
    const missing = new ApiError('Cadastre a credencial SSH', 409, {
      message: 'Cadastre a credencial SSH',
      code: 'missing_credential',
      deviceId: 7,
      transport: 'ssh',
    })
    expect(missingAccessOf(missing)).toEqual({ deviceId: 7, transport: 'ssh' })
    // Mesmo texto, sem o código: não é reconhecido (nada de casar texto).
    expect(missingAccessOf(new ApiError('Cadastre a credencial SSH', 409, {}))).toBeNull()
    expect(missingAccessOf(new Error('x'))).toBeNull()
    expect(
      missingAccessOf(
        new ApiError('x', 409, { code: 'missing_credential', deviceId: 7, transport: 'ftp' })
      )
    ).toBeNull()
    expect(
      missingAccessIn({ error: 'x', missingAccess: { deviceId: 3, transport: 'telnet' } })
    ).toEqual({ deviceId: 3, transport: 'telnet' })
    expect(missingAccessIn({ error: 'x' })).toBeNull()
  })
})
