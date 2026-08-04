import { generateKeyPairSync, createPrivateKey, createPublicKey, randomBytes } from 'node:crypto'

/**
 * Geração de chaves WireGuard (Curve25519) 100% nativa no Node, sem depender do
 * binário `wg`. Permite desenvolvimento em Windows sem Docker.
 */

/** Cabeçalho PKCS#8 fixo de uma chave privada X25519 (RFC 8410). */
const PKCS8_X25519_PREFIX = Buffer.from('302e020100300506032b656e04220420', 'hex')

/** Tamanho, em bytes, de qualquer chave WireGuard (base64 de 32 bytes = 44 chars). */
export const WG_KEY_BYTES = 32

export interface WireGuardKeyPair {
  privateKey: string
  publicKey: string
}

/** Equivalente a `wg genkey` + `wg pubkey`. */
export function generateKeyPair(): WireGuardKeyPair {
  const { publicKey, privateKey } = generateKeyPairSync('x25519')

  return {
    privateKey: privateKey
      .export({ type: 'pkcs8', format: 'der' })
      .subarray(-WG_KEY_BYTES)
      .toString('base64'),
    publicKey: publicKey
      .export({ type: 'spki', format: 'der' })
      .subarray(-WG_KEY_BYTES)
      .toString('base64'),
  }
}

/** Equivalente a `wg pubkey` — deriva a chave pública a partir da privada. */
export function derivePublicKey(privateKeyB64: string): string {
  const der = Buffer.concat([PKCS8_X25519_PREFIX, Buffer.from(privateKeyB64, 'base64')])
  const privateKey = createPrivateKey({ key: der, format: 'der', type: 'pkcs8' })

  return createPublicKey(privateKey)
    .export({ type: 'spki', format: 'der' })
    .subarray(-WG_KEY_BYTES)
    .toString('base64')
}

/** Equivalente a `wg genpsk`. */
export function generatePresharedKey(): string {
  return randomBytes(WG_KEY_BYTES).toString('base64')
}

/** Valida se a string é uma chave WireGuard válida (32 bytes em base64). */
export function isValidKey(key: string): boolean {
  if (typeof key !== 'string' || !/^[A-Za-z0-9+/]{43}=$/.test(key)) {
    return false
  }
  return Buffer.from(key, 'base64').length === WG_KEY_BYTES
}
