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
import 'package:netmonitor_app/features/auth/server_config_page.dart';
import 'package:netmonitor_app/features/core/presentation/alerts/alert_dialogs.dart';
import 'package:netmonitor_app/features/core/presentation/alerts/alerts_page.dart';
import 'package:netmonitor_app/features/core/presentation/audit/audit_page.dart';
import 'package:netmonitor_app/features/core/presentation/backup/backup_page.dart';
import 'package:netmonitor_app/features/core/presentation/dashboard/dashboard_page.dart';
import 'package:netmonitor_app/features/core/presentation/events/events_page.dart';
import 'package:netmonitor_app/features/core/presentation/logs/logs_page.dart';
import 'package:netmonitor_app/features/core/presentation/maintenance/maintenance_windows_page.dart';
import 'package:netmonitor_app/features/core/presentation/monitors/monitor_form_sheet.dart';
import 'package:netmonitor_app/features/core/presentation/monitors/monitors_page.dart';
import 'package:netmonitor_app/features/core/presentation/scheduled_tasks/scheduled_tasks_page.dart';
import 'package:netmonitor_app/features/core/presentation/settings/settings_page.dart';
import 'package:netmonitor_app/features/core/presentation/users/users_page.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:shared_preferences/shared_preferences.dart';

ApiClient _buildMockApi() {
  final dio = Dio()
    ..interceptors.add(
      InterceptorsWrapper(
        onRequest: (options, handler) {
          final path = options.path;
          Object data;
          if (path.endsWith('/logs')) {
            data = <String, dynamic>{
              'data': <Map<String, dynamic>>[
                <String, dynamic>{
                  'id': 101,
                  'deviceId': 1,
                  'deviceName': 'Router-Core-01',
                  'sourceIp': '192.168.1.1',
                  'receivedAt': '2026-06-01T14:00:00Z',
                  'severity': 3,
                  'severityLabel': 'err',
                  'appName': 'dhcpd',
                  'message': 'DHCPNAK on 192.168.1.50 to aa:bb:cc:dd:ee:ff',
                },
                <String, dynamic>{
                  'id': 102,
                  'deviceId': 1,
                  'deviceName': 'Router-Core-01',
                  'sourceIp': '192.168.1.1',
                  'receivedAt': '2026-06-01T13:59:00Z',
                  'severity': 6,
                  'severityLabel': 'info',
                  'appName': 'kernel',
                  'message': 'eth0: link is up 1000Mbps full duplex',
                },
              ],
              'templates': const <Object>[],
              'hasMore': false,
            };
          } else if (path.endsWith('/logs/sources')) {
            data = <String, dynamic>{
              'data': const <Object>[],
              'unknownCount': 0,
            };
          } else if (path.endsWith('/monitors')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 1,
                'name': 'Gateway Principal',
                'type': 'ping',
                'target': '192.168.1.1',
                'status': 'up',
                'isEnabled': true,
                'intervalSeconds': 60,
                'timeoutSeconds': 5,
                'lastLatencyMs': 2.4,
                'deviceId': 1,
                'deviceName': 'Router-Core-01',
              },
              <String, dynamic>{
                'id': 2,
                'name': 'Link Backup Fibra',
                'type': 'ping',
                'target': '10.0.0.1',
                'status': 'down',
                'isEnabled': true,
                'intervalSeconds': 30,
                'timeoutSeconds': 5,
                'deviceId': 2,
                'deviceName': 'Switch-Dist-02',
              },
            ];
          } else if (path.endsWith('/devices')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 1,
                'name': 'Router-Core-01',
                'ipAddress': '192.168.1.1',
                'status': 'online',
              },
              <String, dynamic>{
                'id': 2,
                'name': 'Switch-Dist-02',
                'ipAddress': '10.0.0.1',
                'status': 'offline',
              },
            ];
          } else if (path.endsWith('/alerts')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 10,
                'title': 'Link Backup fora do ar',
                'message': 'Sem resposta ICMP após 3 tentativas.',
                'severity': 'critical',
                'status': 'active',
                'deviceId': 2,
                'deviceName': 'Switch-Dist-02',
                'startedAt': '2026-06-01T13:50:00Z',
              },
            ];
          } else if (path.endsWith('/alerts/rules')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 1,
                'name': 'Latência alta no core',
                'type': 'latency_high',
                'severity': 'warning',
                'isEnabled': true,
                'durationSeconds': 60,
                'condition': <String, dynamic>{
                  'field': 'latency_ms',
                  'operator': '>',
                  'value': 100,
                },
              },
            ];
          } else if (path.endsWith('/audit-logs')) {
            data = <String, dynamic>{
              'data': <Map<String, dynamic>>[
                <String, dynamic>{
                  'id': 1,
                  'action': 'update',
                  'resourceType': 'monitor',
                  'resourceLabel': 'Gateway Principal',
                  'userEmail': 'admin@local',
                  'ipAddress': '192.168.1.10',
                  'createdAt': '2026-06-01T14:00:00Z',
                },
              ],
              'meta': <String, dynamic>{
                'total': 1,
                'currentPage': 1,
                'lastPage': 1,
                'perPage': 25,
              },
            };
          } else if (path.endsWith('/dashboard/catalog')) {
            data = <String, dynamic>{
              'widgets': <Map<String, dynamic>>[
                <String, dynamic>{
                  'type': 'stat_cards',
                  'title': 'Resumo',
                  'description': 'Indicadores principais',
                  'area': 'core',
                  'defaultEnabled': true,
                  'md': 12,
                  'lg': 12,
                },
              ],
              'defaultLayout': <Map<String, dynamic>>[
                <String, dynamic>{
                  'type': 'stat_cards',
                  'size': 'full',
                  'visible': true,
                },
              ],
            };
          } else if (path.endsWith('/dashboard/layout')) {
            data = <String, dynamic>{
              'layout': <Map<String, dynamic>>[
                <String, dynamic>{
                  'id': 'stat_cards',
                  'type': 'stat_cards',
                  'title': 'Resumo',
                  'visible': true,
                  'md': 12,
                  'lg': 12,
                },
              ],
            };
          } else if (path.endsWith('/users')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 1,
                'fullName': 'Administrador',
                'email': 'admin@local',
                'role': 'admin',
                'active': true,
              },
            ];
          } else if (path.endsWith('/scheduled-tasks') ||
              path.endsWith('/maintenance-windows') ||
              path.endsWith('/sites') ||
              path.endsWith('/probes') ||
              path.endsWith('/operating-systems') ||
              path.endsWith('/snmp-profiles')) {
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
    tokenProvider: () => 'test-token',
    onUnauthorized: () {},
    dio: dio,
  );
}

ProviderContainer _makeAdminContainer(ApiClient api) {
  final container = ProviderContainer(
    overrides: <Override>[apiClientProvider.overrideWithValue(api)],
  );
  container.read(authProvider.notifier).state = const AuthState(
    token: 'test-token',
    user: AuthUser(
      id: 1,
      email: 'admin@local',
      fullName: 'Administrador',
      role: UserRole.admin,
    ),
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
  return container;
}

Future<void> _pumpMobile(
  WidgetTester tester,
  ProviderContainer container,
  Widget child, {
  Size size = const Size(390, 844),
}) async {
  tester.view.physicalSize = size;
  tester.view.devicePixelRatio = 1.0;
  addTearDown(() {
    tester.view.resetPhysicalSize();
    tester.view.resetDevicePixelRatio();
  });

  await tester.pumpWidget(
    UncontrolledProviderScope(
      container: container,
      child: MaterialApp(
        theme: AppTheme.light(),
        darkTheme: AppTheme.dark(),
        home: Scaffold(
          body: SafeArea(
            child: Padding(padding: const EdgeInsets.all(12), child: child),
          ),
        ),
      ),
    ),
  );
  await tester.pumpAndSettle();
}

void main() {
  setUp(() {
    SharedPreferences.setMockInitialValues(<String, Object>{
      'netmonitor_dashboard_prompt_dismissed': true,
    });
  });

  testWidgets(
    'LogsPage renders compact mobile bar, UnifiedFilterBar sheet, and logs with zero overflow at 390x844',
    (tester) async {
      final api = _buildMockApi();
      final container = _makeAdminContainer(api);
      addTearDown(container.dispose);

      await _pumpMobile(tester, container, const LogsPage());

      expect(find.text('Logs Syslog'), findsOneWidget);
      expect(find.text('Ao Vivo'), findsOneWidget);
      expect(find.byType(UnifiedFilterBar), findsOneWidget);
      expect(find.text('Filtros'), findsOneWidget);
      expect(find.textContaining('DHCPNAK'), findsOneWidget);
      expect(tester.takeException(), isNull);

      // Open the mobile filter sheet and confirm zero overflow inside.
      await tester.tap(find.text('Filtros'));
      await tester.pumpAndSettle();
      expect(find.text('Filtrar logs'), findsOneWidget);
      expect(find.text('Aplicar'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'EventsPage renders NestedScrollView, compact header, UnifiedFilterBar, and live SSE events at 390x844',
    (tester) async {
      final api = _buildMockApi();
      final container = _makeAdminContainer(api);
      addTearDown(container.dispose);

      await _pumpMobile(tester, container, const EventsPage());

      container
          .read(sseServiceProvider.notifier)
          .dispatchForTest(
            const SseEvent(
              type: 'alert:triggered',
              timestamp: '2026-06-01T14:00:00Z',
              data: <String, dynamic>{
                'id': 1,
                'severity': 'critical',
                'title': 'Link principal degradado',
              },
            ),
          );
      await tester.pumpAndSettle();

      expect(find.byType(NestedScrollView), findsOneWidget);
      expect(find.byType(UnifiedFilterBar), findsOneWidget);
      expect(find.text('Eventos'), findsOneWidget);
      expect(find.text('Pausar'), findsOneWidget);
      expect(find.text('Tempo Real (1)'), findsOneWidget);
      expect(find.text('Link principal degradado'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'MonitorsPage renders symmetrical status chips (no orphan KPI cards) and list above the fold at 390x844',
    (tester) async {
      final api = _buildMockApi();
      final container = _makeAdminContainer(api);
      addTearDown(container.dispose);

      await _pumpMobile(tester, container, const MonitorsPage());

      expect(find.text('Monitores'), findsOneWidget);
      expect(find.byType(UnifiedFilterBar), findsOneWidget);
      expect(find.byType(KpiCard), findsNothing);
      expect(find.text('Todos (2)'), findsOneWidget);
      expect(find.text('Online (1)'), findsOneWidget);
      expect(find.text('Gateway Principal'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'AlertsPage renders NestedScrollView with pinned TabBar and UnifiedFilterBar at 390x844',
    (tester) async {
      final api = _buildMockApi();
      final container = _makeAdminContainer(api);
      addTearDown(container.dispose);

      await _pumpMobile(tester, container, const AlertsPage());

      expect(find.byType(NestedScrollView), findsOneWidget);
      expect(find.byType(UnifiedFilterBar), findsOneWidget);
      expect(find.text('Ativos (1)'), findsOneWidget);
      expect(find.text('Link Backup fora do ar'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'DashboardPage renders compact 1-row top bar and symmetrical 2x2 KPI grid at 390x844',
    (tester) async {
      final api = _buildMockApi();
      final container = _makeAdminContainer(api);
      addTearDown(container.dispose);

      await _pumpMobile(tester, container, const DashboardPage());

      expect(find.text('Dashboard'), findsOneWidget);
      expect(find.text('Personalizar'), findsOneWidget);

      final kpiFinder = find.byType(KpiCard);
      expect(kpiFinder, findsNWidgets(4));

      // Verify 2x2 symmetry: card 0 and card 1 sit on the same horizontal row.
      final firstTop = tester.getTopLeft(kpiFinder.at(0)).dy;
      final secondTop = tester.getTopLeft(kpiFinder.at(1)).dy;
      final thirdTop = tester.getTopLeft(kpiFinder.at(2)).dy;
      expect((firstTop - secondTop).abs(), lessThan(2.0));
      expect(thirdTop, greaterThan(firstTop + 40));
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets(
    'AuditPage, BackupPage, MaintenanceWindowsPage, ScheduledTasksPage, UsersPage, SettingsPage, and ServerConfigPage render with zero overflow at 390x844',
    (tester) async {
      final api = _buildMockApi();
      final container = _makeAdminContainer(api);
      addTearDown(container.dispose);

      for (final page in const <Widget>[
        AuditPage(),
        BackupPage(),
        MaintenanceWindowsPage(),
        ScheduledTasksPage(),
        UsersPage(),
        SettingsPage(),
        ServerConfigPage(),
      ]) {
        await _pumpMobile(tester, container, page);
        expect(
          tester.takeException(),
          isNull,
          reason: '${page.runtimeType} overflowed at 390x844',
        );
      }
    },
  );

  testWidgets(
    'Monitor and AlertRule form sheets stack multi-column fields cleanly at 360x800',
    (tester) async {
      final api = _buildMockApi();
      final container = _makeAdminContainer(api);
      addTearDown(container.dispose);

      await _pumpMobile(
        tester,
        container,
        Consumer(
          builder: (context, ref, _) => Column(
            children: <Widget>[
              FilledButton(
                onPressed: () =>
                    showMonitorFormSheet(context: context, ref: ref),
                child: const Text('Abrir Monitor'),
              ),
              FilledButton(
                onPressed: () =>
                    showAlertRuleFormSheet(context: context, ref: ref),
                child: const Text('Abrir Regra'),
              ),
            ],
          ),
        ),
        size: const Size(360, 800),
      );

      await tester.tap(find.text('Abrir Monitor'));
      await tester.pumpAndSettle();
      expect(find.text('Intervalo (s)'), findsOneWidget);
      expect(find.text('Timeout (s)'), findsOneWidget);
      expect(find.text('Tentativas'), findsOneWidget);
      expect(tester.takeException(), isNull);

      Navigator.of(tester.element(find.text('Novo monitor'))).pop();
      await tester.pumpAndSettle();

      await tester.tap(find.text('Abrir Regra'));
      await tester.pumpAndSettle();
      expect(find.text('Nova regra de alerta'), findsOneWidget);
      expect(find.text('Duração (s)'), findsOneWidget);
      expect(find.text('Cooldown (s)'), findsOneWidget);
      expect(tester.takeException(), isNull);
    },
  );
}
