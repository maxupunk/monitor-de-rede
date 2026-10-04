/**
 * Tipos de dispositivo que o cadastro aceita — espelho de
 * `backend/src/services/devices/kinds.rs`. A descoberta classifica com estes
 * mesmos ids; só `web_device` e `unknown` são dela (dúvida), e dados antigos
 * ainda trazem `access_point`, `gateway` e `unmanaged_switch`.
 */
export const DEVICE_TYPE_OPTIONS = [
  { value: 'router', title: 'Roteador' },
  { value: 'switch', title: 'Switch' },
  { value: 'firewall', title: 'Firewall' },
  { value: 'ap', title: 'Access point' },
  { value: 'server', title: 'Servidor' },
  { value: 'nas', title: 'Armazenamento (NAS)' },
  { value: 'workstation', title: 'Computador' },
  { value: 'mobile', title: 'Celular / tablet' },
  { value: 'printer', title: 'Impressora' },
  { value: 'camera', title: 'Câmera' },
  { value: 'media', title: 'Smart TV / mídia' },
  { value: 'iot', title: 'IoT / automação' },
  { value: 'voip', title: 'Telefone IP / VoIP' },
  { value: 'ups', title: 'Nobreak / energia' },
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

export interface DeviceTypePresentation {
  /** Id canônico; `web_device`/`unknown` só aparecem na descoberta. */
  id: DeviceTypeId | 'web_device' | 'unknown'
  label: string
  icon: string
  color: string
  /** `false` quando ninguém soube dizer o que é o aparelho. */
  isKnown: boolean
}

/** Ícone e cor de cada tipo — cores vivas, nunca cinza (AGENTS §12). */
const TYPE_LOOK: Record<DeviceTypeId, { icon: string; color: string }> = {
  router: { icon: 'mdi-router-network', color: 'primary' },
  switch: { icon: 'mdi-switch', color: 'teal' },
  firewall: { icon: 'mdi-shield-network', color: 'red' },
  ap: { icon: 'mdi-access-point', color: 'cyan' },
  server: { icon: 'mdi-server', color: 'deep-purple' },
  nas: { icon: 'mdi-nas', color: 'indigo' },
  workstation: { icon: 'mdi-desktop-tower-monitor', color: 'blue' },
  mobile: { icon: 'mdi-cellphone', color: 'pink' },
  printer: { icon: 'mdi-printer', color: 'orange' },
  camera: { icon: 'mdi-cctv', color: 'purple' },
  media: { icon: 'mdi-television-classic', color: 'deep-orange' },
  iot: { icon: 'mdi-home-automation', color: 'green' },
  voip: { icon: 'mdi-phone-voip', color: 'light-blue-darken-2' },
  ups: { icon: 'mdi-battery-charging-high', color: 'amber-darken-2' },
  other: { icon: 'mdi-devices', color: 'secondary' },
}

/** Rótulo, ícone e cor de qualquer tipo — do cadastro, da descoberta ou antigo. */
export function deviceTypeMeta(type?: string | null): DeviceTypePresentation {
  const key = (type ?? '').trim().toLowerCase()
  if (key === 'web_device') {
    return {
      id: 'web_device',
      label: 'Dispositivo web',
      icon: 'mdi-web',
      color: 'blue',
      isKnown: true,
    }
  }
  if (!key || key === 'unknown') {
    return {
      id: 'unknown',
      label: 'Desconhecido',
      icon: 'mdi-help-network-outline',
      color: 'warning',
      isKnown: false,
    }
  }
  const id = normalizeDeviceType(key)
  return { id, label: deviceTypeLabel(id), ...TYPE_LOOK[id], isKnown: true }
}
