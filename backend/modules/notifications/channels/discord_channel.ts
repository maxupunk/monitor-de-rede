import {
  HttpNotificationChannel,
  type ChannelRequest,
  type NotificationMessage,
} from './notification_channel.js'

export class DiscordChannel extends HttpNotificationChannel {
  readonly name = 'discord'
  private webhookUrl: string

  constructor(webhookUrl?: string) {
    super()
    this.webhookUrl = webhookUrl || process.env.DISCORD_WEBHOOK_URL || ''
  }

  protected isConfigured(): boolean {
    return Boolean(this.webhookUrl)
  }

  protected buildRequest(message: NotificationMessage): ChannelRequest {
    const color =
      message.severity === 'critical'
        ? 15158332
        : message.severity === 'warning'
          ? 16776960
          : 3447003

    return {
      url: this.webhookUrl,
      body: {
        embeds: [
          {
            title: message.title,
            description: message.body,
            color,
            timestamp: new Date().toISOString(),
          },
        ],
      },
    }
  }
}
