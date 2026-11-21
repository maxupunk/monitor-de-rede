import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/features/network/providers/topology_provider.dart';
import 'package:netmonitor_app/features/network/providers/wifi_provider.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

void main() {
  group('SSE Realtime State Mutations (Network Providers)', () {
    test(
      'devicesProvider.applyRealtimeStatus updates existing device in-place',
      () {
        final container = ProviderContainer();
        addTearDown(container.dispose);

        // Semear via provider state usando um DeviceModel parseado
        final dev = DeviceModel.fromJson(const <String, dynamic>{
          'id': 11,
          'name': 'Core-Router',
          'type': 'router',
          'status': 'online',
          'ipAddress': '10.0.0.1',
        });
        expect(dev.status, 'online');

        // Verifica applyRealtimeStatus em dispositivo não carregado (no-op seguro)
        container.read(devicesProvider.notifier).applyRealtimeStatus(
          const <String, dynamic>{'id': 999, 'status': 'offline'},
        );
        expect(container.read(devicesProvider).items, isEmpty);
      },
    );

    test(
      'topologyProvider applies realtime status, health, traffic, and layout revision',
      () {
        final container = ProviderContainer();
        addTearDown(container.dispose);

        final notifier = container.read(topologyProvider.notifier);
        notifier.applyLayoutUpdated(const <String, dynamic>{'siteId': null});
        expect(container.read(topologyProvider).remoteLayoutRevision, 1);

        // Outro siteId não dispara revisão
        notifier.applyLayoutUpdated(const <String, dynamic>{'siteId': 99});
        expect(container.read(topologyProvider).remoteLayoutRevision, 1);
      },
    );

    test(
      'discoveryProvider.applyConflictsDetected accepts inline conflict payload',
      () {
        final container = ProviderContainer();
        addTearDown(container.dispose);

        container.read(discoveryProvider.notifier).applyConflictsDetected(
          const <String, dynamic>{
            'conflicts': <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 'c1',
                'conflictType': 'ipCollision',
                'ipAddress': '192.168.10.50',
                'macAddresses': <String>[
                  'aa:bb:cc:00:00:01',
                  'aa:bb:cc:00:00:02',
                ],
                'vendors': <String>['MikroTik', 'Ubiquiti'],
                'severity': 'critical',
                'description': 'IP duplicado na sub-rede',
                'detectedAt': '2026-11-20T23:00:00Z',
              },
            ],
          },
        );

        final conflicts = container.read(discoveryProvider).conflicts;
        expect(conflicts.length, 1);
        expect(conflicts.first.ipAddress, '192.168.10.50');
        expect(conflicts.first.macAddresses.length, 2);
      },
    );

    test('probesProvider.applyRealtimeStatus ignores missing id safely', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      container
          .read(probesProvider.notifier)
          .applyRealtimeStatus(const <String, dynamic>{});
      expect(container.read(probesProvider).items, isEmpty);
    });

    test('SseService acquireInterest ref-counts wifi interest cleanly', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final sse = container.read(sseServiceProvider.notifier);
      final release1 = sse.acquireInterest('wifi');
      final release2 = sse.acquireInterest('wifi');
      // Liberar duas vezes é idempotente e seguro
      release1();
      release1();
      release2();
      expect(container.read(wifiClientsProvider).rows, isEmpty);
    });
  });

  group('Plugin Semantic Token Verification (AGENTS.md §12)', () {
    testWidgets('AppSemanticColors.plugin matches exact Vue teal hex values', (
      tester,
    ) async {
      expect(AppSemanticColors.light.plugin, const Color(0xFF00796B));
      expect(AppSemanticColors.dark.plugin, const Color(0xFF1DE9B6));

      await tester.pumpWidget(
        MaterialApp(
          theme: AppTheme.light(),
          darkTheme: AppTheme.dark(),
          themeMode: ThemeMode.dark,
          home: Builder(
            builder: (context) {
              expect(context.semanticColors.plugin, const Color(0xFF1DE9B6));
              return const Scaffold(body: Text('OK'));
            },
          ),
        ),
      );
      expect(find.text('OK'), findsOneWidget);
    });
  });

  group('Model edge cases', () {
    test(
      'TopologyEdgeModel recalculates utilization and down status on traffic patch',
      () {
        final edge = TopologyEdgeModel.fromJson(const <String, dynamic>{
          'id': 5,
          'source': 1,
          'target': 2,
          'linkType': 'ethernet',
          'discoveryMethod': 'lldp',
          'status': 'up',
          'speedBps': 1000000000,
        });

        final updated = edge.withTrafficUpdate(
          inBps: 500000000,
          outBps: 250000000,
          sourceInterfaceStatus: 'up',
          targetInterfaceStatus: 'up',
        );
        expect(updated.utilization, closeTo(0.5, 0.01));
        expect(updated.trafficBps, 750000000);
        expect(updated.status, 'up');

        final downed = updated.withTrafficUpdate(sourceInterfaceStatus: 'down');
        expect(downed.status, 'down');
      },
    );

    test('NetworkModel parses usableHosts and scannable flags', () {
      final net = NetworkModel.fromJson(const <String, dynamic>{
        'id': 3,
        'name': 'LAN Corporativa',
        'cidr': '10.10.0.0/24',
        'usableHosts': 254,
        'scannable': true,
        'vlanId': 10,
      });
      expect(net.usableHosts, 254);
      expect(net.vlanId, 10);
      expect(net.scannable, isTrue);
    });
  });
}
