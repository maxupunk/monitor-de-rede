import 'dart:ui';

import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/network/sse_client.dart';
import 'package:netmonitor_app/core/network/sse_transport.dart';

class _FakeSseTransport implements SseTransport {
  Uri? lastUrl;
  Map<String, String>? lastHeaders;
  VoidCallback? _onOpen;
  void Function(String)? _onMessage;
  void Function(Object)? _onError;
  bool closed = false;

  @override
  void connect({
    required Uri url,
    Map<String, String> headers = const <String, String>{},
    required VoidCallback onOpen,
    required void Function(String rawData) onMessage,
    required void Function(Object error) onError,
  }) {
    closed = false;
    lastUrl = url;
    lastHeaders = headers;
    _onOpen = onOpen;
    _onMessage = onMessage;
    _onError = onError;
  }

  void simulateOpen() => _onOpen?.call();
  void simulateMessage(String raw) => _onMessage?.call(raw);
  void simulateError(Object error) => _onError?.call(error);

  @override
  void close() {
    closed = true;
  }
}

void main() {
  group('SseLineParser', () {
    test(
      'parses chunked SSE frames, ignores comments and joins multiline data',
      () {
        final parser = SseLineParser();
        expect(parser.feed(': keep-alive\n\n'), isEmpty);

        final first = parser.feed('data: {"type":"areas:upd');
        expect(first, isEmpty);

        final second = parser.feed('ated","data":{}}\n\n');
        expect(second, <String>['{"type":"areas:updated","data":{}}']);

        final multiline = parser.feed('data: linha1\r\ndata: linha2\r\n\r\n');
        expect(multiline, <String>['linha1\nlinha2']);
      },
    );
  });

  group('computeSseBackoffDelay', () {
    test('grows exponentially and caps at 30 seconds', () {
      expect(computeSseBackoffDelay(0), const Duration(seconds: 1));
      expect(computeSseBackoffDelay(1), const Duration(seconds: 2));
      expect(computeSseBackoffDelay(2), const Duration(seconds: 4));
      expect(computeSseBackoffDelay(3), const Duration(seconds: 8));
      expect(computeSseBackoffDelay(4), const Duration(seconds: 16));
      expect(computeSseBackoffDelay(5), const Duration(seconds: 30));
      expect(computeSseBackoffDelay(10), const Duration(seconds: 30));
    });
  });

  group('SseClient', () {
    test('emits parsed events and reconnects after error with backoff', () async {
      final transports = <_FakeSseTransport>[];
      final client = SseClient(
        transportFactory: () {
          final t = _FakeSseTransport();
          transports.add(t);
          return t;
        },
        backoffCalculator: (_) => const Duration(milliseconds: 15),
      );

      final received = <SseEvent>[];
      final states = <SseConnectionState>[];
      final sub1 = client.events.listen(received.add);
      final sub2 = client.connectionStates.listen(states.add);

      client.connect(
        urlBuilder: () =>
            Uri.parse('http://127.0.0.1:3333/api/events/stream?token=abc'),
      );

      expect(transports.length, 1);
      transports.first.simulateOpen();
      expect(client.isConnected, isTrue);

      transports.first.simulateMessage(
        '{"type":"alert:triggered","data":{"id":42},"timestamp":"2026-11-20T22:00:00Z"}',
      );
      // Ignora payload malformado sem quebrar o stream.
      transports.first.simulateMessage('not-json');

      await Future<void>.delayed(Duration.zero);
      expect(received.length, 1);
      expect(received.first.type, 'alert:triggered');
      expect(received.first.data['id'], 42);

      // Simula queda de rede -> agenda reconexão one-shot.
      transports.first.simulateError(Exception('drop'));
      expect(client.state, SseConnectionState.reconnecting);
      expect(client.reconnectAttempts, 1);

      await Future<void>.delayed(const Duration(milliseconds: 40));
      expect(transports.length, 2);

      transports[1].simulateOpen();
      expect(client.isConnected, isTrue);
      expect(client.reconnectAttempts, 0);

      client.disconnect();
      expect(transports[1].closed, isTrue);
      expect(client.state, SseConnectionState.disconnected);

      await sub1.cancel();
      await sub2.cancel();
      client.dispose();
      expect(states, contains(SseConnectionState.connected));
    });
  });
}
