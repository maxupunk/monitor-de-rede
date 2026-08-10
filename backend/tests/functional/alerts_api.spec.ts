import { test } from '@japa/runner'
import testUtils from '@adonisjs/core/services/test_utils'
import { DateTime } from 'luxon'
import Site from '#models/site'
import Device from '#models/device'
import Monitor from '#models/monitor'
import AlertRule from '#models/alert_rule'
import AlertEvent from '#models/alert_event'
import { ResultProcessor } from '#modules/monitoring/result_processor'

/**
 * As requisições usam `client.visit('rota.nomeada')` em vez da URL literal: o
 * cliente tipado resolve a resposta pelo padrão da rota, então GET e POST na
 * mesma URL virariam uma união inútil (`AlertRule | AlertRule[]`). Pela rota
 * nomeada o tipo vem do handler exato.
 */
test.group('Alerts API - Functional Tests', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  test('POST /api/alert-rules deve criar uma nova regra de alerta com sucesso', async ({
    client,
    assert,
  }) => {
    const response = await client.visit('alerts.rules_store').json({
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

  test('GET /api/alert-rules/catalog deve listar as regras pré-configuradas', async ({
    client,
    assert,
  }) => {
    const response = await client.visit('alerts.catalog_index')

    response.assertStatus(200)
    const body = response.body()
    assert.isArray(body.templates)
    assert.isAbove(body.templates.length, 0)
    assert.isObject(body.categories)

    const downgrade = body.templates.find(
      (template) => template.key === 'interface_speed_downgrade'
    )
    assert.exists(downgrade, 'a regra de downgrade de negociação deve estar no catálogo')
    assert.isFalse(downgrade?.applied)
  })

  test('POST /api/alert-rules/catalog deve aplicar as regras e nunca duplicar', async ({
    client,
    assert,
  }) => {
    const keys = ['device_offline', 'interface_speed_downgrade']

    const first = await client.visit('alerts.catalog_apply').json({ keys })
    first.assertStatus(201)
    assert.lengthOf(first.body().created, 2)
    assert.lengthOf(first.body().skipped, 0)

    // Reaplicar a mesma seleção não pode criar regra repetida
    const second = await client.visit('alerts.catalog_apply').json({ keys })
    second.assertStatus(201)
    assert.lengthOf(second.body().created, 0)
    assert.lengthOf(second.body().skipped, 2)

    assert.lengthOf(await AlertRule.all(), 2)

    const catalog = await client.visit('alerts.catalog_index')
    const applied = catalog.body().templates.filter((template) => template.applied)
    assert.lengthOf(applied, 2)
  })

  test('POST /api/alert-rules/catalog deve recusar seleção vazia', async ({ client }) => {
    const response = await client.visit('alerts.catalog_apply').json({ keys: [] })
    response.assertStatus(422)
  })

  test('Disparo de alerta no ResultProcessor e consulta via GET /api/alerts', async ({
    client,
    assert,
  }) => {
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

    const listResponse = await client.visit('alerts.index')
    listResponse.assertStatus(200)

    // Sem `page` o endpoint devolve o array cru; com `page`, o envelope paginado
    // usado pelo histórico com scroll infinito.
    const alerts = listResponse.body() as Array<{ alertRuleId: number | null; status: string }>
    assert.isArray(alerts)
    assert.lengthOf(alerts, 1)
    assert.equal(alerts[0].alertRuleId, rule.id)
    assert.equal(alerts[0].status, 'active')

    const pagedResponse = await client.get('/api/alerts?page=1&limit=10')
    pagedResponse.assertStatus(200)
    const paged = pagedResponse.body() as { data: unknown[]; meta: { total: number } }
    assert.lengthOf(paged.data, 1)
    assert.equal(paged.meta.total, 1)
  })

  test('POST /api/alerts/:id/acknowledge e POST /api/alerts/:id/silence', async ({
    client,
    assert,
  }) => {
    const site = await Site.create({ name: 'Site Acknowledgment', active: true })
    const device = await Device.create({
      siteId: site.id,
      name: 'Switch Central',
      type: 'switch',
      status: 'online',
    })
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

    const ackRes = await client.visit('alerts.acknowledge', { id: event.id })
    ackRes.assertStatus(200)

    await event.refresh()
    assert.equal(event.status, 'acknowledged')

    const silenceRes = await client.visit('alerts.silence', { id: event.id }).json({ minutes: 45 })
    silenceRes.assertStatus(200)

    await event.refresh()
    assert.equal(event.status, 'silenced')
  })

  test('POST /api/alerts/:id/acknowledge deve resolver automaticamente o alerta se o monitor responder UP', async ({
    client,
    assert,
  }) => {
    const site = await Site.create({ name: 'Site AutoResolve', active: true })
    const device = await Device.create({
      siteId: site.id,
      name: 'Host Localhost',
      type: 'server',
      status: 'online',
    })
    const monitor = await Monitor.create({
      deviceId: device.id,
      type: 'ping',
      name: 'Ping Localhost',
      configuration: { host: '127.0.0.1' },
      intervalSeconds: 60,
      timeoutSeconds: 5,
      retryCount: 1,
      enabled: true,
      status: 'down',
    })

    const rule = await AlertRule.create({
      monitorId: monitor.id,
      name: 'Host indisponivel',
      type: 'device_offline',
      condition: { field: 'status', operator: 'eq', value: 'down' },
      severity: 'critical',
      enabled: true,
    })

    const event = await AlertEvent.create({
      alertRuleId: rule.id,
      deviceId: device.id,
      monitorId: monitor.id,
      scopeKey: `monitor:${monitor.id}`,
      status: 'active',
      severity: 'critical',
      startedAt: DateTime.now(),
      message: 'Ping falhou',
    })

    // Ao reconhecer um alerta de monitor que está UP (127.0.0.1 responde ping), ele resolve automaticamente
    const ackRes = await client.visit('alerts.acknowledge', { id: event.id })
    ackRes.assertStatus(200)
    assert.isTrue(ackRes.body().resolved)

    await event.refresh()
    assert.equal(event.status, 'resolved')
    assert.exists(event.resolvedAt)
  })

  test('POST /api/alerts/verify-all deve verificar todos os alertas pendentes', async ({
    client,
    assert,
  }) => {
    const site = await Site.create({ name: 'Site VerifyAll', active: true })
    const device = await Device.create({ siteId: site.id, name: 'Servidor Teste', type: 'server' })
    const monitor = await Monitor.create({
      deviceId: device.id,
      type: 'ping',
      name: 'Ping Local',
      configuration: { host: '127.0.0.1' },
      enabled: true,
      status: 'down',
    })

    await AlertEvent.create({
      monitorId: monitor.id,
      scopeKey: `monitor:${monitor.id}`,
      status: 'active',
      severity: 'critical',
      startedAt: DateTime.now(),
      message: 'Indisponivel',
    })

    const verifyRes = await client.post('/api/alerts/verify-all')
    verifyRes.assertStatus(200)
    assert.isNumber(verifyRes.body().resolvedCount)
    assert.isAbove(verifyRes.body().resolvedCount, 0)
  })

  test('GET /api/monitors/:id/alerts reflete acknowledge e silence do alerta', async ({
    client,
    assert,
  }) => {
    const site = await Site.create({ name: 'Site Monitor Alerts', active: true })
    const device = await Device.create({
      siteId: site.id,
      name: 'Servidor Monitor',
      type: 'server',
      status: 'online',
    })
    const monitor = await Monitor.create({
      deviceId: device.id,
      type: 'ping',
      name: 'Ping Servidor',
      configuration: {},
      intervalSeconds: 60,
      timeoutSeconds: 5,
      retryCount: 1,
      enabled: true,
      status: 'up',
    })

    const event = await AlertEvent.create({
      monitorId: monitor.id,
      status: 'active',
      severity: 'critical',
      startedAt: DateTime.now(),
      message: 'Servidor sem resposta',
    })

    const historyBefore = await client.get(`/api/monitors/${monitor.id}/alerts?page=1&limit=10`)
    historyBefore.assertStatus(200)
    assert.equal(historyBefore.body().data[0].status, 'active')

    await client.visit('alerts.acknowledge', { id: event.id })

    const historyAck = await client.get(`/api/monitors/${monitor.id}/alerts?page=1&limit=10`)
    historyAck.assertStatus(200)
    assert.equal(historyAck.body().data[0].status, 'acknowledged')

    await client.visit('alerts.silence', { id: event.id }).json({ minutes: 30 })

    const historySilenced = await client.get(`/api/monitors/${monitor.id}/alerts?page=1&limit=10`)
    historySilenced.assertStatus(200)
    assert.equal(historySilenced.body().data[0].status, 'silenced')
  })
})
