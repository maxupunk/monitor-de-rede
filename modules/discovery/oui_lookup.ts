/**
 * Lookup leve de vendor a partir dos 3 primeiros octetos do MAC (OUI).
 *
 * Mantém uma lista embutida dos principais fabricantes de equipamentos de rede
 * para evitar depender de bases externas ou grandes pacotes npm. O mapa pode
 * ser expandido conforme necessidade.
 */

const OUI_MAP: Record<string, string> = {
  '00:00:5E': 'ICANN / Virtualization',
  '00:01:42': 'Cisco Systems',
  '00:03:7F': 'Intel',
  '00:04:4D': 'Avaya',
  '00:08:5C': 'Huawei',
  '00:0A:CD': 'IBM',
  '00:0B:86': 'Ubiquiti Networks',
  '00:0C:29': 'VMware',
  '00:0C:41': 'Huawei',
  '00:0E:CF': 'Dell',
  '00:0F:34': 'Hewlett-Packard',
  '00:10:18': 'Intel',
  '00:11:32': 'Synology',
  '00:11:85': 'Hewlett-Packard',
  '00:13:20': 'MikroTik',
  '00:13:72': 'MikroTik',
  '00:13:CE': 'Siemens',
  '00:14:22': 'Dell',
  '00:15:17': 'Hewlett-Packard',
  '00:15:5D': 'Microsoft',
  '00:15:C5': 'MikroTik',
  '00:16:17': 'Hewlett-Packard',
  '00:18:8B': 'MikroTik',
  '00:18:DE': 'Hewlett-Packard',
  '00:19:06': 'MikroTik',
  '00:19:99': 'Hewlett-Packard',
  '00:19:BB': 'MikroTik',
  '00:19:D1': 'Hewlett-Packard',
  '00:1A:3F': 'MikroTik',
  '00:1A:4D': 'Hewlett-Packard',
  '00:1A:70': 'Hewlett-Packard',
  '00:1B:11': 'Hewlett-Packard',
  '00:1B:21': 'Intel',
  '00:1B:78': 'Hewlett-Packard',
  '00:1C:C4': 'Hewlett-Packard',
  '00:1D:60': 'Hewlett-Packard',
  '00:1E:0B': 'Hewlett-Packard',
  '00:1E:37': 'Hewlett-Packard',
  '00:1E:C1': 'Realtek',
  '00:1F:29': 'Hewlett-Packard',
  '00:1F:F3': 'McAfee',
  '00:21:5A': 'Hewlett-Packard',
  '00:21:9B': 'Hewlett-Packard',
  '00:22:19': 'Hewlett-Packard',
  '00:22:64': 'Hewlett-Packard',
  '00:23:7D': 'Hewlett-Packard',
  '00:24:81': 'Hewlett-Packard',
  '00:25:B3': 'Hewlett-Packard',
  '00:26:55': 'Hewlett-Packard',
  '00:50:56': 'VMware',
  '00:60:B0': 'Hewlett-Packard',
  '00:80:48': 'Hewlett-Packard',
  '00:90:0B': 'Hewlett-Packard',
  '00:90:7A': 'Hewlett-Packard',
  '00:A0:C9': 'Intel',
  '00:A0:D1': 'Hewlett-Packard',
  '00:AA:02': 'Hewlett-Packard',
  '00:BB:3A': 'Hewlett-Packard',
  '00:C0:4F': 'Dell',
  '00:E0:4C': 'Realtek',
  '00:E0:98': 'MikroTik',
  '04:18:D6': 'Ubiquiti Networks',
  '08:00:07': 'Apple',
  '18:E8:DD': 'Hewlett-Packard',
  '24:4B:FE': 'Hewlett-Packard',
  '28:80:23': 'Hewlett-Packard',
  '2C:59:8A': 'Hewlett-Packard',
  '2C:76:8A': 'Hewlett-Packard',
  '30:E1:71': 'Hewlett-Packard',
  '3C:5A:37': 'Hewlett-Packard',
  '3C:61:04': 'Espressif',
  '44:A8:42': 'Hewlett-Packard',
  '48:4D:7E': 'Hewlett-Packard',
  '4C:CC:6A': 'Hewlett-Packard',
  '54:13:10': 'Hewlett-Packard',
  '58:20:B1': 'Hewlett-Packard',
  '60:E3:27': 'Hewlett-Packard',
  '64:51:06': 'Hewlett-Packard',
  '68:B5:99': 'Hewlett-Packard',
  '6C:3B:E5': 'Hewlett-Packard',
  '70:54:D2': 'Hewlett-Packard',
  '74:46:A0': 'Hewlett-Packard',
  '78:AC:C0': 'Hewlett-Packard',
  '7C:61:93': 'Hewlett-Packard',
  '80:C1:6E': 'Hewlett-Packard',
  '84:34:97': 'Hewlett-Packard',
  '88:51:FB': 'Hewlett-Packard',
  '8C:DC:D4': 'Hewlett-Packard',
  '90:E2:BA': 'Hewlett-Packard',
  '94:57:A5': 'Hewlett-Packard',
  '98:4B:E1': 'Hewlett-Packard',
  '9C:DC:71': 'Hewlett-Packard',
  'A0:48:1C': 'Hewlett-Packard',
  'A4:12:42': 'Hewlett-Packard',
  'A4:5D:36': 'Hewlett-Packard',
  'A8:66:7F': 'Hewlett-Packard',
  'AC:16:2D': 'Hewlett-Packard',
  'AC:A3:1E': 'Hewlett-Packard',
  'B0:0B:D5': 'Hewlett-Packard',
  'B4:8C:9D': 'Hewlett-Packard',
  'B8:83:03': 'Hewlett-Packard',
  'BC:83:A7': 'Hewlett-Packard',
  'C0:2E:25': 'Hewlett-Packard',
  'C0:91:34': 'Hewlett-Packard',
  'C4:34:6B': 'Hewlett-Packard',
  'C8:08:73': 'Hewlett-Packard',
  'C8:CB:B8': 'Hewlett-Packard',
  'CC:3E:5F': 'Hewlett-Packard',
  'D0:67:E5': 'Hewlett-Packard',
  'D4:85:64': 'Hewlett-Packard',
  'D8:9D:67': 'Hewlett-Packard',
  'DC:4A:3E': 'Hewlett-Packard',
  'E0:07:1B': 'Hewlett-Packard',
  'E0:70:EA': 'Hewlett-Packard',
  'E4:11:5B': 'Hewlett-Packard',
  'E8:39:35': 'Hewlett-Packard',
  'EC:B1:D7': 'Hewlett-Packard',
  'F0:92:1C': 'Hewlett-Packard',
  'F4:30:B9': 'Hewlett-Packard',
  'F8:0D:E0': 'Hewlett-Packard',
  'FC:15:B4': 'Hewlett-Packard',
}

/**
 * Normaliza um MAC address para o formato `AA:BB:CC` dos 3 primeiros octetos.
 */
function normalizeMacPrefix(macAddress: string): string | null {
  const clean = macAddress
    .toUpperCase()
    .replace(/[^0-9A-F]/g, '')
    .slice(0, 6)

  if (clean.length < 6) return null

  return `${clean.slice(0, 2)}:${clean.slice(2, 4)}:${clean.slice(4, 6)}`
}

/**
 * Retorna o nome do fabricante para o MAC informado, ou `null` se o OUI não
 * estiver na base local.
 */
export function lookupVendor(macAddress: string): string | null {
  const prefix = normalizeMacPrefix(macAddress)
  if (!prefix) return null
  return OUI_MAP[prefix] ?? null
}
