import {
  HttpNotificationChannel,
  type ChannelRequest,
  type NotificationMessage,
} from './notification_channel.js'

export class TelegramChannel extends HttpNotificationChannel {
  readonly name = 'telegram'
  private botToken: string
  private chatId: string

  constructor(botToken?: string, chatId?: string) {
    super()
    this.botToken = botToken || process.env.TELEGRAM_BOT_TOKEN || ''
    this.chatId = chatId || process.env.TELEGRAM_CHAT_ID || ''
  }

  protected isConfigured(): boolean {
    return Boolean(this.botToken && this.chatId)
  }

  protected buildRequest(message: NotificationMessage): ChannelRequest {
    const text = `🚨 *[${message.severity.toUpperCase()}] ${message.title}*\n\n${message.body}`

    return {
      url: `https://api.telegram.org/bot${this.botToken}/sendMessage`,
      body: {
        chat_id: this.chatId,
        text,
        parse_mode: 'Markdown',
      },
    }
  }
}
