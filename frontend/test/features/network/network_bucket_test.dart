import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/areas/composition_registries.dart';
import 'package:netmonitor_app/areas/network_area.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/navigation/nav_models.dart';
import 'package:netmonitor_app/core/network/sse_client.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/models/link_test_models.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/providers/link_tests_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/features/network/providers/topology_provider.dart';
import 'package:netmonitor_app/features/network/providers/wifi_provider.dart';
import 'package:netmonitor_app/features/network/widgets/wifi_qr_sheet.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

void main() {
  group('NetworkAreaContribution (ADR 017 / 019)', () {
    test(
      'declares exact routeMeta areas: core for /devices, network for network routes',
      () {
        expect(networkArea.routeMeta['/devices']?.area, ProductArea.core);
        expect(networkArea.routeMeta['/devices/:id']?.area, ProductArea.core);
        expect(networkArea.routeMeta['/topology']?.area, ProductArea.network);
        expect(networkArea.routeMeta['/topology']?.fullBleed, isTrue);
        expect(networkArea.routeMeta['/discovery']?.area, ProductArea.network);
        expect(networkArea.routeMeta['/networks']?.area, ProductArea.network);
        expect(networkArea.routeMeta['/sites']?.area, ProductArea.network);
        expect(networkArea.routeMeta['/link-tests']?.area, ProductArea.network);
        expect(networkArea.routeMeta['/wifi']?.area, ProductArea.network);
        expect(networkArea.routeMeta['/probes']?.area, ProductArea.network);
        expect(networkArea.routeMeta['/probes']?.aiScreen, isNull);
      },
    );

    test(
      'registers all 5 network dashboard widgets, 3 navActions, and 3 settings cards',
      () {
        expect(
          networkArea.widgets.keys,
          containsAll(<String>[
            'dns_latency',
            'saas_services',
            'saas_heatmap',
            'ether_bandwidth',
            'bandwidth_vs_latency',
          ]),
        );
        expect(
          networkArea.navActions.keys,
          containsAll(<NavAction>[
            NavAction.playbook,
            NavAction.traceroute,
            NavAction.speedTest,
          ]),
        );
        expect(networkArea.settingsCards.length, 3);
        expect(networkArea.settingsCards.first.tab, 'conectividade');
        expect(networkArea.settingsCards.first.area, ProductArea.core);
      },
    );
  });

  group('Network SSE Handlers (ZERO polling)', () {
    test(
      'discovery:scan updates scan session snapshot without HTTP refetch',
      () {
        final container = ProviderContainer();
        addTearDown(container.dispose);

        final sse = container.read(sseServiceProvider.notifier);
        final remove = sse.registerRoutes(
          networkArea.sseBuilder!(container.read(Provider<Ref>((ref) => ref))),
        );
        addTearDown(remove);

        sse.dispatchForTest(
          const SseEvent(
            type: 'discovery:scan',
            timestamp: '2026-11-20T23:00:00Z',
            data: <String, dynamic>{
              'runId': 42,
              'networkId': 7,
              'status': 'running',
              'phase': 'identify',
              'progressCurrent': 12,
              'progressTotal': 24,
              'hosts': <Map<String, dynamic>>[
                <String, dynamic>{
                  'ipAddress': '192.168.1.10',
                  'hostname': 'sw-core',
                  'confidence': 95,
                },
                <String, dynamic>{
                  'ipAddress': '192.168.1.2',
                  'hostname': 'gw-main',
                  'confidence': 99,
                },
              ],
            },
          ),
        );

        final state = container.read(discoveryProvider);
        expect(state.isScanning, isTrue);
        expect(state.scan.progressPercent, closeTo(50.0, 0.1));
        expect(state.scan.hosts.length, 2);
        // Ordenação numérica IPv4 (.2 antes de .10)
        expect(state.scan.hosts.first.ipAddress, '192.168.1.2');
      },
    );

    test(
      'wifi:snapshot, wifi:watched, and wifi:roamed mutate wifiClientsProvider live',
      () {
        final container = ProviderContainer();
        addTearDown(container.dispose);

        final notifier = container.read(wifiClientsProvider.notifier);
        notifier.applySnapshot(const <String, dynamic>{
          'deviceId': 10,
          'deviceName': 'AP-Sala',
          'status': 'ok',
          'canPing': true,
          'stations': <Map<String, dynamic>>[
            <String, dynamic>{
              'mac': 'aa:bb:cc:dd:ee:01',
              'signalDbm': -58,
              'signalGrade': 'excellent',
              'health': 'good',
              'linkScore': 0.9,
              'ssid': 'CorpWiFi',
              'band': '5GHz',
            },
          ],
        });
        notifier.applyWatched(const <String, dynamic>{
          'clients': <Map<String, dynamic>>[
            <String, dynamic>{
              'id': 1,
              'mac': 'aa:bb:cc:dd:ee:01',
              'name': 'Cam Entrada',
              'state': 'online',
              'deviceId': 10,
              'deviceName': 'AP-Sala',
            },
          ],
        });
        notifier.applyRoamed(const <String, dynamic>{
          'id': 99,
          'mac': 'aa:bb:cc:dd:ee:01',
          'fromDeviceName': 'AP-Corredor',
          'toDeviceName': 'AP-Sala',
          'signalBeforeDbm': -79,
          'signalAfterDbm': -58,
          'occurredAt': '2026-11-20T22:00:00Z',
          'pingPong': false,
        });

        final wifi = container.read(wifiClientsProvider);
        expect(wifi.rows.length, 1);
        expect(wifi.watched.length, 1);
        expect(wifi.roams.length, 1);
        expect(wifi.roams.first.toDeviceName, 'AP-Sala');
      },
    );

    test(
      'link_test:sample caps ring buffer at 120 and ignores out-of-order samples',
      () {
        final container = ProviderContainer();
        addTearDown(container.dispose);

        final notifier = container.read(linkTestsProvider.notifier);
        for (var i = 1; i <= 140; i++) {
          notifier.applySample(<String, dynamic>{
            'testId': 5,
            'legSeq': 0,
            'flow': 'forward',
            'bps': i * 1000000.0,
            'tMs': i * 1000,
          });
        }
        // Fora de ordem ignorado
        notifier.applySample(const <String, dynamic>{
          'testId': 5,
          'legSeq': 0,
          'flow': 'forward',
          'bps': 999.0,
          'tMs': 5000,
        });

        final samples = container
            .read(linkTestsProvider)
            .samplesFor(5, 0, 'forward');
        expect(samples.length, linkTestSampleLimit);
        expect(samples.last.tMs, 140000);
      },
    );
  });

  group('Pure Network Helpers & Parsers', () {
    test(
      'SnmpTestResult normalizes both direct test and autoDetect payloads',
      () {
        final direct = SnmpTestResult.fromJson(const <String, dynamic>{
          'success': true,
          'system': <String, dynamic>{
            'sysName': 'RB4011',
            'sysDescr': 'RouterOS',
          },
        });
        expect(direct.responded, isTrue);
        expect(direct.sysName, 'RB4011');

        final detect = SnmpTestResult.fromJson(const <String, dynamic>{
          'detected': true,
          'version': 'v2c',
          'community': 'public',
          'result': <String, dynamic>{
            'success': true,
            'system': <String, dynamic>{'sysName': 'Switch-01'},
          },
        });
        expect(detect.responded, isTrue);
        expect(detect.version, 'v2c');
        expect(detect.sysName, 'Switch-01');
      },
    );

    test(
      'buildWifiQrPayload escapes special characters and handles open networks',
      () {
        expect(
          buildWifiQrPayload(
            ssid: 'Corp;WiFi',
            password: r'p@ss:word\1',
            security: 'WPA',
          ),
          r'WIFI:T:WPA;S:Corp\;WiFi;P:p@ss\:word\\1;H:false;;',
        );
        expect(
          buildWifiQrPayload(ssid: 'Guest', password: '', security: 'WPA'),
          'WIFI:T:nopass;S:Guest;P:;H:false;;',
        );
      },
    );

    test(
      'computeTopologyLayout produces finite positions for all 4 layout modes',
      () {
        const nodes = <TopologyNodeModel>[
          TopologyNodeModel(
            id: 1,
            name: 'Core',
            type: 'router',
            status: 'online',
          ),
          TopologyNodeModel(
            id: 2,
            name: 'SW1',
            type: 'switch',
            status: 'online',
          ),
          TopologyNodeModel(
            id: 3,
            name: 'AP1',
            type: 'access_point',
            status: 'online',
          ),
        ];
        const edges = <TopologyEdgeModel>[
          TopologyEdgeModel(
            id: 10,
            sourceDeviceId: 1,
            targetDeviceId: 2,
            linkType: 'ethernet',
            discoveryMethod: 'lldp',
            status: 'up',
          ),
        ];

        for (final mode in TopologyLayoutMode.values) {
          final pos = computeTopologyLayout(nodes, edges, mode);
          expect(pos.length, 3);
          for (final p in pos.values) {
            expect(p.x.isFinite, isTrue);
            expect(p.y.isFinite, isTrue);
          }
        }
      },
    );

    test('compareIpAddresses sorts IPv4 numerically', () {
      final ips = <String>['10.0.0.10', '10.0.0.2', '10.0.0.100', '10.0.0.1']
        ..sort(compareIpAddresses);
      expect(ips, <String>['10.0.0.1', '10.0.0.2', '10.0.0.10', '10.0.0.100']);
    });

    test(
      'DeviceDetailTabRegistry resolves plugin tabs with SemanticTone.plugin',
      () {
        DeviceDetailTabRegistry.register(
          DeviceDetailTabContribution(
            id: 'plugin-test',
            title: 'Plugin Test',
            icon: Icons.extension,
            order: 99,
            area: ProductArea.network,
            tone: SemanticTone.plugin,
            builder: (_, _) => const SizedBox.shrink(),
          ),
        );
        final tabs = DeviceDetailTabRegistry.resolve(
          deviceId: 1,
          visibleAreas: const <ProductArea>{
            ProductArea.core,
            ProductArea.network,
          },
        );
        final found = tabs.where((t) => t.id == 'plugin-test').firstOrNull;
        expect(found, isNotNull);
        expect(found!.tone, SemanticTone.plugin);
      },
    );

    test('DeviceModel and LinkTestPlanModel parse cleanly', () {
      final dev = DeviceModel.fromJson(const <String, dynamic>{
        'id': 7,
        'name': 'MikroTik CCR',
        'type': 'router',
        'status': 'online',
        'ipAddress': '10.0.0.1',
        'snmpEnabled': true,
      });
      expect(dev.hasIp, isTrue);
      expect(dev.snmpEnabled, isTrue);

      final plan = LinkTestPlanModel.fromJson(const <String, dynamic>{
        'kind': 'link',
        'estimatedSeconds': 24,
        'legs': <Map<String, dynamic>>[
          <String, dynamic>{
            'seq': 0,
            'kind': 'link',
            'status': 'pending',
            'runnable': true,
            'durationS': 10,
            'a': <String, dynamic>{'kind': 'device', 'name': 'A'},
            'b': <String, dynamic>{'kind': 'device', 'name': 'B'},
          },
        ],
      });
      expect(plan.isReady, isTrue);
      expect(plan.estimatedSeconds, 24);
    });
  });
}
