import { describe, expect, it } from 'vitest'
import type { CredentialView } from '@/bindings/CredentialView'
import {
  credentialKindFor,
  credentialReady,
  credentialSubtitle,
  pickShellCredential,
} from '@/utils/deviceCredentials'

function credential(overrides: Partial<CredentialView>): CredentialView {
  return {
    kind: 'ssh',
    username: 'admin',
    storage: 'vault',
    port: 22,
    https: false,
    viaProbeId: null,
    hasStoredSecret: true,
    sessionActive: false,
    ...overrides,
  }
}

describe('credenciais do equipamento', () => {
  it('o MAC-Telnet entra com o login do Telnet, como no backend', () => {
    expect(credentialKindFor('ssh')).toBe('ssh')
    expect(credentialKindFor('telnet')).toBe('telnet')
    expect(credentialKindFor('mactelnet')).toBe('telnet')
  })

  it('pronta é a guardada com senha ou a de sessão já informada', () => {
    expect(credentialReady(credential({}))).toBe(true)
    expect(credentialReady(credential({ hasStoredSecret: false }))).toBe(false)
    expect(credentialReady(credential({ storage: 'ask', hasStoredSecret: false }))).toBe(false)
    expect(credentialReady(credential({ storage: 'ask', sessionActive: true }))).toBe(true)
  })

  it('o subtítulo diz porta, senha e por onde sai', () => {
    expect(credentialSubtitle(credential({ viaProbeId: 3 }))).toBe(
      'porta 22 · senha guardada cifrada · via agente #3'
    )
  })

  it('prefere a do próprio tipo, e uma pronta vence uma que pede senha', () => {
    const ssh = credential({ kind: 'ssh' })
    const telnet = credential({ kind: 'telnet', port: 23 })
    const telnetPendente = credential({ kind: 'telnet', storage: 'ask', hasStoredSecret: false })

    expect(pickShellCredential([telnet, ssh], 'ssh')).toBe(ssh)
    expect(pickShellCredential([ssh, telnet], 'telnet')).toBe(telnet)
    expect(pickShellCredential([ssh, telnetPendente], 'telnet')).toBe(ssh)
    expect(pickShellCredential([telnetPendente], 'mactelnet')).toBe(telnetPendente)
  })

  it('HTTP não abre shell e não serve para ativar o log', () => {
    expect(pickShellCredential([credential({ kind: 'http', port: 80 })], 'ssh')).toBeNull()
  })
})
