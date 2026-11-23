import 'package:web/web.dart' as web;

String? platformSessionRead(String key) {
  try {
    return web.window.sessionStorage.getItem(key);
  } catch (_) {
    return null;
  }
}

void platformSessionWrite(String key, String value) {
  try {
    web.window.sessionStorage.setItem(key, value);
  } catch (_) {
    // Sem sessionStorage (modo privado restrito) a sessão não sobrevive ao F5.
  }
}

void platformSessionRemove(String key) {
  try {
    web.window.sessionStorage.removeItem(key);
  } catch (_) {}
}
