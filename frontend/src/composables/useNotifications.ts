import { ref, onMounted } from 'vue'
import { useEventsStore } from '@/stores/events'
import { pushService, type TestPushResponse } from '@/services/pushService'

export type NotificationPermissionState = 'default' | 'granted' | 'denied' | 'unsupported'

const permissionState = ref<NotificationPermissionState>('default')
const notificationsEnabled = ref<boolean>(true)
const isWebPushSupported = ref<boolean>(false)
const isSubscribed = ref<boolean>(false)
const isSubscribing = ref<boolean>(false)
let isInitialized = false

/**
 * Converte chave pública VAPID de Base64 URL para Uint8Array para o PushManager.
 */
function urlBase64ToUint8Array(base64String: string): Uint8Array {
  const padding = '='.repeat((4 - (base64String.length % 4)) % 4)
  const base64 = (base64String + padding).replace(/-/g, '+').replace(/_/g, '/')
  const rawData = window.atob(base64)
  const outputArray = new Uint8Array(rawData.length)
  for (let i = 0; i < rawData.length; ++i) {
    outputArray[i] = rawData.charCodeAt(i)
  }
  return outputArray
}

export function useNotifications() {
  const eventsStore = useEventsStore()

  function checkSupportAndPermission() {
    if (typeof window === 'undefined') return

    const hasNotification = 'Notification' in window
    const hasServiceWorker = 'serviceWorker' in navigator
    const hasPushManager = 'PushManager' in window

    isWebPushSupported.value = hasNotification && hasServiceWorker && hasPushManager

    if (!hasNotification) {
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
        // Tenta registrar o Web Push automaticamente ao conceder permissão
        if (isWebPushSupported.value) {
          void subscribeToWebPush()
        } else {
          sendLocalNotification('Notificações Ativadas', {
            body: 'Você receberá alertas de rede em tempo real no seu dispositivo.',
            icon: '/pwa-192x192.png',
          })
        }
        return true
      }
      return false
    } catch (err) {
      console.error('Erro ao solicitar permissão de notificação:', err)
      return false
    }
  }

  async function checkWebPushSubscription(): Promise<boolean> {
    if (typeof window === 'undefined' || !isWebPushSupported.value) {
      isSubscribed.value = false
      return false
    }

    try {
      const registration = await navigator.serviceWorker.ready
      const subscription = await registration.pushManager.getSubscription()
      isSubscribed.value = !!subscription
      return isSubscribed.value
    } catch (err) {
      console.error('Erro ao verificar subscrição Web Push:', err)
      isSubscribed.value = false
      return false
    }
  }

  async function subscribeToWebPush(): Promise<boolean> {
    if (!isWebPushSupported.value) {
      return false
    }

    isSubscribing.value = true
    try {
      if (Notification.permission !== 'granted') {
        const granted = await requestPermission()
        if (!granted) {
          isSubscribing.value = false
          return false
        }
      }

      const vapidPublicKey = await pushService.getVapidPublicKey()
      if (!vapidPublicKey) {
        throw new Error('Chave pública VAPID não recebida do servidor.')
      }

      const registration = await navigator.serviceWorker.ready
      let subscription = await registration.pushManager.getSubscription()

      if (!subscription) {
        const applicationServerKey = urlBase64ToUint8Array(vapidPublicKey)
        subscription = await registration.pushManager.subscribe({
          userVisibleOnly: true,
          applicationServerKey: applicationServerKey as unknown as BufferSource,
        })
      }

      const jsonSub = subscription.toJSON()
      if (!jsonSub.endpoint || !jsonSub.keys?.p256dh || !jsonSub.keys?.auth) {
        throw new Error('Chaves da subscrição de push inválidas ou ausentes.')
      }

      await pushService.saveSubscription({
        endpoint: jsonSub.endpoint,
        keys: {
          p256dh: jsonSub.keys.p256dh,
          auth: jsonSub.keys.auth,
        },
        userAgent: navigator.userAgent,
      })

      isSubscribed.value = true
      notificationsEnabled.value = true
      localStorage.setItem('pwa_notifications_enabled', 'true')
      return true
    } catch (err) {
      console.error('Erro ao registrar subscrição Web Push:', err)
      return false
    } finally {
      isSubscribing.value = false
    }
  }

  async function unsubscribeFromWebPush(): Promise<boolean> {
    if (!isWebPushSupported.value) {
      return false
    }

    isSubscribing.value = true
    try {
      const registration = await navigator.serviceWorker.ready
      const subscription = await registration.pushManager.getSubscription()

      if (subscription) {
        const endpoint = subscription.endpoint
        await subscription.unsubscribe()
        await pushService.deleteSubscription(endpoint)
      }

      isSubscribed.value = false
      return true
    } catch (err) {
      console.error('Erro ao cancelar subscrição Web Push:', err)
      return false
    } finally {
      isSubscribing.value = false
    }
  }

  async function toggleWebPush(enable: boolean): Promise<boolean> {
    if (enable) {
      return await subscribeToWebPush()
    } else {
      return await unsubscribeFromWebPush()
    }
  }

  async function sendTestPush(): Promise<TestPushResponse> {
    return await pushService.sendTestPush()
  }

  function setNotificationsEnabled(val: boolean) {
    notificationsEnabled.value = val
    localStorage.setItem('pwa_notifications_enabled', val ? 'true' : 'false')
    if (val && !isSubscribed.value && isWebPushSupported.value) {
      void subscribeToWebPush()
    } else if (!val && isSubscribed.value) {
      void unsubscribeFromWebPush()
    }
  }

  async function sendLocalNotification(title: string, options?: NotificationOptions) {
    if (
      permissionState.value !== 'granted' ||
      !notificationsEnabled.value ||
      typeof window === 'undefined' ||
      !('Notification' in window)
    ) {
      return
    }

    try {
      if ('serviceWorker' in navigator) {
        const registration = await navigator.serviceWorker.ready
        await registration.showNotification(title, {
          icon: '/pwa-192x192.png',
          badge: '/pwa-192x192.png',
          ...options,
        })
        return
      }

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
    void checkWebPushSubscription()

    // Ouve eventos de alertas críticos via SSE para quando a aba está aberta
    eventsStore.onEvent('alert:triggered', (data) => {
      const title = String(data.title || data.ruleName || 'Novo Alerta de Rede')
      const message = String(data.message || 'Um novo alerta foi gerado no monitoramento.')
      void sendLocalNotification(`🚨 ${title}`, {
        body: message,
        tag: `alert-${data.id || Date.now()}`,
      })
    })

    // Ouve resultados de monitores que falharam
    eventsStore.onEvent('monitor:result', (data) => {
      if (data.status === 'down' || data.status === 'offline') {
        const monitorName = String(data.monitorName || data.name || 'Monitor de Rede')
        void sendLocalNotification(`🔴 Monitor Fora do Ar: ${monitorName}`, {
          body: `O monitor ${monitorName} não respondeu ao teste.`,
          tag: `monitor-down-${data.monitorId || Date.now()}`,
        })
      }
    })

    // Ouve status de dispositivos offline
    eventsStore.onEvent('device:status', (data) => {
      if (data.status === 'offline') {
        const deviceName = String(data.name || data.deviceName || 'Dispositivo')
        void sendLocalNotification(`⚠️ Dispositivo Offline: ${deviceName}`, {
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
    isWebPushSupported,
    isSubscribed,
    isSubscribing,
    requestPermission,
    subscribeToWebPush,
    unsubscribeFromWebPush,
    toggleWebPush,
    checkWebPushSubscription,
    sendTestPush,
    setNotificationsEnabled,
    sendLocalNotification,
    sendNotification: sendLocalNotification,
    initListeners,
  }
}
