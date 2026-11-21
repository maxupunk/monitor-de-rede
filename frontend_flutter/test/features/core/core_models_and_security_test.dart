import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/features/auth/login_page.dart';
import 'package:netmonitor_app/features/core/models/admin_models.dart';
import 'package:netmonitor_app/features/core/models/dashboard_models.dart';
import 'package:netmonitor_app/features/core/models/event_models.dart';
import 'package:netmonitor_app/features/core/models/monitor_models.dart';
import 'package:netmonitor_app/features/core/models/operations_models.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

void main() {
  group('Login open-redirect guard', () {
    test(
      'allows relative paths and blocks protocol-relative or external URLs',
      () {
        expect(sanitizeLoginRedirect('/monitors'), '/monitors');
        expect(sanitizeLoginRedirect('/alerts?tab=rules'), '/alerts?tab=rules');
        expect(sanitizeLoginRedirect('//evil.example/phish'), '/');
        expect(sanitizeLoginRedirect('https://evil.example'), '/');
        expect(sanitizeLoginRedirect('/login'), '/');
        expect(sanitizeLoginRedirect('/login?redirect=/'), '/');
        expect(sanitizeLoginRedirect(null), '/');
        expect(sanitizeLoginRedirect(''), '/');
      },
    );
  });

  group('MonitorModel & MonitorStats', () {
    test('computes uptime and latency stats accurately', () {
      const results = <MonitorResult>[
        MonitorResult(
          id: 1,
          monitorId: 10,
          status: 'up',
          startedAt: '2026-01-01T00:00:00Z',
          finishedAt: '2026-01-01T00:00:01Z',
          durationMs: 100,
          latencyMs: 10,
        ),
        MonitorResult(
          id: 2,
          monitorId: 10,
          status: 'down',
          startedAt: '2026-01-01T00:01:00Z',
          finishedAt: '2026-01-01T00:01:01Z',
          durationMs: 5000,
          latencyMs: null,
        ),
        MonitorResult(
          id: 3,
          monitorId: 10,
          status: 'up',
          startedAt: '2026-01-01T00:02:00Z',
          finishedAt: '2026-01-01T00:02:01Z',
          durationMs: 120,
          latencyMs: 30,
        ),
      ];

      final stats = MonitorStats.compute(results);
      expect(stats.totalChecks, 3);
      expect(stats.upChecks, 2);
      expect(stats.uptimePercentage, 66.7);
      expect(stats.avgLatency, 20.0);
      expect(stats.minLatency, 10.0);
      expect(stats.maxLatency, 30.0);
      expect(stats.lastLatency, 30.0);
    });

    test('identifies SNMP gauge and traffic monitors', () {
      final traffic = MonitorModel.fromJson(<String, dynamic>{
        'id': 1,
        'deviceId': 2,
        'name': 'WAN Traffic',
        'type': 'snmp',
        'target': '10.0.0.1',
        'status': 'online',
        'isEnabled': true,
        'configuration': <String, dynamic>{
          'mode': 'interface_traffic',
          'ifName': 'ether1',
        },
      });
      expect(traffic.isSnmpGauge, isTrue);
      expect(traffic.isTrafficMonitor, isTrue);
      expect(traffic.normalizedStatus, 'up');

      final disabled = traffic.copyWith(isEnabled: false);
      expect(disabled.normalizedStatus, 'disabled');
    });
  });

  group('MaintenanceWindow status calculation', () {
    test('classifies active, scheduled and ended windows locally', () {
      final now = DateTime(2026, 6, 15, 12, 0);
      final active = MaintenanceWindow(
        id: 1,
        name: 'Atualização Core',
        startsAt: DateTime(2026, 6, 15, 11, 0).toIso8601String(),
        endsAt: DateTime(2026, 6, 15, 13, 0).toIso8601String(),
        createdAt: '',
        updatedAt: '',
      );
      final scheduled = MaintenanceWindow(
        id: 2,
        name: 'Futura',
        startsAt: DateTime(2026, 6, 16, 10, 0).toIso8601String(),
        endsAt: DateTime(2026, 6, 16, 12, 0).toIso8601String(),
        createdAt: '',
        updatedAt: '',
      );
      final ended = MaintenanceWindow(
        id: 3,
        name: 'Passada',
        startsAt: DateTime(2026, 6, 14, 10, 0).toIso8601String(),
        endsAt: DateTime(2026, 6, 14, 12, 0).toIso8601String(),
        createdAt: '',
        updatedAt: '',
      );

      expect(active.statusAt(now), 'active');
      expect(scheduled.statusAt(now), 'scheduled');
      expect(ended.statusAt(now), 'ended');
    });
  });

  group('DashboardCatalog & Backup models', () {
    test('parses dashboard catalog and resolves widget sizes', () {
      final catalog = DashboardCatalog.fromJson(<String, dynamic>{
        'widgets': <Map<String, dynamic>>[
          <String, dynamic>{
            'type': 'stat_cards',
            'title': 'Resumo',
            'description': 'KPIs principais',
            'category': 'summary',
            'icon': 'mdi-view-dashboard',
            'allowMultiple': false,
            'defaultSize': 'full',
            'configKeys': <String>[],
            'compatibleResources': <String>[],
            'area': 'core',
          },
        ],
        'defaultLayout': <Map<String, dynamic>>[
          <String, dynamic>{
            'type': 'stat_cards',
            'size': 'full',
            'visible': true,
          },
        ],
      });

      expect(catalog.widgets.length, 1);
      expect(catalog.widgets.first.area, ProductArea.core);
      expect(catalog.widgets.first.defaultSize, WidgetSize.full);
      expect(WidgetSize.fromColumns(4, 6), WidgetSize.third);
      expect(WidgetSize.fromColumns(6, 6), WidgetSize.half);
      expect(WidgetSize.fromColumns(8, 12), WidgetSize.twoThirds);
      expect(WidgetSize.fromColumns(12, 12), WidgetSize.full);
    });

    test('parses BackupRestoreResponse with signInAgain flag', () {
      final res = BackupRestoreResponse.fromJson(<String, dynamic>{
        'counts': <String, dynamic>{
          'scope': 'full',
          'totalRows': 1250,
          'tables': <Map<String, dynamic>>[
            <String, dynamic>{'table': 'devices', 'rows': 42},
            <String, dynamic>{'table': 'monitors', 'rows': 80},
          ],
        },
        'signInAgain': true,
      });

      expect(res.signInAgain, isTrue);
      expect(res.counts.isFull, isTrue);
      expect(res.counts.totalRows, 1250);
      expect(res.counts.tables.length, 2);
    });

    test('describes SSE events with semantic tone', () {
      final p = EventPresentation.describe('alert:triggered', <String, dynamic>{
        'severity': 'critical',
        'title': 'Roteador Core offline',
      });
      expect(p.category, 'Alertas');
      expect(p.tone, SemanticTone.error);
      expect(p.summary, 'Roteador Core offline');
    });
  });
}
