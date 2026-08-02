import type { NotificationChannel, NotificationMessage } from './notification_channel.js'

export class TelegramChannel implements NotificationChannel {
  name = 'telegram'
  private botToken: string
  private chatId: string

  constructor(botToken?: string, chatId?: string) {
    this.botToken = botToken || process.env.TELEGRAM_BOT_TOKEN || ''
    this.chatId = chatId || process.env.TELEGRAM_CHAT_ID || ''
  }

  async send(message: NotificationMessage): Promise<boolean> {
    if (!this.botToken || !this.chatId) {
      return false
    }

    const text = `🚨 *[${message.severity.toUpperCase()}] ${message.title}*\n\n${message.body}`

    try {
      const url = `https://api.telegram.org/bot${this.botToken}/sendMessage`
      const res = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          chat_id: this.chatId,
          text,
          parse_mode: 'Markdown',
        }),
      })
      return res.ok
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err)
      console.error(`[TelegramChannel] Erro ao enviar mensagem no Telegram: ${msg}`)
      return false
    }
  }
}
