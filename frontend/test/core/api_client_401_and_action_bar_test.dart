import 'dart:convert';
import 'dart:typed_data';

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/utils/device_defaults.dart';
import 'package:netmonitor_app/shared/widgets/section_card.dart';
import 'package:netmonitor_app/shared/widgets/semantic_chip.dart';
import 'package:netmonitor_app/shared/widgets/symmetrical_action_bar.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class _MockHttpAdapter implements HttpClientAdapter {
  _MockHttpAdapter(this.handler);

  final ResponseBody Function(RequestOptions options) handler;

  @override
  Future<ResponseBody> fetch(
    RequestOptions options,
    Stream<Uint8List>? requestStream,
    Future<void>? cancelFuture,
  ) async {
    return handler(options);
  }

  @override
  void close({bool force = false}) {}
}

ResponseBody _jsonResponse(
  int statusCode,
  Object? body, {
  String statusMessage = 'Unauthorized',
}) {
  final encoded = body == null
      ? ''
      : (body is String ? body : jsonEncode(body));
  return ResponseBody.fromString(
    encoded,
    statusCode,
    statusMessage: statusMessage,
    headers: <String, List<String>>{
      Headers.contentTypeHeader: <String>[
        body is String ? Headers.textPlainContentType : Headers.jsonContentType,
      ],
    },
  );
}

void main() {
  group('ApiClient 401 session isolation vs remote target 401', () {
    test(
      'POST /devices/3/access/dedicated returning 401 NEVER calls onUnauthorized and formats clear PT-BR message',
      () async {
        var unauthorizedCalls = 0;
        final dio = Dio(BaseOptions(validateStatus: (_) => true))
          ..httpClientAdapter = _MockHttpAdapter((options) {
            return _jsonResponse(401, 'Unauthorized');
          });

        final client = ApiClient(
          baseUrlProvider: () => 'http://localhost:5150/api',
          tokenProvider: () => 'valid-jwt-token',
          onUnauthorized: () => unauthorizedCalls++,
          dio: dio,
        );

        await expectLater(
          () => client.post<Map<String, dynamic>>(
            '/devices/3/access/dedicated',
            data: <String, dynamic>{'username': 'admin', 'password': 'wrong'},
          ),
          throwsA(
            isA<ApiException>()
                .having((e) => e.statusCode, 'statusCode', 401)
                .having(
                  (e) => e.message,
                  'message',
                  'Usuário ou senha recusados pelo equipamento.',
                ),
          ),
        );
        expect(unauthorizedCalls, 0);
      },
    );

    test(
      'other remote-target / non-session 401 endpoints NEVER log out NetMonitor session',
      () async {
        var unauthorizedCalls = 0;
        final dio = Dio(BaseOptions(validateStatus: (_) => true))
          ..httpClientAdapter = _MockHttpAdapter((_) {
            return _jsonResponse(401, <String, dynamic>{
              'message': 'Usuário ou senha recusados pelo equipamento.',
            });
          });

        final client = ApiClient(
          baseUrlProvider: () => 'http://localhost:5150/api',
          tokenProvider: () => 'valid-jwt-token',
          onUnauthorized: () => unauthorizedCalls++,
          dio: dio,
        );

        const nonSessionPaths = <String>[
          '/auth/login',
          '/auth/setup',
          '/devices/7/credentials',
          '/devices/7/credentials/ssh/session',
          '/devices/7/plugins/2/actions/apply',
          '/logs/devices/7/provision',
          '/plugins/2/fleet/identify',
          '/snmp/walk',
          '/databases/1/test',
          '/storages/1/test',
          '/ai/ollama/tags',
          '/ai/decider/test',
          '/deploys/webhook',
        ];

        for (final path in nonSessionPaths) {
          await expectLater(
            () => client.post<Map<String, dynamic>>(path),
            throwsA(isA<ApiException>()),
          );
        }

        expect(unauthorizedCalls, 0);
      },
    );

    test(
      'GET /auth/me (and protected session routes) returning 401 DOES invoke onUnauthorized',
      () async {
        var unauthorizedCalls = 0;
        final dio = Dio(BaseOptions(validateStatus: (_) => true))
          ..httpClientAdapter = _MockHttpAdapter((_) {
            return _jsonResponse(401, <String, dynamic>{
              'message': 'Não autenticado',
            });
          });

        final client = ApiClient(
          baseUrlProvider: () => 'http://localhost:5150/api',
          tokenProvider: () => 'expired-jwt-token',
          onUnauthorized: () => unauthorizedCalls++,
          dio: dio,
        );

        await expectLater(
          () => client.get<Map<String, dynamic>>('/auth/me'),
          throwsA(
            isA<ApiException>().having((e) => e.statusCode, 'statusCode', 401),
          ),
        );
        expect(unauthorizedCalls, 1);

        await expectLater(
          () => client.get<Map<String, dynamic>>('/monitors'),
          throwsA(isA<ApiException>()),
        );
        expect(unauthorizedCalls, 2);
      },
    );
  });

  group('device_defaults helpers', () {
    test(
      'defaultDeviceAdminUsername maps OpenWrt/Linux to root, MikroTik to admin, Ubiquiti to ubnt',
      () {
        expect(defaultDeviceAdminUsername('openwrt'), 'root');
        expect(defaultDeviceAdminUsername('OpenWrt 23.05.3'), 'root');
        expect(defaultDeviceAdminUsername('Ubuntu 24.04 Linux'), 'root');
        expect(defaultDeviceAdminUsername('pfSense'), 'root');
        expect(defaultDeviceAdminUsername(null), 'root');
        expect(defaultDeviceAdminUsername(''), 'root');

        expect(defaultDeviceAdminUsername('mikrotik'), 'admin');
        expect(defaultDeviceAdminUsername('MikroTik RouterOS 7.15'), 'admin');
        expect(defaultDeviceAdminUsername('FortiGate FortiOS'), 'admin');
        expect(defaultDeviceAdminUsername('Intelbras'), 'admin');

        expect(defaultDeviceAdminUsername('Ubiquiti UniFi'), 'ubnt');
        expect(defaultDeviceAdminUsername('EdgeOS'), 'ubnt');
        expect(defaultDeviceAdminUsername('airMAX'), 'ubnt');
      },
    );

    test(
      'isDeviceAuthFailure and formatDeviceAuthFailureMessage detect 401/403/codes and hint default login',
      () {
        const err401 = ApiException(
          message: 'Usuário ou senha recusados pelo equipamento.',
          statusCode: 401,
        );
        const errCode = ApiException(
          message: 'Credencial inválida',
          statusCode: 422,
          payload: ApiErrorPayload(code: 'invalid_credentials'),
        );
        const err500 = ApiException(
          message: 'Erro interno do servidor',
          statusCode: 500,
        );

        expect(isDeviceAuthFailure(err401), isTrue);
        expect(isDeviceAuthFailure(errCode), isTrue);
        expect(isDeviceAuthFailure(err500), isFalse);

        final openwrtMsg = formatDeviceAuthFailureMessage(
          err401,
          platformOrOs: 'openwrt',
        );
        expect(openwrtMsg, contains("'root'"));
        expect(openwrtMsg, contains('OpenWrt'));

        final mikrotikMsg = formatDeviceAuthFailureMessage(
          err401,
          platformOrOs: 'mikrotik',
        );
        expect(mikrotikMsg, contains("'admin'"));
      },
    );
  });

  group('SymmetricalActionBar at 360px mobile & desktop', () {
    Future<void> pumpBar(
      WidgetTester tester, {
      required int count,
      double viewportWidth = 360,
      bool primaryFullWidthWhenOdd = false,
    }) async {
      tester.view.physicalSize = Size(viewportWidth, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(
        MaterialApp(
          theme: AppTheme.dark(),
          home: Scaffold(
            body: Padding(
              padding: const EdgeInsets.all(16),
              child: SymmetricalActionBar(
                spacing: 8,
                runSpacing: 8,
                primaryFullWidthWhenOdd: primaryFullWidthWhenOdd,
                children: List<Widget>.generate(
                  count,
                  (i) => FilledButton(
                    key: ValueKey<String>('btn_$i'),
                    onPressed: () {},
                    child: Text('Ação ${i + 1}'),
                  ),
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(tester.takeException(), isNull);
    }

    testWidgets('1 button spans 100% full width (328px inside 16px padding)', (
      tester,
    ) async {
      await pumpBar(tester, count: 1);
      final size0 = tester.getSize(find.byKey(const ValueKey<String>('btn_0')));
      expect(size0.width, closeTo(328, 0.5));
    });

    testWidgets('2 buttons split 50% / 50% symmetrically across full width', (
      tester,
    ) async {
      await pumpBar(tester, count: 2);
      final rect0 = tester.getRect(find.byKey(const ValueKey<String>('btn_0')));
      final rect1 = tester.getRect(find.byKey(const ValueKey<String>('btn_1')));
      expect(rect0.top, closeTo(rect1.top, 0.5));
      expect(rect0.width, closeTo(160, 0.5));
      expect(rect1.width, closeTo(160, 0.5));
      expect(rect1.right - rect0.left, closeTo(328, 0.5));
    });

    testWidgets(
      '3 buttons on 360px mobile render 50%/50% Row 1 + 100% Row 2 (zero half-width orphan)',
      (tester) async {
        await pumpBar(tester, count: 3);
        final rect0 = tester.getRect(
          find.byKey(const ValueKey<String>('btn_0')),
        );
        final rect1 = tester.getRect(
          find.byKey(const ValueKey<String>('btn_1')),
        );
        final rect2 = tester.getRect(
          find.byKey(const ValueKey<String>('btn_2')),
        );

        expect(rect0.top, closeTo(rect1.top, 0.5));
        expect(rect0.width, closeTo(160, 0.5));
        expect(rect1.width, closeTo(160, 0.5));
        expect(rect2.top, greaterThan(rect0.bottom));
        expect(rect2.width, closeTo(328, 0.5));
      },
    );

    testWidgets(
      '3 buttons with primaryFullWidthWhenOdd: true render 100% Row 1 + 50%/50% Row 2',
      (tester) async {
        await pumpBar(tester, count: 3, primaryFullWidthWhenOdd: true);
        final rect0 = tester.getRect(
          find.byKey(const ValueKey<String>('btn_0')),
        );
        final rect1 = tester.getRect(
          find.byKey(const ValueKey<String>('btn_1')),
        );
        final rect2 = tester.getRect(
          find.byKey(const ValueKey<String>('btn_2')),
        );

        expect(rect0.width, closeTo(328, 0.5));
        expect(rect1.top, greaterThan(rect0.bottom));
        expect(rect1.top, closeTo(rect2.top, 0.5));
        expect(rect1.width, closeTo(160, 0.5));
        expect(rect2.width, closeTo(160, 0.5));
      },
    );

    testWidgets(
      '4 buttons on 360px mobile render two 50%/50% full-width rows',
      (tester) async {
        await pumpBar(tester, count: 4);
        final rect0 = tester.getRect(
          find.byKey(const ValueKey<String>('btn_0')),
        );
        final rect1 = tester.getRect(
          find.byKey(const ValueKey<String>('btn_1')),
        );
        final rect2 = tester.getRect(
          find.byKey(const ValueKey<String>('btn_2')),
        );
        final rect3 = tester.getRect(
          find.byKey(const ValueKey<String>('btn_3')),
        );

        expect(rect0.width, closeTo(160, 0.5));
        expect(rect1.width, closeTo(160, 0.5));
        expect(rect2.width, closeTo(160, 0.5));
        expect(rect3.width, closeTo(160, 0.5));
        expect(rect2.top, greaterThan(rect0.bottom));
        expect(rect2.top, closeTo(rect3.top, 0.5));
      },
    );
  });

  group('SectionCard smart trailing placement at 360px', () {
    testWidgets(
      'keeps compact SemanticChip inline on the right at 360px while stacking wide Row/SegmentedButton below',
      (tester) async {
        tester.view.physicalSize = const Size(360, 844);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        await tester.pumpWidget(
          MaterialApp(
            theme: AppTheme.dark(),
            home: Scaffold(
              body: SingleChildScrollView(
                padding: const EdgeInsets.all(12),
                child: Column(
                  children: <Widget>[
                    const SectionCard(
                      icon: Icons.favorite_outline,
                      title: 'Saúde geral',
                      trailing: SemanticChip(
                        label: '100,0%',
                        tone: SemanticTone.success,
                      ),
                      child: Text('Resumo operacional'),
                    ),
                    const SizedBox(height: 12),
                    SectionCard(
                      icon: Icons.dns_outlined,
                      title: 'Servidores DNS',
                      trailing: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: <Widget>[
                          OutlinedButton(
                            onPressed: () {},
                            child: const Text('Testar lote'),
                          ),
                          const SizedBox(width: 8),
                          FilledButton(
                            onPressed: () {},
                            child: const Text('Adicionar'),
                          ),
                        ],
                      ),
                      child: const Text('Lista DNS'),
                    ),
                  ],
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);

        // 1) Compact chip '100,0%' fica inline à direita de 'Saúde geral' (mesma faixa vertical).
        final healthTitleRect = tester.getRect(find.text('Saúde geral'));
        final chipRect = tester.getRect(find.text('100,0%'));
        expect(chipRect.left, greaterThan(healthTitleRect.right));
        expect(
          (chipRect.center.dy - healthTitleRect.center.dy).abs(),
          lessThan(16),
        );

        // 2) Barra multi-botão (Row) empilha abaixo do título 'Servidores DNS'.
        final dnsTitleRect = tester.getRect(find.text('Servidores DNS'));
        final btnRect = tester.getRect(find.text('Testar lote'));
        expect(btnRect.top, greaterThan(dnsTitleRect.bottom));
      },
    );
  });
}
