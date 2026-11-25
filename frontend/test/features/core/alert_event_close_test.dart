import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/features/core/presentation/alerts/alerts_page.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

Map<String, dynamic> _vpnAlert(String status) => <String, dynamic>{
  'id': 7,
  'title': 'Túnel VPN caiu — hostinger-srv-1',
  'message': 'Túnel VPN de hostinger-srv-1 caiu. Sem sinal há 5 min.',
  'status': status,
  'severity': 'critical',
  'deviceName': 'hostinger-srv-1',
  'startedAt': '2026-11-18T10:00:00Z',
};

/// A central de alertas com um alerta de evento (sem monitor).
ApiClient _api(List<String> calls) {
  final dio = Dio()
    ..interceptors.add(
      InterceptorsWrapper(
        onRequest: (options, handler) {
          final path = options.path;
          if (options.method == 'POST') calls.add(path);
          final Object data;
          if (path.endsWith('/alerts/7/verify')) {
            data = <String, dynamic>{
              'message': 'Este alerta vem de um evento, não de um monitor.',
              'checkable': false,
              'resolved': false,
              'event': _vpnAlert('acknowledged'),
            };
          } else if (path.endsWith('/alerts/7/close')) {
            data = <String, dynamic>{
              'message': 'Alerta #7 encerrado.',
              'resolved': true,
              'event': _vpnAlert('resolved'),
            };
          } else if (path.endsWith('/alerts')) {
            data = <Map<String, dynamic>>[_vpnAlert('acknowledged')];
          } else if (path.endsWith('/alerts/rules')) {
            data = const <Object>[];
          } else {
            data = <String, dynamic>{};
          }
          handler.resolve(
            Response<dynamic>(
              requestOptions: options,
              statusCode: 200,
              data: data,
            ),
          );
        },
      ),
    );
  return ApiClient(
    baseUrlProvider: () => 'http://127.0.0.1/api',
    tokenProvider: () => 'jwt',
    onUnauthorized: () {},
    dio: dio,
  );
}

void main() {
  testWidgets(
    'alerta de evento: verificar explica e oferece encerrar, que pede confirmação',
    (tester) async {
      tester.view.physicalSize = const Size(1280, 1600);
      tester.view.devicePixelRatio = 1;
      addTearDown(tester.view.reset);
      final calls = <String>[];
      final container = ProviderContainer(
        overrides: <Override>[apiClientProvider.overrideWithValue(_api(calls))],
      );
      addTearDown(container.dispose);
      container.read(authProvider.notifier).state = const AuthState(
        token: 'jwt',
        user: AuthUser(id: 1, email: 'admin@local', role: UserRole.admin),
      );
      container.read(areasProvider.notifier).apply(<String, dynamic>{
        'areas': <Map<String, dynamic>>[
          <String, dynamic>{
            'area': 'core',
            'title': 'Núcleo',
            'summary': 'Base',
            'icon': '',
            'enabled': true,
            'optional': false,
          },
        ],
      });

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            theme: AppTheme.light(),
            home: const Scaffold(body: AlertsPage()),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('Túnel VPN caiu — hostinger-srv-1'), findsOneWidget);
      expect(find.textContaining('Alerta de evento'), findsOneWidget);

      await tester.tap(find.text('Verificar agora'));
      await tester.pumpAndSettle();
      expect(find.textContaining('vem de um evento'), findsOneWidget);

      // O atalho da mensagem leva à confirmação; cancelar não encerra.
      await tester.tap(find.widgetWithText(SnackBarAction, 'Encerrar'));
      await tester.pumpAndSettle();
      expect(find.text('Encerrar o alerta?'), findsOneWidget);
      await tester.tap(find.text('Cancelar'));
      await tester.pumpAndSettle();
      expect(calls.where((path) => path.endsWith('/close')), isEmpty);

      await tester.tap(find.widgetWithText(OutlinedButton, 'Encerrar'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Encerrar alerta'));
      await tester.pumpAndSettle();

      expect(
        calls.where((path) => path.endsWith('/alerts/7/close')),
        hasLength(1),
      );
      expect(find.text('Túnel VPN caiu — hostinger-srv-1'), findsNothing);
      expect(tester.takeException(), isNull);
    },
  );
}
