{{flutter_js}}
{{flutter_build_config}}

// Inicialização sem `serviceWorkerSettings` para preservar `/netmonitor_push_sw.js`
// no escopo `/` sem conflito com `flutter_service_worker.js`.
_flutter.loader.load();
