/// <reference lib="webworker" />
import { precacheAndRoute, cleanupOutdatedCaches } from 'workbox-precaching'

declare const self: ServiceWorkerGlobalScope

// Limpa caches antigos e registra os arquivos do manifesto do VitePWA
cleanupOutdatedCaches()
precacheAndRoute(self.__WB_MANIFEST || [])

self.addEventListener('install', () => {
  void self.skipWaiting()
})

self.addEventListener('activate', (event) => {
  event.waitUntil(self.clients.claim())
})

// Ouve eventos de Web Push enviados pelo backend (quando app aberto ou fechado)
self.addEventListener('push', (event) => {
  if (!event.data) {
    return
  }

  try {
    const data = event.data.json()
    const title = data.title || '🚨 Alerta de Rede - NetMonitor'
    const options = {
      body: data.body || 'Um novo evento foi gerado na sua infraestrutura.',
      icon: data.icon || '/pwa-192x192.png',
      badge: data.badge || '/pwa-192x192.png',
      tag: data.tag || `netmonitor-alert-${Date.now()}`,
      data: data.data || { url: '/alerts' },
      vibrate: [200, 100, 200],
      renotify: true,
    } as NotificationOptions

    event.waitUntil(self.registration.showNotification(title, options))
  } catch {
    const text = event.data.text()
    event.waitUntil(
      self.registration.showNotification('Alerta de Rede', {
        body: text,
        icon: '/pwa-192x192.png',
        badge: '/pwa-192x192.png',
        data: { url: '/alerts' },
      })
    )
  }
})

// Ouve o clique na notificação nativa do sistema operacional
self.addEventListener('notificationclick', (event) => {
  event.notification.close()

  const targetUrl = (event.notification.data?.url as string) || '/alerts'

  event.waitUntil(
    self.clients.matchAll({ type: 'window', includeUncontrolled: true }).then((clientList) => {
      for (const client of clientList) {
        if ('focus' in client && client.url.includes(self.location.origin)) {
          if ('navigate' in client) {
            void client.navigate(targetUrl)
          }
          return client.focus()
        }
      }
      if (self.clients.openWindow) {
        return self.clients.openWindow(targetUrl)
      }
    })
  )
})
