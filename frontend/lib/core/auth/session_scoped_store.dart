import 'session_scoped_store_memory.dart'
    if (dart.library.js_interop) 'session_scoped_store_web.dart';

/// Armazenamento que vive só durante a sessão do app: `sessionStorage` na Web
/// (some ao fechar a aba) e memória do processo no Android. Usado quando o
/// operador desmarca "Permanecer logado".
abstract final class SessionScopedStore {
  static String? read(String key) => platformSessionRead(key);

  static void write(String key, String value) =>
      platformSessionWrite(key, value);

  static void remove(String key) => platformSessionRemove(key);
}
