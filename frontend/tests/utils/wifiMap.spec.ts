import { describe, expect, it } from 'vitest'
import { paletteFromTheme } from '@/utils/topology/style'
import { formatDistanceRange } from '@/utils/formatters'
import { deviceKind } from '@/utils/wifiMap/deviceKind'
import {
  apNodeId,
  buildWifiMapElements,
  linkEdgeId,
  macOfElement,
  stationNodeId,
} from '@/utils/wifiMap/elements'
import {
  AP_CORE,
  CAP_METERS,
  MAX_RADIUS,
  dedupeByMac,
  layoutWifiMap,
  radiusFor,
  seedOf,
  type MapAp,
} from '@/utils/wifiMap/layout'
import { linkColor, linkWidth } from '@/utils/wifiMap/style'
import {
  emptyFilter,
  filterStations,
  stationRows,
  type WifiStationRow,
} from '@/utils/wifiPresentation'
import { snapshot, station } from './wifiFixtures'

const palette = paletteFromTheme({
  dark: false,
  colors: {
    background: '#FFFFFF',
    surface: '#FFFFFF',
    'on-surface': '#000000',
    primary: '#1565C0',
    success: '#2E7D32',
    warning: '#F9A825',
    error: '#C62828',
    info: '#0277BD',
  },
})

function row(mac: string, deviceId: number, extra: Partial<WifiStationRow> = {}): WifiStationRow {
  return {
    ...station(mac, -60),
    deviceId,
    deviceName: `AP ${deviceId}`,
    canPing: true,
    ...extra,
  }
}

const ap = (deviceId: number, name = `AP ${deviceId}`): MapAp => ({
  deviceId,
  deviceName: name,
  status: 'ok',
  error: null,
})

describe('escala do mapa Wi-Fi', () => {
  it('cresce com a distância, fica fora do AP e tem teto', () => {
    expect(radiusFor(0)).toBe(AP_CORE)
    expect(radiusFor(2)).toBeLessThan(radiusFor(5))
    expect(radiusFor(5)).toBeLessThan(radiusFor(20))
    expect(radiusFor(CAP_METERS)).toBeCloseTo(MAX_RADIUS)
    expect(radiusFor(500)).toBeCloseTo(MAX_RADIUS)
    expect(radiusFor(null)).toBeGreaterThan(AP_CORE)
    expect(radiusFor(null)).toBeLessThan(MAX_RADIUS)
  })

  it('o ângulo de cada aparelho é estável pelo MAC', () => {
    expect(seedOf('aa:bb:cc:dd:ee:01')).toBe(seedOf('aa:bb:cc:dd:ee:01'))
    expect(seedOf('aa:bb:cc:dd:ee:01')).not.toBe(seedOf('aa:bb:cc:dd:ee:02'))
    const seed = seedOf('qualquer')
    expect(seed).toBeGreaterThanOrEqual(0)
    expect(seed).toBeLessThan(1)
  })
})

describe('layoutWifiMap', () => {
  it('põe cada AP na sua célula, sem sobreposição', () => {
    const layout = layoutWifiMap([ap(2, 'Cozinha'), ap(1, 'Sala'), ap(3, 'Varanda')], [])
    expect(layout.aps.map((item) => item.ap.deviceName)).toEqual(['Cozinha', 'Sala', 'Varanda'])
    for (const [index, a] of layout.aps.entries()) {
      for (const b of layout.aps.slice(index + 1)) {
        expect(Math.hypot(a.x - b.x, a.y - b.y)).toBeGreaterThan(MAX_RADIUS * 2)
      }
    }
    expect(layout.rings.filter((ring) => ring.deviceId === 1).map((ring) => ring.meters)).toEqual([
      5, 10, 20, 40,
    ])
  })

  it('aparelho fica à distância estimada do AP dele', () => {
    const perto = row('aa:00:00:00:00:01', 1, {
      distance: { meters: 3, minM: 2, maxM: 5, calibrated: true, environment: 'office' },
    })
    const longe = row('aa:00:00:00:00:02', 1, {
      distance: { meters: 30, minM: 20, maxM: 45, calibrated: true, environment: 'office' },
    })
    const layout = layoutWifiMap([ap(1)], [perto, longe])
    const center = layout.aps[0]
    const radius = (mac: string) => {
      const placed = layout.stations.find((item) => item.row.mac === mac)!
      return Math.hypot(placed.x - center.x, placed.y - center.y)
    }
    expect(radius(perto.mac)).toBeCloseTo(radiusFor(3))
    expect(radius(longe.mac)).toBeCloseTo(radiusFor(30))
  })

  it('quem entra não embaralha a ordem de quem já estava', () => {
    const fixos = ['aa:00:00:00:00:01', 'aa:00:00:00:00:02', 'aa:00:00:00:00:03'].map((mac) =>
      row(mac, 1)
    )
    const ordem = (layout: ReturnType<typeof layoutWifiMap>) => {
      const center = layout.aps[0]!
      return layout.stations
        .filter((placed) => fixos.some((item) => item.mac === placed.row.mac))
        .map((placed) => ({
          mac: placed.row.mac,
          angle:
            (Math.atan2(placed.y - center.y, placed.x - center.x) + Math.PI * 2.5) % (Math.PI * 2),
        }))
        .sort((a, b) => a.angle - b.angle)
        .map((item) => item.mac)
    }
    const antes = ordem(layoutWifiMap([ap(1)], fixos))
    const depois = ordem(layoutWifiMap([ap(1)], [...fixos, row('aa:00:00:00:00:09', 1)]))
    expect(depois).toEqual(antes)
  })

  it('muitos aparelhos no mesmo AP não se empilham', () => {
    const muitos = Array.from({ length: 60 }, (_, index) =>
      row(`aa:00:00:00:01:${index.toString(16).padStart(2, '0')}`, 1)
    )
    const layout = layoutWifiMap([ap(1)], muitos)
    const points = layout.stations
    let tooClose = 0
    for (const [index, a] of points.entries()) {
      for (const b of points.slice(index + 1)) {
        if (Math.hypot(a.x - b.x, a.y - b.y) < 20) tooClose += 1
      }
    }
    expect(tooClose).toBe(0)
  })

  it('aparelho visto por dois APs no roaming aparece uma vez, no AP atual', () => {
    const sobra = row('aa:00:00:00:00:01', 1, { inactiveMs: 45_000, signalDbm: -50 })
    const atual = row('aa:00:00:00:00:01', 2, { inactiveMs: 200, signalDbm: -70 })
    expect(dedupeByMac([sobra, atual]).map((item) => item.deviceId)).toEqual([2])
    const empate = row('aa:00:00:00:00:01', 2, { inactiveMs: 100, signalDbm: -70 })
    const melhor = row('aa:00:00:00:00:01', 1, { inactiveMs: 300, signalDbm: -55 })
    expect(dedupeByMac([empate, melhor]).map((item) => item.deviceId)).toEqual([1])
  })
})

describe('elementos do mapa Wi-Fi', () => {
  it('o aparelho leva o estado em classes, e a cor nunca é o único sinal', () => {
    const vigiado = row('aa:00:00:00:00:01', 1, { health: 'power_save' })
    const ruim = row('aa:00:00:00:00:02', 1, { health: 'critical', linkScore: 0.1 })
    const elements = buildWifiMapElements({
      layout: layoutWifiMap([ap(1)], [vigiado, ruim]),
      palette,
      watched: new Map([[vigiado.mac, { name: 'Câmera da porta' }]]),
      selectedMac: ruim.mac,
    })
    const byId = new Map(elements.map((element) => [element.data.id, element]))
    const camera = byId.get(stationNodeId(vigiado.mac))!
    expect(camera.classes).toContain('watched')
    expect(camera.classes).toContain('sleeping')
    expect(camera.data.label).toBe('Câmera da porta\n-60 dBm')
    const critico = byId.get(stationNodeId(ruim.mac))!
    expect(critico.classes).toContain('attention')
    expect(critico.classes).toContain('selected')
    expect(critico.data.color).toBe(linkColor(0.1, palette))
    const linha = byId.get(linkEdgeId(1, ruim.mac))!
    expect(linha.data.source).toBe(apNodeId(1))
    expect(linha.classes).toContain('focus')
    expect(linha.data.distanceLabel).toBe('~4–9 m')
    expect(byId.get(apNodeId(1))!.data.label).toBe('AP 1\n2 aparelhos')
  })

  it('a troca de AP troca a linha (o id leva o AP)', () => {
    expect(linkEdgeId(1, 'aa:bb:cc:dd:ee:01')).not.toBe(linkEdgeId(2, 'aa:bb:cc:dd:ee:01'))
    expect(macOfElement(linkEdgeId(12, 'aa:bb:cc:dd:ee:01'))).toBe('aa:bb:cc:dd:ee:01')
    expect(macOfElement(stationNodeId('aa:bb:cc:dd:ee:01'))).toBe('aa:bb:cc:dd:ee:01')
    expect(macOfElement(apNodeId(3))).toBeNull()
  })

  it('AP com leitura falhando aparece marcado e escrito', () => {
    const elements = buildWifiMapElements({
      layout: layoutWifiMap([{ ...ap(1), status: 'failed', error: 'SSH recusado' }], []),
      palette,
      watched: new Map(),
      selectedMac: null,
    })
    const apNode = elements.find((element) => element.data.id === apNodeId(1))!
    expect(apNode.classes).toContain('ap-failed')
    expect(apNode.data.label).toContain('leitura falhou')
  })
})

describe('cor e espessura da linha', () => {
  it('vai do vermelho ao verde passando pelo amarelo', () => {
    expect(linkColor(0, palette).toUpperCase()).toBe('#C62828')
    expect(linkColor(0.5, palette).toUpperCase()).toBe('#F9A825')
    expect(linkColor(1, palette).toUpperCase()).toBe('#2E7D32')
    expect(linkColor(Number.NaN, palette).toUpperCase()).toBe('#C62828')
  })

  it('taxa maior, linha mais grossa', () => {
    expect(linkWidth(null)).toBe(2)
    expect(linkWidth(6e6)).toBe(1.5)
    expect(linkWidth(1200e6)).toBe(6)
    expect(linkWidth(100e6)).toBeGreaterThan(linkWidth(20e6))
  })
})

describe('tipo do aparelho', () => {
  it('pelo nome do DHCP primeiro, depois pelo fabricante', () => {
    expect(deviceKind({ hostname: 'iPhone-de-Joao', vendor: 'Apple' })).toBe('phone')
    expect(deviceKind({ hostname: 'MacBook-Pro', vendor: 'Apple' })).toBe('laptop')
    expect(deviceKind({ hostname: 'iPad', vendor: null })).toBe('tablet')
    expect(deviceKind({ hostname: 'LGwebOSTV', vendor: null })).toBe('tv')
    expect(deviceKind({ hostname: null, vendor: 'Espressif Inc.' })).toBe('iot')
    expect(deviceKind({ hostname: 'tomada-sala', vendor: null })).toBe('iot')
    expect(deviceKind({ hostname: null, vendor: 'Hikvision Digital' })).toBe('camera')
    expect(deviceKind({ hostname: null, vendor: null })).toBe('generic')
  })
})

describe('distância e filtros', () => {
  it('a faixa nunca é um número exato', () => {
    expect(formatDistanceRange(3, 6)).toBe('~3–6 m')
    expect(formatDistanceRange(null, 6)).toBe('—')
  })

  it('filtra por rede e só vigiados, e busca pelo nome dado ao vigiado', () => {
    const rows = stationRows([
      snapshot(1, 'AP Sala', [
        station('aa:00:00:00:00:01', -60, 'healthy', { ssid: 'Casa' }),
        station('aa:00:00:00:00:02', -60, 'healthy', { ssid: 'Visitas' }),
      ]),
    ])
    const watched = new Map([['aa:00:00:00:00:02', { name: 'Câmera portão' }]])
    const only = (change: Partial<ReturnType<typeof emptyFilter>>) =>
      filterStations(rows, { ...emptyFilter(), ...change }, watched).map((item) => item.mac)
    expect(only({ ssid: 'Casa' })).toEqual(['aa:00:00:00:00:01'])
    expect(only({ watchedOnly: true })).toEqual(['aa:00:00:00:00:02'])
    expect(only({ search: 'portão' })).toEqual(['aa:00:00:00:00:02'])
  })
})
