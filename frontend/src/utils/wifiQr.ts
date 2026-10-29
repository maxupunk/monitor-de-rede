/**
 * O QR code que põe um celular numa rede Wi-Fi pela câmera (o formato
 * `WIFI:` do ZXing, que Android e iOS leem). Funções puras: o texto do código
 * e o desenho dos módulos; quem lê a senha e mostra é o `WifiQrDialog`.
 */

/** Como o celular entra: com senha WPA (WPA2 e WPA3), WEP ou sem senha. */
export type WifiQrSecurity = 'WPA' | 'WEP' | 'nopass'

export interface WifiQrNetwork {
  ssid: string
  /** A segurança como o roteador a chama (`psk2`, `sae-mixed`, `owe`, `none`…). */
  encryption: string
  password?: string
  hidden?: boolean
}

/**
 * A segurança do QR a partir da do roteador. A rede aberta protegida (OWE)
 * entra sem senha — o celular negocia a criptografia sozinho.
 */
export function wifiQrSecurity(encryption: string): WifiQrSecurity {
  const value = encryption.trim().toLowerCase()
  if (value === '' || value === 'none' || value.startsWith('owe')) return 'nopass'
  if (value.startsWith('wep')) return 'WEP'
  return 'WPA'
}

/** A rede pede senha para entrar? */
export function wifiNeedsPassword(encryption: string): boolean {
  return wifiQrSecurity(encryption) !== 'nopass'
}

/** `\`, `;`, `,`, `:` e `"` têm significado no formato e vão escapados. */
function escape(text: string): string {
  return text.replace(/([\\;,:"])/g, '\\$1')
}

/** O texto do QR code (`WIFI:T:WPA;S:Casa;P:segredo;;`). */
export function wifiQrPayload(network: WifiQrNetwork): string {
  const security = wifiQrSecurity(network.encryption)
  const parts = [`T:${security}`, `S:${escape(network.ssid)}`]
  if (security !== 'nopass') parts.push(`P:${escape(network.password ?? '')}`)
  if (network.hidden) parts.push('H:true')
  return `WIFI:${parts.join(';')};;`
}

/**
 * Os módulos escuros de uma matriz de QR como um `d` de `<path>` SVG, um
 * quadrado de 1×1 por módulo — o `viewBox` é `0 0 tamanho tamanho`.
 */
export function qrPath(matrix: boolean[][]): string {
  const parts: string[] = []
  matrix.forEach((row, y) =>
    row.forEach((dark, x) => {
      if (dark) parts.push(`M${x} ${y}h1v1h-1z`)
    })
  )
  return parts.join('')
}
