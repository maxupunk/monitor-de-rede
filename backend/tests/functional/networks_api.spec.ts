import { test } from '@japa/runner'
import { DateTime } from 'luxon'
import testUtils from '@adonisjs/core/services/test_utils'
import Site from '#models/site'
import Network from '#models/network'
import DiscoveryRun from '#models/discovery_run'

test.group('Networks API - Varredura por bloco de IP', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  async function createNetwork(cidr: string, name = 'LAN Matriz') {
    const site = await Site.create({ name: 'Matriz', active: true })
    return Network.create({
      siteId: site.id,
      name,
      cidr,
      scanEnabled: false,
      scanInterval: 3600,
      active: true,
    })
  }

  test('POST /api/networks/:id/scan deve enfileirar a varredura sem executá-la na request', async ({
    client,
    assert,
  }) => {
    const network = await createNetwork('192.168.50.0/24')

    const response = await client.post(`/api/networks/${network.id}/scan`)

    // 202: o processo HTTP não varre — quem executa é o scheduler
    response.assertStatus(202)
    const body = response.body() as {
      alreadyQueued: boolean
      usableHosts: number
      run: { id: number; status: string }
    }
    assert.isFalse(body.alreadyQueued)
    assert.equal(body.usableHosts, 254)

    const run = await DiscoveryRun.findOrFail(body.run.id)
    assert.equal(run.networkId, network.id)
    assert.equal(run.status, 'pending')
    assert.equal(run.configuration?.cidr, '192.168.50.0/24')
  })

  test('varredura repetida na mesma rede deve reaproveitar a execução pendente', async ({
    client,
    assert,
  }) => {
    const network = await createNetwork('10.20.30.0/24')

    const first = await client.post(`/api/networks/${network.id}/scan`)
    const second = await client.post(`/api/networks/${network.id}/scan`)

    first.assertStatus(202)
    second.assertStatus(202)

    const firstBody = first.body() as { run: { id: number } }
    const secondBody = second.body() as { alreadyQueued: boolean; run: { id: number } }

    assert.isTrue(secondBody.alreadyQueued)
    assert.equal(secondBody.run.id, firstBody.run.id)

    const runs = await DiscoveryRun.query().where('networkId', network.id)
    assert.lengthOf(runs, 1)
  })

  test('rede com faixa CIDR inválida deve ser recusada com 422', async ({ client, assert }) => {
    const network = await createNetwork('rede-do-escritorio')

    const response = await client.post(`/api/networks/${network.id}/scan`)

    response.assertStatus(422)

    const runs = await DiscoveryRun.query().where('networkId', network.id)
    assert.lengthOf(runs, 0)
  })

  test('PUT /api/networks/:id deve devolver o mesmo formato do índice', async ({
    client,
    assert,
  }) => {
    // A store do frontend substitui a linha da tabela pela resposta do PUT. Se
    // ela vier sem os campos derivados, a rede recém-editada passa a exibir
    // "faixa inválida" e "Site #undefined" até um novo GET.
    const network = await createNetwork('10.0.0.0/20')

    const response = await client
      .put(`/api/networks/${network.id}`)
      .json({ cidr: '10.0.0.0/24', name: 'LAN Matriz', gateway: '10.0.0.1' })

    response.assertStatus(200)
    const body = response.body() as {
      cidr: string
      scannable: boolean
      usableHosts: number
      site: { name: string } | null
    }

    assert.equal(body.cidr, '10.0.0.0/24')
    assert.isTrue(body.scannable)
    assert.equal(body.usableHosts, 254)
    assert.equal(body.site?.name, 'Matriz')
  })

  test('POST /api/networks deve devolver os campos derivados e aceitar rede sem site', async ({
    client,
    assert,
  }) => {
    const response = await client
      .post('/api/networks')
      .json({ name: 'Sem local', cidr: '172.16.0.0/24' })

    response.assertStatus(201)
    const body = response.body() as {
      siteId: number | null
      scannable: boolean
      usableHosts: number
      site: unknown
    }

    assert.isNotOk(body.siteId)
    assert.isTrue(body.scannable)
    assert.equal(body.usableHosts, 254)
    assert.isNull(body.site)
  })

  test('corrigir o CIDR deve atualizar a varredura pendente em vez de manter a faixa antiga', async ({
    client,
    assert,
  }) => {
    const network = await createNetwork('10.0.0.0/20')

    const first = await client.post(`/api/networks/${network.id}/scan`)
    const runId = (first.body() as { run: { id: number } }).run.id
    const queued = await DiscoveryRun.findOrFail(runId)
    assert.equal(queued.configuration?.cidr, '10.0.0.0/20')

    await client.put(`/api/networks/${network.id}`).json({ cidr: '10.0.0.0/24' })
    const second = await client.post(`/api/networks/${network.id}/scan`)

    const body = second.body() as { alreadyQueued: boolean; usableHosts: number }
    assert.isTrue(body.alreadyQueued)
    assert.equal(body.usableHosts, 254)

    // A run reaproveitada precisa apontar para a faixa corrigida.
    const run = await DiscoveryRun.findOrFail(runId)
    assert.equal(run.configuration?.cidr, '10.0.0.0/24')
    assert.equal(run.configuration?.usableHosts, 254)
  })

  test('varredura abandonada não deve bloquear novos pedidos da mesma rede', async ({
    client,
    assert,
  }) => {
    const network = await createNetwork('192.168.90.0/24')

    // Simula o scan ao vivo de /discovery cujo processo morreu no meio.
    const stuck = await DiscoveryRun.create({
      networkId: network.id,
      status: 'running',
      startedAt: DateTime.now().minus({ hours: 2 }),
      configuration: { cidr: network.cidr },
    })

    const response = await client.post(`/api/networks/${network.id}/scan`)
    response.assertStatus(202)

    const body = response.body() as { alreadyQueued: boolean; run: { id: number } }
    assert.isFalse(body.alreadyQueued)
    assert.notEqual(body.run.id, stuck.id)

    await stuck.refresh()
    assert.equal(stuck.status, 'failed')
  })

  test('GET /api/networks deve informar se a faixa é varredurável', async ({ client, assert }) => {
    await createNetwork('192.168.1.0/24', 'Valida')
    await createNetwork('sem-cidr', 'Invalida')

    const response = await client.get('/api/networks')
    response.assertStatus(200)

    const networks = response.body() as Array<{
      name: string
      scannable: boolean
      usableHosts: number
    }>

    const valid = networks.find((net) => net.name === 'Valida')!
    const invalid = networks.find((net) => net.name === 'Invalida')!

    assert.isTrue(valid.scannable)
    assert.equal(valid.usableHosts, 254)
    assert.isFalse(invalid.scannable)
    assert.equal(invalid.usableHosts, 0)
  })
})
