import { describe, expect, it } from 'vitest'
import { deviceToolGroups, type DeviceToolAccess } from '@/utils/deviceTools'

const ALL: DeviceToolAccess = {
  snmpScan: true,
  snmpCollect: true,
  quickPing: true,
  traceroute: true,
  ports: true,
  playbook: true,
}

describe('ferramentas do cabeçalho do equipamento', () => {
  it('agrupa SNMP e diagnóstico, na ordem da tela', () => {
    const groups = deviceToolGroups(ALL)
    expect(groups.map((group) => [group.key, group.tools.map((tool) => tool.key)])).toEqual([
      ['snmp', ['snmpScan', 'snmpCollect']],
      ['diagnostics', ['quickPing', 'traceroute', 'ports', 'playbook']],
    ])
  })

  it('só mostra o que o equipamento permite e some com o grupo vazio', () => {
    const groups = deviceToolGroups({ ...ALL, snmpScan: false, snmpCollect: false, ports: false })
    expect(groups.map((group) => group.key)).toEqual(['diagnostics'])
    expect(groups[0].tools.map((tool) => tool.key)).toEqual(['quickPing', 'traceroute', 'playbook'])
  })

  it('sem nenhuma permissão não há ferramenta', () => {
    const none = Object.fromEntries(Object.keys(ALL).map((key) => [key, false])) as DeviceToolAccess
    expect(deviceToolGroups(none)).toEqual([])
  })
})
