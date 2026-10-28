import { describe, expect, it } from 'vitest'
import {
  channelText,
  countStations,
  emptyFilter,
  filterStations,
  isWifiDeviceSnapshot,
  isWifiWatchedSnapshot,
  looksFixed,
  needsAttention,
  stationDetails,
  stationName,
  stationRows,
} from '@/utils/wifiPresentation'
import { snapshot, station } from './wifiFixtures'

const rows = stationRows([
  snapshot(1, 'AP Sala', [
    station('aa:bb:cc:dd:ee:01', -50, 'healthy', { hostname: 'notebook' }),
    station('aa:bb:cc:dd:ee:02', -66, 'power_save', { band: '2g' }),
  ]),
  snapshot(2, 'AP Depósito', [
    station('aa:bb:cc:dd:ee:03', -74, 'fair'),
    station('aa:bb:cc:dd:ee:04', -86, 'critical', { vendor: 'Espressif' }),
  ]),
])

describe('apresentação Wi-Fi', () => {
  it('junta os roteadores com quem precisa de atenção primeiro', () => {
    expect(rows.map((row) => row.mac.slice(-2))).toEqual(['04', '03', '02', '01'])
    expect(rows[0]).toMatchObject({ deviceId: 2, deviceName: 'AP Depósito', canPing: true })
  })

  it('repouso não é problema da rede', () => {
    expect(needsAttention('power_save')).toBe(false)
    expect(needsAttention('high_retries')).toBe(true)
    expect(countStations(rows)).toEqual({ all: 4, attention: 2, powerSave: 1 })
  })

  it('filtra por estado, banda, roteador e busca', () => {
    const only = (change: Partial<ReturnType<typeof emptyFilter>>) =>
      filterStations(rows, { ...emptyFilter(), ...change }).map((row) => row.mac.slice(-2))
    expect(only({ health: 'attention' })).toEqual(['04', '03'])
    expect(only({ health: 'power_save' })).toEqual(['02'])
    expect(only({ band: '2g' })).toEqual(['02'])
    expect(only({ deviceId: 1 })).toEqual(['02', '01'])
    expect(only({ search: 'NOTEBOOK' })).toEqual(['01'])
    expect(only({ search: 'depósito' })).toEqual(['04', '03'])
  })

  it('dá nome à estação com o que se sabe dela', () => {
    expect(stationName(station('a', -50, 'healthy', { hostname: 'tv-sala' }))).toBe('tv-sala')
    expect(stationName(station('a', -50, 'healthy', { vendor: 'Apple' }))).toBe('Aparelho Apple')
    expect(stationName(station('a', -50, 'healthy', { randomizedMac: true }))).toBe(
      'Aparelho com MAC privado'
    )
    expect(stationName(station('a', -50))).toBe('Aparelho sem nome')
  })

  it('reconhece o payload do snapshot', () => {
    expect(isWifiDeviceSnapshot(snapshot(1, 'AP', []))).toBe(true)
    expect(isWifiDeviceSnapshot({ deviceId: 1, deviceName: 'AP', status: 'x', stations: [] })).toBe(
      false
    )
    expect(isWifiDeviceSnapshot(null)).toBe(false)
  })

  it('sugere vigiar quem está há um dia conectado com MAC de fábrica', () => {
    expect(looksFixed(station('a', -60, 'healthy', { connectedS: 90_000 }))).toBe(true)
    expect(looksFixed(station('a', -60, 'healthy', { connectedS: 3_600 }))).toBe(false)
    expect(
      looksFixed(station('a', -60, 'healthy', { connectedS: 90_000, randomizedMac: true }))
    ).toBe(false)
    expect(looksFixed(station('a', -60))).toBe(false)
  })

  it('reconhece a lista dos vigiados', () => {
    expect(isWifiWatchedSnapshot({ clients: [] })).toBe(true)
    expect(isWifiWatchedSnapshot({ clients: [{ id: 1, mac: 'aa:bb:cc:dd:ee:01' }] })).toBe(true)
    expect(isWifiWatchedSnapshot({ clients: [{ id: '1' }] })).toBe(false)
    expect(isWifiWatchedSnapshot({})).toBe(false)
  })
})

describe('detalhes da estação', () => {
  it('canal com largura, ou só o canal, ou nada', () => {
    const base = station('aa:bb:cc:dd:ee:09', -60, 'healthy')
    expect(channelText(base)).toBe('canal 36 · 80 MHz')
    expect(channelText({ ...base, channelWidthMhz: null })).toBe('canal 36')
    expect(channelText({ ...base, channel: null })).toBe('')
  })

  it('junta retransmissão e tempo conectado só quando o AP informa', () => {
    const base = station('aa:bb:cc:dd:ee:09', -60, 'healthy')
    expect(stationDetails(base)).toBe('')
    expect(stationDetails({ ...base, txRetriesPct: 12.5 })).toBe('12,5% de retransmissão')
    expect(stationDetails({ ...base, txRetriesPct: 12.5, connectedS: 3_600 })).toMatch(
      /^12,5% de retransmissão · conectado há /
    )
  })
})
