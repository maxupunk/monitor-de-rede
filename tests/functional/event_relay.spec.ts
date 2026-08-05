import { test } from '@japa/runner'
import testUtils from '@adonisjs/core/services/test_utils'
import EventOutbox from '#models/event_outbox'
import { EventBus, PROCESS_ORIGIN, type SystemEvent } from '#modules/events/event_bus'
import { EventRelay } from '#modules/events/event_relay'

/**
 * O monitoramento roda em processos separados do servidor HTTP, então o
 * EventBus em memória não alcança as conexões SSE sozinho. Estes testes
 * cobrem a ponte via tabela `event_outbox`.
 */
test.group('Ponte de eventos entre processos', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  group.each.teardown(() => {
    EventBus.getInstance().clearListeners()
    EventBus.getInstance().disableCrossProcessPublishing()
    EventRelay.getInstance().stop()
  })

  test('emit publica na caixa de saída quando a propagação está ligada', async ({ assert }) => {
    const eventBus = EventBus.getInstance()
    eventBus.enableCrossProcessPublishing()

    eventBus.emit('monitor:result', { monitorId: 42, status: 'up' })

    // A gravação é fire-and-forget: aguarda o ciclo de I/O concluir
    await new Promise((resolve) => setTimeout(resolve, 150))

    const rows = await EventOutbox.query().where('type', 'monitor:result')
    assert.lengthOf(rows, 1)
    assert.equal(rows[0].origin, PROCESS_ORIGIN)
    assert.equal(rows[0].payload.monitorId, 42)
  })

  test('emit não grava nada com a propagação desligada', async ({ assert }) => {
    const eventBus = EventBus.getInstance()
    eventBus.disableCrossProcessPublishing()

    eventBus.emit('monitor:result', { monitorId: 7 })
    await new Promise((resolve) => setTimeout(resolve, 150))

    assert.lengthOf(await EventOutbox.all(), 0)
  })

  test('relay entrega aos ouvintes locais o evento gravado por outro processo', async ({
    assert,
  }) => {
    const eventBus = EventBus.getInstance()
    const received: SystemEvent[] = []
    eventBus.subscribe((event) => received.push(event))

    await EventRelay.getInstance().start()

    // Simula o scheduler: outro processo, portanto outra origem
    await EventOutbox.create({
      type: 'monitor:result',
      origin: 'scheduler-process',
      payload: { monitorId: 99, status: 'down', latencyMs: null },
    })

    // Aguarda mais de um ciclo de leitura (POLL_INTERVAL_MS = 1000)
    await new Promise((resolve) => setTimeout(resolve, 1600))

    assert.lengthOf(received, 1)
    assert.equal(received[0].type, 'monitor:result')
    assert.equal(received[0].data.monitorId, 99)
  })

  test('relay ignora eventos gravados pelo próprio processo', async ({ assert }) => {
    const eventBus = EventBus.getInstance()
    const received: SystemEvent[] = []
    eventBus.subscribe((event) => received.push(event))

    await EventRelay.getInstance().start()

    await EventOutbox.create({
      type: 'alert:triggered',
      origin: PROCESS_ORIGIN,
      payload: { alertEventId: 1 },
    })

    await new Promise((resolve) => setTimeout(resolve, 1600))

    // Já teria sido entregue no `emit` local — reentregar duplicaria o alerta
    assert.lengthOf(received, 0)
  })
})
