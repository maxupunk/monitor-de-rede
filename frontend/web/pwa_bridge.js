(function () {
  var deferredPrompt = null;
  var listeners = [];

  function notify() {
    for (var i = 0; i < listeners.length; i++) {
      try {
        listeners[i]();
      } catch (_) {}
    }
  }

  window.addEventListener('beforeinstallprompt', function (event) {
    event.preventDefault();
    deferredPrompt = event;
    notify();
  });

  window.addEventListener('appinstalled', function () {
    deferredPrompt = null;
    notify();
  });

  window.__netmonitorPwa = {
    canInstall: function () {
      return deferredPrompt !== null;
    },
    isInstalled: function () {
      return (
        window.matchMedia('(display-mode: standalone)').matches ||
        window.navigator.standalone === true
      );
    },
    isIosSafari: function () {
      var ua = window.navigator.userAgent || '';
      var isIos = /iPad|iPhone|iPod/.test(ua);
      var isSafari = /Safari/.test(ua) && !/CriOS|FxiOS|EdgiOS/.test(ua);
      return isIos && isSafari;
    },
    promptInstall: function () {
      if (!deferredPrompt) {
        return Promise.resolve(false);
      }
      var promptEvent = deferredPrompt;
      deferredPrompt = null;
      promptEvent.prompt();
      return promptEvent.userChoice
        .then(function (choice) {
          notify();
          return choice && choice.outcome === 'accepted';
        })
        .catch(function () {
          notify();
          return false;
        });
    },
    subscribe: function (cb) {
      listeners.push(cb);
    }
  };

  var PUSH_SW_URL = '/netmonitor_push_sw.js';
  var PUSH_DEFAULT_ICON = '/icons/Icon-192.png';

  function isNotificationSupported() {
    return typeof window !== 'undefined' && 'Notification' in window;
  }

  function isWebPushSupported() {
    return (
      typeof window !== 'undefined' &&
      'serviceWorker' in navigator &&
      'PushManager' in window &&
      'Notification' in window &&
      window.isSecureContext === true
    );
  }

  function urlBase64ToUint8Array(base64String) {
    var padding = '='.repeat((4 - (base64String.length % 4)) % 4);
    var base64 = (base64String + padding).replace(/-/g, '+').replace(/_/g, '/');
    var rawData = window.atob(base64);
    var outputArray = new Uint8Array(rawData.length);
    for (var i = 0; i < rawData.length; i++) {
      outputArray[i] = rawData.charCodeAt(i);
    }
    return outputArray;
  }

  function waitForActiveRegistration(registration) {
    if (!registration) {
      return Promise.reject(new Error('Falha ao registrar Service Worker.'));
    }
    if (registration.active) {
      return Promise.resolve(registration);
    }
    var worker = registration.installing || registration.waiting;
    if (!worker) {
      return navigator.serviceWorker.ready;
    }
    return new Promise(function (resolve) {
      if (worker.state === 'activated' || registration.active) {
        resolve(registration);
        return;
      }
      var onStateChange = function () {
        if (worker.state === 'activated' || registration.active) {
          worker.removeEventListener('statechange', onStateChange);
          resolve(registration);
        } else if (worker.state === 'redundant') {
          worker.removeEventListener('statechange', onStateChange);
          resolve(navigator.serviceWorker.ready);
        }
      };
      worker.addEventListener('statechange', onStateChange);
    });
  }

  function ensurePushRegistration() {
    if (!('serviceWorker' in navigator)) {
      return Promise.reject(new Error('Service Worker indisponível.'));
    }
    return navigator.serviceWorker
      .register(PUSH_SW_URL, { scope: '/' })
      .then(waitForActiveRegistration);
  }

  function serializeSubscription(subscription, includeUserAgent) {
    if (!subscription) {
      return null;
    }
    var json = subscription.toJSON ? subscription.toJSON() : null;
    if (!json || !json.endpoint || !json.keys || !json.keys.p256dh || !json.keys.auth) {
      return null;
    }
    var payload = {
      endpoint: json.endpoint,
      keys: {
        p256dh: json.keys.p256dh,
        auth: json.keys.auth
      }
    };
    if (includeUserAgent) {
      payload.userAgent = window.navigator.userAgent || '';
    }
    return JSON.stringify(payload);
  }

  window.__netmonitorPush = {
    isSupported: function () {
      return isNotificationSupported();
    },
    isWebPushSupported: function () {
      return isWebPushSupported();
    },
    getPermission: function () {
      if (!isNotificationSupported()) {
        return 'unsupported';
      }
      return window.Notification.permission || 'default';
    },
    requestPermission: function () {
      if (!isNotificationSupported()) {
        return Promise.resolve('unsupported');
      }
      try {
        return Promise.resolve(window.Notification.requestPermission()).then(
          function (result) {
            return typeof result === 'string' ? result : 'default';
          },
          function () {
            return window.Notification.permission || 'default';
          }
        );
      } catch (_) {
        return Promise.resolve(window.Notification.permission || 'default');
      }
    },
    registerServiceWorker: function () {
      if (!isWebPushSupported()) {
        return Promise.resolve(false);
      }
      return ensurePushRegistration().then(
        function () {
          return true;
        },
        function () {
          return false;
        }
      );
    },
    getSubscription: function () {
      if (!isWebPushSupported()) {
        return Promise.resolve(null);
      }
      return ensurePushRegistration()
        .then(function (reg) {
          return reg.pushManager.getSubscription();
        })
        .then(
          function (sub) {
            return serializeSubscription(sub, false);
          },
          function () {
            return null;
          }
        );
    },
    subscribe: function (vapidPublicKey) {
      if (!isWebPushSupported()) {
        return Promise.reject(
          new Error('Web Push não suportado neste navegador ou contexto.')
        );
      }
      if (!vapidPublicKey || typeof vapidPublicKey !== 'string') {
        return Promise.reject(new Error('Chave pública VAPID ausente.'));
      }
      return ensurePushRegistration()
        .then(function (reg) {
          return reg.pushManager.getSubscription().then(function (existing) {
            if (existing) {
              return existing;
            }
            var applicationServerKey = urlBase64ToUint8Array(vapidPublicKey);
            return reg.pushManager.subscribe({
              userVisibleOnly: true,
              applicationServerKey: applicationServerKey
            });
          });
        })
        .then(function (sub) {
          var serialized = serializeSubscription(sub, true);
          if (!serialized) {
            throw new Error(
              'Chaves da subscrição Web Push inválidas ou ausentes.'
            );
          }
          return serialized;
        });
    },
    unsubscribe: function () {
      if (!isWebPushSupported()) {
        return Promise.resolve(null);
      }
      return ensurePushRegistration()
        .then(function (reg) {
          return reg.pushManager.getSubscription();
        })
        .then(function (sub) {
          if (!sub) {
            return null;
          }
          var endpoint = sub.endpoint || null;
          return sub.unsubscribe().then(function () {
            return endpoint;
          });
        });
    },
    showLocalNotification: function (title, body, tag, url) {
      if (!isNotificationSupported() || window.Notification.permission !== 'granted') {
        return Promise.resolve(false);
      }
      var targetUrl = typeof url === 'string' && url ? url : '/alerts';
      var options = {
        body: typeof body === 'string' ? body : '',
        icon: PUSH_DEFAULT_ICON,
        badge: PUSH_DEFAULT_ICON,
        data: { url: targetUrl }
      };
      if (typeof tag === 'string' && tag) {
        options.tag = tag;
      }

      var showFallback = function () {
        try {
          var notification = new window.Notification(title, options);
          notification.onclick = function () {
            try {
              window.focus();
            } catch (_) {}
            notification.close();
          };
          return true;
        } catch (_) {
          return false;
        }
      };

      if ('serviceWorker' in navigator && window.isSecureContext === true) {
        return ensurePushRegistration()
          .then(function (reg) {
            return reg.showNotification(title, options).then(function () {
              return true;
            });
          })
          .catch(function () {
            return showFallback();
          });
      }

      return Promise.resolve(showFallback());
    }
  };
})();
