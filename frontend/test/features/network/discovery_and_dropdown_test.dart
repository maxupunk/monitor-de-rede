import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/pages/discovery_page.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class _StubAuthNotifier extends AuthNotifier {
  @override
  AuthState build() => const AuthState(
    token: 'test-token',
    user: AuthUser(
      id: 1,
      fullName: 'Admin',
      email: 'admin@local',
      role: UserRole.admin,
    ),
  );
}

class _StubNetworksNotifier extends NetworksNotifier {
  @override
  NetworksState build() => NetworksState(
    items: <NetworkModel>[
      NetworkModel.fromJson(const <String, dynamic>{
        'id': 1,
        'name': 'LAN Principal',
        'cidr': '10.0.0.0/24',
      }),
    ],
  );

  @override
  Future<void> fetchNetworks() async {}
}

class _StubDevicesNotifier extends DevicesNotifier {
  @override
  DevicesState build() => const DevicesState();

  @override
  Future<void> fetchDevices() async {}
}

class _StubDiscoveryNotifier extends DiscoveryNotifier {
  @override
  DiscoveryState build() => DiscoveryState(
    environment: const DiscoveryEnvironmentModel(
      containerized: true,
      isHostMode: false,
    ),
    scan: ScanSessionState(
      networkId: 1,
      status: 'completed',
      phase: 'idle',
      progressCurrent: 254,
      progressTotal: 254,
      hosts: List<StreamedDiscoveryHost>.generate(
        25,
        (i) => StreamedDiscoveryHost(
          ipAddress: '10.0.0.${i + 1}',
          hostname: 'HeartOfGold-${i + 1}',
          macAddress: 'd6:8c:72:59:65:${i.toRadixString(16).padLeft(2, '0')}',
          vendor: 'Sinovoip (Banana Pi)',
          openPorts: const <int>[22, 53, 80, 443],
        ),
      ),
    ),
  );

  @override
  Future<void> fetchDiscoveryRuns() async {}

  @override
  Future<void> fetchEnvironment() async {}

  @override
  Future<void> fetchConflicts() async {}
}

void main() {
  testWidgets(
    'AppDropdownFormField truncates long labels inside narrow card without overflow',
    (tester) async {
      await tester.pumpWidget(
        MaterialApp(
          theme: AppTheme.dark(),
          home: Scaffold(
            body: Center(
              child: SizedBox(
                width: 150,
                child: Card(
                  child: Padding(
                    padding: const EdgeInsets.all(8),
                    child: AppDropdownFormField<int>(
                      initialValue: 1,
                      decoration: const InputDecoration(labelText: 'Rede IP'),
                      items: const <DropdownMenuItem<int>>[
                        DropdownMenuItem<int>(
                          value: 1,
                          child: Text(
                            'LAN Principal Corporativa Muito Longa (10.255.255.0/24)',
                          ),
                        ),
                      ],
                      onChanged: (_) {},
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.text('Rede IP'), findsOneWidget);
    },
  );

  testWidgets(
    'ResponsiveFieldRow stacks on compact mobile and splits into Row on wide screens',
    (tester) async {
      Widget buildForWidth(double width) => MaterialApp(
        theme: AppTheme.dark(),
        home: Scaffold(
          body: SizedBox(
            width: width,
            child: const ResponsiveFieldRow(
              children: <Widget>[
                SizedBox(key: ValueKey('a'), height: 40),
                SizedBox(key: ValueKey('b'), height: 40),
              ],
            ),
          ),
        ),
      );

      await tester.pumpWidget(buildForWidth(360));
      final aMobile = tester.getTopLeft(find.byKey(const ValueKey('a')));
      final bMobile = tester.getTopLeft(find.byKey(const ValueKey('b')));
      expect(bMobile.dy, greaterThan(aMobile.dy));
      expect(bMobile.dx, equals(aMobile.dx));

      await tester.pumpWidget(buildForWidth(600));
      final aWide = tester.getTopLeft(find.byKey(const ValueKey('a')));
      final bWide = tester.getTopLeft(find.byKey(const ValueKey('b')));
      expect(bWide.dy, equals(aWide.dy));
      expect(bWide.dx, greaterThan(aWide.dx));
    },
  );

  testWidgets(
    'DiscoveryPage renders compact mobile layout with collapsible header and zero overflows',
    (tester) async {
      tester.view.physicalSize = const Size(390, 844);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(
        ProviderScope(
          overrides: <Override>[
            authProvider.overrideWith(_StubAuthNotifier.new),
            networksProvider.overrideWith(_StubNetworksNotifier.new),
            devicesProvider.overrideWith(_StubDevicesNotifier.new),
            discoveryProvider.overrideWith(_StubDiscoveryNotifier.new),
          ],
          child: MaterialApp(
            theme: AppTheme.dark(),
            home: const Scaffold(
              body: SafeArea(
                child: Padding(
                  padding: EdgeInsets.all(12),
                  child: DiscoveryPage(),
                ),
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      expect(find.text('Descoberta de Rede'), findsOneWidget);
      expect(find.text('Resultados (25)'), findsOneWidget);
      expect(find.text('Varrer'), findsOneWidget);
      expect(find.text('HeartOfGold-1'), findsOneWidget);
      expect(find.text('HeartOfGold-2'), findsOneWidget);

      // Rola a lista para baixo: cabeçalho recolhe e libera área de lista.
      await tester.drag(find.byType(NestedScrollView), const Offset(0, -300));
      await tester.pumpAndSettle();

      expect(tester.takeException(), isNull);
      // TabBar permanece fixo no topo enquanto novos itens aparecem.
      expect(find.text('Resultados (25)'), findsOneWidget);
    },
  );
}
