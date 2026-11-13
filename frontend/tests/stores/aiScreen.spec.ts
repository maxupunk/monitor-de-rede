import { beforeEach, describe, expect, it } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { useAiScreenStore } from '@/stores/aiScreen'

describe('aiScreen store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('a página que sai não apaga o detalhe da que entrou', () => {
    const store = useAiScreenStore()
    const logs = store.claimDetail()
    store.setDetail(logs, 'erro e acima')
    expect(store.detail).toBe('erro e acima')

    const docker = store.claimDetail()
    store.setDetail(logs, 'atrasado')
    expect(store.detail).toBeNull()
    store.setDetail(docker, 'host central')
    store.releaseDetail(logs)
    expect(store.detail).toBe('host central')
    store.releaseDetail(docker)
    expect(store.detail).toBeNull()
  })

  it('dispensar vale só para a tela dispensada', () => {
    const store = useAiScreenStore()
    store.dismiss('logs')
    expect(store.isDismissed('logs')).toBe(true)
    expect(store.isDismissed('docker')).toBe(false)
    store.restore()
    expect(store.isDismissed('logs')).toBe(false)
  })
})
