import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/areas/core_area.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/network/sse_client.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/features/core/models/monitor_models.dart';
import 'package:netmonitor_app/features/core/providers/alerts_provider.dart';
import 'package:netmonitor_app/features/core/providers/logs_provider.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/features/core/providers/scheduled_tasks_provider.dart';

void main() {
  group('Core AreaContribution contract', () {
    test(
      'registers all 11 core dashboard widgets, device tabs and shell overlay',
      () {
        expect(coreArea.id, ProductArea.core);
        expect(
          coreArea.widgets.keys,
          containsAll(<String>[
            'stat_cards',
            'health_gauge',
            'active_alerts',
            'latency_time_series',
            'event_distribution',
            'unstable_targets',
            'events_feed',
            'network_monitors',
            'cpu_usage',
            'ram_usage',
            'binary_status',
          ]),
        );
        expect(
          coreArea.deviceDetailTabs.map((t) => t.id),
          containsAll(<String>['monitors', 'rules', 'logs']),
        );
        expect(coreArea.shellOverlays, isNotEmpty);
        expect(
          coreArea.aiScreens.keys,
          containsAll(<String>['dashboard', 'monitors', 'alerts', 'logs']),
        );
      },
    );
  });

  group('Core SSE realtime mutations (zero polling)', () {
    test(
      'monitor:result updates monitor status and recentResults in memory',
      () {
        final container = ProviderContainer();
        addTearDown(container.dispose);

        final sse = container.read(sseServiceProvider.notifier);
        final unregister = sse.registerRoutes(
          coreArea.sseBuilder!(container.read(Provider<Ref>((ref) => ref))),
        );
        addTearDown(unregister);

        // Seed a monitor in state via realtime-friendly copy
        final notifier = container.read(monitorsProvider.notifier);
        notifier.state = notifier.state.copyWith(
          monitors: const <MonitorModel>[
            MonitorModel(
              id: 42,
              deviceId: 7,
              name: 'Gateway ICMP',
              type: 'ping',
              target: '192.168.1.1',
              intervalSeconds: 30,
              timeoutSeconds: 5,
              status: 'up',
              isEnabled: true,
            ),
          ],
        );

        sse.dispatchForTest(
          const SseEvent(
            type: 'monitor:result',
            timestamp: '2026-06-01T10:00:00Z',
            data: <String, dynamic>{
              'monitorId': 42,
              'status': 'down',
              'latencyMs': 0,
              'finishedAt': '2026-06-01T10:00:00Z',
              'message': 'Request timed out',
            },
          ),
        );

        final updated = container.read(monitorsProvider).monitors.first;
        expect(updated.normalizedStatus, 'down');
        expect(updated.recentResults.length, 1);
        expect(updated.recentResults.first.message, 'Request timed out');
      },
    );

    test('alert lifecycle, log templates and scheduled_task SSE events mutate stores', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final sse = container.read(sseServiceProvider.notifier);
      final unregister = sse.registerRoutes(
        coreArea.sseBuilder!(container.read(Provider<Ref>((ref) => ref))),
      );
      addTearDown(unregister);

      // Alert triggered -> acknowledged -> resolved
      sse.dispatchForTest(
        const SseEvent(
          type: 'alert:updated',
          timestamp: '2026-06-01T10:00:00Z',
          data: <String, dynamic>{
            'id': 99,
            'severity': 'critical',
            'status': 'active',
            'title': 'Link principal fora',
            'message': 'Sem resposta ICMP',
            'createdAt': '2026-06-01T10:00:00Z',
          },
        ),
      );
      expect(container.read(alertsProvider).activeAlerts.length, 1);

      sse.dispatchForTest(
        const SseEvent(
          type: 'alert:acknowledged',
          timestamp: '2026-06-01T10:01:00Z',
          data: <String, dynamic>{'id': 99},
        ),
      );
      expect(
        container.read(alertsProvider).alertEvents.first.status,
        'acknowledged',
      );

      sse.dispatchForTest(
        const SseEvent(
          type: 'alert:resolved',
          timestamp: '2026-06-01T10:02:00Z',
          data: <String, dynamic>{
            'id': 99,
            'resolvedAt': '2026-06-01T10:02:00Z',
          },
        ),
      );
      expect(container.read(alertsProvider).activeAlerts, isEmpty);
      expect(container.read(alertsProvider).resolvedAlerts.length, 1);

      // Log template classified
      sse.dispatchForTest(
        const SseEvent(
          type: 'logs:template_classified',
          timestamp: '2026-06-01T10:03:00Z',
          data: <String, dynamic>{
            'templateHash': 'abc123',
            'template': 'login failure for user *',
            'example': 'login failure for user admin',
            'category': 'auth_failure',
            'confirmed': true,
            'alertRegex': 'login failure',
          },
        ),
      );
      expect(
        container.read(logsProvider).templates['abc123']?.category,
        'auth_failure',
      );

      // Scheduled task dynamic ephemeral check
      sse.dispatchForTest(
        const SseEvent(
          type: 'scheduled_task:updated',
          timestamp: '2026-06-01T10:04:00Z',
          data: <String, dynamic>{'id': 5, 'status': 'success'},
        ),
      );
      expect(container.read(scheduledTasksProvider).lastEventTaskId, 5);
      // Success is ephemeral -> not pushed into recentEvents feed
      expect(
        container
            .read(sseServiceProvider)
            .recentEvents
            .where((e) => e.type == 'scheduled_task:updated'),
        isEmpty,
      );

      sse.dispatchForTest(
        const SseEvent(
          type: 'scheduled_task:updated',
          timestamp: '2026-06-01T10:05:00Z',
          data: <String, dynamic>{'id': 5, 'status': 'failed'},
        ),
      );
      expect(
        container
            .read(sseServiceProvider)
            .recentEvents
            .where((e) => e.type == 'scheduled_task:updated')
            .length,
        1,
      );
    });
  });
}
