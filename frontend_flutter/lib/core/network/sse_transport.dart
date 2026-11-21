import 'dart:ui';

import 'sse_transport_io.dart'
    if (dart.library.js_interop) 'sse_transport_web.dart' as platform;

/// Interface de transporte SSE (Web usa `EventSource`; Android/IO usa stream HTTP).
abstract class SseTransport {
  void connect({
    required Uri url,
    Map<String, String> headers = const <String, String>{},
    required VoidCallback onOpen,
    required void Function(String rawData) onMessage,
    required void Function(Object error) onError,
  });

  void close();
}

SseTransport createPlatformSseTransport() => platform.createTransport();
