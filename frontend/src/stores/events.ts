import { defineStore } from 'pinia'
import { ref } from 'vue'
import { useDevicesStore } from './devices'
import { useAlertsStore } from './alerts'
import { useMonitorsStore } from './monitors'

export interface RealtimeEventPayload {
  event: string
  data: Record<string, unknown>
  timestamp?: string
}

export const useEventsStore = defineStore('events', () => {
  const isConnected = ref(false)
  const recentEvents = ref<RealtimeEventPayload[]>([])
  let eventSource: EventSource | null = null

  function connect() {
    if (eventSource) return

    const token = localStorage.getItem('auth_token')
    const url = token
      ? `/api/events/stream?token=${encodeURIComponent(token)}`
      : '/api/events/stream'

    eventSource = new EventSource(url)

    eventSource.onopen = () => {
      isConnected.value = true
    }

    eventSource.onmessage = (msg) => {
      try {
        const payload = JSON.parse(msg.data) as RealtimeEventPayload
        handleIncomingEvent(payload)
      } catch {
        // Ignore unparseable message
      }
    }

    eventSource.onerror = () => {
      isConnected.value = false
      disconnect()
      // Retry connection after 5 seconds
      setTimeout(() => connect(), 5000)
    }
  }

  function handleIncomingEvent(payload: RealtimeEventPayload) {
    recentEvents.value.unshift(payload)
    if (recentEvents.value.length > 50) {
      recentEvents.value.pop()
    }

    const devicesStore = useDevicesStore()
    const alertsStore = useAlertsStore()
    const monitorsStore = useMonitorsStore()

    switch (payload.event) {
      case 'device.status':
      case 'device.updated':
        if (payload.data?.id && payload.data?.status) {
          devicesStore.updateDeviceStatus(
            Number(payload.data.id),
            payload.data.status as 'online' | 'offline' | 'warning' | 'unknown'
          )
        }
        break
      case 'alert.triggered':
      case 'alert.created':
        alertsStore.fetchActiveAlerts()
        break
      case 'monitor.checked':
      case 'monitor.result':
        monitorsStore.fetchMonitors()
        break
    }
  }

  function disconnect() {
    if (eventSource) {
      eventSource.close()
      eventSource = null
    }
    isConnected.value = false
  }

  return {
    isConnected,
    recentEvents,
    connect,
    disconnect,
  }
})
