import { test } from '@japa/runner'
import testUtils from '@adonisjs/core/services/test_utils'
import AlertRule from '#models/alert_rule'
import { RuleEvaluator } from '#modules/alerts/rule_evaluator'
import { AlertRuleCatalogService } from '#modules/alerts/catalog/alert_rule_catalog_service'
import {
  ALERT_RULE_TEMPLATES,
  recommendedTemplates,
} from '#modules/alerts/catalog/alert_rule_templates'
import { EventBus } from '#modules/events/event_bus'
import { NotificationService } from '#modules/notifications/notification_service'

test.group('Motor de Alertas & Notificações - Unit Tests', () => {
  test('RuleEvaluator deve avaliar operadores de condição corretamente', async ({ assert }) => {
    const evaluator = new RuleEvaluator()

    assert.isTrue(
      evaluator.evaluate({ field: 'status', operator: 'eq', value: 'down' }, { status: 'down' })
    )
    assert.isFalse(
      evaluator.evaluate({ field: 'status', operator: 'eq', value: 'up' }, { status: 'down' })
    )

    assert.isTrue(
      evaluator.evaluate({ field: 'latencyMs', operator: 'gt', value: 100 }, { latencyMs: 150 })
    )
    assert.isFalse(
      evaluator.evaluate({ field: 'latencyMs', operator: 'gt', value: 200 }, { latencyMs: 150 })
    )

    assert.isTrue(
      evaluator.evaluate(
        { field: 'message', operator: 'contains', value: 'Timeout' },
        { message: 'Connection Timeout Error' }
      )
    )
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

  test('NotificationService deve despachar mensagens aos canais cadastrados', async ({
    assert,
  }) => {
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

test.group('Catálogo de Regras Pré-configuradas', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  test('todo template deve ter chave única e condição completa', ({ assert }) => {
    const keys = ALERT_RULE_TEMPLATES.map((template) => template.key)
    assert.equal(new Set(keys).size, keys.length)

    for (const template of ALERT_RULE_TEMPLATES) {
      assert.isNotEmpty(template.name, `template ${template.key} sem nome`)
      assert.isNotEmpty(template.description, `template ${template.key} sem descrição`)
      assert.isNotEmpty(template.condition.field, `template ${template.key} sem campo`)
      assert.isNotEmpty(template.condition.operator, `template ${template.key} sem operador`)
      assert.exists(template.condition.value, `template ${template.key} sem valor`)
    }
  })

  test('apply deve criar as regras escolhidas e marcá-las com o template de origem', async ({
    assert,
  }) => {
    const service = new AlertRuleCatalogService()
    const result = await service.apply(['device_offline', 'interface_speed_downgrade'])

    assert.lengthOf(result.created, 2)
    assert.lengthOf(result.skipped, 0)

    const rules = await AlertRule.query().orderBy('id', 'asc')
    assert.lengthOf(rules, 2)
    assert.deepEqual(
      rules.map((rule) => rule.templateKey),
      ['device_offline', 'interface_speed_downgrade']
    )
    assert.isTrue(rules.every((rule) => rule.enabled))
  })

  test('apply deve ser idempotente: regra existente não é recriada', async ({ assert }) => {
    const service = new AlertRuleCatalogService()
    await service.apply(['device_offline'])

    const second = await service.apply(['device_offline', 'latency_high'])

    assert.lengthOf(second.created, 1)
    assert.deepEqual(second.skipped, [{ key: 'device_offline', reason: 'already_exists' }])
    assert.lengthOf(await AlertRule.all(), 2)
  })

  test('apply deve reconhecer regra equivalente criada à mão e não duplicar', async ({
    assert,
  }) => {
    await AlertRule.create({
      name: 'Minha regra de queda',
      type: 'device_offline',
      condition: { field: 'status', operator: 'eq', value: 'down' },
      severity: 'critical',
      durationSeconds: 0,
      enabled: true,
    })

    const result = await new AlertRuleCatalogService().apply(['device_offline'])

    assert.lengthOf(result.created, 0)
    assert.deepEqual(result.skipped, [{ key: 'device_offline', reason: 'already_exists' }])
    assert.lengthOf(await AlertRule.all(), 1)
  })

  test('apply deve ignorar chave desconhecida', async ({ assert }) => {
    const result = await new AlertRuleCatalogService().apply(['nao_existe'])

    assert.lengthOf(result.created, 0)
    assert.deepEqual(result.skipped, [{ key: 'nao_existe', reason: 'unknown_template' }])
  })

  test('describe deve marcar como aplicadas apenas as regras já existentes', async ({ assert }) => {
    const service = new AlertRuleCatalogService()
    await service.apply(['device_offline'])

    const catalog = await service.describe()
    const offline = catalog.find((template) => template.key === 'device_offline')
    const latency = catalog.find((template) => template.key === 'latency_high')

    assert.isTrue(offline?.applied)
    assert.exists(offline?.ruleId)
    assert.isFalse(latency?.applied)
    assert.isNull(latency?.ruleId ?? null)
  })

  test('ensureDefaults deve provisionar as regras básicas apenas em banco vazio', async ({
    assert,
  }) => {
    const service = new AlertRuleCatalogService()
    const first = await service.ensureDefaults()

    assert.lengthOf(first.created, recommendedTemplates().length)

    // Operador removeu uma regra básica: o restart não pode ressuscitá-la
    await AlertRule.query().where('templateKey', 'device_offline').delete()
    const second = await service.ensureDefaults()

    assert.lengthOf(second.created, 0)
    assert.isNull(await AlertRule.findBy('templateKey', 'device_offline'))
  })
})
