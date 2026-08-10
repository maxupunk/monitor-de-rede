import {
  HttpNotificationChannel,
  type ChannelRequest,
  type NotificationMessage,
} from './notification_channel.js'

export class WebhookChannel extends HttpNotificationChannel {
  readonly name = 'webhook'
  private url: string

  constructor(url?: string) {
    super()
    this.url = url || process.env.GENERIC_WEBHOOK_URL || ''
  }

  protected isConfigured(): boolean {
    return Boolean(this.url)
  }

  protected buildRequest(message: NotificationMessage): ChannelRequest {
    return {
      url: this.url,
      body: {
        event: 'notification',
        timestamp: new Date().toISOString(),
        message,
      },
    }
  }
}
