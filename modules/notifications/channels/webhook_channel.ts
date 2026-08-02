import type { NotificationChannel, NotificationMessage } from './notification_channel.js'

export class WebhookChannel implements NotificationChannel {
  name = 'webhook'
  private url: string

  constructor(url?: string) {
    this.url = url || process.env.GENERIC_WEBHOOK_URL || ''
  }

  async send(message: NotificationMessage): Promise<boolean> {
    if (!this.url) {
      return false
    }

    try {
      const res = await fetch(this.url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          event: 'notification',
          timestamp: new Date().toISOString(),
          message,
        }),
      })
      return res.ok
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err)
      console.error(`[WebhookChannel] Erro ao enviar para Webhook genérico: ${msg}`)
      return false
    }
  }
}
