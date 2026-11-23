import 'dart:async';
import 'dart:convert';
import 'dart:ui';

import 'package:http/http.dart' as http;
import 'package:netmonitor_app/core/network/sse_client.dart';
import 'package:netmonitor_app/core/network/sse_transport.dart';

SseTransport createTransport() => _IoSseTransport();

class _IoSseTransport implements SseTransport {
  http.Client? _client;
  StreamSubscription<String>? _subscription;
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
    final client = http.Client();
    _client = client;

    final request = http.Request('GET', url)
      ..headers['Accept'] = 'text/event-stream'
      ..headers['Cache-Control'] = 'no-cache'
      ..headers.addAll(headers);

    final parser = SseLineParser();

    unawaited(() async {
      try {
        final response = await client.send(request);
        if (_closed) return;
        if (response.statusCode < 200 || response.statusCode >= 300) {
          onError(SseHttpException(response.statusCode));
          return;
        }
        onOpen();

        _subscription = response.stream
            .transform(utf8.decoder)
            .listen(
              (chunk) {
                if (_closed) return;
                for (final payload in parser.feed(chunk)) {
                  onMessage(payload);
                }
              },
              onError: (Object err) {
                if (!_closed) onError(err);
              },
              onDone: () {
                if (!_closed) {
                  for (final payload in parser.flush()) {
                    onMessage(payload);
                  }
                  onError(Exception('SSE stream closed'));
                }
              },
              cancelOnError: true,
            );
      } catch (err) {
        if (!_closed) onError(err);
      }
    }());
  }

  @override
  void close() {
    _closed = true;
    unawaited(_subscription?.cancel());
    _subscription = null;
    _client?.close();
    _client = null;
  }
}
