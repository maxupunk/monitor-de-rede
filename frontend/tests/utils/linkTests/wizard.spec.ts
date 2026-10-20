import { describe, expect, it } from 'vitest'
import {
  READ_ONLY_TEST_REASON,
  draftFromRequest,
  issueHint,
  linkOptionSubtitle,
  planEndpoints,
  requestFromDraft,
  runBlockers,
  targetErrors,
  fixKey,
  fixLabel,
  legNeedsSteps,
  legOpenIssues,
  installPackageConfirmText,
  packageInstallResult,
  packagesText,
} from '@/utils/linkTests/wizard'
import { accessFix, endpoint, installFix, packageFix, plan } from './fixtures'

describe('assistente do teste de enlace', () => {
  it('rascunho a partir do pedido do mapa (ou vazio: ponta a ponta)', () => {
    expect(draftFromRequest(null)).toEqual({
      kind: 'end_to_end',
      linkId: null,
      sourceDeviceId: null,
      targetDeviceId: null,
    })
    expect(draftFromRequest({ kind: 'server', targetDeviceId: 7 })).toMatchObject({
      kind: 'server',
      targetDeviceId: 7,
    })
  })

  it('valida o alvo por tipo', () => {
    expect(targetErrors(draftFromRequest({ kind: 'link' }))).toEqual(['Escolha o enlace a medir.'])
    expect(targetErrors(draftFromRequest({ kind: 'link', linkId: 4 }))).toEqual([])
    // Destino depois de um switch sem IP: as duas pontas, sem enlace.
    expect(targetErrors(draftFromRequest({ kind: 'link', sourceDeviceId: 3 }))).toEqual([
      'Escolha o enlace a medir.',
    ])
    expect(
      targetErrors(draftFromRequest({ kind: 'link', sourceDeviceId: 3, targetDeviceId: 9 }))
    ).toEqual([])
    expect(
      requestFromDraft({ kind: 'link', linkId: null, sourceDeviceId: 3, targetDeviceId: 9 })
    ).toMatchObject({ kind: 'link', sourceDeviceId: 3, targetDeviceId: 9 })
    expect(
      requestFromDraft({ kind: 'link', linkId: null, sourceDeviceId: 3, targetDeviceId: 9 })
    ).not.toHaveProperty('linkId')
    expect(targetErrors(draftFromRequest({ kind: 'path', sourceDeviceId: 3 }))).toEqual([
      'Escolha até onde medir.',
    ])
    expect(targetErrors(draftFromRequest({ kind: 'end_to_end', targetDeviceId: 4 }))).toEqual([
      'Escolha de onde medir.',
    ])
    expect(
      requestFromDraft({ kind: 'end_to_end', linkId: null, sourceDeviceId: 3, targetDeviceId: 9 })
    ).toMatchObject({ kind: 'end_to_end', sourceDeviceId: 3, targetDeviceId: 9 })
    expect(
      targetErrors(draftFromRequest({ kind: 'path', sourceDeviceId: 3, targetDeviceId: 3 }))
    ).toEqual(['Origem e destino precisam ser equipamentos diferentes.'])
    expect(targetErrors(draftFromRequest({ kind: 'server' }))).toEqual(['Escolha até onde medir.'])
  })

  it('pedido só com os campos do tipo e a especificação completa', () => {
    const server = requestFromDraft(
      { kind: 'server', linkId: 9, sourceDeviceId: 1, targetDeviceId: 7 },
      { durationS: 10 }
    )
    expect(server).toEqual({
      kind: 'server',
      targetDeviceId: 7,
      spec: { durationS: 10, direction: 'both', streams: 1, limitMbps: null },
    })
    const link = requestFromDraft(
      { kind: 'link', linkId: 4, sourceDeviceId: 1, targetDeviceId: 2 },
      undefined,
      true
    )
    expect(link).toMatchObject({ kind: 'link', linkId: 4, sourceDeviceId: 1, confirm: true })
  })

  it('o que impede rodar: perfil, bloqueios e plano sem perna executável', () => {
    expect(runBlockers(null, true)).toEqual(['Calcule o plano antes de iniciar.'])
    expect(runBlockers(plan(), true)).toEqual([])
    expect(runBlockers(plan(), false)).toEqual([READ_ONLY_TEST_REASON])
    expect(runBlockers(plan({ blocking: ['Falta a credencial SSH de Dev-1'] }), true)).toEqual([
      'Falta a credencial SSH de Dev-1',
    ])
    const stuck = plan()
    stuck.legs[0].runnable = false
    expect(runBlockers(stuck, true)[0]).toContain('Nenhuma perna')
  })

  it('dica do problema sem passo aponta para a ficha do equipamento citado', () => {
    const ends = [endpoint(1, 'OpenWrt-Sala'), endpoint(2, 'MikroTik-Torre')]
    expect(issueHint('MikroTik-Torre não tem IP cadastrado', ends)).toEqual({
      text: 'MikroTik-Torre não tem IP cadastrado',
      to: '/devices/2',
      actionLabel: 'Abrir equipamento',
    })
    expect(issueHint('Sem caminho conhecido', ends).to).toBeNull()
    expect(planEndpoints(plan())).toHaveLength(2)
  })

  it('passos que faltam: rótulo, identidade e a perna que só espera por eles', () => {
    const install = installFix()
    const access = accessFix()
    expect(fixLabel(install)).toBe('Instalar o plugin «OpenWrt — teste de enlace» em AP-terraço')
    expect(fixLabel(access)).toBe('Cadastrar o acesso SSH de AP-terraço')
    expect(fixKey(install)).toBe('install:7:30')
    expect(fixKey(access)).toBe('access:7:ssh')

    const waiting = plan({ fixes: [install, access] })
    waiting.legs[0].runnable = false
    waiting.legs[0].issues = [install.message, access.message]
    waiting.legs[0].fixes = [install, access]
    expect(legOpenIssues(waiting.legs[0])).toEqual([])
    expect(legNeedsSteps(waiting.legs[0])).toBe(true)
    expect(runBlockers(waiting, true)).toEqual([
      'Faça o que falta (no topo do plano) para liberar o teste.',
    ])

    waiting.legs[0].issues.push('AP-terraço não tem IP cadastrado')
    expect(legOpenIssues(waiting.legs[0])).toEqual(['AP-terraço não tem IP cadastrado'])
    expect(legNeedsSteps(waiting.legs[0])).toBe(false)
  })

  it('pacote que falta: rótulo, identidade e o que a confirmação explica', () => {
    const fix = packageFix()
    expect(fixLabel(fix)).toBe('Instalar o iperf3 em AP-terraço')
    expect(fixKey(fix)).toBe('package:7:iperf3')
    expect(packagesText({ packages: ['iperf3', 'mikrotik-btest', 'x'], engine: 'iperf3' })).toBe(
      'iperf3, mikrotik-btest e x'
    )
    expect(packagesText({ packages: [], engine: 'btest' })).toBe('btest')
    const text = installPackageConfirmText(fix)
    expect(text).toContain('Instala o pacote iperf3 em AP-terraço')
    expect(text).toContain('gerenciador de pacotes')
    expect(text).toContain('não altera a configuração')

    const waiting = plan({ fixes: [fix] })
    waiting.legs[0].runnable = false
    waiting.legs[0].issues = [fix.message]
    waiting.legs[0].fixes = [fix]
    expect(legNeedsSteps(waiting.legs[0])).toBe(true)
  })

  it('desfecho da instalação: pronto (com aviso), falha da execução ou "não pronto" com o motivo', () => {
    expect(
      packageInstallResult({ status: 'succeeded', output: { ready: true }, error: null })
    ).toEqual({ ok: true, message: null, warning: null })
    expect(
      packageInstallResult({
        status: 'succeeded',
        output: { ready: true, warning: ' Recomendo acertar o NTP do roteador. ' },
        error: null,
      })
    ).toEqual({ ok: true, message: null, warning: 'Recomendo acertar o NTP do roteador.' })
    expect(
      packageInstallResult({
        status: 'succeeded',
        output: { ready: false, message: ' Sem internet no roteador ' },
        error: null,
      })
    ).toEqual({ ok: false, message: 'Sem internet no roteador', warning: null })
    expect(
      packageInstallResult({ status: 'failed', output: null, error: 'Unknown package' })
    ).toEqual({ ok: false, message: 'Unknown package', warning: null })
    expect(packageInstallResult({ status: 'cancelled', output: null, error: null }).ok).toBe(false)
  })
})

describe('enlace na lista do assistente', () => {
  it('portas e velocidade quando o mapa sabe; enlace manual não vira "? ↔ ?"', () => {
    expect(linkOptionSubtitle('ether1', 'eth0', '1 Gbps')).toBe('ether1 ↔ eth0 · 1 Gbps')
    expect(linkOptionSubtitle('ether1', null, null)).toBe('ether1 ↔ ?')
    expect(linkOptionSubtitle(null, null, '100 Mbps')).toBe('100 Mbps')
    expect(linkOptionSubtitle(null, undefined, null)).toBe('Portas não identificadas')
  })
})
