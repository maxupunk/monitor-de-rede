import { describe, expect, it } from 'vitest'
import type { RegistryScanSummary } from '@/bindings/RegistryScanSummary'
import {
  daemonJson,
  dockerCommands,
  isCommitTag,
  registryAccess,
  registryHost,
  scanColor,
  scanSentence,
  shortDigest,
  shortTag,
} from '@/utils/registryPresentation'

function scan(partial: Partial<RegistryScanSummary>): RegistryScanSummary {
  return {
    status: 'done',
    critical: 0,
    high: 0,
    medium: 0,
    low: 0,
    unknown: 0,
    packages: 10,
    source: 'sbom',
    scannedAt: null,
    ...partial,
  }
}

describe('registryPresentation', () => {
  it('monta o endereço e os comandos do docker', () => {
    const host = registryHost({ hostname: '10.8.0.1', port: '3333' })
    expect(host).toBe('10.8.0.1:3333')
    expect(registryHost({ hostname: 'central.local', port: '' })).toBe('central.local')
    expect(JSON.parse(daemonJson(host))).toEqual({ 'insecure-registries': ['10.8.0.1:3333'] })
    const commands = dockerCommands(host, 'loja/api', 'v1')
    expect(commands.login).toBe('docker login 10.8.0.1:3333 -u token')
    expect(commands.pull).toBe('docker pull 10.8.0.1:3333/loja/api:v1')
    expect(commands.push).toContain('docker push 10.8.0.1:3333/loja/api:v1')
  })

  it('diz se o acesso é interno e se precisa de insecure-registries', () => {
    const vpn = registryAccess({
      hostname: '10.8.0.1',
      port: '3333',
      protocol: 'http:',
      origin: 'http://10.8.0.1:3333',
    })
    expect(vpn).toEqual({ host: '10.8.0.1:3333', secure: false, internal: true })
    const edge = registryAccess({
      hostname: 'monitor.exemplo.com',
      port: '',
      protocol: 'https:',
      origin: 'https://monitor.exemplo.com',
    })
    expect(edge).toEqual({ host: 'monitor.exemplo.com', secure: true, internal: false })
  })

  it('abrevia tag de commit e digest como o backend', () => {
    const sha = 'a'.repeat(40)
    expect(isCommitTag(sha)).toBe(true)
    expect(isCommitTag('latest')).toBe(false)
    expect(shortTag(sha)).toBe('aaaaaaa')
    expect(shortTag('v1.2')).toBe('v1.2')
    expect(shortDigest(`sha256:${'b'.repeat(64)}`)).toBe(`sha256:${'b'.repeat(12)}`)
  })

  it('resume a análise em uma frase e uma cor', () => {
    expect(scanSentence(null)).toBe('Ainda não analisada')
    expect(scanSentence(scan({}))).toBe('Nenhuma vulnerabilidade conhecida')
    expect(scanSentence(scan({ critical: 1, high: 2 }))).toBe('1 crítica(s) e 2 alta(s)')
    expect(scanSentence(scan({ status: 'no_sbom' }))).toContain('Sem lista')
    expect(scanColor(scan({ critical: 1 }))).toBe('error')
    expect(scanColor(scan({ high: 1 }))).toBe('deep-orange')
    expect(scanColor(scan({}))).toBe('success')
    expect(scanColor(scan({ status: 'pending' }))).toBe('secondary')
  })
})
