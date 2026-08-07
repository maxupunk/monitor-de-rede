import { test } from '@japa/runner'
import testUtils from '@adonisjs/core/services/test_utils'
import Site from '#models/site'
import Device from '#models/device'
import Monitor from '#models/monitor'
import MonitorResult from '#models/monitor_result'
import { DateTime } from 'luxon'

test.group('Monitors API - Functional Tests', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  test('POST /api/monitors deve cadastrar um monitor com sucesso usando target/port ou configuration', async ({
    client,
    assert,
  }) => {
    const site = await Site.create({ name: 'Site Teste', active: true })
    const device = await Device.create({
      siteId: site.id,
      name: 'Servidor Web',
      type: 'server',
      status: 'unknown',
    })

    const response = await client.visit('monitors.store').json({
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

  test('POST /api/monitors/:id/run deve executar o monitor e retornar o resultado', async ({
    client,
    assert,
  }) => {
    const site = await Site.create({ name: 'Site Teste', active: true })
    const device = await Device.create({
      siteId: site.id,
      name: 'Gateway',
      type: 'router',
      status: 'unknown',
    })

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

    const response = await client.visit('monitors.run', { id: monitor.id }).timeout(5000)

    response.assertStatus(200)
    assert.exists(response.body().result)
    assert.exists(response.body().result.startedAt)

    await monitor.refresh()
    assert.notEqual(monitor.status, 'unknown')

    await device.refresh()
    assert.notEqual(device.status, 'unknown')
  })

  test('GET /api/monitors e GET /api/monitors/:id devem retornar estatísticas de latência e recentResults', async ({
    client,
    assert,
  }) => {
    const site = await Site.create({ name: 'Site Teste', active: true })
    const device = await Device.create({
      siteId: site.id,
      name: 'Servidor',
      type: 'server',
      status: 'online',
    })

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

    const indexRes = await client.visit('monitors.index')
    indexRes.assertStatus(200)
    assert.isArray(indexRes.body())
    assert.isTrue(indexRes.body().length > 0)
    assert.exists(indexRes.body()[0].recentResults)
    assert.equal(indexRes.body()[0].recentResults.length, 2)

    const showRes = await client.visit('monitors.show', { id: monitor.id })
    showRes.assertStatus(200)
    assert.exists(showRes.body().stats)
    assert.equal(showRes.body().stats.avgLatency, 20)
    assert.equal(showRes.body().stats.minLatency, 10)
    assert.equal(showRes.body().stats.maxLatency, 30)
    assert.equal(showRes.body().stats.lastLatency, 30)
    assert.equal(showRes.body().stats.uptimePercentage, 100)
    assert.equal(showRes.body().stats.totalChecks, 2)
  })

  test('GET /api/devices/:id/monitors deve devolver o mesmo payload de GET /api/monitors', async ({
    client,
    assert,
  }) => {
    const site = await Site.create({ name: 'Site Paridade', active: true })
    const device = await Device.create({
      siteId: site.id,
      name: 'Servidor Paridade',
      type: 'server',
      status: 'online',
    })

    const monitor = await Monitor.create({
      deviceId: device.id,
      type: 'ping',
      name: 'Ping Paridade',
      configuration: { host: '127.0.0.1' },
      intervalSeconds: 60,
      timeoutSeconds: 5,
      enabled: true,
      status: 'up',
    })

    const now = DateTime.now()
    for (let i = 1; i <= 3; i++) {
      await MonitorResult.create({
        monitorId: monitor.id,
        status: 'up',
        startedAt: now.minus({ minutes: 10 - i }),
        finishedAt: now.minus({ minutes: 10 - i }),
        durationMs: 10 + i,
        latencyMs: 5 + i,
        message: `Execucao #${i}`,
      })
    }

    // A aba "Monitores" de /devices/:id usa o mesmo componente de listagem de
    // /monitors: se um dos endpoints parar de enviar histórico ou vínculos, a
    // tela do equipamento perde a linha do tempo em silêncio.
    const deviceRes = await client.get(`/api/devices/${device.id}/monitors`)
    deviceRes.assertStatus(200)

    const listRes = await client.visit('monitors.index')
    listRes.assertStatus(200)

    type ListedMonitor = Record<string, any> & { id: number }
    const fromDevice = (deviceRes.body() as ListedMonitor[])[0]
    const fromList = (listRes.body() as ListedMonitor[]).find((mon) => mon.id === monitor.id)!

    assert.exists(fromDevice)
    assert.exists(fromList)
    assert.deepEqual(Object.keys(fromDevice).sort(), Object.keys(fromList).sort())
    assert.equal(fromDevice.recentResults.length, 3)
    assert.equal(fromDevice.device.id, device.id)
    assert.equal(fromDevice.latencyMs, 8)
    assert.equal(fromDevice.isEnabled, fromList.isEnabled)
    assert.deepEqual(fromDevice.recentResults, fromList.recentResults)
  })

  test('GET /api/monitors/:id/results deve retornar resultados paginados em ordem decrescente', async ({
    client,
    assert,
  }) => {
    const site = await Site.create({ name: 'Site Paginacao', active: true })
    const device = await Device.create({
      siteId: site.id,
      name: 'Servidor Paginacao',
      type: 'server',
      status: 'online',
    })

    const monitor = await Monitor.create({
      deviceId: device.id,
      type: 'ping',
      name: 'Ping Paginacao',
      configuration: { host: '127.0.0.1' },
      intervalSeconds: 60,
      timeoutSeconds: 5,
      enabled: true,
      status: 'up',
    })

    const now = DateTime.now()
    for (let i = 1; i <= 25; i++) {
      await MonitorResult.create({
        monitorId: monitor.id,
        status: 'up',
        startedAt: now.minus({ minutes: 30 - i }),
        finishedAt: now.minus({ minutes: 30 - i }),
        durationMs: 10 + i,
        latencyMs: 5 + i,
        message: `Execucao #${i}`,
      })
    }

    const resPage1 = await client.get(`/api/monitors/${monitor.id}/results?page=1&limit=20`)
    resPage1.assertStatus(200)
    const body1 = resPage1.body() as {
      data: unknown[]
      meta: { currentPage: number; lastPage: number; total: number }
    }
    assert.exists(body1.data)
    assert.equal(body1.data.length, 20)
    assert.equal(body1.meta.currentPage, 1)
    assert.equal(body1.meta.lastPage, 2)
    assert.equal(body1.meta.total, 25)

    const resPage2 = await client.get(`/api/monitors/${monitor.id}/results?page=2&limit=20`)
    resPage2.assertStatus(200)
    const body2 = resPage2.body() as { data: unknown[] }
    assert.equal(body2.data.length, 5)
  })

  test('POST /api/monitors/:id/disable e /enable devem atualizar isEnabled mantendo o status do monitor', async ({
    client,
    assert,
  }) => {
    const site = await Site.create({ name: 'Site Teste', active: true })
    const device = await Device.create({
      siteId: site.id,
      name: 'Servidor 2',
      type: 'server',
      status: 'online',
    })

    const monitor = await Monitor.create({
      deviceId: device.id,
      type: 'ping',
      name: 'Ping Servidor 2',
      configuration: { host: '127.0.0.1' },
      intervalSeconds: 60,
      timeoutSeconds: 5,
      enabled: true,
      status: 'up',
    })

    const disableRes = await client.visit('monitors.disable', { id: monitor.id })
    disableRes.assertStatus(200)
    assert.equal(disableRes.body().isEnabled, false)
    assert.equal(disableRes.body().status, 'up')

    const enableRes = await client.visit('monitors.enable', { id: monitor.id })
    enableRes.assertStatus(200)
    assert.equal(enableRes.body().isEnabled, true)
    assert.equal(enableRes.body().status, 'up')
  })

  test('DELETE /api/monitors/:id deve apagar o monitor e TODO o seu histórico de resultados', async ({
    client,
    assert,
  }) => {
    const site = await Site.create({ name: 'Site Exclusão', active: true })
    const device = await Device.create({
      siteId: site.id,
      name: 'Servidor Exclusão',
      type: 'server',
      status: 'online',
    })

    const monitor = await Monitor.create({
      deviceId: device.id,
      type: 'ping',
      name: 'Ping Exclusão',
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
      startedAt: now,
      finishedAt: now,
      durationMs: 10,
      latencyMs: 5,
      message: 'OK',
    })

    const deleteRes = await client.delete(`/api/monitors/${monitor.id}`)
    deleteRes.assertStatus(204)

    const remainingResults = await MonitorResult.query().where('monitorId', monitor.id)
    assert.isEmpty(remainingResults)

    const remainingMonitors = await Monitor.query().where('id', monitor.id)
    assert.isEmpty(remainingMonitors)
  })
})
