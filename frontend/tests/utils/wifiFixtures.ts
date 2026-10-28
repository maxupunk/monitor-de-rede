import type { WifiDeviceSnapshot } from '@/bindings/WifiDeviceSnapshot'
import type { WifiHealth } from '@/bindings/WifiHealth'
import type { WifiStation } from '@/bindings/WifiStation'

export function station(
  mac: string,
  signalDbm: number,
  health: WifiHealth = 'healthy',
  extra: Partial<WifiStation> = {}
): WifiStation {
  return {
    mac,
    randomizedMac: false,
    vendor: null,
    ip: '192.168.1.10',
    hostname: null,
    ssid: 'Loja',
    bssid: null,
    radio: 'phy0',
    band: '5g',
    channel: 36,
    channelWidthMhz: 80,
    signalDbm,
    signalAvgDbm: null,
    noiseDbm: null,
    snrDb: null,
    rxRateBps: null,
    txRateBps: null,
    mcs: null,
    txRetriesPct: null,
    inactiveMs: null,
    connectedS: null,
    signalGrade: 'good',
    snrGrade: null,
    retriesGrade: null,
    health,
    healthReason: null,
    ...extra,
  }
}

export function snapshot(
  deviceId: number,
  deviceName: string,
  stations: WifiStation[]
): WifiDeviceSnapshot {
  return {
    deviceId,
    deviceName,
    status: 'ok',
    readAt: '2026-10-26T12:00:00Z',
    error: null,
    stations,
    canPing: true,
  }
}
