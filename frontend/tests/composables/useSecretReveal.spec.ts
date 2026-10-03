import { describe, expect, it } from 'vitest'
import { useSecretReveal, withoutUnchanged } from '@/composables/useSecretReveal'
import { revealFields } from '@/utils/pluginSettings'

const rede = {
  type: 'object',
  properties: {
    ssid: { type: 'string', title: 'Nome da rede' },
    key: { type: 'string', title: 'Senha', secret: true, reveal: 'reveal_key' },
    outra: { type: 'string', title: 'Outra', reveal: 'reveal_key' },
  },
}
const sala = { deviceId: 7, name: 'AP Sala' }

describe('senha atual ao editar', () => {
  it('só campo secreto com `reveal` é lido do equipamento', () => {
    expect(revealFields(rede)).toEqual([{ name: 'key', action: 'reveal_key' }])
  })

  it('preenche com a senha lida e diz de onde veio', async () => {
    const calls: unknown[] = []
    const secrets = useSecretReveal(async (deviceId, action, params) => {
      calls.push({ deviceId, action, params })
      return { key: 'SenhaDaLoja1' }
    })
    const filled = await secrets.load(rede, { ssid: 'Loja' }, sala)
    expect(filled).toEqual({ key: 'SenhaDaLoja1' })
    expect(calls).toEqual([{ deviceId: 7, action: 'reveal_key', params: { ssid: 'Loja' } }])
    expect(secrets.notes.value.key).toContain('lida agora de AP Sala')
  })

  it('sem senha ou com falha, explica e deixa o campo vazio (que mantém)', async () => {
    const vazia = useSecretReveal(async () => ({ key: '' }))
    expect(await vazia.load(rede, { ssid: 'Aberta' }, sala)).toEqual({})
    expect(vazia.notes.value.key).toContain('não tem senha')

    const falha = useSecretReveal(async () => {
      throw new Error('sem acesso')
    })
    expect(await falha.load(rede, { ssid: 'Loja' }, sala)).toEqual({})
    expect(falha.notes.value.key).toContain('sem acesso')
  })

  it('a senha que voltaria igual não vai: não é troca de senha', () => {
    const revealed = { key: 'SenhaDaLoja1' }
    expect(withoutUnchanged({ ssid: 'Loja', key: 'SenhaDaLoja1' }, revealed)).toEqual({
      ssid: 'Loja',
    })
    expect(withoutUnchanged({ ssid: 'Loja', key: 'NovaSenha99' }, revealed)).toEqual({
      ssid: 'Loja',
      key: 'NovaSenha99',
    })
  })
})
