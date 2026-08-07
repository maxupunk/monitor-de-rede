import { test } from '@japa/runner'
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
