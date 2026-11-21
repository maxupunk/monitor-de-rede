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
})();
