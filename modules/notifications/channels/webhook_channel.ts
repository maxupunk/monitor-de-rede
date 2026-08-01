import type { NotificationChannel, NotificationMessage } from './notification_channel.js'

export class WebhookChannel implements NotificationChannel {
  async send(_message: NotificationMessage): Promise<void> {
    // Envio por Webhook
  }
}
