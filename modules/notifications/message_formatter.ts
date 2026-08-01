import type { NotificationMessage } from './channels/notification_channel.js'

export class MessageFormatter {
  formatAlertMessage(title: string, details: string, severity: 'info' | 'warning' | 'critical'): NotificationMessage {
    return { title, body: details, severity }
  }
}
