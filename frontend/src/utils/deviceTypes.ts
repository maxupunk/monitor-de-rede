/**
 * Tipos de dispositivo que o cadastro aceita — espelho de
 * `backend/src/services/devices/kinds.rs`. A descoberta ainda devolve
 * `access_point`, `web_device` e `unknown`; `normalizeDeviceType` traduz.
 */
export const DEVICE_TYPE_OPTIONS = [
  { value: 'router', title: 'Roteador' },
  { value: 'switch', title: 'Switch' },
  { value: 'firewall', title: 'Firewall' },
  { value: 'ap', title: 'Access point' },
  { value: 'server', title: 'Servidor' },
  { value: 'printer', title: 'Impressora' },
  { value: 'camera', title: 'Câmera' },
  { value: 'other', title: 'Outro' },
] as const

export type DeviceTypeId = (typeof DEVICE_TYPE_OPTIONS)[number]['value']

/** Tipos de infraestrutura de rede (inclui os sinônimos antigos gravados). */
export const INFRA_DEVICE_TYPES: ReadonlySet<string> = new Set([
  'router',
  'switch',
  'firewall',
  'gateway',
  'unmanaged_switch',
  'ap',
  'access_point',
])

const ALIASES: Record<string, DeviceTypeId> = {
  access_point: 'ap',
  gateway: 'router',
  unmanaged_switch: 'switch',
}

/** Um tipo vindo da descoberta ou de dado antigo, como o select do cadastro aceita. */
export function normalizeDeviceType(type?: string | null): DeviceTypeId {
  const key = (type ?? '').trim().toLowerCase()
  const alias = ALIASES[key]
  if (alias) return alias
  const known = DEVICE_TYPE_OPTIONS.find((option) => option.value === key)
  return known ? known.value : 'other'
}

/** Rótulo em português de um tipo do cadastro. */
export function deviceTypeLabel(type?: string | null): string {
  const id = normalizeDeviceType(type)
  return DEVICE_TYPE_OPTIONS.find((option) => option.value === id)?.title ?? 'Outro'
}
