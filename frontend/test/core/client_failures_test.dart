import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/network/client_failure_reporter.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';

/// Adaptador que nunca responde: a falha acontece antes da resposta HTTP.
class _FailingAdapter implements HttpClientAdapter {
  _FailingAdapter(this.type);

  final DioExceptionType type;

  @override
  Future<ResponseBody> fetch(
    RequestOptions options,
    Stream<Uint8List>? requestStream,
    Future<void>? cancelFuture,
  ) async {
    throw DioException(requestOptions: options, type: type);
  }

  @override
  void close({bool force = false}) {}
}

class _RespondingAdapter implements HttpClientAdapter {
  _RespondingAdapter(this.response);

  final ResponseBody response;

  @override
  Future<ResponseBody> fetch(
    RequestOptions options,
    Stream<Uint8List>? requestStream,
    Future<void>? cancelFuture,
  ) async => response;

  @override
  void close({bool force = false}) {}
}

ApiClient _client(DioExceptionType type, List<ClientFailure> seen) {
  return ApiClient(
    baseUrlProvider: () => 'http://central.invalid/api',
    tokenProvider: () => 'jwt',
    onUnauthorized: () {},
    onNetworkFailure: seen.add,
    dio: Dio(BaseOptions(validateStatus: (_) => true))
      ..httpClientAdapter = _FailingAdapter(type),
  );
}

ClientFailure _failure({bool timedOut = true}) => ClientFailure(
  method: 'POST',
  path: '/docker/hosts/agent-9/containers/abc/stop',
  timedOut: timedOut,
  elapsed: const Duration(seconds: 15),
);

void main() {
  group('ApiClient sem resposta', () {
    test('tempo esgotado diz quanto esperou e avisa a central', () async {
      final seen = <ClientFailure>[];
      final client = _client(DioExceptionType.receiveTimeout, seen);

      await expectLater(
        () => client.post<dynamic>(
          '/docker/hosts/agent-9/containers/abc/stop',
          timeout: const Duration(seconds: 15),
        ),
        throwsA(
          isA<NetworkException>()
              .having((e) => e.timedOut, 'timedOut', isTrue)
              .having((e) => e.message, 'message', contains('15 s')),
        ),
      );
      expect(seen, hasLength(1));
      expect(seen.single.method, 'POST');
      expect(seen.single.path, '/docker/hosts/agent-9/containers/abc/stop');
      expect(seen.single.timedOut, isTrue);
    });

    test('conexão interrompida continua "falha de conexão"', () async {
      final seen = <ClientFailure>[];
      final client = _client(DioExceptionType.connectionError, seen);

      await expectLater(
        () => client.get<dynamic>('/docker/hosts/agent-9/compose'),
        throwsA(
          isA<NetworkException>()
              .having((e) => e.timedOut, 'timedOut', isFalse)
              .having(
                (e) => e.message,
                'message',
                'Falha de conexão com o servidor.',
              ),
        ),
      );
      expect(seen.single.method, 'GET');
      expect(seen.single.timedOut, isFalse);
    });

    test('o próprio aviso que falha não gera outro aviso', () async {
      final seen = <ClientFailure>[];
      final client = _client(DioExceptionType.connectionError, seen);

      await expectLater(
        () => client.post<dynamic>(clientFailuresPath),
        throwsA(isA<NetworkException>()),
      );
      expect(seen, isEmpty);
    });
  });

  group('ClientFailureReporter', () {
    test('manda o aviso no formato que a central valida', () async {
      final sent = <Map<String, dynamic>>[];
      final reporter = ClientFailureReporter(
        (body) async => sent.add(body),
        currentPage: () => '/docker/compose',
      );

      reporter.report(_failure());
      await pumpEventQueue();

      expect(sent.single, <String, dynamic>{
        'method': 'POST',
        'path': '/api/docker/hosts/agent-9/containers/abc/stop',
        'kind': 'timeout',
        'elapsedMs': 15000,
        'page': '/docker/compose',
      });
    });

    test(
      'central fora de alcance: guarda e manda junto com o próximo',
      () async {
        var reachable = false;
        final sent = <Map<String, dynamic>>[];
        final reporter = ClientFailureReporter((body) async {
          if (!reachable) throw const NetworkException();
          sent.add(body);
        }, currentPage: () => null);

        reporter.report(_failure(timedOut: false));
        await pumpEventQueue();
        expect(reporter.pendingCount, 1);

        reachable = true;
        reporter.report(_failure());
        await pumpEventQueue();
        expect(sent.map((body) => body['kind']), <String>[
          'network',
          'timeout',
        ]);
        expect(reporter.pendingCount, 0);
      },
    );

    test('aviso recusado pela central não fica preso na fila', () async {
      final reporter = ClientFailureReporter(
        (_) async => throw const ApiException(message: 'x', statusCode: 422),
        currentPage: () => null,
      );
      reporter.report(_failure());
      await pumpEventQueue();
      expect(reporter.pendingCount, 0);
    });

    test('a fila tem teto', () async {
      final reporter = ClientFailureReporter(
        (_) async => throw const NetworkException(),
        currentPage: () => null,
      );
      for (var i = 0; i < ClientFailureReporter.maxPending + 5; i++) {
        reporter.report(_failure());
        await pumpEventQueue();
      }
      expect(reporter.pendingCount, ClientFailureReporter.maxPending);
    });
  });

  test('503 com código: a tela cita o incidente que está no log', () async {
    final client = ApiClient(
      baseUrlProvider: () => 'http://central.invalid/api',
      tokenProvider: () => 'jwt',
      onUnauthorized: () {},
      dio: Dio(BaseOptions(validateStatus: (_) => true))
        ..httpClientAdapter = _RespondingAdapter(
          ResponseBody.fromString(
            '{"message":"O agente agent-9 está desconectado"}',
            503,
            headers: <String, List<String>>{
              Headers.contentTypeHeader: <String>[Headers.jsonContentType],
              incidentHeader: <String>['7F3A9C21'],
            },
          ),
        ),
    );

    await expectLater(
      () => client.get<dynamic>('/docker/hosts/agent-9/containers'),
      throwsA(
        isA<ApiException>().having(
          (e) => e.message,
          'message',
          'O agente agent-9 está desconectado (incidente 7F3A9C21)',
        ),
      ),
    );
    expect(
      withIncidentCode('Erro (incidente AB12)', 'AB12'),
      'Erro (incidente AB12)',
    );
    expect(withIncidentCode('Erro', null), 'Erro');
  });

  test('describeActionError só suaviza o tempo esgotado', () {
    expect(
      describeActionError(
        NetworkException.timeout(const Duration(seconds: 15)),
      ),
      allOf(contains('15 s'), contains('pode ter sido concluída')),
    );
    expect(
      describeActionError(const NetworkException()),
      'Falha de conexão com o servidor.',
    );
    expect(
      describeActionError(
        const ApiException(message: 'Recusado', statusCode: 409),
      ),
      'Recusado',
    );
  });
}
