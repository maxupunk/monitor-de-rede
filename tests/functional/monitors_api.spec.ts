import { test } from '@japa/runner'
import testUtils from '@adonisjs/core/services/test_utils'
import Site from '#models/site'
import Device from '#models/device'
import Monitor from '#models/monitor'
import MonitorResult from '#models/monitor_result'
import { DateTime } from 'luxon'

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

    const response = await client.post(`/api/monitors/${monitor.id}/run`).timeout(5000)

    response.assertStatus(200)
    assert.exists(response.body().result)
    assert.exists(response.body().result.startedAt)

    await monitor.refresh()
    assert.notEqual(monitor.status, 'unknown')

    await device.refresh()
    assert.notEqual(device.status, 'unknown')
  })

  test('GET /api/monitors e GET /api/monitors/:id devem retornar estatísticas de latência e recentResults', async ({ client, assert }) => {
    const site = await Site.create({ name: 'Site Teste', active: true })
    const device = await Device.create({ siteId: site.id, name: 'Servidor', type: 'server', status: 'online' })

    const monitor = await Monitor.create({
      deviceId: device.id,
      type: 'ping',
      name: 'Ping Servidor',
      configuration: { host: '127.0.0.1' },
      intervalSeconds: 60,
      timeoutSeconds: 5,
      enabled: true,
      status: 'up',
    })

    const now = DateTime.now()
    await MonitorResult.create({
      monitorId: monitor.id,
      status: 'up',
      startedAt: now.minus({ minutes: 5 }),
      finishedAt: now.minus({ minutes: 5 }),
      durationMs: 15,
      latencyMs: 10,
      message: 'OK',
    })

    await MonitorResult.create({
      monitorId: monitor.id,
      status: 'up',
      startedAt: now.minus({ minutes: 2 }),
      finishedAt: now.minus({ minutes: 2 }),
      durationMs: 25,
      latencyMs: 30,
      message: 'OK',
    })

    const indexRes = await client.get('/api/monitors')
    indexRes.assertStatus(200)
    assert.isArray(indexRes.body())
    assert.isTrue(indexRes.body().length > 0)
    assert.exists(indexRes.body()[0].recentResults)
    assert.equal(indexRes.body()[0].recentResults.length, 2)

    const showRes = await client.get(`/api/monitors/${monitor.id}`)
    showRes.assertStatus(200)
    assert.exists(showRes.body().stats)
    assert.equal(showRes.body().stats.avgLatency, 20)
    assert.equal(showRes.body().stats.minLatency, 10)
    assert.equal(showRes.body().stats.maxLatency, 30)
    assert.equal(showRes.body().stats.lastLatency, 30)
    assert.equal(showRes.body().stats.uptimePercentage, 100)
    assert.equal(showRes.body().stats.totalChecks, 2)
  })
})
