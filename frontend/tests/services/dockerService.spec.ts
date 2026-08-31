import { beforeEach, describe, expect, it, vi } from 'vitest'
import { dockerService } from '@/services/dockerService'
import { apiService } from '@/services/apiService'

describe('dockerService', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  it('codifica ids Docker antes de montar a rota', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue({})

    await dockerService.container('sha256:abc/def')

    expect(get).toHaveBeenCalledWith('/docker/containers/sha256%3Aabc%2Fdef')
  })

  it('serializa filtros de log no contrato do backend', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue([])

    await dockerService.logs('abc', { tail: 500, since: 10, until: 20, timestamps: true })

    expect(get).toHaveBeenCalledWith(
      '/docker/containers/abc/logs?tail=500&since=10&until=20&timestamps=true'
    )
  })

  it('envia force explicitamente nas remoções', async () => {
    const remove = vi.spyOn(apiService, 'delete').mockResolvedValue({ success: true, message: '' })

    await dockerService.removeContainer('abc', true)

    expect(remove).toHaveBeenCalledWith('/docker/containers/abc?force=true')
  })
})
