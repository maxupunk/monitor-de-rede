import { errorMessage } from '#modules/shared/errors'

export interface NotificationMessage {
  title: string
  body: string
  severity: 'info' | 'warning' | 'critical'
  metadata?: Record<string, unknown>
}

export interface NotificationChannel {
  name: string
  send(message: NotificationMessage): Promise<boolean>
}

/** Requisição HTTP que um canal precisa montar para entregar a mensagem */
export interface ChannelRequest {
  url: string
  body: unknown
  headers?: Record<string, string>
}

/**
 * Todo canal de webhook (Telegram, Discord, genérico) faz o mesmo POST JSON com
 * o mesmo tratamento de erro — a única diferença é qual URL/payload montar a
 * partir da mensagem. Subclasses só descrevem isso; o envio HTTP mora aqui.
 */
export abstract class HttpNotificationChannel implements NotificationChannel {
  abstract readonly name: string

  /** Falta configuração (token/URL) para este canal — `send` retorna `false` sem tentar. */
  protected abstract isConfigured(): boolean

  protected abstract buildRequest(message: NotificationMessage): ChannelRequest

  async send(message: NotificationMessage): Promise<boolean> {
    if (!this.isConfigured()) return false

    const { url, body, headers } = this.buildRequest(message)

    try {
      const res = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', ...headers },
        body: JSON.stringify(body),
      })
      return res.ok
    } catch (err: unknown) {
      console.error(`[${this.name}] Erro ao enviar notificação: ${errorMessage(err)}`)
      return false
    }
  }
}
