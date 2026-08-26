import { describe, expect, it } from 'vitest'
import {
  buildProvisionAddressOptions,
  createLogSetupTarget,
  isProvisionSessionCurrent,
  normalizeComboboxAddress,
  observedApplicationAddress,
  resolveProvisionOperatingSystem,
  sameProvisionAddress,
} from '@/utils/syslogProvision'

const usable = [
  {
    id: 'lan',
    label: 'Rede local',
    description: 'Equipamentos na LAN',
    value: '192.168.1.10',
  },
  {
    id: 'vpn',
    label: 'Túnel VPN',
    description: 'Equipamentos no WireGuard',
    value: '10.8.0.1',
  },
]

describe('campo único do destino de Syslog', () => {
  it('normaliza seleção conhecida e texto livre para o mesmo valor', () => {
    expect(normalizeComboboxAddress({ value: ' 10.8.0.1 ' })).toBe('10.8.0.1')
    expect(normalizeComboboxAddress(' netmonitor.exemplo ')).toBe('netmonitor.exemplo')
  })

  it('coloca a sugestão do backend no primeiro lugar', () => {
    const options = buildProvisionAddressOptions(usable, 'vpn')
    expect(options.map((option) => option.value)).toEqual(['10.8.0.1', '192.168.1.10'])
    expect(options[0].suggested).toBe(true)
  })

  it('inclui o endereço observado pelo backend mesmo antes de ele existir no catálogo', () => {
    const options = buildProvisionAddressOptions(usable.slice(1), 'lan', {
      value: '10.0.0.10',
      label: 'Rede local',
      description: 'Endereço externo antes da bridge do Docker',
    })
    expect(options[0]).toEqual({
      value: '10.0.0.10',
      title: 'Rede local — 10.0.0.10',
      subtitle: 'Endereço externo antes da bridge do Docker',
      suggested: true,
    })
  })

  it('preserva o OpenWrt do snapshot quando a nova sonda ainda não respondeu', () => {
    expect(resolveProvisionOperatingSystem('', 'openwrt')).toBe('openwrt')
    expect(resolveProvisionOperatingSystem('routeros', 'openwrt')).toBe('routeros')
  })

  it('envia ao backend o host externo visto pela aplicação', () => {
    expect(observedApplicationAddress({ hostname: ' 10.0.0.10 ' })).toBe('10.0.0.10')
    expect(observedApplicationAddress({ hostname: '' })).toBeNull()
  })

  it('remove endereços equivalentes sem depender do id', () => {
    const options = buildProvisionAddressOptions(
      [...usable, { ...usable[0], id: 'custom:lan', label: 'LAN duplicada' }],
      null
    )
    expect(options).toHaveLength(2)
    expect(sameProvisionAddress(' NETMONITOR.EXEMPLO ', 'netmonitor.exemplo')).toBe(true)
  })

  it('fotografa exatamente o dispositivo salvo e conserva a detecção automática', () => {
    const target = createLogSetupTarget(
      7,
      {
        id: 42,
        name: 'OpenWrt da borda',
        ipAddress: ' 10.0.0.1 ',
        effectiveOperatingSystem: 'linux',
      },
      'auto',
      'openwrt'
    )

    expect(target).toEqual({
      sessionId: 7,
      id: 42,
      name: 'OpenWrt da borda',
      host: '10.0.0.1',
      operatingSystem: 'openwrt',
    })
    expect(Object.isFrozen(target)).toBe(true)
  })

  it('rejeita respostas de uma sessão ou dispositivo anterior', () => {
    expect(isProvisionSessionCurrent(true, 2, 2, 42, 42)).toBe(true)
    expect(isProvisionSessionCurrent(true, 1, 2, 42, 42)).toBe(false)
    expect(isProvisionSessionCurrent(true, 2, 2, 1, 2)).toBe(false)
    expect(isProvisionSessionCurrent(false, 2, 2, 42, 42)).toBe(false)
  })
})
