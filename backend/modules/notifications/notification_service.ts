import type { NotificationChannel, NotificationMessage } from './channels/notification_channel.js'
import { EmailChannel } from './channels/email_channel.js'
import { TelegramChannel } from './channels/telegram_channel.js'
import { DiscordChannel } from './channels/discord_channel.js'
import { WebhookChannel } from './channels/webhook_channel.js'
import { errorMessage } from '#modules/shared/errors'

export class NotificationService {
  private channels: NotificationChannel[] = []

  constructor(defaultChannels = true) {
    if (defaultChannels) {
      this.channels.push(new EmailChannel())
      this.channels.push(new TelegramChannel())
      this.channels.push(new DiscordChannel())
      this.channels.push(new WebhookChannel())
    }
  }

  registerChannel(channel: NotificationChannel): void {
    this.channels.push(channel)
  }

  clearChannels(): void {
    this.channels = []
  }

  async notify(message: NotificationMessage): Promise<void> {
    for (const channel of this.channels) {
      try {
        await channel.send(message)
      } catch (err: unknown) {
        console.error(`[NotificationService] Falha no canal ${channel.name}: ${errorMessage(err)}`)
      }
    }
  }
}
