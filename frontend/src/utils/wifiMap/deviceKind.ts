/**
 * Que tipo de aparelho é cada cliente Wi-Fi — só para o ícone do mapa. Não há
 * dado preciso: a dica vem do nome do DHCP e do fabricante do MAC (OUI). Na
 * dúvida, o ícone genérico; nunca um palpite apresentado como fato (o painel
 * mostra o fabricante e o nome de verdade).
 */
import {
  mdiCctv,
  mdiCellphone,
  mdiDevices,
  mdiGamepadVariant,
  mdiHomeAutomation,
  mdiLaptop,
  mdiPrinter,
  mdiSpeaker,
  mdiTablet,
  mdiTelevision,
} from '@mdi/js'
import type { WifiStation } from '@/bindings/WifiStation'

export type WifiDeviceKind =
  | 'phone'
  | 'tablet'
  | 'laptop'
  | 'tv'
  | 'speaker'
  | 'camera'
  | 'printer'
  | 'console'
  | 'iot'
  | 'generic'

export interface WifiDeviceKindPresentation {
  label: string
  /** Classe `mdi-*` (`v-icon`). */
  icon: string
  /** Caminho SVG 24×24 (Cytoscape). */
  path: string
}

export const DEVICE_KIND: Record<WifiDeviceKind, WifiDeviceKindPresentation> = {
  phone: { label: 'Celular', icon: 'mdi-cellphone', path: mdiCellphone },
  tablet: { label: 'Tablet', icon: 'mdi-tablet', path: mdiTablet },
  laptop: { label: 'Computador', icon: 'mdi-laptop', path: mdiLaptop },
  tv: { label: 'TV ou streaming', icon: 'mdi-television', path: mdiTelevision },
  speaker: { label: 'Caixa de som', icon: 'mdi-speaker', path: mdiSpeaker },
  camera: { label: 'Câmera', icon: 'mdi-cctv', path: mdiCctv },
  printer: { label: 'Impressora', icon: 'mdi-printer', path: mdiPrinter },
  console: { label: 'Videogame', icon: 'mdi-gamepad-variant', path: mdiGamepadVariant },
  iot: { label: 'Automação / IoT', icon: 'mdi-home-automation', path: mdiHomeAutomation },
  generic: { label: 'Aparelho', icon: 'mdi-devices', path: mdiDevices },
}

/** Pistas no nome do DHCP, na ordem em que são conferidas (a primeira vence). */
const HOSTNAME_HINTS: ReadonlyArray<[RegExp, WifiDeviceKind]> = [
  [/ipad|tablet|\btab\b|galaxy-?tab|kindle|fire-?hd/, 'tablet'],
  [
    /iphone|android|galaxy|pixel|redmi|xiaomi|moto|motorola|poco|oneplus|huawei|celular|phone|sm-[a-z]\d/,
    'phone',
  ],
  [
    /macbook|laptop|notebook|desktop|\bpc\b|-pc\b|thinkpad|ideapad|vivobook|zenbook|inspiron|latitude|dell|lenovo|surface|imac/,
    'laptop',
  ],
  [
    /\btv\b|-tv|tv-|smarttv|bravia|webos|tizen|roku|chromecast|firetv|fire-tv|appletv|apple-tv|androidtv|mibox|tvbox/,
    'tv',
  ],
  [/echo|alexa|sonos|homepod|nest-?(mini|audio)|google-?home|speaker/, 'speaker'],
  [
    /cam\b|camera|camara|ipc|hikvision|dahua|intelbras-?(im|vip)|ezviz|imou|doorbell|campainha/,
    'camera',
  ],
  [/print|impressora|epson|brother|canon|laserjet|officejet|deskjet|\bhp[-_]/, 'printer'],
  [/playstation|\bps[45]\b|xbox|nintendo|switch/, 'console'],
  [
    /esp[-_]?\d|esp32|esp8266|tasmota|shelly|sonoff|tuya|smartlife|broadlink|tomada|lampada|lamp|bulb|plug|sensor|wled|zigbee/,
    'iot',
  ],
]

/** Pistas no fabricante (OUI). */
const VENDOR_HINTS: ReadonlyArray<[RegExp, WifiDeviceKind]> = [
  [/espressif|tuya|shelly|sonoff|itead|broadlink|signify|philips lighting|tp-link.*smart/, 'iot'],
  [/hikvision|dahua|ezviz|axis comm|hangzhou.*vision/, 'camera'],
  [/seiko epson|brother|canon|hewlett|\bhp\b|lexmark|xerox|kyocera/, 'printer'],
  [/roku|vizio|tcl|hisense|lg electronics|sony visual/, 'tv'],
  [/sonos|amazon/, 'speaker'],
  [/nintendo|sony interactive|microsoft.*xbox/, 'console'],
  [/apple|samsung|motorola|xiaomi|huawei|oneplus|oppo|vivo mobile|realme|google/, 'phone'],
  [/intel|liteon|azurewave|dell|lenovo|asustek|hon hai|quanta|compal|wistron|pegatron/, 'laptop'],
]

/** O tipo provável do aparelho, pelo nome do DHCP e pelo fabricante. */
export function deviceKind(station: Pick<WifiStation, 'hostname' | 'vendor'>): WifiDeviceKind {
  const hostname = station.hostname?.toLowerCase() ?? ''
  if (hostname) {
    const byName = HOSTNAME_HINTS.find(([pattern]) => pattern.test(hostname))
    if (byName) return byName[1]
  }
  const vendor = station.vendor?.toLowerCase() ?? ''
  if (vendor) {
    const byVendor = VENDOR_HINTS.find(([pattern]) => pattern.test(vendor))
    if (byVendor) return byVendor[1]
  }
  return 'generic'
}
