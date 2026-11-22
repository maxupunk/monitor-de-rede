import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/pages/discovery_page.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class _AdminAuthNotifier extends AuthNotifier {
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

class _ViewerAuthNotifier extends AuthNotifier {
  @override
  AuthState build() => const AuthState(
    token: 'viewer-token',
    user: AuthUser(
      id: 2,
      fullName: 'Viewer',
      email: 'viewer@local',
      role: UserRole.viewer,
    ),
  );
}

class _StubNetworksNotifier extends NetworksNotifier {
  @override
  NetworksState build() => NetworksState(
    items: <NetworkModel>[
      NetworkModel.fromJson(const <String, dynamic>{
        'id': 1,
        'name': 'LAN Corporativa Matriz',
        'cidr': '10.10.0.0/24',
        'siteId': 1,
      }),
    ],
  );

  @override
  Future<void> fetchNetworks() async {}
}

class _RecordingDevicesNotifier extends DevicesNotifier {
  final List<Map<String, dynamic>> createdPayloads = <Map<String, dynamic>>[];

  @override
  DevicesState build() => DevicesState(
    items: <DeviceModel>[
      DeviceModel.fromJson(const <String, dynamic>{
        'id': 1,
        'name': 'AP OpenWrt Recepção',
        'type': 'access_point',
        'status': 'online',
        'ipAddress': '10.10.0.25',
        'macAddress': 'AA:BB:CC:DD:EE:02',
        'vendor': 'OpenWrt',
        'siteId': 1,
        'networkId': 1,
      }),
    ],
  );

  @override
  Future<void> fetchDevices() async {}

  @override
  Future<DeviceModel?> createDevice(Map<String, dynamic> payload) async {
    createdPayloads.add(payload);
    final created = DeviceModel.fromJson(<String, dynamic>{
      'id': 100 + createdPayloads.length,
      'name': payload['name'] ?? 'Novo',
      'type': payload['type'] ?? 'other',
      'status': 'online',
      'ipAddress': payload['ipAddress'],
      'macAddress': payload['macAddress'],
      'vendor': payload['vendor'],
      'siteId': payload['siteId'],
      'networkId': payload['networkId'],
    });
    state = state.copyWith(items: <DeviceModel>[...state.items, created]);
    return created;
  }
}

class _StubDiscoveryNotifier extends DiscoveryNotifier {
  @override
  DiscoveryState build() => const DiscoveryState(
    environment: DiscoveryEnvironmentModel(
      containerized: true,
      isHostMode: true,
    ),
    scan: ScanSessionState(
      runId: 12,
      networkId: 1,
      status: 'completed',
      phase: 'done',
      progressCurrent: 254,
      progressTotal: 254,
      hosts: <StreamedDiscoveryHost>[
        StreamedDiscoveryHost(
          ipAddress: '10.10.0.25',
          macAddress: 'AA:BB:CC:DD:EE:02',
          hostname: 'AP-OpenWrt-Recepcao',
          vendor: 'OpenWrt',
          deviceType: 'access_point',
          openPorts: <int>[22, 80, 443],
          confidence: 95,
          data: <String, dynamic>{'os': 'OpenWrt'},
        ),
        StreamedDiscoveryHost(
          ipAddress: '10.10.0.44',
          macAddress: '24:5E:BE:88:10:99',
          hostname: 'NAS-Backup-QNAP',
          vendor: 'QNAP Systems',
          deviceType: 'server',
          openPorts: <int>[22, 80, 443, 445, 8080, 8443, 9000],
          confidence: 90,
        ),
        StreamedDiscoveryHost(
          ipAddress: '10.10.0.99',
          macAddress: 'DC:2C:6E:11:22:99',
          deviceType: 'router',
          openPorts: <int>[22, 8291],
          confidence: 88,
          data: <String, dynamic>{'operatingSystem': 'RouterOS'},
          vendor: 'MikroTik',
        ),
      ],
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
  group('StreamedDiscoveryHost presentation getters', () {
    test('formats IP + MAC subtitle without repeating IP when unnamed', () {
      const named = StreamedDiscoveryHost(
        ipAddress: '10.10.0.25',
        hostname: 'AP-OpenWrt-Recepcao',
        macAddress: 'AA:BB:CC:DD:EE:02',
      );
      expect(named.bestName, 'AP-OpenWrt-Recepcao');
      expect(named.addressSubtitle, '10.10.0.25 · AA:BB:CC:DD:EE:02');

      const unnamedWithMac = StreamedDiscoveryHost(
        ipAddress: '10.10.0.99',
        macAddress: 'DC:2C:6E:11:22:99',
      );
      expect(unnamedWithMac.bestName, '10.10.0.99');
      expect(unnamedWithMac.addressSubtitle, 'DC:2C:6E:11:22:99');

      const unnamedNoMac = StreamedDiscoveryHost(ipAddress: '10.10.0.100');
      expect(unnamedNoMac.bestName, '10.10.0.100');
      expect(unnamedNoMac.addressSubtitle, isNull);
    });

    test('deduplicates system and vendor cleanly and summarizes ports', () {
      const sameVendorOs = StreamedDiscoveryHost(
        ipAddress: '10.10.0.25',
        vendor: 'OpenWrt',
        data: <String, dynamic>{'os': 'OpenWrt 23.05'},
        openPorts: <int>[22, 80, 443],
      );
      expect(sameVendorOs.systemOrVendorLabel, 'OpenWrt 23.05');
      expect(sameVendorOs.openPortsSummary, 'Portas: 22, 80, 443');

      const distinctVendorOs = StreamedDiscoveryHost(
        ipAddress: '10.10.0.1',
        vendor: 'MikroTik',
        data: <String, dynamic>{'operatingSystem': 'RouterOS'},
        openPorts: <int>[21, 22, 23, 80, 161, 443, 8291, 8728],
      );
      expect(distinctVendorOs.systemOrVendorLabel, 'MikroTik · RouterOS');
      expect(
        distinctVendorOs.openPortsSummary,
        'Portas: 21, 22, 23, 80, 161, 443 +2',
      );
    });
  });

  testWidgets(
    'DiscoveryPage renders 4-line host cards without Checkbox and aligns tonal icons',
    (tester) async {
      tester.view.physicalSize = const Size(390, 844);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(
        ProviderScope(
          overrides: <Override>[
            authProvider.overrideWith(_AdminAuthNotifier.new),
            networksProvider.overrideWith(_StubNetworksNotifier.new),
            devicesProvider.overrideWith(_RecordingDevicesNotifier.new),
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

      // Nenhum Checkbox ou CheckboxListTile deve existir nos cards da lista principal.
      expect(find.byType(Checkbox), findsNothing);
      expect(find.byType(CheckboxListTile), findsNothing);

      // Ícones tonais à esquerda alinhados verticalmente na mesma coluna (mesmo X)
      // tanto para host gerenciado quanto para hosts novos.
      final managedIconPos = tester.getTopLeft(
        find.byIcon(Icons.wifi_tethering),
      );
      final newServerIconPos = tester.getTopLeft(
        find.byIcon(Icons.dns_outlined),
      );
      expect(managedIconPos.dx, equals(newServerIconPos.dx));

      // Hierarquia estrita em 4 linhas empilhadas verticalmente (dy crescente):
      final line1 = tester.getTopLeft(find.text('AP-OpenWrt-Recepcao'));
      final line2 = tester.getTopLeft(
        find.text('10.10.0.25 · AA:BB:CC:DD:EE:02'),
      );
      final line3 = tester.getTopLeft(find.text('OpenWrt'));
      final line4 = tester.getTopLeft(find.text('Portas: 22, 80, 443'));

      expect(line2.dy, greaterThan(line1.dy));
      expect(line3.dy, greaterThan(line2.dy));
      expect(line4.dy, greaterThan(line3.dy));
      expect(line1.dx, equals(line2.dx));
      expect(line2.dx, equals(line3.dx));
      expect(line3.dx, equals(line4.dx));

      // Card de host novo também segue as 4 linhas com truncamento +N de portas e botão Adotar.
      expect(find.text('NAS-Backup-QNAP'), findsOneWidget);
      expect(find.text('10.10.0.44 · 24:5E:BE:88:10:99'), findsOneWidget);
      expect(find.text('QNAP Systems'), findsOneWidget);
      expect(
        find.text('Portas: 22, 80, 443, 445, 8080, 8443 +1'),
        findsOneWidget,
      );
      expect(find.text('Gerenciado'), findsOneWidget);
      expect(
        find.widgetWithText(FilledButton, 'Adotar'),
        findsAtLeastNWidgets(1),
      );
    },
  );

  testWidgets(
    'DiscoveryPage opens batch adoption sheet with CheckboxListTile, select all, clear and confirm',
    (tester) async {
      tester.view.physicalSize = const Size(390, 844);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      final devicesNotifier = _RecordingDevicesNotifier();

      await tester.pumpWidget(
        ProviderScope(
          overrides: <Override>[
            authProvider.overrideWith(_AdminAuthNotifier.new),
            networksProvider.overrideWith(_StubNetworksNotifier.new),
            devicesProvider.overrideWith(() => devicesNotifier),
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

      // 2 hosts novos -> botão compacto "Adotar em lote (2)" visível; nenhum Checkbox na lista.
      expect(find.text('Adotar em lote (2)'), findsOneWidget);
      expect(find.byType(Checkbox), findsNothing);

      await tester.tap(find.text('Adotar em lote (2)'));
      await tester.pumpAndSettle();

      // Dentro do modal há CheckboxListTile apenas para os 2 hosts novos.
      expect(find.byType(CheckboxListTile), findsNWidgets(2));
      expect(find.text('0 selecionados'), findsOneWidget);

      // Selecionar todos marca os 2 hosts novos.
      await tester.tap(find.text('Selecionar todos'));
      await tester.pumpAndSettle();
      expect(find.text('2 selecionados'), findsOneWidget);

      // Limpar desmarca todos.
      await tester.tap(find.text('Limpar'));
      await tester.pumpAndSettle();
      expect(find.text('0 selecionados'), findsOneWidget);

      // Seleciona ambos novamente e confirma adoção em lote.
      await tester.tap(find.text('Selecionar todos'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Adotar selecionados'));
      await tester.pumpAndSettle();

      // Modal fechado -> nenhum CheckboxListTile na tela; ambos criados no inventário.
      expect(find.byType(CheckboxListTile), findsNothing);
      expect(find.byType(Checkbox), findsNothing);
      expect(devicesNotifier.createdPayloads.length, 2);
      expect(
        devicesNotifier.createdPayloads.map((p) => p['ipAddress']),
        containsAll(<String>['10.10.0.44', '10.10.0.99']),
      );
      expect(find.text('2 hosts adotados no inventário.'), findsOneWidget);
      // Como agora todos os 3 são gerenciados, o botão de lote desaparece.
      expect(find.textContaining('Adotar em lote'), findsNothing);
    },
  );

  testWidgets(
    'DiscoveryPage shows Novo chip and hides adopt actions for viewer',
    (tester) async {
      tester.view.physicalSize = const Size(390, 844);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      addTearDown(tester.view.resetDevicePixelRatio);

      await tester.pumpWidget(
        ProviderScope(
          overrides: <Override>[
            authProvider.overrideWith(_ViewerAuthNotifier.new),
            networksProvider.overrideWith(_StubNetworksNotifier.new),
            devicesProvider.overrideWith(_RecordingDevicesNotifier.new),
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

      expect(find.byType(Checkbox), findsNothing);
      expect(find.textContaining('Adotar'), findsNothing);
      expect(find.text('Novo'), findsAtLeastNWidgets(1));
    },
  );
}
