/**
 * Aritmética de CIDR IPv4 para a tela.
 *
 * Existe só para responder "esta faixa colide com alguma rede já cadastrada?".
 * A validação de verdade continua no backend — o que se ganha aqui é dizer isso
 * **antes** de gravar, e não depois de o operador descobrir que dois roteadores
 * receberam o mesmo endereço.
 */

export interface CidrRange {
  first: number
  last: number
  prefix: number
}

/** `null` para qualquer coisa que não seja um CIDR IPv4 legível. */
export function parseCidr(texto: string): CidrRange | null {
  const bruto = texto.trim()
  const [endereco, prefixo] = bruto.split('/')
  if (!endereco || prefixo === undefined) return null

  const prefix = Number(prefixo)
  if (!Number.isInteger(prefix) || prefix < 0 || prefix > 32) return null

  const octetos = endereco.split('.')
  if (octetos.length !== 4) return null
  let base = 0
  for (const octeto of octetos) {
    const valor = Number(octeto)
    if (!Number.isInteger(valor) || valor < 0 || valor > 255 || octeto.trim() === '') return null
    base = base * 256 + valor
  }

  // `>>>` e não `>>`: o deslocamento com sinal transformaria /0 e /1 em números
  // negativos, e a comparação de faixas passaria a mentir.
  const mascara = prefix === 0 ? 0 : (0xffffffff << (32 - prefix)) >>> 0
  const first = (base & mascara) >>> 0
  const last = (first | (~mascara >>> 0)) >>> 0
  return { first, last, prefix }
}

export function cidrOverlaps(a: string, b: string): boolean {
  const primeira = parseCidr(a)
  const segunda = parseCidr(b)
  if (!primeira || !segunda) return false
  return primeira.first <= segunda.last && segunda.first <= primeira.last
}

/**
 * Faixas privadas de reserva para o túnel, na ordem em que são oferecidas.
 *
 * Todas em /24: 254 endereços é mais do que qualquer parque de roteadores que
 * este sistema atende, e um prefixo maior só aumentaria a chance de colidir com
 * a LAN de alguém.
 */
export const VPN_CIDR_CANDIDATES = [
  '10.8.0.0/24',
  '10.9.0.0/24',
  '10.88.0.0/24',
  '172.31.9.0/24',
  '192.168.243.0/24',
]

/** As candidatas que não colidem com nenhuma das faixas ocupadas. */
export function freeVpnCidrs(ocupadas: string[]): string[] {
  return VPN_CIDR_CANDIDATES.filter(
    (candidata) => !ocupadas.some((ocupada) => cidrOverlaps(candidata, ocupada))
  )
}
