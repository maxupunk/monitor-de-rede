import type { NotificationChannel, NotificationMessage } from './notification_channel.js'
import { errorMessage } from '#modules/shared/errors'

export class EmailChannel implements NotificationChannel {
  name = 'email'
  private recipient: string

  constructor(recipient?: string) {
    this.recipient = recipient || process.env.SMTP_TO || 'admin@monitor.local'
  }

  async send(message: NotificationMessage): Promise<boolean> {
    try {
      console.log(
        `[EmailChannel] Enviando e-mail para ${this.recipient}: [${message.severity.toUpperCase()}] ${message.title} - ${message.body}`
      )
      return true
    } catch (err: unknown) {
      console.error(`[EmailChannel] Erro ao enviar e-mail: ${errorMessage(err)}`)
      return false
    }
  }
}
