import { describe, expect, it } from 'vitest'
import {
  linkInterfaceCandidates,
  scanInterfaceCandidates,
  type ScanResult,
} from '@/stores/deviceDetail'

describe('scanInterfaceCandidates', () => {
  it('junta o tráfego do escaneamento a cada interface', () => {
    const scan: ScanResult = {
      systemInfo: {},
      cpuInfo: {},
      memoryInfo: {},
      interfaces: [
        {
          ifIndex: 5,
          ifName: 'ether5',
          ifAlias: 'WAN-Vivo',
          ifType: 6,
          ifSpeed: 1_000_000_000,
          ifOperStatus: 'up',
          isMonitored: false,
        },
        { ifIndex: 6, ifName: 'ether6', isMonitored: false },
      ],
      traffic: [{ ifIndex: 5, inOctets: 900, outOctets: 100 }],
      collectorErrors: {},
      hasCpuMonitor: false,
      hasMemoryMonitor: false,
      snmpResponded: true,
    }

    const [wan, idle] = scanInterfaceCandidates(scan)

    expect(wan).toMatchObject({
      ifIndex: 5,
      name: 'ether5',
      alias: 'WAN-Vivo',
      operUp: true,
      inOctets: 900,
      outOctets: 100,
    })
    expect(idle).toMatchObject({ ifIndex: 6, operUp: null, inOctets: null })
  })
})

describe('linkInterfaceCandidates', () => {
  it('usa a posição como chave quando o cadastro não tem ifIndex', () => {
    const [first, second] = linkInterfaceCandidates([
      { name: 'ether1', operStatus: 'DOWN' },
      { name: 'ether2', ifIndex: 2, operStatus: 'up' },
    ])
    expect(first).toMatchObject({ ifIndex: -1, operUp: false })
    expect(second).toMatchObject({ ifIndex: 2, operUp: true })
  })
})
