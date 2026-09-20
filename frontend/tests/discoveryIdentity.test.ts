import { describe, expect, it } from 'vitest'
import { discoveryDeviceName, discoveryDeviceTypeInfo, discoveryIdentity } from '@/stores/discovery'

describe('discoveryIdentity', () => {
  it('lê a identidade do snapshot SSE', () => {
    expect(
      discoveryIdentity({ data: { identity: { operatingSystem: 'openwrt', label: 'OpenWrt' } } })
    ).toMatchObject({ operatingSystem: 'openwrt', label: 'OpenWrt' })
  })

  it('lê a identidade persistida nos detalhes da descoberta', () => {
    expect(
      discoveryIdentity({
        data: { details: { identity: { operatingSystem: 'other', label: 'Outro sistema' } } },
      })
    ).toMatchObject({ operatingSystem: 'other', label: 'Outro sistema' })
  })
})

describe('discoveryDeviceName', () => {
  it('prioriza sysName de SNMP sobre hostname e mdnsName', () => {
    expect(
      discoveryDeviceName({
        hostname: 'my-host',
        mdnsName: 'my-mdns',
        data: { identity: { sysName: 'Volt' } },
      })
    ).toBe('Volt')
  })

  it('usa hostname caso sysName não exista', () => {
    expect(
      discoveryDeviceName({
        hostname: 'printer.lan',
        mdnsName: 'printer-mdns',
        data: {},
      })
    ).toBe('printer.lan')
  })

  it('usa mdnsName caso sysName e hostname não existam', () => {
    expect(
      discoveryDeviceName({
        hostname: null,
        mdnsName: 'camera.local',
        data: {},
      })
    ).toBe('camera.local')
  })

  it('retorna null quando nenhum nome estiver disponível', () => {
    expect(
      discoveryDeviceName({
        hostname: null,
        mdnsName: null,
        data: {},
      })
    ).toBeNull()
  })
})

describe('discoveryDeviceTypeInfo', () => {
  it('reconhece câmera e retorna ícone e cor correspondentes', () => {
    const info = discoveryDeviceTypeInfo('camera')
    expect(info.isKnown).toBe(true)
    expect(info.label).toBe('Câmera')
    expect(info.icon).toBe('mdi-cctv')
  })

  it('reconhece web_device como Dispositivo Web', () => {
    const info = discoveryDeviceTypeInfo('web_device')
    expect(info.isKnown).toBe(true)
    expect(info.label).toBe('Dispositivo Web')
    expect(info.icon).toBe('mdi-web')
  })

  it('reconhece unknown como não conhecido', () => {
    const info = discoveryDeviceTypeInfo('unknown')
    expect(info.isKnown).toBe(false)
    expect(info.label).toBe('Desconhecido')
  })
})
