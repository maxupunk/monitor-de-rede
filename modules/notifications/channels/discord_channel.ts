import type { NotificationChannel, NotificationMessage } from './notification_channel.js'

export class DiscordChannel implements NotificationChannel {
  async send(_message: NotificationMessage): Promise<void> {
    // Envio por Discord
  }
}
