import { test } from '@japa/runner'
import testUtils from '@adonisjs/core/services/test_utils'
import { DateTime } from 'luxon'
import Site from '#models/site'
import Device from '#models/device'
import Monitor from '#models/monitor'
import AlertRule from '#models/alert_rule'
import AlertEvent from '#models/alert_event'
import { ResultProcessor } from '#modules/monitoring/result_processor'

test.group('Alerts API - Functional Tests', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  test('POST /api/alert-rules deve criar uma nova regra de alerta com sucesso', async ({ client, assert }) => {
    const response = await client.post('/api/alert-rules').json({
      name: 'Regra Queda de Ping',
      type: 'device_offline',
      condition: { field: 'status', operator: 'eq', value: 'down' },
      severity: 'critical',
      enabled: true,
    })

    response.assertStatus(201)
    assert.exists(response.body().id)
    assert.equal(response.body().name, 'Regra Queda de Ping')

    const dbRule = await AlertRule.find(response.body().id)
    assert.exists(dbRule)
    assert.equal(dbRule?.severity, 'critical')
  })

  test('Disparo de alerta no ResultProcessor e consulta via GET /api/alerts', async ({ client, assert }) => {
    const site = await Site.create({ name: 'Site Alertas', active: true })
    const device = await Device.create({
      siteId: site.id,
      name: 'Servidor Web',
      type: 'server',
      status: 'unknown',
    })

    const monitor = await Monitor.create({
      deviceId: device.id,
      type: 'http',
      name: 'HTTP Check Web',
      configuration: { url: 'http://127.0.0.1:9999/fail' },
      intervalSeconds: 60,
      timeoutSeconds: 5,
      retryCount: 1,
      enabled: true,
      status: 'unknown',
    })

    const rule = await AlertRule.create({
      monitorId: monitor.id,
      name: 'Serviço HTTP Fora do Ar',
      type: 'http_failure',
      condition: { field: 'status', operator: 'eq', value: 'down' },
      severity: 'critical',
      enabled: true,
    })

    const processor = new ResultProcessor()
    const now = new Date()
    await processor.processResult(monitor.id, {
      success: false,
      status: 'down',
      durationMs: 150,
      startedAt: now,
      finishedAt: now,
      message: 'HTTP 500 Server Error',
    })

    const listResponse = await client.get('/api/alerts')
    listResponse.assertStatus(200)
    assert.isArray(listResponse.body())
    assert.lengthOf(listResponse.body(), 1)
    assert.equal(listResponse.body()[0].alertRuleId, rule.id)
    assert.equal(listResponse.body()[0].status, 'active')
  })

  test('POST /api/alerts/:id/acknowledge e POST /api/alerts/:id/silence', async ({ client, assert }) => {
    const site = await Site.create({ name: 'Site Acknowledgment', active: true })
    const device = await Device.create({ siteId: site.id, name: 'Switch Central', type: 'switch', status: 'online' })
    const monitor = await Monitor.create({
      deviceId: device.id,
      type: 'ping',
      name: 'Ping Switch',
      configuration: {},
      intervalSeconds: 60,
      timeoutSeconds: 5,
      retryCount: 1,
      enabled: true,
      status: 'up',
    })

    const rule = await AlertRule.create({
      monitorId: monitor.id,
      name: 'Perda de Pacote Switch',
      type: 'custom',
      condition: { field: 'status', operator: 'eq', value: 'down' },
      severity: 'warning',
      enabled: true,
    })

    const event = await AlertEvent.create({
      alertRuleId: rule.id,
      deviceId: device.id,
      monitorId: monitor.id,
      status: 'active',
      severity: 'warning',
      startedAt: DateTime.now(),
      message: 'Ping indisponível',
    })

    const ackRes = await client.post(`/api/alerts/${event.id}/acknowledge`)
    ackRes.assertStatus(200)

    await event.refresh()
    assert.equal(event.status, 'acknowledged')

    const silenceRes = await client.post(`/api/alerts/${event.id}/silence`).json({ minutes: 45 })
    silenceRes.assertStatus(200)

    await event.refresh()
    assert.equal(event.status, 'silenced')
  })
})
