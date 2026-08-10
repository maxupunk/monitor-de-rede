import { test } from '@japa/runner'
import testUtils from '@adonisjs/core/services/test_utils'
import { DateTime } from 'luxon'
import Site from '#models/site'
import Device from '#models/device'
import Monitor from '#models/monitor'
import MonitorResult from '#models/monitor_result'

async function createDnsMonitor(
  name: string,
  configuration: Record<string, unknown>
): Promise<Monitor> {
  const site = await Site.create({ name: `Site ${name}`, active: true })
  const device = await Device.create({
    siteId: site.id,
    name: `Equipamento ${name}`,
    type: 'server',
    status: 'unknown',
  })

  return Monitor.create({
    deviceId: device.id,
    name,
    type: 'dns',
    configuration,
    intervalSeconds: 60,
    timeoutSeconds: 5,
    retryCount: 3,
    enabled: true,
    status: 'up',
  })
}

async function recordResult(monitor: Monitor, latencyMs: number, server: string, status = 'up') {
  const now = DateTime.now()
  await MonitorResult.create({
    monitorId: monitor.id,
    status: status as 'up' | 'down',
    startedAt: now,
    finishedAt: now,
    durationMs: Math.round(latencyMs),
    latencyMs,
    message: null,
    data: { server, protocol: 'udp', avgLookupTimeMs: latencyMs },
  })
}

test.group('DNS API - Functional Tests', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  test('GET /api/dns/performance deve ranquear servidores do mais rápido ao mais lento', async ({
    client,
    assert,
  }) => {
    const rapido = await createDnsMonitor('DNS Cloudflare', {
      domain: 'exemplo.com',
      dnsServer: '1.1.1.1',
      protocol: 'udp',
    })
    const lento = await createDnsMonitor('DNS Lento', {
      domain: 'exemplo.com',
      dnsServer: '192.0.2.10',
      protocol: 'udp',
    })

    await recordResult(rapido, 8.5, '1.1.1.1')
    await recordResult(rapido, 11.5, '1.1.1.1')
    await recordResult(lento, 120, '192.0.2.10')
    await recordResult(lento, 140, '192.0.2.10')

    const response = await client.get('/api/dns/performance?hours=24')

    response.assertStatus(200)
    const body = response.body()

    assert.equal(body.monitorCount, 2)
    assert.lengthOf(body.ranking, 2)

    assert.equal(body.ranking[0].server, '1.1.1.1')
    assert.equal(body.ranking[0].avgLookupTimeMs, 10)
    assert.equal(body.ranking[0].minLookupTimeMs, 8.5)
    assert.equal(body.ranking[0].maxLookupTimeMs, 11.5)
    assert.equal(body.ranking[0].successRate, 100)
    assert.equal(body.ranking[0].totalChecks, 2)

    assert.equal(body.ranking[1].server, '192.0.2.10')
    assert.equal(body.ranking[1].avgLookupTimeMs, 130)
  })

  test('GET /api/dns/performance deve considerar falhas na taxa de sucesso', async ({
    client,
    assert,
  }) => {
    const monitor = await createDnsMonitor('DNS Instável', {
      domain: 'exemplo.com',
      dnsServer: '9.9.9.9',
      protocol: 'udp',
    })

    await recordResult(monitor, 20, '9.9.9.9')
    await recordResult(monitor, 0, '9.9.9.9', 'down')

    const response = await client.get('/api/dns/performance')

    response.assertStatus(200)
    const entry = response.body().ranking[0]

    assert.equal(entry.totalChecks, 2)
    assert.equal(entry.successRate, 50)
    // Apenas as checagens bem-sucedidas entram na média de latência
    assert.equal(entry.avgLookupTimeMs, 20)
  })

  test('GET /api/dns/performance deve responder vazio sem monitores DNS', async ({
    client,
    assert,
  }) => {
    const response = await client.get('/api/dns/performance')

    response.assertStatus(200)
    assert.equal(response.body().monitorCount, 0)
    assert.isEmpty(response.body().ranking)
  })

  test('POST /api/dns/benchmark deve validar o payload recebido', async ({ client }) => {
    const response = await client.post('/api/dns/benchmark').json({
      servers: [{ server: '' }],
    })

    response.assertStatus(422)
  })

  test('POST /api/dns/lookup deve devolver a medição estruturada mesmo em falha', async ({
    client,
    assert,
  }) => {
    const response = await client.post('/api/dns/lookup').json({
      hostname: 'servidor.local',
      server: '127.0.0.1:9',
      protocol: 'tcp',
      timeoutMs: 400,
    })

    response.assertStatus(200)
    const body = response.body()

    assert.isFalse(body.success)
    assert.isNotNull(body.error)
    assert.equal(body.protocol, 'tcp')
    assert.isNumber(body.lookupTimeMs)
  }).timeout(5000)

  test('POST /api/dns/benchmark deve ranquear os servidores informados', async ({
    client,
    assert,
  }) => {
    const response = await client.post('/api/dns/benchmark').json({
      servers: [{ server: '127.0.0.1:9', label: 'Inacessível', protocol: 'udp' }],
      hostnames: ['servidor.local'],
      timeoutMs: 400,
    })

    response.assertStatus(200)
    const body = response.body()

    assert.lengthOf(body.ranking, 1)
    assert.equal(body.ranking[0].label, 'Inacessível')
    assert.isNull(body.ranking[0].avgLookupTimeMs)
    assert.equal(body.ranking[0].failedQueries, 1)
    assert.isNotNull(body.ranking[0].error)
  }).timeout(5000)
})
