import { describe, expect, it, vi } from 'vitest'
import { nextTick, ref } from 'vue'
import { useVerifiedSave } from '@/composables/useVerifiedSave'

function setup(results: Array<{ ok: boolean } | Error>) {
  const host = ref('nas')
  const test = vi.fn(async () => {
    const next = results.shift()
    if (next instanceof Error) throw next
    return next ?? { ok: true }
  })
  const verified = useVerifiedSave({
    signature: () => host.value,
    test,
    succeeded: (result: { ok: boolean }) => result.ok,
  })
  return { host, test, verified }
}

describe('testar antes de salvar', () => {
  it('salva sem testar de novo quando o teste ainda vale', async () => {
    const { test, verified } = setup([{ ok: true }])
    await verified.run()
    expect((await verified.ready()).ok).toBe(true)
    expect(test).toHaveBeenCalledTimes(1)
  })

  it('mudar a conexão invalida o teste', async () => {
    const { host, verified } = setup([{ ok: true }])
    await verified.run()
    host.value = 'outro-nas'
    await nextTick()
    expect(verified.result.value).toBeNull()
  })

  it('falha arma o "salvar mesmo assim", e o segundo clique salva sem testar', async () => {
    const { test, verified } = setup([{ ok: false }])
    expect((await verified.ready()).ok).toBe(false)
    expect(verified.forceSave.value).toBe(true)
    expect((await verified.ready()).ok).toBe(true)
    expect(test).toHaveBeenCalledTimes(1)
  })

  it('erro da requisição vira mensagem e também arma o "salvar mesmo assim"', async () => {
    const { verified } = setup([new Error('Não foi possível conectar: recusado')])
    expect((await verified.ready()).ok).toBe(false)
    expect(verified.error.value).toContain('recusado')
    expect(verified.forceSave.value).toBe(true)
  })
})
