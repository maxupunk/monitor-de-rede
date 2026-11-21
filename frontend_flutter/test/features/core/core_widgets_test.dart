import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/network/sse_client.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/features/auth/area_off_page.dart';
import 'package:netmonitor_app/features/auth/login_page.dart';
import 'package:netmonitor_app/features/auth/setup_page.dart';
import 'package:netmonitor_app/features/core/presentation/events/events_page.dart';
import 'package:netmonitor_app/features/core/presentation/settings/settings_page.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

Widget _wrap(Widget child, {List<Override> overrides = const <Override>[]}) {
  return ProviderScope(
    overrides: overrides,
    child: MaterialApp(
      theme: AppTheme.light(),
      darkTheme: AppTheme.dark(),
      home: Scaffold(body: child),
    ),
  );
}

void main() {
  testWidgets('LoginPage displays restored backup notice when restored=true', (
    tester,
  ) async {
    await tester.pumpWidget(_wrap(const LoginPage(restored: true)));
    expect(find.textContaining('Backup completo restaurado'), findsOneWidget);
    expect(find.text('Entrar'), findsOneWidget);
  });

  testWidgets(
    'SetupPage shows SETUP_TOKEN field and CLI hint commands by default',
    (tester) async {
      await tester.pumpWidget(_wrap(const SetupPage()));
      expect(find.text('Token de instalação (SETUP_TOKEN)'), findsOneWidget);
      expect(find.text('docker compose logs netmonitor'), findsOneWidget);
      expect(
        find.text(
          'docker compose exec netmonitor backend-cli task auth_setup_token',
        ),
        findsOneWidget,
      );
    },
  );

  testWidgets('AreaOffPage renders access-denied mode vs admin enable button', (
    tester,
  ) async {
    final noAccessContainer = ProviderContainer();
    addTearDown(noAccessContainer.dispose);
    noAccessContainer.read(authProvider.notifier).state = const AuthState(
      token: 't',
      user: AuthUser(id: 2, email: 'viewer@local', role: UserRole.viewer),
    );
    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: noAccessContainer,
        child: MaterialApp(
          theme: AppTheme.light(),
          home: const Scaffold(
            body: AreaOffPage(areaId: 'servers', reason: 'acesso'),
          ),
        ),
      ),
    );
    expect(find.text('Você não tem acesso a esta área'), findsOneWidget);
    expect(find.text('Ligar área'), findsNothing);

    final container = ProviderContainer();
    addTearDown(container.dispose);
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
        <String, dynamic>{
          'area': 'servers',
          'title': 'Servidores',
          'summary': 'Docker e deploys',
          'icon': '',
          'enabled': false,
          'optional': true,
        },
      ],
    });
    container.read(authProvider.notifier).state = const AuthState(
      token: 't',
      user: AuthUser(id: 1, email: 'admin@local', role: UserRole.admin),
    );

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          theme: AppTheme.light(),
          home: const Scaffold(body: AreaOffPage(areaId: 'servers')),
        ),
      ),
    );
    expect(find.text('Esta área está desligada'), findsOneWidget);
    expect(find.text('Ligar área'), findsOneWidget);
    expect(find.text('Ir para Configurações'), findsOneWidget);
  });

  testWidgets('EventsPage renders live SSE events without polling', (
    tester,
  ) async {
    final container = ProviderContainer();
    addTearDown(container.dispose);

    await tester.pumpWidget(
      UncontrolledProviderScope(
        container: container,
        child: MaterialApp(
          theme: AppTheme.light(),
          home: const Scaffold(body: EventsPage()),
        ),
      ),
    );

    container
        .read(sseServiceProvider.notifier)
        .dispatchForTest(
          const SseEvent(
            type: 'alert:triggered',
            timestamp: '2026-06-01T14:00:00Z',
            data: <String, dynamic>{
              'id': 1,
              'severity': 'critical',
              'title': 'Switch core indisponível',
            },
          ),
        );
    await tester.pump();

    expect(find.text('Alerta disparado'), findsOneWidget);
    expect(find.text('Switch core indisponível'), findsOneWidget);
  });

  testWidgets(
    'SettingsPage renders dynamic Core tabs from SettingsSectionRegistry',
    (tester) async {
      final mockDio = Dio()
        ..interceptors.add(
          InterceptorsWrapper(
            onRequest: (options, handler) {
              handler.resolve(
                Response<dynamic>(
                  requestOptions: options,
                  statusCode: 200,
                  data: <String, dynamic>{},
                ),
              );
            },
          ),
        );
      final mockApi = ApiClient(
        baseUrlProvider: () => 'http://127.0.0.1/api',
        tokenProvider: () => 'test-token',
        onUnauthorized: () {},
        dio: mockDio,
      );

      await tester.pumpWidget(
        _wrap(
          const SettingsPage(),
          overrides: <Override>[apiClientProvider.overrideWithValue(mockApi)],
        ),
      );
      await tester.pumpAndSettle();

      expect(find.text('Configurações'), findsOneWidget);
      expect(find.text('Geral'), findsOneWidget);
      expect(find.text('Conectividade'), findsOneWidget);
      expect(find.text('Notificações'), findsOneWidget);
      expect(find.widgetWithText(ChoiceChip, 'Sistema'), findsOneWidget);
    },
  );
}
