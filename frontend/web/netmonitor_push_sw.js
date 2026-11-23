/* Service Worker de Web Push do NetMonitor (escopo '/'). */

var DEFAULT_ICON = '/icons/Icon-192.png';
var DEFAULT_BADGE = '/icons/Icon-192.png';
var DEFAULT_URL = '/alerts';

self.addEventListener('install', function () {
  self.skipWaiting();
});

self.addEventListener('activate', function (event) {
  event.waitUntil(self.clients.claim());
});

self.addEventListener('push', function (event) {
  if (!event.data) {
    return;
  }

  try {
    var payload = event.data.json();
    var title =
      (payload && typeof payload.title === 'string' && payload.title) ||
      '🚨 Alerta de Rede - NetMonitor';

    var rawActions = Array.isArray(payload && payload.actions)
      ? payload.actions
      : [];
    var actionUrls = {};
    var notificationActions = [];

    for (var i = 0; i < rawActions.length; i++) {
      var item = rawActions[i];
      if (!item || typeof item.action !== 'string' || !item.action) {
        continue;
      }
      if (typeof item.url === 'string' && item.url) {
        actionUrls[item.action] = item.url;
      }
      notificationActions.push({
        action: item.action,
        title: typeof item.title === 'string' ? item.title : item.action,
      });
    }

    var baseData =
      payload && payload.data && typeof payload.data === 'object'
        ? Object.assign({}, payload.data)
        : { url: DEFAULT_URL };
    if (typeof baseData.url !== 'string' || !baseData.url) {
      baseData.url = DEFAULT_URL;
    }
    baseData.actionUrls = actionUrls;

    var options = {
      body:
        (payload && typeof payload.body === 'string' && payload.body) ||
        'Um novo evento foi gerado na sua infraestrutura.',
      icon:
        (payload && typeof payload.icon === 'string' && payload.icon) ||
        DEFAULT_ICON,
      badge:
        (payload && typeof payload.badge === 'string' && payload.badge) ||
        DEFAULT_BADGE,
      tag:
        (payload && typeof payload.tag === 'string' && payload.tag) ||
        'netmonitor-alert-' + Date.now(),
      data: baseData,
      actions: notificationActions,
      vibrate: Array.isArray(payload && payload.vibrate)
        ? payload.vibrate
        : [200, 100, 200],
      renotify:
        payload && typeof payload.renotify === 'boolean'
          ? payload.renotify
          : true,
    };

    event.waitUntil(self.registration.showNotification(title, options));
  } catch (_) {
    var text = '';
    try {
      text = event.data.text();
    } catch (_) {
      text = 'Um novo evento foi gerado na sua infraestrutura.';
    }
    event.waitUntil(
      self.registration.showNotification('Alerta de Rede', {
        body: text,
        icon: DEFAULT_ICON,
        badge: DEFAULT_BADGE,
        data: { url: DEFAULT_URL },
      })
    );
  }
});

self.addEventListener('notificationclick', function (event) {
  event.notification.close();

  var data = (event.notification && event.notification.data) || {};
  var actionUrls =
    data.actionUrls && typeof data.actionUrls === 'object'
      ? data.actionUrls
      : {};
  var targetUrl =
    (event.action && actionUrls[event.action]) ||
    (typeof data.url === 'string' && data.url) ||
    DEFAULT_URL;

  event.waitUntil(
    self.clients
      .matchAll({ type: 'window', includeUncontrolled: true })
      .then(function (clientList) {
        for (var i = 0; i < clientList.length; i++) {
          var client = clientList[i];
          if (
            client &&
            typeof client.url === 'string' &&
            client.url.indexOf(self.location.origin) === 0 &&
            'focus' in client
          ) {
            if ('navigate' in client && typeof client.navigate === 'function') {
              return client.navigate(targetUrl).then(
                function (navigatedClient) {
                  return (navigatedClient || client).focus();
                },
                function () {
                  return client.focus();
                }
              );
            }
            return client.focus();
          }
        }
        if (self.clients.openWindow) {
          return self.clients.openWindow(targetUrl);
        }
        return undefined;
      })
  );
});
