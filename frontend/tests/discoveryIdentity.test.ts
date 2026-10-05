import { describe, expect, it } from 'vitest'
import {
  compareIpAddresses,
  discoveryDescription,
  discoveryDeviceName,
  discoveryIdentity,
  discoveryLaya,
  discoveryPortLabel,
  discoveryReasons,
  discoverySources,
  discoveryTypeMeta,
  discoveryVendorLabel,
} from '@/utils/discoveryPresentation'
import { deviceTypeMeta, normalizeDeviceType } from '@/utils/deviceTypes'

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

describe('deviceTypeMeta', () => {
  it('reconhece câmera e retorna ícone e cor correspondentes', () => {
    const info = deviceTypeMeta('camera')
    expect(info.isKnown).toBe(true)
    expect(info.label).toBe('Câmera')
    expect(info.icon).toBe('mdi-cctv')
  })

  it('reconhece web_device como Dispositivo Web', () => {
    const info = deviceTypeMeta('web_device')
    expect(info.isKnown).toBe(true)
    expect(info.label).toBe('Dispositivo web')
    expect(info.icon).toBe('mdi-web')
  })

  it('reconhece unknown como não conhecido', () => {
    const info = deviceTypeMeta('unknown')
    expect(info.isKnown).toBe(false)
    expect(info.label).toBe('Desconhecido')
  })

  it('traz os tipos novos da descoberta e traduz os antigos', () => {
    expect(deviceTypeMeta('iot').label).toBe('IoT / automação')
    expect(deviceTypeMeta('media').icon).toBe('mdi-television-classic')
    expect(deviceTypeMeta('access_point').id).toBe('ap')
  })
})

describe('evidências da descoberta', () => {
  const live = {
    ipAddress: '10.0.0.30',
    deviceType: 'media',
    openPorts: [8008, 8009],
    data: {
      sources: ['icmp', 'mdns', 'http'],
      services: { '8008': 'Google Cast' },
      mdns: { instances: ['TV da Sala'], model: 'Chromecast Ultra' },
      classification: { reasons: ['Anuncia Google Cast por mDNS'], alternative: 'iot' },
    },
  }

  it('lê nome dado pelo dono, modelo, motivos e fontes', () => {
    expect(discoveryDeviceName(live)).toBe('TV da Sala')
    expect(discoveryDescription(live)).toBe('Chromecast Ultra')
    expect(discoveryReasons(live)).toEqual(['Anuncia Google Cast por mDNS'])
    expect(discoverySources(live)).toEqual(['Ping', 'mDNS', 'Web'])
    expect(discoveryPortLabel(live, 8008)).toBe('Google Cast')
    expect(discoveryTypeMeta(live).label).toBe('Smart TV / mídia')
  })

  it('lê o mesmo do resultado persistido', () => {
    const persisted = { ...live, data: { openPorts: [8008], details: live.data } }
    expect(discoveryDeviceName(persisted)).toBe('TV da Sala')
    expect(discoveryReasons(persisted)).toHaveLength(1)
  })

  it('explica o fabricante ausente de um MAC aleatório', () => {
    expect(discoveryVendorLabel({ data: { macPrivate: true } })).toBe('MAC aleatório (privacidade)')
    expect(discoveryVendorLabel({ data: { details: { macPrivate: true } } })).toBe(
      'MAC aleatório (privacidade)'
    )
    expect(discoveryVendorLabel({ data: {} })).toBe('Fabricante não identificado')
    expect(discoveryVendorLabel({ vendor: 'Espressif', data: {} })).toBe('Espressif')
  })

  it('ordena IPs numericamente', () => {
    expect(['10.0.0.10', '10.0.0.9', '10.0.0.100'].sort(compareIpAddresses)).toEqual([
      '10.0.0.9',
      '10.0.0.10',
      '10.0.0.100',
    ])
  })
})

describe('discoveryLaya', () => {
  const camera = { value: 'camera', confidence: 91, model: 'laya:multilingual' }

  it('lê o palpite do stream ao vivo e do resultado persistido', () => {
    expect(discoveryLaya({ data: { laya: { deviceType: camera } } })?.deviceType).toEqual(camera)
    expect(
      discoveryLaya({ data: { details: { laya: { deviceType: camera } } } })?.deviceType
    ).toEqual(camera)
    expect(discoveryLaya({ data: { details: {} } })).toBeNull()
  })
})

describe('normalizeDeviceType', () => {
  it('traduz o vocabulário da descoberta para o do cadastro', () => {
    expect(normalizeDeviceType('access_point')).toBe('ap')
    expect(normalizeDeviceType('iot')).toBe('iot')
    expect(normalizeDeviceType('camera')).toBe('camera')
    expect(normalizeDeviceType('web_device')).toBe('other')
    expect(normalizeDeviceType('unknown')).toBe('other')
    expect(normalizeDeviceType(null)).toBe('other')
  })
})
