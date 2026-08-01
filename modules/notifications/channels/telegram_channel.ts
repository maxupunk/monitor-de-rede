import type { NotificationChannel, NotificationMessage } from './notification_channel.js'

export class TelegramChannel implements NotificationChannel {
  async send(message: NotificationMessage): Promise<void> {
    // Envio por Telegram
  }
}
