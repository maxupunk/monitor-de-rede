import type { NotificationChannel, NotificationMessage } from './channels/notification_channel.js'

export class NotificationService {
  private channels: NotificationChannel[] = []

  registerChannel(channel: NotificationChannel) {
    this.channels.push(channel)
  }

  async notify(message: NotificationMessage) {
    for (const channel of this.channels) {
      await channel.send(message)
    }
  }
}
