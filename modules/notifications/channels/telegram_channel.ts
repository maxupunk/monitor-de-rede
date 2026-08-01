import type { NotificationChannel, NotificationMessage } from './notification_channel.js'

export class TelegramChannel implements NotificationChannel {
  async send(_message: NotificationMessage): Promise<void> {
    // Envio por Telegram
  }
}
