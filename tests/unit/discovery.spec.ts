import { test } from '@japa/runner'
import { DeviceIdentifier } from '#modules/discovery/device_identifier'
import { DiscoveryMerger } from '#modules/discovery/discovery_merger'

test.group('Descoberta de Dispositivos - Unit Tests', () => {
  test('DeviceIdentifier deve classificar corretamente o tipo de equipamento', ({ assert }) => {
    const identifier = new DeviceIdentifier()

    assert.equal(identifier.identifyType({ hostname: 'main-router-01' }), 'router')
    assert.equal(identifier.identifyType({ hostname: 'core-switch-24p' }), 'switch')
    assert.equal(identifier.identifyType({ hostname: 'office-printer-hp' }), 'printer')
    assert.equal(identifier.identifyType({ hostname: 'desk-pc', openPorts: [445] }), 'server')
    assert.equal(identifier.identifyType({ hostname: 'camera-front', openPorts: [554] }), 'camera')
    assert.equal(identifier.identifyType({ hostname: 'unknown-device' }), 'unknown')
  })

  test('DiscoveryMerger deve mesclar resultados por IP e MAC sem duplicar', ({ assert }) => {
    const merger = new DiscoveryMerger()

    const list1 = [
      { ipAddress: '192.168.1.1', macAddress: 'AA:BB:CC:11:22:33', confidence: 50 },
      { ipAddress: '192.168.1.10', hostname: 'pc-joao', confidence: 40 },
    ]

    const list2 = [
      { ipAddress: '192.168.1.1', hostname: 'router.local', confidence: 80 },
      { ipAddress: '192.168.1.20', macAddress: 'DD:EE:FF:44:55:66', confidence: 60 },
    ]

    const merged = merger.mergeResults([list1, list2])

    assert.lengthOf(merged, 3)

    const routerHost = merged.find((h) => h.ipAddress === '192.168.1.1')
    assert.exists(routerHost)
    assert.equal(routerHost?.hostname, 'router.local')
    assert.equal(routerHost?.macAddress, 'AA:BB:CC:11:22:33')
    assert.equal(routerHost?.confidence, 80)
  })
})
