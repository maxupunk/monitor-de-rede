import { test } from '@japa/runner'
import testUtils from '@adonisjs/core/services/test_utils'
import crypto from 'node:crypto'
import Probe from '#models/probe'
import Site from '#models/site'
import Device from '#models/device'
import Monitor from '#models/monitor'
import MonitorResult from '#models/monitor_result'
import { ProbeTaskDispatcher } from '#modules/probes/probe_task_dispatcher'

test.group('Probes API - Functional Tests', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  const rawToken = 'secret-test-token-123'
  const tokenHash = crypto.createHash('sha256').update(rawToken).digest('hex')

  test('POST /api/probes/heartbeat deve atualizar o status do probe para online', async ({ client, assert }) => {
    const probe = await Probe.create({
      name: 'Probe Remoto 1',
      tokenHash,
      status: 'pending',
    })

    const response = await client
      .post('/api/probes/heartbeat')
      .header('x-probe-token', rawToken)
      .json({
        version: '1.2.0',
        configuration: { arch: 'x64' },
      })

    response.assertStatus(200)
    assert.equal(response.body().status, 'ok')
    assert.equal(response.body().probeId, probe.id)

    await probe.refresh()
    assert.equal(probe.status, 'online')
    assert.equal(probe.version, '1.2.0')
  })

  test('GET /api/probes/tasks deve retornar as tarefas pendentes para o probe', async ({ client, assert }) => {
    const probe = await Probe.create({
      name: 'Probe Remoto 2',
      tokenHash,
      status: 'online',
    })

    const dispatcher = new ProbeTaskDispatcher()
    dispatcher.dispatchTask(probe.id, {
      id: 'task-test-1',
      monitorId: 10,
      type: 'ping',
      timeoutMs: 1000,
      payload: { host: '127.0.0.1' },
    })

    const response = await client
      .get('/api/probes/tasks')
      .header('x-probe-token', rawToken)

    response.assertStatus(200)
    assert.isArray(response.body().tasks)
    assert.lengthOf(response.body().tasks, 1)
    assert.equal(response.body().tasks[0].id, 'task-test-1')
  })

  test('POST /api/probes/results deve gravar os resultados do monitor enviado pelo probe', async ({ client, assert }) => {
    const probe = await Probe.create({
      name: 'Probe Remoto 3',
      tokenHash,
      status: 'online',
    })

    const site = await Site.create({ name: 'Site Teste', active: true })
    const device = await Device.create({
      siteId: site.id,
      name: 'Roteador Teste',
      type: 'router',
      status: 'unknown',
    })

    const monitor = await Monitor.create({
      deviceId: device.id,
      probeId: probe.id,
      type: 'ping',
      name: 'Ping Check',
      configuration: { host: '192.168.1.1' },
      intervalSeconds: 60,
      timeoutSeconds: 5,
      retryCount: 1,
      enabled: true,
      status: 'unknown',
    })

    const now = new Date()
    const response = await client
      .post('/api/probes/results')
      .header('x-probe-token', rawToken)
      .json({
        results: [
          {
            monitorId: monitor.id,
            taskId: 'task-999',
            result: {
              success: true,
              status: 'up',
              durationMs: 12,
              startedAt: now.toISOString(),
              finishedAt: now.toISOString(),
              message: 'Ping OK',
              metrics: [{ name: 'latency', value: 12, unit: 'ms' }],
              data: {},
            },
          },
        ],
      })

    response.assertStatus(200)
    assert.equal(response.body().count, 1)

    await monitor.refresh()
    assert.equal(monitor.status, 'up')

    const dbResult = await MonitorResult.query().where('monitorId', monitor.id).first()
    assert.exists(dbResult)
    assert.equal(dbResult?.status, 'up')
    assert.equal(dbResult?.probeId, probe.id)
  })

  test('POST /api/probes/:id/revoke deve revogar o probe e barrar acessos futuros', async ({ client, assert }) => {
    const probe = await Probe.create({
      name: 'Probe Cancelado',
      tokenHash,
      status: 'online',
    })

    const revokeResponse = await client.post(`/api/probes/${probe.id}/revoke`)
    revokeResponse.assertStatus(200)

    await probe.refresh()
    assert.equal(probe.status, 'revoked')

    const heartbeatResponse = await client
      .post('/api/probes/heartbeat')
      .header('x-probe-token', rawToken)

    heartbeatResponse.assertStatus(401)
  })
})
