import { test } from '@japa/runner'
import testUtils from '@adonisjs/core/services/test_utils'
import Site from '#models/site'
import Device from '#models/device'
import Monitor from '#models/monitor'

test.group('Monitors API - Functional Tests', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  test('POST /api/monitors deve cadastrar um monitor com sucesso usando target/port ou configuration', async ({ client, assert }) => {
    const site = await Site.create({ name: 'Site Teste', active: true })
    const device = await Device.create({ siteId: site.id, name: 'Servidor Web', type: 'server', status: 'unknown' })

    const response = await client.post('/api/monitors').json({
      deviceId: device.id,
      name: 'Check HTTP Google',
      type: 'http',
      target: 'google.com',
      intervalSeconds: 60,
      timeoutSeconds: 5,
    })

    response.assertStatus(201)
    assert.equal(response.body().name, 'Check HTTP Google')
    assert.deepEqual(response.body().configuration, { url: 'http://google.com' })
    assert.equal(response.body().target, 'http://google.com')
    assert.equal(response.body().enabled, true)
    assert.equal(response.body().isEnabled, true)
  })

  test('POST /api/monitors/:id/run deve executar o monitor e retornar o resultado', async ({ client, assert }) => {
    const site = await Site.create({ name: 'Site Teste', active: true })
    const device = await Device.create({ siteId: site.id, name: 'Gateway', type: 'router', status: 'unknown' })

    const monitor = await Monitor.create({
      deviceId: device.id,
      type: 'ping',
      name: 'Ping Gateway',
      configuration: { host: '127.0.0.1' },
      intervalSeconds: 60,
      timeoutSeconds: 5,
      enabled: true,
      status: 'unknown',
    })

    const response = await client.post(`/api/monitors/${monitor.id}/run`)

    response.assertStatus(200)
    assert.exists(response.body().result)
    assert.exists(response.body().result.startedAt)

    await monitor.refresh()
    assert.notEqual(monitor.status, 'unknown')

    await device.refresh()
    assert.notEqual(device.status, 'unknown')
  })
})
