import { ref, onMounted } from 'vue'
import { useEventsStore } from '@/stores/events'

export type NotificationPermissionState = 'default' | 'granted' | 'denied' | 'unsupported'

const permissionState = ref<NotificationPermissionState>('default')
const notificationsEnabled = ref<boolean>(true)
let isInitialized = false

export function useNotifications() {
  const eventsStore = useEventsStore()

  function checkSupportAndPermission() {
    if (typeof window === 'undefined' || !('Notification' in window)) {
      permissionState.value = 'unsupported'
      return
    }

    permissionState.value = Notification.permission as NotificationPermissionState
    const stored = localStorage.getItem('pwa_notifications_enabled')
    if (stored !== null) {
      notificationsEnabled.value = stored === 'true'
    }
  }

  async function requestPermission(): Promise<boolean> {
    if (typeof window === 'undefined' || !('Notification' in window)) {
      permissionState.value = 'unsupported'
      return false
    }

    try {
      const result = await Notification.requestPermission()
      permissionState.value = result as NotificationPermissionState

      if (result === 'granted') {
        notificationsEnabled.value = true
        localStorage.setItem('pwa_notifications_enabled', 'true')
        sendNotification('Notificações Ativadas', {
          body: 'Você receberá alertas de rede em tempo real no seu dispositivo.',
          icon: '/pwa-192x192.png',
        })
        return true
      }
      return false
    } catch (err) {
      console.error('Erro ao solicitar permissão de notificação:', err)
      return false
    }
  }

  function setNotificationsEnabled(val: boolean) {
    notificationsEnabled.value = val
    localStorage.setItem('pwa_notifications_enabled', val ? 'true' : 'false')
  }

  function sendNotification(title: string, options?: NotificationOptions) {
    if (
      permissionState.value !== 'granted' ||
      !notificationsEnabled.value ||
      typeof window === 'undefined' ||
      !('Notification' in window)
    ) {
      return
    }

    try {
      const notification = new Notification(title, {
        icon: '/pwa-192x192.png',
        badge: '/pwa-192x192.png',
        ...options,
      })

      notification.onclick = () => {
        window.focus()
        notification.close()
      }
    } catch (err) {
      console.error('Erro ao emitir notificação nativa:', err)
    }
  }

  function initListeners() {
    if (isInitialized) return
    isInitialized = true

    checkSupportAndPermission()

    // Ouve eventos de alertas críticos
    eventsStore.onEvent('alert:triggered', (data) => {
      const title = String(data.title || data.ruleName || 'Novo Alerta de Rede')
      const message = String(data.message || 'Um novo alerta foi gerado no monitoramento.')
      sendNotification(`🚨 ${title}`, {
        body: message,
        tag: `alert-${data.id || Date.now()}`,
      })
    })

    // Ouve resultados de monitores que falharam
    eventsStore.onEvent('monitor:result', (data) => {
      if (data.status === 'down' || data.status === 'offline') {
        const monitorName = String(data.monitorName || data.name || 'Monitor de Rede')
        sendNotification(`🔴 Monitor Fora do Ar: ${monitorName}`, {
          body: `O monitor ${monitorName} não respondeu ao teste.`,
          tag: `monitor-down-${data.monitorId || Date.now()}`,
        })
      }
    })

    // Ouve status de dispositivos offline
    eventsStore.onEvent('device:status', (data) => {
      if (data.status === 'offline') {
        const deviceName = String(data.name || data.deviceName || 'Dispositivo')
        sendNotification(`⚠️ Dispositivo Offline: ${deviceName}`, {
          body: `O dispositivo ${deviceName} perdeu conexão com a rede.`,
          tag: `device-offline-${data.id || Date.now()}`,
        })
      }
    })
  }

  onMounted(() => {
    initListeners()
  })

  return {
    permissionState,
    notificationsEnabled,
    requestPermission,
    setNotificationsEnabled,
    sendNotification,
    initListeners,
  }
}
