import logger from '@adonisjs/core/services/logger'

/**
 * Proteções dos endpoints sensíveis (§5): rate limit por usuário e registro de
 * auditoria de todo download de configuração de VPN.
 */

export interface RateLimitDecision {
  allowed: boolean
  remaining: number
  retryAfterSeconds: number
}

/** Janela deslizante em memória — suficiente para um processo de API. */
export class SlidingWindowRateLimiter {
  private hits = new Map<string, number[]>()

  constructor(
    private limit = 10,
    private windowMs = 60_000
  ) {}

  consume(key: string): RateLimitDecision {
    const now = Date.now()
    const windowStart = now - this.windowMs
    const previous = (this.hits.get(key) ?? []).filter((timestamp) => timestamp > windowStart)

    if (previous.length >= this.limit) {
      const retryAfterMs = previous[0] + this.windowMs - now
      this.hits.set(key, previous)
      return {
        allowed: false,
        remaining: 0,
        retryAfterSeconds: Math.max(1, Math.ceil(retryAfterMs / 1000)),
      }
    }

    previous.push(now)
    this.hits.set(key, previous)

    return { allowed: true, remaining: this.limit - previous.length, retryAfterSeconds: 0 }
  }

  reset(): void {
    this.hits.clear()
  }
}

export interface VpnAuditEntry {
  action: 'config_download' | 'qrcode_download' | 'key_rotation' | 'peer_revoked' | 'peer_created'
  peerId: number | null
  userId: number | string | null
  ipAddress: string | null
  details?: Record<string, unknown>
}

/** Trilha de auditoria: quem acessou, quando e qual peer. */
export class VpnAuditLogger {
  log(entry: VpnAuditEntry): void {
    logger.info(
      {
        audit: 'vpn',
        action: entry.action,
        peerId: entry.peerId,
        userId: entry.userId,
        requestIp: entry.ipAddress,
        at: new Date().toISOString(),
        ...entry.details,
      },
      `[VPN][auditoria] ${entry.action} peer=${entry.peerId ?? '-'} usuario=${entry.userId ?? 'anônimo'}`
    )
  }
}

/** Instâncias compartilhadas pelos controllers. */
export const sensitiveEndpointLimiter = new SlidingWindowRateLimiter(10, 60_000)
export const vpnAuditLogger = new VpnAuditLogger()
