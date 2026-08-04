/**
 * Guarda em memória (nunca no banco) as chaves privadas de cliente recém-geradas.
 *
 * A chave fica disponível apenas até a primeira leitura ou até expirar — depois
 * disso, só resta rotacionar o peer, exatamente como descrito no §3.4 do
 * roadmap da VPN.
 */
export interface StoredSecret {
  value: string
  expiresAt: number
}

export class EphemeralSecretStore {
  private secrets = new Map<string, StoredSecret>()

  constructor(private ttlMs: number = 15 * 60 * 1000) {}

  private purgeExpired(now: number): void {
    for (const [key, secret] of this.secrets) {
      if (secret.expiresAt <= now) {
        this.secrets.delete(key)
      }
    }
  }

  put(key: string, value: string): void {
    this.purgeExpired(Date.now())
    this.secrets.set(key, { value, expiresAt: Date.now() + this.ttlMs })
  }

  /** Lê e descarta: a segunda chamada devolve `null`. */
  consume(key: string): string | null {
    const now = Date.now()
    this.purgeExpired(now)

    const secret = this.secrets.get(key)
    if (!secret) return null

    this.secrets.delete(key)
    return secret.value
  }

  /** Indica se ainda existe segredo disponível, sem consumi-lo. */
  has(key: string): boolean {
    this.purgeExpired(Date.now())
    return this.secrets.has(key)
  }

  clear(): void {
    this.secrets.clear()
  }
}

/** Instância compartilhada pelo processo da API. */
export const clientKeyStore = new EphemeralSecretStore()
