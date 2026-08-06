import { test } from '@japa/runner'
import dgram from 'node:dgram'
import { PortScannerService } from '#modules/network_tools/port_scanner_service'
import { UdpProbeRegistry } from '#modules/network_tools/udp_probe_registry'

test.group('PortScannerService - Unit Tests', () => {
  test('UdpProbeRegistry deve retornar probe específico para portas conhecidas e fallback para desconhecidas', ({
    assert,
  }) => {
    const dnsProbe = UdpProbeRegistry.getProbe(53)
    const ntpProbe = UdpProbeRegistry.getProbe(123)
    const unknownProbe = UdpProbeRegistry.getProbe(9999)

    assert.isTrue(Buffer.isBuffer(dnsProbe))
    assert.isTrue(dnsProbe.length > 0)
    assert.isTrue(Buffer.isBuffer(ntpProbe))
    assert.equal(ntpProbe.length, 48)
    assert.equal(unknownProbe.length, 1)
  })

  test('PortScannerService deve escanear portas UDP e invocar onResult', async ({ assert }) => {
    // Subir servidor UDP dummy para responder na porta 15353
    const dummyServer = dgram.createSocket('udp4')
    await new Promise<void>((resolve) => dummyServer.bind(15353, '127.0.0.1', () => resolve()))

    dummyServer.on('message', (_msg, rinfo) => {
      dummyServer.send(Buffer.from('PONG'), rinfo.port, rinfo.address)
    })

    try {
      const service = new PortScannerService()
      const receivedItems: number[] = []

      const results = await service.scan('127.0.0.1', [15353], 'udp', 500, {
        onResult: (item) => {
          receivedItems.push(item.port)
        },
      })

      assert.equal(results.length, 1)
      assert.equal(results[0].port, 15353)
      assert.equal(results[0].status, 'open')
      assert.deepEqual(receivedItems, [15353])
    } finally {
      await new Promise<void>((resolve) => dummyServer.close(() => resolve()))
    }
  }).timeout(5000)
})
