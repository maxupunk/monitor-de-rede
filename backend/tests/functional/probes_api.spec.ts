import { test } from '@japa/runner'
import testUtils from '@adonisjs/core/services/test_utils'
import crypto from 'node:crypto'
import { DateTime } from 'luxon'
import Probe from '#models/probe'
import ProbeTaskRecord from '#models/probe_task'
import Site from '#models/site'
import Device from '#models/device'
import Monitor from '#models/monitor'
import MonitorResult from '#models/monitor_result'
import { ProbeTaskDispatcher, TASK_TTL_SECONDS } from '#modules/probes/probe_task_dispatcher'
import {
  isProbeAlive,
  ProbeWatchdog,
  PROBE_OFFLINE_AFTER_SECONDS,
} from '#modules/probes/probe_liveness'

test.group('Probes API - Functional Tests', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  const rawToken = 'secret-test-token-123'
  const tokenHash = crypto.createHash('sha256').update(rawToken).digest('hex')

  test('POST /api/probes/heartbeat deve atualizar o status do probe para online', async ({
    client,
    assert,
  }) => {
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

  /** Monitor mínimo para pendurar tarefas — a fila tem FK para `monitors`. */
  async function createMonitor(probeId: number | null, name = 'Ping Check'): Promise<Monitor> {
    const site = await Site.create({ name: `Site ${name}`, active: true })
    const device = await Device.create({
      siteId: site.id,
      name: `Equipamento ${name}`,
      type: 'router',
      status: 'unknown',
    })

    return Monitor.create({
      deviceId: device.id,
      probeId,
      type: 'ping',
      name,
      configuration: { host: '192.168.1.1' },
      intervalSeconds: 60,
      timeoutSeconds: 5,
      retryCount: 1,
      enabled: true,
      status: 'unknown',
    })
  }

  test('GET /api/probes/tasks deve entregar tarefas enfileiradas por outro processo', async ({
    client,
    assert,
  }) => {
    const probe = await Probe.create({
      name: 'Probe Remoto 2',
      tokenHash,
      status: 'online',
    })
    const monitor = await createMonitor(probe.id)

    // O scheduler roda em outro processo: a fila precisa estar no banco, e não
    // na memória de quem despachou, senão o probe consulta uma fila vazia.
    const dispatcher = new ProbeTaskDispatcher()
    await dispatcher.dispatchTask(probe.id, {
      id: 'task-test-1',
      monitorId: monitor.id,
      type: 'ping',
      timeoutMs: 1000,
      payload: { host: '127.0.0.1' },
    })

    const response = await client.get('/api/probes/tasks').header('x-probe-token', rawToken)

    response.assertStatus(200)
    assert.isArray(response.body().tasks)
    assert.lengthOf(response.body().tasks, 1)
    assert.equal(response.body().tasks[0].id, 'task-test-1')
    assert.equal(response.body().tasks[0].monitorId, monitor.id)

    // Entregue uma única vez: o segundo polling não repete a checagem.
    const repeated = await client.get('/api/probes/tasks').header('x-probe-token', rawToken)
    assert.lengthOf(repeated.body().tasks, 0)
  })

  test('a fila deve manter no máximo uma tarefa pendente por monitor', async ({
    client,
    assert,
  }) => {
    const probe = await Probe.create({ name: 'Probe Remoto 4', tokenHash, status: 'online' })
    const monitor = await createMonitor(probe.id)
    const dispatcher = new ProbeTaskDispatcher()

    // Probe fora do ar por vários ciclos: sem substituição, ele voltaria e
    // executaria uma avalanche de checagens vencidas de uma vez só.
    for (const id of ['task-1', 'task-2', 'task-3']) {
      await dispatcher.dispatchTask(probe.id, {
        id,
        monitorId: monitor.id,
        type: 'ping',
        timeoutMs: 1000,
        payload: { host: '127.0.0.1' },
      })
    }

    const response = await client.get('/api/probes/tasks').header('x-probe-token', rawToken)

    assert.lengthOf(response.body().tasks, 1)
    assert.equal(response.body().tasks[0].id, 'task-3')
  })

  test('tarefa vencida não deve ser entregue', async ({ client, assert }) => {
    const probe = await Probe.create({ name: 'Probe Remoto 5', tokenHash, status: 'online' })
    const monitor = await createMonitor(probe.id)
    const dispatcher = new ProbeTaskDispatcher()

    await dispatcher.dispatchTask(probe.id, {
      id: 'task-velha',
      monitorId: monitor.id,
      type: 'ping',
      timeoutMs: 1000,
      payload: { host: '127.0.0.1' },
    })

    // Executar agora uma checagem enfileirada há muito tempo produziria um
    // resultado carimbado com a hora errada.
    const queued = await ProbeTaskRecord.query().where('monitorId', monitor.id).firstOrFail()
    queued.createdAt = DateTime.now().minus({ seconds: TASK_TTL_SECONDS + 60 })
    await queued.save()

    const response = await client.get('/api/probes/tasks').header('x-probe-token', rawToken)

    assert.lengthOf(response.body().tasks, 0)

    // E some da fila: descartada, não reapresentada no próximo polling.
    const remaining = await ProbeTaskRecord.query().where('monitorId', monitor.id).first()
    assert.isNull(remaining)
  })

  test('probe sem heartbeat deve ser marcado offline pelo watchdog', async ({ assert }) => {
    const vivo = await Probe.create({
      name: 'Probe Vivo',
      tokenHash,
      status: 'online',
      lastSeenAt: DateTime.now(),
    })
    const mudo = await Probe.create({
      name: 'Probe Mudo',
      tokenHash: 'outro-hash',
      status: 'online',
      lastSeenAt: DateTime.now().minus({ seconds: PROBE_OFFLINE_AFTER_SECONDS + 60 }),
    })

    assert.isTrue(isProbeAlive(vivo))
    assert.isFalse(isProbeAlive(mudo))

    const changed = await new ProbeWatchdog().markStaleProbesOffline()
    assert.equal(changed, 1)

    await vivo.refresh()
    await mudo.refresh()
    assert.equal(vivo.status, 'online')
    assert.equal(mudo.status, 'offline')
  })

  test('POST /api/probes/results deve gravar os resultados do monitor enviado pelo probe', async ({
    client,
    assert,
  }) => {
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

  test('POST /api/probes/:id/revoke deve revogar o probe e barrar acessos futuros', async ({
    client,
    assert,
  }) => {
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
