export interface NotificationMessage {
  title: string
  body: string
  severity: 'info' | 'warning' | 'critical'
  metadata?: Record<string, unknown>
}

export interface NotificationChannel {
  send(message: NotificationMessage): Promise<void>
}
