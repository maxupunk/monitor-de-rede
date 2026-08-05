import { test } from '@japa/runner'
import { DateTime } from 'luxon'
import testUtils from '@adonisjs/core/services/test_utils'
import Device from '#models/device'
import Monitor from '#models/monitor'
import { DeviceStatusService } from '#modules/monitoring/device_status_service'
import { EventBus, type SystemEvent } from '#modules/events/event_bus'

async function createDevice(status: Device['status'] = 'unknown') {
  return Device.create({
    name: 'mppt',
    ipAddress: '10.0.0.34',
    type: 'host',
    status,
  })
}

async function createMonitor(
  deviceId: number,
  status: Monitor['status'],
  overrides: Partial<{ enabled: boolean; type: Monitor['type']; name: string }> = {}
) {
  return Monitor.create({
    deviceId,
    name: overrides.name ?? `monitor-${status}`,
    type: overrides.type ?? 'ping',
    configuration: {},
    intervalSeconds: 60,
    timeoutSeconds: 5,
    retryCount: 1,
    enabled: overrides.enabled ?? true,
    status,
  })
}

function captureEvents(): { events: SystemEvent[]; stop: () => void } {
  const events: SystemEvent[] = []
  const stop = EventBus.getInstance().subscribe((event) => events.push(event))
  return { events, stop }
}

test.group('Status de dispositivo', (group) => {
  group.each.setup(() => testUtils.db().truncate())
  group.each.teardown(() => EventBus.getInstance().clearListeners())

  test('aggregate consolida os monitores em um único status', ({ assert }) => {
    assert.equal(DeviceStatusService.aggregate(['up', 'up']), 'online')
    assert.equal(DeviceStatusService.aggregate(['down', 'down']), 'offline')
    // Ping responde e SNMP não: degradado, e não uma disputa online/offline
    assert.equal(DeviceStatusService.aggregate(['up', 'down']), 'warning')
    assert.equal(DeviceStatusService.aggregate(['up', 'warning']), 'warning')
    // Monitores sem leitura conclusiva não derrubam nem sustentam o dispositivo
    assert.equal(DeviceStatusService.aggregate(['up', 'unknown', 'disabled']), 'online')
    assert.equal(DeviceStatusService.aggregate(['unknown', 'disabled'], 'offline'), 'offline')
    assert.equal(DeviceStatusService.aggregate([], 'online'), 'online')
  })

  test('não publica device:status quando o status permanece o mesmo', async ({ assert }) => {
    const device = await createDevice('offline')
    await createMonitor(device.id, 'down')

    const { events, stop } = captureEvents()
    const transition = await new DeviceStatusService().refreshFromMonitors(device, {
      observedStatus: 'offline',
    })
    stop()

    assert.isFalse(transition.changed)
    assert.equal(device.status, 'offline')
    assert.lengthOf(events, 0)
  })

  test('publica device:status com o estado anterior quando há transição real', async ({
    assert,
  }) => {
    const device = await createDevice('offline')
    await createMonitor(device.id, 'up')

    const { events, stop } = captureEvents()
    const transition = await new DeviceStatusService().refreshFromMonitors(device, {
      observedStatus: 'online',
    })
    stop()

    assert.isTrue(transition.changed)
    assert.equal(transition.previousStatus, 'offline')
    assert.lengthOf(events, 1)
    assert.equal(events[0].type, 'device:status')
    assert.equal(events[0].data.status, 'online')
    assert.equal(events[0].data.previousStatus, 'offline')

    await device.refresh()
    assert.equal(device.status, 'online')
  })

  test('coleta SNMP não sobe o dispositivo enquanto o ping estiver caindo', async ({ assert }) => {
    const device = await createDevice('offline')
    await createMonitor(device.id, 'down', { name: 'ping-mppt' })

    const { events, stop } = captureEvents()
    // Cenário da coleta SNMP: observa contato, mas quem decide são os monitores
    const transition = await new DeviceStatusService().refreshFromMonitors(device, {
      observedStatus: 'online',
    })
    stop()

    assert.isFalse(transition.changed)
    assert.equal(device.status, 'offline')
    assert.lengthOf(events, 0)
  })

  test('monitor desabilitado não participa da consolidação', async ({ assert }) => {
    const device = await createDevice('online')
    await createMonitor(device.id, 'up')
    await createMonitor(device.id, 'down', { enabled: false, name: 'snmp-desligado' })

    const transition = await new DeviceStatusService().refreshFromMonitors(device)

    assert.isFalse(transition.changed)
    assert.equal(device.status, 'online')
  })

  test('lastSeenAt avança sem gerar evento', async ({ assert }) => {
    const device = await createDevice('online')
    await createMonitor(device.id, 'up')

    const { events, stop } = captureEvents()
    const seenAt = DateTime.now()
    await new DeviceStatusService().refreshFromMonitors(device, { seenAt })
    stop()

    assert.lengthOf(events, 0)
    await device.refresh()
    assert.isNotNull(device.lastSeenAt)
  })
})
