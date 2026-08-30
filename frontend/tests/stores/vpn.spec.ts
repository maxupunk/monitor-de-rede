import { describe, expect, it } from 'vitest'
import {
  vpnProfileIcon,
  vpnProfileLabel,
  vpnProfileSupportsQrCode,
  type VpnProfileOption,
} from '@/stores/vpn'

const profiles: VpnProfileOption[] = [
  {
    profile: 'future-device',
    label: 'Dispositivo Futuro',
    icon: 'mdi-chip',
    supportsQrCode: true,
  },
]

describe('apresentação dos adapters VPN', () => {
  it('usa o catálogo recebido do backend sem lista fixa no frontend', () => {
    expect(vpnProfileLabel('future-device', profiles)).toBe('Dispositivo Futuro')
    expect(vpnProfileIcon('future-device', profiles)).toBe('mdi-chip')
    expect(vpnProfileSupportsQrCode('future-device', profiles)).toBe(true)
  })

  it('mantém fallback legível para snapshot antigo ou adapter removido', () => {
    expect(vpnProfileLabel('legacy', profiles)).toBe('legacy')
    expect(vpnProfileIcon('legacy', profiles)).toBe('mdi-devices')
    expect(vpnProfileSupportsQrCode('legacy', profiles)).toBe(false)
  })
})
