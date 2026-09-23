import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createDockerService, dockerHostsService, dockerService } from '@/services/dockerService'
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

  it('limpa o log real pela rota administrativa do container', async () => {
    const remove = vi.spyOn(apiService, 'delete').mockResolvedValue({ success: true, message: '' })

    await dockerService.clearLogs('sha256:abc/def')

    expect(remove).toHaveBeenCalledWith('/docker/containers/sha256%3Aabc%2Fdef/logs')
  })

  it('envia force explicitamente nas remoções', async () => {
    const remove = vi.spyOn(apiService, 'delete').mockResolvedValue({ success: true, message: '' })

    await dockerService.removeContainer('abc', true)

    expect(remove).toHaveBeenCalledWith('/docker/containers/abc?force=true')
  })

  it('mantém as rotas históricas no host local e prefixa os hosts remotos', async () => {
    const get = vi.spyOn(apiService, 'get').mockResolvedValue({})

    await dockerService.status()
    await createDockerService('agent-3').status()
    await createDockerService('agent-3').history('7d')

    expect(get).toHaveBeenNthCalledWith(1, '/docker/status')
    expect(get).toHaveBeenNthCalledWith(2, '/docker/hosts/agent-3/status')
    expect(get).toHaveBeenNthCalledWith(3, '/docker/hosts/agent-3/history?range=7d')
  })

  it('operações longas e compose vão para o host certo', async () => {
    const post = vi.spyOn(apiService, 'post').mockResolvedValue({})
    const remote = createDockerService('agent-3')

    await remote.updateContainer('web')
    await remote.pullImage('nginx:alpine')
    await remote.composeAction('portal', 'up', 'web')
    await remote.followLogs('web', 'all')

    expect(post).toHaveBeenNthCalledWith(1, '/docker/hosts/agent-3/containers/web/update')
    expect(post).toHaveBeenNthCalledWith(2, '/docker/hosts/agent-3/images/pull', {
      image: 'nginx:alpine',
    })
    expect(post).toHaveBeenNthCalledWith(3, '/docker/hosts/agent-3/compose/portal', {
      action: 'up',
      service: 'web',
    })
    expect(post).toHaveBeenNthCalledWith(4, '/docker/hosts/agent-3/containers/web/logs/follow', {
      tail: 'all',
    })
  })

  it('encerra o stream de logs fora do escopo de host', async () => {
    const remove = vi.spyOn(apiService, 'delete').mockResolvedValue({ stopped: true })

    await dockerHostsService.stopLogStream('abc/def')

    expect(remove).toHaveBeenCalledWith('/docker/log-streams/abc%2Fdef')
  })
})
