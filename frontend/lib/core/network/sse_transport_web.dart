import 'dart:js_interop';
import 'dart:ui';

import 'package:netmonitor_app/core/network/sse_transport.dart';
import 'package:web/web.dart' as web;

SseTransport createTransport() => _WebSseTransport();

class _WebSseTransport implements SseTransport {
  web.EventSource? _eventSource;
  bool _closed = false;

  @override
  void connect({
    required Uri url,
    Map<String, String> headers = const <String, String>{},
    required VoidCallback onOpen,
    required void Function(String rawData) onMessage,
    required void Function(Object error) onError,
  }) {
    close();
    _closed = false;

    final source = web.EventSource(url.toString());
    _eventSource = source;

    source.addEventListener(
      'open',
      ((web.Event _) {
        if (!_closed) onOpen();
      }).toJS,
    );

    source.addEventListener(
      'message',
      ((web.MessageEvent event) {
        if (_closed) return;
        final raw = event.data;
        if (raw.isA<JSString>()) {
          onMessage((raw as JSString).toDart);
        }
      }).toJS,
    );

    source.addEventListener(
      'error',
      ((web.Event _) {
        if (!_closed) onError(Exception('EventSource error'));
      }).toJS,
    );
  }

  @override
  void close() {
    _closed = true;
    _eventSource?.close();
    _eventSource = null;
  }
}
