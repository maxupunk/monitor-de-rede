import type { NotificationChannel, NotificationMessage } from './notification_channel.js'

export class EmailChannel implements NotificationChannel {
  async send(message: NotificationMessage): Promise<void> {
    // Envio por E-mail
  }
}
