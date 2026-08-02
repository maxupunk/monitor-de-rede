import { test } from '@japa/runner'
import { RuleEvaluator } from '#modules/alerts/rule_evaluator'
import { EventBus } from '#modules/events/event_bus'
import { NotificationService } from '#modules/notifications/notification_service'

test.group('Motor de Alertas & Notificações - Unit Tests', () => {
  test('RuleEvaluator deve avaliar operadores de condição corretamente', async ({ assert }) => {
    const evaluator = new RuleEvaluator()

    assert.isTrue(evaluator.evaluate({ field: 'status', operator: 'eq', value: 'down' }, { status: 'down' }))
    assert.isFalse(evaluator.evaluate({ field: 'status', operator: 'eq', value: 'up' }, { status: 'down' }))

    assert.isTrue(evaluator.evaluate({ field: 'latencyMs', operator: 'gt', value: 100 }, { latencyMs: 150 }))
    assert.isFalse(evaluator.evaluate({ field: 'latencyMs', operator: 'gt', value: 200 }, { latencyMs: 150 }))

    assert.isTrue(evaluator.evaluate({ field: 'message', operator: 'contains', value: 'Timeout' }, { message: 'Connection Timeout Error' }))
  })

  test('EventBus deve emitir e notificar ouvintes inscritos', async ({ assert }) => {
    const eventBus = EventBus.getInstance()
    eventBus.clearListeners()

    let received = false
    let payloadData = ''

    const unsubscribe = eventBus.subscribe((event) => {
      received = true
      payloadData = String(event.data.testKey)
    })

    eventBus.emit('test:event', { testKey: 'hello_sse' })

    assert.isTrue(received)
    assert.equal(payloadData, 'hello_sse')

    unsubscribe()
  })

  test('NotificationService deve despachar mensagens aos canais cadastrados', async ({ assert }) => {
    const service = new NotificationService(false)

    let sent = false
    service.registerChannel({
      name: 'mock_channel',
      async send() {
        sent = true
        return true
      },
    })

    await service.notify({
      title: 'Teste',
      body: 'Corpo da Notificação',
      severity: 'warning',
    })

    assert.isTrue(sent)
  })
})
