import { test } from '@japa/runner'
import testUtils from '@adonisjs/core/services/test_utils'
import Site from '#models/site'
import Device from '#models/device'
import DeviceLink from '#models/device_link'

test.group('Topology & SNMP API - Functional Tests', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  test('GET /api/topology deve retornar o grafo de nós e arestas', async ({ client, assert }) => {
    const site = await Site.create({ name: 'Datacenter SP', active: true })
    const dev1 = await Device.create({ siteId: site.id, name: 'R1-Core', type: 'router', status: 'online' })
    const dev2 = await Device.create({ siteId: site.id, name: 'SW1-Access', type: 'switch', status: 'online' })

    await DeviceLink.create({
      sourceDeviceId: dev1.id,
      targetDeviceId: dev2.id,
      linkType: 'lldp',
      discoveryMethod: 'snmp_lldp',
      confidence: 95,
      confirmed: true,
    })

    const response = await client.get('/api/topology')

    response.assertStatus(200)
    assert.exists(response.body().nodes)
    assert.exists(response.body().edges)
    assert.lengthOf(response.body().nodes, 2)
    assert.lengthOf(response.body().edges, 1)
  })

  test('POST /api/topology/links deve criar uma nova ligação manual', async ({ client, assert }) => {
    const site = await Site.create({ name: 'Matriz', active: true })
    const dev1 = await Device.create({ siteId: site.id, name: 'Router-A', type: 'router', status: 'online' })
    const dev2 = await Device.create({ siteId: site.id, name: 'Router-B', type: 'router', status: 'online' })

    const response = await client.post('/api/topology/links').json({
      source_device_id: dev1.id,
      target_device_id: dev2.id,
    })

    response.assertStatus(201)
    assert.equal(response.body().sourceDeviceId, dev1.id)
    assert.equal(response.body().targetDeviceId, dev2.id)
    assert.equal(response.body().linkType, 'manual')

    const dbLink = await DeviceLink.find(response.body().id)
    assert.exists(dbLink)
  })

  test('DELETE /api/topology/links/:id deve remover uma ligação existente', async ({ client, assert }) => {
    const site = await Site.create({ name: 'Filial', active: true })
    const dev1 = await Device.create({ siteId: site.id, name: 'Dev-1', type: 'generic', status: 'online' })
    const dev2 = await Device.create({ siteId: site.id, name: 'Dev-2', type: 'generic', status: 'online' })

    const link = await DeviceLink.create({
      sourceDeviceId: dev1.id,
      targetDeviceId: dev2.id,
      linkType: 'manual',
      discoveryMethod: 'user_defined',
      confidence: 100,
      confirmed: true,
    })

    const response = await client.delete(`/api/topology/links/${link.id}`)

    response.assertStatus(200)
    const deleted = await DeviceLink.find(link.id)
    assert.isNull(deleted)
  })

  test('POST /api/topology/recalculate deve executar a inferência e retornar contagem', async ({ client, assert }) => {
    const response = await client.post('/api/topology/recalculate')

    response.assertStatus(200)
    assert.exists(response.body().inferredCount)
  })

  test('POST /api/devices/:id/snmp/poll deve executar varredura SNMP mockada', async ({ client, assert }) => {
    const site = await Site.create({ name: 'Lab', active: true })
    const dev = await Device.create({ siteId: site.id, name: 'MockSwitch', type: 'switch', status: 'unknown' })

    const response = await client.post(`/api/devices/${dev.id}/snmp/poll`).json({
      community: 'public',
      version: 'v2c',
    })

    response.assertStatus(200)
    assert.equal(response.body().message, 'Varredura SNMP executada com sucesso')

    await dev.refresh()
    assert.equal(dev.status, 'online')
  })
})
