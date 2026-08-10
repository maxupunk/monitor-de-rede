import { test } from '@japa/runner'
import { DeviceIdentifier } from '#modules/discovery/device_identifier'
import { DiscoveryMerger } from '#modules/discovery/discovery_merger'
import { lookupVendor } from '#modules/discovery/oui_lookup'
import {
  expandCidr,
  isScannableCidr,
  parseCidrRange,
  MAX_SCAN_HOSTS,
} from '#modules/discovery/cidr_range'

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

  test('OUILookup deve identificar vendor a partir do MAC', ({ assert }) => {
    assert.equal(lookupVendor('00:0B:86:00:00:00'), 'Ubiquiti Networks')
    assert.equal(lookupVendor('00:13:20:00:00:00'), 'MikroTik')
    assert.equal(lookupVendor('00:50:56:00:00:00'), 'VMware')
    assert.isNull(lookupVendor('FF:FF:FF:FF:FF:FF'))
  })

  test('DeviceIdentifier deve inferir tipo a partir do vendor', ({ assert }) => {
    const identifier = new DeviceIdentifier()

    assert.equal(identifier.identifyType({ vendor: 'MikroTik' }), 'router')
    assert.equal(identifier.identifyType({ vendor: 'Ubiquiti Networks' }), 'access_point')
    assert.equal(identifier.identifyType({ vendor: 'Synology' }), 'server')
    assert.equal(identifier.identifyType({ vendor: 'Desconhecido' }), 'unknown')
  })
})

test.group('Descoberta - Expansão de faixas CIDR', () => {
  test('deve expandir um /24 sem incluir rede e broadcast', ({ assert }) => {
    const addresses = expandCidr('192.168.1.0/24')

    assert.lengthOf(addresses, 254)
    assert.equal(addresses[0], '192.168.1.1')
    assert.equal(addresses[253], '192.168.1.254')
    assert.notInclude(addresses, '192.168.1.0')
    assert.notInclude(addresses, '192.168.1.255')
  })

  test('deve normalizar o endereço de rede quando vem um host da faixa', ({ assert }) => {
    const range = parseCidrRange('192.168.1.77/24')

    assert.equal(range.networkAddress, '192.168.1.0')
    assert.equal(range.usableHosts, 254)
    assert.isFalse(range.truncated)
  })

  test('deve preservar o primeiro octeto acima de 127', ({ assert }) => {
    // Aritmética com `<<` em 32 bits com sinal produzia endereços negativos aqui
    const addresses = expandCidr('200.150.10.0/30')

    assert.deepEqual(addresses, ['200.150.10.1', '200.150.10.2'])
  })

  test('deve tratar host avulso como faixa de um endereço', ({ assert }) => {
    assert.deepEqual(expandCidr('10.0.0.5'), ['10.0.0.5'])
    assert.equal(parseCidrRange('10.0.0.5').prefix, 32)
  })

  test('faixa maior que o teto deve ser truncada e sinalizada', ({ assert }) => {
    const range = parseCidrRange('10.0.0.0/16')
    assert.equal(range.usableHosts, 65534)
    assert.isTrue(range.truncated)

    // Truncar sem avisar faria a varredura parecer completa cobrindo 1,5% da faixa
    assert.lengthOf(expandCidr('10.0.0.0/16'), MAX_SCAN_HOSTS)
  })

  test('faixas malformadas devem ser rejeitadas', ({ assert }) => {
    assert.isFalse(isScannableCidr('192.168.1.0/33'))
    assert.isFalse(isScannableCidr('192.168.1.0/4'))
    assert.isFalse(isScannableCidr('999.1.1.1/24'))
    assert.isFalse(isScannableCidr('nao-e-um-ip'))
    assert.isFalse(isScannableCidr(''))
    assert.isTrue(isScannableCidr('172.16.0.0/22'))
  })
})
