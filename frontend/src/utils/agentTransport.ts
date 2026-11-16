/** Aviso sobre o caminho escolhido para o agente alcançar a central. */
export interface TransportNotice {
  type: 'warning' | 'info'
  text: string
}

const PRIVATE_IPV4 =
  /^(10\.|127\.|192\.168\.|169\.254\.|172\.(1[6-9]|2\d|3[01])\.|100\.(6[4-9]|[7-9]\d|1[01]\d|12[0-7])\.)/

/** Host de uma URL `http(s)://host[:porta]`, sem colchetes de IPv6. */
function hostOf(serverUrl: string): string {
  const authority = serverUrl.replace(/^https?:\/\//, '').split('/')[0] ?? ''
  if (authority.startsWith('[')) return authority.slice(1, authority.indexOf(']'))
  return authority.split(':')[0] ?? ''
}

/** O endereço fica numa rede privada (LAN, CGNAT, túnel)? */
export function isPrivateHost(host: string): boolean {
  const value = host.toLowerCase()
  if (value === 'localhost' || PRIVATE_IPV4.test(value)) return true
  // IPv6 local (ULA e link-local); um nome como `fdominio.com` não é IPv6.
  return value.includes(':') && /^(f[cd]|fe80)/.test(value)
}

/**
 * O que o operador precisa saber antes de colar o comando: HTTP puro pela
 * internet expõe o código, a credencial e todos os dados do servidor; e o
 * endereço do túnel só responde a quem já está na VPN.
 */
export function transportNotice(
  serverUrl: string,
  addressKind: string | null
): TransportNotice | null {
  if (addressKind === 'vpn') {
    return {
      type: 'info',
      text: 'Este endereço só responde dentro do túnel: o servidor precisa já estar na VPN. Para subir o túnel e o agente de uma vez, feche e conecte o servidor escolhendo o caminho da VPN.',
    }
  }
  if (serverUrl.startsWith('http://') && !isPrivateHost(hostOf(serverUrl))) {
    return {
      type: 'warning',
      text: 'Este endereço usa HTTP pela internet: o código de instalação, a credencial e todos os dados do servidor trafegam sem criptografia. Prefira a VPN ou um domínio com HTTPS.',
    }
  }
  return null
}
