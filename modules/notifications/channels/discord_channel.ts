import type { NotificationChannel, NotificationMessage } from './notification_channel.js'

export class DiscordChannel implements NotificationChannel {
  name = 'discord'
  private webhookUrl: string

  constructor(webhookUrl?: string) {
    this.webhookUrl = webhookUrl || process.env.DISCORD_WEBHOOK_URL || ''
  }

  async send(message: NotificationMessage): Promise<boolean> {
    if (!this.webhookUrl) {
      return false
    }

    const color = message.severity === 'critical' ? 15158332 : message.severity === 'warning' ? 16776960 : 3447003

    const payload = {
      embeds: [
        {
          title: message.title,
          description: message.body,
          color,
          timestamp: new Date().toISOString(),
        },
      ],
    }

    try {
      const res = await fetch(this.webhookUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      })
      return res.ok
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err)
      console.error(`[DiscordChannel] Erro ao enviar mensagem para Discord Webhook: ${msg}`)
      return false
    }
  }
}
