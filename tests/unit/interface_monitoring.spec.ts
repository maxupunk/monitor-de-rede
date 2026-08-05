import { test } from '@japa/runner'
import {
  formatSpeed,
  normalizeSpeed,
  InterfaceMonitoringService,
} from '#modules/monitoring/interface_monitoring_service'
import { AlertRuleCatalogService } from '#modules/alerts/catalog/alert_rule_catalog_service'
import Device from '#models/device'
import DeviceInterface from '#models/device_interface'
import AlertEvent from '#models/alert_event'
import testUtils from '@adonisjs/core/services/test_utils'

/**
 * O serviço de interfaces não decide mais o que é alerta: ele publica fatos e o
 * motor aplica as regras cadastradas. Por isso cada cenário abaixo aplica antes
 * a regra do catálogo correspondente.
 */
function applyRules(...keys: string[]) {
  return new AlertRuleCatalogService().apply(keys)
}

test.group('Interface Monitoring - Unit Tests', (group) => {
  group.each.setup(() => testUtils.db().truncate())

  test('formatSpeed deve formatar corretamente conexões de 2.5 Gbps, 1 Gbps, 100 Mbps e 10 Mbps', ({
    assert,
  }) => {
    assert.equal(formatSpeed(2_500_000_000), '2.5 Gbps')
    assert.equal(formatSpeed(1_000_000_000), '1 Gbps')
    assert.equal(formatSpeed(100_000_000), '100 Mbps')
    assert.equal(formatSpeed(10_000_000), '10 Mbps')
    assert.equal(formatSpeed(10_000_000_000), '10 Gbps')
    assert.equal(formatSpeed(0), 'Desconhecido')
    assert.equal(formatSpeed(null), 'Desconhecido')
  })

  test('normalizeSpeed deve descartar leituras não conclusivas de ifSpeed', ({ assert }) => {
    assert.equal(normalizeSpeed(2_500_000_000), 2_500_000_000)
    assert.equal(normalizeSpeed('1000000000'), 1_000_000_000)
    assert.isNull(normalizeSpeed(0))
    assert.isNull(normalizeSpeed(null))
    // Teto do contador de 32 bits: link acima de 4,29 Gbps sem `ifHighSpeed`
    assert.isNull(normalizeSpeed(4_294_967_295))
  })

  test('evaluateInterfaceState NÃO deve gerar evento quando a leitura satura em 32 bits', async ({
    assert,
  }) => {
    await applyRules('interface_speed_downgrade', 'interface_speed_upgrade')

    const device = await Device.create({
      name: 'Switch-Core-10G',
      ipAddress: '192.168.1.4',
      type: 'switch',
      status: 'online',
    })

    const iface = await DeviceInterface.create({
      deviceId: device.id,
      snmpIndex: 3,
      name: 'Te0/1',
      speed: 4_294_967_295,
      adminStatus: 'up',
      operStatus: 'up',
    })

    const service = new InterfaceMonitoringService()

    // 10 Gbps reais -> teto do ifSpeed: leitura do agente, não renegociação
    await service.evaluateInterfaceState(device, iface, 'up', 10_000_000_000)

    const events = await AlertEvent.query().where('deviceId', device.id)
    assert.equal(events.length, 0)
  })

  test('evaluateInterfaceState deve registrar evento quando o status alterar de UP para DOWN', async ({
    assert,
  }) => {
    await applyRules('interface_link_down')

    const device = await Device.create({
      name: 'Router-Core-01',
      ipAddress: '192.168.1.1',
      type: 'router',
      status: 'online',
    })

    const iface = await DeviceInterface.create({
      deviceId: device.id,
      snmpIndex: 1,
      name: 'eth0',
      speed: 1_000_000_000,
      adminStatus: 'up',
      operStatus: 'down',
    })

    const service = new InterfaceMonitoringService()

    // Status alterou de 'up' para 'down'
    await service.evaluateInterfaceState(device, iface, 'up', 1_000_000_000)

    const events = await AlertEvent.query().where('deviceId', device.id)
    assert.equal(events.length, 1)
    assert.equal(events[0].severity, 'warning')
    assert.equal(events[0].scopeKey, `interface:${iface.id}`)
    assert.include(events[0].message!, 'eth0 alterou status: UP ➔ DOWN')
  })

  test('evaluateInterfaceState deve gerar ALERTA DE DOWNGRADE quando a negociação cair de 1Gbps para 100Mbps', async ({
    assert,
  }) => {
    await applyRules('interface_speed_downgrade')

    const device = await Device.create({
      name: 'Switch-Access-01',
      ipAddress: '192.168.1.2',
      type: 'switch',
      status: 'online',
    })

    const iface = await DeviceInterface.create({
      deviceId: device.id,
      snmpIndex: 2,
      name: 'Gi0/1',
      speed: 100_000_000, // Agora negociado a 100 Mbps
      adminStatus: 'up',
      operStatus: 'up',
    })

    const service = new InterfaceMonitoringService()

    // Anteriormente negociado a 1 Gbps (1_000_000_000)
    await service.evaluateInterfaceState(device, iface, 'up', 1_000_000_000)

    const events = await AlertEvent.query().where('deviceId', device.id)
    assert.equal(events.length, 1)
    assert.equal(events[0].status, 'active')
    assert.equal(events[0].severity, 'warning')
    assert.include(events[0].message!, 'sofreu downgrade de velocidade: 1 Gbps ➔ 100 Mbps')
  })

  test('sem a regra de downgrade aplicada, o downgrade não vira alerta', async ({ assert }) => {
    const device = await Device.create({
      name: 'Switch-Access-02',
      ipAddress: '192.168.1.5',
      type: 'switch',
      status: 'online',
    })

    const iface = await DeviceInterface.create({
      deviceId: device.id,
      snmpIndex: 2,
      name: 'Gi0/2',
      speed: 100_000_000,
      adminStatus: 'up',
      operStatus: 'up',
    })

    await new InterfaceMonitoringService().evaluateInterfaceState(
      device,
      iface,
      'up',
      1_000_000_000
    )

    const events = await AlertEvent.query().where('deviceId', device.id)
    assert.equal(events.length, 0)
  })

  test('evaluateInterfaceState deve registrar renegociação para cima quando a regra informativa está ativa', async ({
    assert,
  }) => {
    await applyRules('interface_speed_upgrade')

    const device = await Device.create({
      name: 'Switch-Access-01',
      ipAddress: '192.168.1.2',
      type: 'switch',
      status: 'online',
    })

    const iface = await DeviceInterface.create({
      deviceId: device.id,
      snmpIndex: 2,
      name: 'Gi0/1',
      speed: 1_000_000_000, // Agora negociado a 1 Gbps
      adminStatus: 'up',
      operStatus: 'up',
    })

    const service = new InterfaceMonitoringService()

    // Anteriormente negociado a 100 Mbps
    await service.evaluateInterfaceState(device, iface, 'up', 100_000_000)

    const events = await AlertEvent.query().where('deviceId', device.id)
    assert.equal(events.length, 1)
    assert.equal(events[0].severity, 'info')
    assert.include(events[0].message!, '100 Mbps ➔ 1 Gbps')
  })

  test('retorno do link deve normalizar o alerta aberto da interface', async ({ assert }) => {
    await applyRules('interface_link_down')

    const device = await Device.create({
      name: 'Router-Core-03',
      ipAddress: '192.168.1.6',
      type: 'router',
      status: 'online',
    })

    const iface = await DeviceInterface.create({
      deviceId: device.id,
      snmpIndex: 1,
      name: 'eth1',
      speed: 1_000_000_000,
      adminStatus: 'up',
      operStatus: 'down',
    })

    const service = new InterfaceMonitoringService()
    await service.evaluateInterfaceState(device, iface, 'up', 1_000_000_000)

    iface.operStatus = 'up'
    await iface.save()
    await service.evaluateInterfaceState(device, iface, 'down', 1_000_000_000)

    const events = await AlertEvent.query().where('deviceId', device.id)
    assert.equal(events.length, 1)
    assert.equal(events[0].status, 'resolved')
  })

  test('evaluateInterfaceState NÃO deve gerar evento quando a velocidade não for alterada (ex: 1Gbps para 1Gbps)', async ({
    assert,
  }) => {
    await applyRules('interface_speed_downgrade', 'interface_speed_upgrade', 'interface_link_down')

    const device = await Device.create({
      name: 'Router-Core-02',
      ipAddress: '192.168.1.3',
      type: 'router',
      status: 'online',
    })

    const iface = await DeviceInterface.create({
      deviceId: device.id,
      snmpIndex: 1,
      name: 'wan',
      speed: 1_000_000_000,
      adminStatus: 'up',
      operStatus: 'up',
    })

    const service = new InterfaceMonitoringService()

    // Mesma velocidade: 1 Gbps -> 1 Gbps
    await service.evaluateInterfaceState(device, iface, 'up', 1_000_000_000)

    const events = await AlertEvent.query().where('deviceId', device.id)
    assert.equal(events.length, 0)
  })
})
