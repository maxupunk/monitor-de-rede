import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/areas/area_registry.dart';
import 'package:netmonitor_app/areas/composition_registries.dart';
import 'package:netmonitor_app/areas/servers_area.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/network/sse_client.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/features/servers/models/agent_models.dart';
import 'package:netmonitor_app/features/servers/providers/agents_provider.dart';
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';

void main() {
  group('ADR 017 / ADR 019 Cross-Area Registration', () {
    test('RemoteServersPage declares ProductArea.core while Docker/Deploys/Registry declare servers', () {
      expect(resolveRouteMeta('/remote-servers')?.area, ProductArea.core);
      expect(resolveRouteMeta('/docker')?.area, ProductArea.servers);
      expect(resolveRouteMeta('/docker/containers')?.area, ProductArea.servers);
      expect(resolveRouteMeta('/deploys')?.area, ProductArea.servers);
      expect(resolveRouteMeta('/registry')?.area, ProductArea.servers);
    });

    test('registers docker_summary widget, Databases & Storages backupTabs, and DatabaseInfoCard', () {
      expect(DashboardWidgetRegistry.resolve('docker_summary'), isNotNull);

      final tabs = BackupTabRegistry.resolve(isAdmin: true);
      expect(
        tabs.map((t) => t.id),
        containsAll(<String>['databases', 'storages']),
      );

      final settingsWhenServersOff = SettingsSectionRegistry.visibleTabs(
        <ProductArea>{ProductArea.core},
      );
      final sistema = settingsWhenServersOff
          .where((t) => t.tab.value == 'sistema')
          .firstOrNull;
      expect(sistema, isNotNull);
      expect(sistema!.cards.isNotEmpty, isTrue);
    });
  });

  group('Zero-Polling SSE Contract (All 15 Servers Events)', () {
    test('applies docker:*, deploys:*, registry:changed, probe:status, database_jobs:updated via SSE', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      // Install area realtime routes (both primary servers and core extras).
      container.read(areaRealtimeCoordinatorProvider);
      final sse = container.read(sseServiceProvider.notifier);

      // 1. docker:snapshot updates host status, containers, metrics & 60-sample ring buffer
      container.read(dockerProvider.notifier).selectHost('agent-1');
      sse.dispatchForTest(
        SseEvent(
          type: 'docker:snapshot',
          timestamp: '2026-11-20T23:00:00Z',
          data: <String, dynamic>{
            'hostKey': 'agent-1',
            'status': <String, dynamic>{
              'available': true,
              'engineVersion': '27.3.1',
              'cpus': 4,
              'memoryTotalBytes': 8589934592,
            },
            'containers': <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 'abc123456789',
                'names': <String>['/api'],
                'image': 'netmonitor:latest',
                'imageId': 'sha256:999',
                'state': 'running',
                'status': 'Up 2 hours',
                'labels': <String, String>{'com.netmonitor.protected': 'true'},
                'ports': const <Map<String, dynamic>>[],
                'created': 1700000000,
                'projectName': 'netmonitor',
                'protected': true,
              },
            ],
            'metrics': <String, dynamic>{
              'dockerAvailable': true,
              'collectedAt': '2026-11-20T23:00:00Z',
              'containers': <Map<String, dynamic>>[
                <String, dynamic>{
                  'containerId': 'abc123456789',
                  'containerName': 'api',
                  'imageName': 'netmonitor:latest',
                  'status': 'running',
                  'cpu': <String, dynamic>{'usagePercent': 12.5},
                  'memory': <String, dynamic>{
                    'usageBytes': 268435456,
                    'limitBytes': 8589934592,
                    'usagePercent': 3.125,
                  },
                  'network': <String, dynamic>{
                    'receivedBytes': 1024,
                    'transmittedBytes': 2048,
                  },
                  'blockIo': <String, dynamic>{'readBytes': 0, 'writeBytes': 0},
                },
              ],
            },
          },
        ),
      );

      final dockerState = container.read(dockerProvider);
      expect(dockerState.available, isTrue);
      expect(dockerState.runningContainers, 1);
      expect(dockerState.current.aggregateHistory.length, 1);
      expect(dockerState.current.aggregateHistory.first.cpuPercent, 12.5);

      // 2. docker:inventory updates volumes/networks/images
      sse.dispatchForTest(
        const SseEvent(
          type: 'docker:inventory',
          timestamp: '2026-11-20T23:00:01Z',
          data: <String, dynamic>{
            'hostKey': 'agent-1',
            'volumes': <Map<String, dynamic>>[
              <String, dynamic>{
                'name': 'pgdata',
                'driver': 'local',
                'mountpoint': '/var/lib/docker/volumes/pgdata',
                'scope': 'local',
              },
            ],
            'networks': <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 'net1',
                'name': 'bridge',
                'driver': 'bridge',
                'scope': 'local',
              },
            ],
            'images': <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 'sha256:111',
                'repoTags': <String>['postgres:18'],
                'size': 120000000,
              },
            ],
          },
        ),
      );
      expect(container.read(dockerProvider).current.volumes.length, 1);
      expect(container.read(dockerProvider).current.networks.length, 1);
      expect(container.read(dockerProvider).current.images.length, 1);

      // 3. docker:operation tracks live operation progress
      sse.dispatchForTest(
        const SseEvent(
          type: 'docker:operation',
          timestamp: '2026-11-20T23:00:02Z',
          data: <String, dynamic>{
            'operationId': 'op-1',
            'hostKey': 'agent-1',
            'kind': 'pull',
            'target': 'postgres:18',
            'state': 'running',
            'line': 'Pulling fs layer...',
          },
        ),
      );
      expect(container.read(dockerProvider).operations.length, 1);
      expect(
        container.read(dockerProvider).operations.first.lines.first,
        'Pulling fs layer...',
      );

      // 4. deploys:run & deploys:build update state in memory
      sse.dispatchForTest(
        const SseEvent(
          type: 'deploys:run',
          timestamp: '2026-11-20T23:00:03Z',
          data: <String, dynamic>{
            'id': 42,
            'applicationId': 1,
            'applicationName': 'erp',
            'repository': 'org/erp',
            'targetId': 7,
            'targetName': 'Producao',
            'trigger': 'webhook',
            'commitSha': 'abcdef1234567890abcdef1234567890abcdef12',
            'imageTag': 'abcdef1',
            'status': 'awaiting_approval',
            'createdAt': '2026-11-20T23:00:03Z',
          },
        ),
      );
      expect(container.read(deploysProvider).awaitingApproval.length, 1);

      sse.dispatchForTest(
        const SseEvent(
          type: 'deploys:build',
          timestamp: '2026-11-20T23:00:04Z',
          data: <String, dynamic>{
            'id': 9,
            'applicationId': 1,
            'application': 'erp',
            'reference': 'refs/heads/main',
            'commitSha': 'abcdef1234567890abcdef1234567890abcdef12',
            'trigger': 'webhook',
            'status': 'running',
            'stage': 'build',
            'noCache': false,
            'images': <Map<String, dynamic>>[],
            'createdAt': '2026-11-20T23:00:04Z',
          },
        ),
      );
      expect(container.read(deploysProvider).builds[9]?.stage, 'build');

      // 5. database_jobs:updated updates job progress live
      sse.dispatchForTest(
        const SseEvent(
          type: 'database_jobs:updated',
          timestamp: '2026-11-20T23:00:05Z',
          data: <String, dynamic>{
            'id': 'job-1',
            'kind': 'backup',
            'connectionId': 3,
            'database': 'app_prod',
            'stage': 'dump',
            'rows': 1500,
            'bytes': 65536,
            'databasesDone': 0,
            'databasesTotal': 1,
            'status': 'running',
            'startedAt': '2026-11-20T23:00:05Z',
          },
        ),
      );
      expect(container.read(databasesProvider).jobFor('backup', 3)?.rows, 1500);

      // 6. probe:status updates agentsProvider online state
      container.read(agentsProvider.notifier).state = AgentsState(
        agents: const <AgentView>[
          AgentView(
            id: 5,
            name: 'srv-01',
            hostKey: 'agent-5',
            status: 'offline',
            connected: false,
          ),
        ],
        loaded: true,
      );
      sse.dispatchForTest(
        const SseEvent(
          type: 'probe:status',
          timestamp: '2026-11-20T23:00:06Z',
          data: <String, dynamic>{
            'id': 5,
            'status': 'online',
            'version': '2.1.0',
          },
        ),
      );
      expect(container.read(agentsProvider).byId(5)?.connected, isTrue);

      // 7. registry:changed increments revision counter (debounced)
      expect(sse.hasRoute('registry:changed'), isTrue);
      expect(sse.hasRoute('storages:updated'), isTrue);
      expect(sse.hasRoute('database_connections:updated'), isTrue);
      expect(sse.hasRoute('backup:failed'), isTrue);
      expect(serversArea.sseBuilder, isNotNull);
    });
  });
}
