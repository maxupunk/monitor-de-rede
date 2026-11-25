import 'dart:io';
import 'dart:ui' as ui;

import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/app/router.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/config/server_config.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/network/sse_client.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/core/providers/dashboard_provider.dart';
import 'package:netmonitor_app/features/core/providers/onboarding_provider.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart'
    hide AgentRouteOption;
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/models/link_test_models.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/models/topology_models.dart';
import 'package:netmonitor_app/features/network/models/wifi_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/diagnostics_provider.dart';
import 'package:netmonitor_app/features/network/providers/link_tests_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/features/network/providers/topology_provider.dart';
import 'package:netmonitor_app/features/network/providers/wifi_provider.dart';
import 'package:netmonitor_app/features/network/topology/widgets/topology_sheets.dart';
import 'package:netmonitor_app/features/network/widgets/device_form_sheet.dart';
import 'package:netmonitor_app/features/plugins/data/plugins_repository.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugin_apps_provider.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/features/servers/models/agent_models.dart'
    as srv_agent;
import 'package:netmonitor_app/features/servers/models/database_models.dart'
    as srv_db;
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/models/registry_models.dart';
import 'package:netmonitor_app/features/servers/models/storage_models.dart';
import 'package:netmonitor_app/features/servers/providers/agents_provider.dart';
import 'package:netmonitor_app/features/servers/providers/databases_provider.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/features/servers/providers/registry_provider.dart';
import 'package:netmonitor_app/features/servers/providers/storages_provider.dart';
import 'package:netmonitor_app/features/vpn/models/vpn_models.dart';
import 'package:netmonitor_app/features/vpn/providers/vpn_provider.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:shared_preferences/shared_preferences.dart';

Directory _resolvePrintsDir() {
  final cwd = Directory.current.path.replaceAll('\\', '/');
  if (cwd.endsWith('/frontend')) {
    return Directory('../logs/prints');
  }
  return Directory('logs/prints');
}

Future<void> _loadRealSdkFonts() async {
  const sdkFontsDir = r'C:\Apps\flutter\bin\cache\artifacts\material_fonts';
  final dir = Directory(sdkFontsDir);
  if (!dir.existsSync()) {
    throw StateError('Flutter SDK material_fonts not found at $sdkFontsDir');
  }

  Future<ByteData> readFont(String fileName) async {
    final bytes = await File('$sdkFontsDir\\$fileName').readAsBytes();
    return ByteData.sublistView(Uint8List.fromList(bytes));
  }

  final robotoRegular = await readFont('roboto-regular.ttf');
  final robotoMedium = await readFont('roboto-medium.ttf');
  final robotoBold = await readFont('roboto-bold.ttf');
  final robotoLight = await readFont('roboto-light.ttf');
  final robotoBlack = await readFont('roboto-black.ttf');
  final materialIcons = await readFont('materialicons-regular.otf');

  for (final family in const <String>[
    'Roboto',
    'FlutterTest',
    '.SF Pro Text',
  ]) {
    final loader = FontLoader(family)
      ..addFont(Future<ByteData>.value(robotoLight))
      ..addFont(Future<ByteData>.value(robotoRegular))
      ..addFont(Future<ByteData>.value(robotoMedium))
      ..addFont(Future<ByteData>.value(robotoBold))
      ..addFont(Future<ByteData>.value(robotoBlack));
    await loader.load();
  }

  final monoLoader = FontLoader('monospace')
    ..addFont(Future<ByteData>.value(robotoRegular))
    ..addFont(Future<ByteData>.value(robotoBold));
  await monoLoader.load();

  final iconsLoader = FontLoader('MaterialIcons')
    ..addFont(Future<ByteData>.value(materialIcons));
  await iconsLoader.load();
}

ThemeData _withRealRoboto(ThemeData base) {
  const buttonText = TextStyle(
    fontFamily: 'Roboto',
    fontWeight: FontWeight.w600,
  );
  return base.copyWith(
    textTheme: base.textTheme.apply(fontFamily: 'Roboto'),
    primaryTextTheme: base.primaryTextTheme.apply(fontFamily: 'Roboto'),
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(
        minimumSize: const Size(48, 44),
        padding: const EdgeInsets.symmetric(horizontal: 18, vertical: 12),
        shape: RoundedRectangleBorder(borderRadius: AppTheme.borderRadius),
        textStyle: buttonText,
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        minimumSize: const Size(48, 44),
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        side: BorderSide(color: base.colorScheme.outline),
        shape: RoundedRectangleBorder(borderRadius: AppTheme.borderRadius),
        textStyle: buttonText,
      ),
    ),
    textButtonTheme: TextButtonThemeData(
      style: TextButton.styleFrom(
        minimumSize: const Size(44, 44),
        shape: RoundedRectangleBorder(borderRadius: AppTheme.borderRadius),
        textStyle: buttonText,
      ),
    ),
    segmentedButtonTheme: SegmentedButtonThemeData(
      style: SegmentedButton.styleFrom(
        textStyle: const TextStyle(
          fontFamily: 'Roboto',
          fontWeight: FontWeight.w600,
          fontSize: 13,
        ),
      ),
    ),
    chipTheme: base.chipTheme.copyWith(
      labelStyle:
          (base.chipTheme.labelStyle ??
                  const TextStyle(fontWeight: FontWeight.w600))
              .copyWith(fontFamily: 'Roboto'),
    ),
    tabBarTheme: base.tabBarTheme.copyWith(
      labelStyle: const TextStyle(
        fontFamily: 'Roboto',
        fontWeight: FontWeight.w600,
        fontSize: 13,
      ),
      unselectedLabelStyle: const TextStyle(
        fontFamily: 'Roboto',
        fontWeight: FontWeight.w500,
        fontSize: 13,
      ),
    ),
  );
}

// ============================================================================
// Mock API Client & Seeded Riverpod Notifiers
// ============================================================================

ApiClient _buildSeededApiClient() {
  final dio = Dio()
    ..interceptors.add(
      InterceptorsWrapper(
        onRequest: (options, handler) {
          final path = options.path;
          Object data = <String, dynamic>{};

          if (path.endsWith('/monitors/1/stats')) {
            data = <String, dynamic>{
              'uptimePercent': 99.94,
              'avgLatencyMs': 2.8,
              'minLatencyMs': 1.9,
              'maxLatencyMs': 8.4,
              'totalChecks': 1440,
              'failedChecks': 1,
            };
          } else if (path.endsWith('/monitors/1/history')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 1,
                'status': 'up',
                'latencyMs': 2.4,
                'checkedAt': '2026-11-22T14:00:00Z',
              },
              <String, dynamic>{
                'id': 2,
                'status': 'up',
                'latencyMs': 3.1,
                'checkedAt': '2026-11-22T14:01:00Z',
              },
              <String, dynamic>{
                'id': 3,
                'status': 'up',
                'latencyMs': 2.7,
                'checkedAt': '2026-11-22T14:02:00Z',
              },
            ];
          } else if (path.endsWith('/monitors/1')) {
            data = <String, dynamic>{
              'id': 1,
              'name': 'Gateway Principal Fibra',
              'type': 'ping',
              'target': '10.10.0.1',
              'status': 'up',
              'isEnabled': true,
              'intervalSeconds': 30,
              'timeoutSeconds': 5,
              'lastLatencyMs': 2.4,
              'deviceId': 1,
              'deviceName': 'MikroTik CCR2004 Principal',
            };
          } else if (path.endsWith('/monitors')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 1,
                'name': 'Gateway Principal Fibra',
                'type': 'ping',
                'target': '10.10.0.1',
                'status': 'up',
                'isEnabled': true,
                'intervalSeconds': 30,
                'timeoutSeconds': 5,
                'lastLatencyMs': 2.4,
                'deviceId': 1,
                'deviceName': 'MikroTik CCR2004 Principal',
              },
              <String, dynamic>{
                'id': 2,
                'name': 'Link Rádio Filial Campinas',
                'type': 'ping',
                'target': '10.20.0.1',
                'status': 'down',
                'isEnabled': true,
                'intervalSeconds': 30,
                'timeoutSeconds': 5,
                'deviceId': 3,
                'deviceName': 'Switch Distribuição Campinas',
              },
              <String, dynamic>{
                'id': 3,
                'name': 'Portal HTTPS Interno',
                'type': 'http',
                'target': 'https://intranet.local/health',
                'status': 'up',
                'isEnabled': true,
                'intervalSeconds': 60,
                'timeoutSeconds': 10,
                'lastLatencyMs': 18.6,
                'deviceId': 1,
                'deviceName': 'MikroTik CCR2004 Principal',
              },
            ];
          } else if (path.endsWith('/alerts/rules')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 1,
                'name': 'Queda de Link Crítico',
                'type': 'device_offline',
                'severity': 'critical',
                'isEnabled': true,
                'durationSeconds': 60,
                'cooldownSeconds': 300,
                'condition': <String, dynamic>{
                  'field': 'status',
                  'operator': '==',
                  'value': 'offline',
                },
              },
              <String, dynamic>{
                'id': 2,
                'name': 'Latência Alta no Core (> 80ms)',
                'type': 'latency_high',
                'severity': 'warning',
                'isEnabled': true,
                'durationSeconds': 120,
                'cooldownSeconds': 600,
                'condition': <String, dynamic>{
                  'field': 'latency_ms',
                  'operator': '>',
                  'value': 80,
                },
              },
            ];
          } else if (path.endsWith('/alerts')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 10,
                'title': 'Link Rádio Filial fora do ar',
                'message':
                    'Sem resposta ICMP em 10.20.0.1 após 3 tentativas consecutivas.',
                'severity': 'critical',
                'status': 'active',
                'deviceId': 3,
                'deviceName': 'Switch Distribuição Campinas',
                'startedAt': '2026-11-22T13:45:00Z',
              },
              <String, dynamic>{
                'id': 11,
                'title': 'Sinal Wi-Fi degradado na Recepção',
                'message':
                    '2 clientes abaixo de -78 dBm no AP OpenWrt Recepção.',
                'severity': 'warning',
                'status': 'acknowledged',
                'deviceId': 2,
                'deviceName': 'AP OpenWrt Recepção',
                'startedAt': '2026-11-22T13:10:00Z',
              },
            ];
          } else if (path.endsWith('/events')) {
            data = <String, dynamic>{
              'data': <Map<String, dynamic>>[
                <String, dynamic>{
                  'id': 501,
                  'eventType': 'alert_triggered',
                  'severity': 'critical',
                  'message': 'Link Rádio Filial entrou em estado OFFLINE.',
                  'deviceName': 'Switch Distribuição Campinas',
                  'createdAt': '2026-11-22T13:45:00Z',
                },
                <String, dynamic>{
                  'id': 502,
                  'eventType': 'interface_up',
                  'severity': 'info',
                  'message': 'Interface sfp-sfpplus1 negociou 10Gbps Full.',
                  'deviceName': 'MikroTik CCR2004 Principal',
                  'createdAt': '2026-11-22T13:30:00Z',
                },
              ],
              'meta': <String, dynamic>{
                'total': 2,
                'currentPage': 1,
                'lastPage': 1,
                'perPage': 25,
              },
            };
          } else if (path.endsWith('/logs/sources')) {
            data = <String, dynamic>{
              'data': <Map<String, dynamic>>[
                <String, dynamic>{
                  'sourceIp': '10.10.0.1',
                  'deviceId': 1,
                  'deviceName': 'MikroTik CCR2004 Principal',
                  'count': 128,
                },
                <String, dynamic>{
                  'sourceIp': '10.10.0.25',
                  'deviceId': 2,
                  'deviceName': 'AP OpenWrt Recepção',
                  'count': 42,
                },
              ],
              'unknownCount': 1,
            };
          } else if (path.endsWith('/logs')) {
            data = <String, dynamic>{
              'data': <Map<String, dynamic>>[
                <String, dynamic>{
                  'id': 101,
                  'deviceId': 2,
                  'deviceName': 'AP OpenWrt Recepção',
                  'sourceIp': '10.10.0.25',
                  'receivedAt': '2026-11-22T14:00:00Z',
                  'severity': 4,
                  'severityLabel': 'warning',
                  'appName': 'hostapd',
                  'message':
                      'wlan1: STA aa:bb:cc:dd:ee:22 IEEE 802.11: disassociated due to inactivity',
                },
                <String, dynamic>{
                  'id': 102,
                  'deviceId': 1,
                  'deviceName': 'MikroTik CCR2004 Principal',
                  'sourceIp': '10.10.0.1',
                  'receivedAt': '2026-11-22T13:59:12Z',
                  'severity': 6,
                  'severityLabel': 'info',
                  'appName': 'dhcpd',
                  'message':
                      'DHCPACK on 10.10.0.105 to aa:bb:cc:dd:ee:10 (Notebook-Diretoria)',
                },
                <String, dynamic>{
                  'id': 103,
                  'deviceId': 3,
                  'deviceName': 'Switch Distribuição Campinas',
                  'sourceIp': '10.20.0.1',
                  'receivedAt': '2026-11-22T13:45:02Z',
                  'severity': 3,
                  'severityLabel': 'err',
                  'appName': 'ospfd',
                  'message':
                      'OSPF neighbor 10.10.0.1 state changed from Full to Down',
                },
              ],
              'templates': const <Object>[],
              'hasMore': false,
            };
          } else if (path.endsWith('/maintenance-windows')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 1,
                'name': 'Atualização Firmware Core Noturna',
                'description':
                    'Janela mensal de manutenção preventiva nos switches.',
                'startsAt': '2026-11-23T02:00:00Z',
                'endsAt': '2026-11-23T04:00:00Z',
                'status': 'scheduled',
                'suppressNotifications': true,
              },
            ];
          } else if (path.endsWith('/scheduled-tasks/meta')) {
            data = <String, dynamic>{
              'taskTypes': <Map<String, dynamic>>[
                <String, dynamic>{
                  'type': 'network_discovery',
                  'label': 'Descoberta de Rede',
                  'description': 'Varre sub-redes IP agendadas.',
                },
              ],
            };
          } else if (path.endsWith('/devices')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 1,
                'name': 'MikroTik CCR2004 Principal',
                'ipAddress': '10.10.0.1',
                'status': 'online',
              },
              <String, dynamic>{
                'id': 2,
                'name': 'AP OpenWrt Recepção',
                'ipAddress': '10.10.0.25',
                'status': 'online',
              },
              <String, dynamic>{
                'id': 3,
                'name': 'Switch Distribuição Campinas',
                'ipAddress': '10.20.0.1',
                'status': 'offline',
              },
            ];
          } else if (path.endsWith('/scheduled-tasks')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 1,
                'name': 'Varredura Automática LAN Matriz',
                'tool': 'Descoberta de Rede',
                'cron': '0 */6 * * *',
                'scheduleLabel': 'A cada 6 horas',
                'enabled': true,
                'lastStatus': 'succeeded',
                'lastRunAt': '2026-11-22T12:00:00Z',
                'nextRunAt': '2026-11-22T18:00:00Z',
                'lastMessage': '18 hosts ativos verificados sem conflitos.',
              },
            ];
          } else if (path.endsWith('/users')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 1,
                'fullName': 'Administrador Principal',
                'email': 'admin@netmonitor.local',
                'role': 'admin',
                'active': true,
              },
              <String, dynamic>{
                'id': 2,
                'fullName': 'Operador NOC',
                'email': 'noc@netmonitor.local',
                'role': 'operator',
                'active': true,
              },
            ];
          } else if (path.endsWith('/audit-logs')) {
            data = <String, dynamic>{
              'data': <Map<String, dynamic>>[
                <String, dynamic>{
                  'id': 1,
                  'action': 'update',
                  'resourceType': 'device',
                  'resourceLabel': 'AP OpenWrt Recepção',
                  'userEmail': 'admin@netmonitor.local',
                  'ipAddress': '10.10.0.50',
                  'createdAt': '2026-11-22T13:55:00Z',
                },
                <String, dynamic>{
                  'id': 2,
                  'action': 'create',
                  'resourceType': 'topology_link',
                  'resourceLabel': 'Core-CCR - AP OpenWrt Recepção',
                  'userEmail': 'admin@netmonitor.local',
                  'ipAddress': '10.10.0.50',
                  'createdAt': '2026-11-22T13:40:00Z',
                },
              ],
              'meta': <String, dynamic>{
                'total': 2,
                'currentPage': 1,
                'lastPage': 1,
                'perPage': 25,
              },
            };
          } else if (path.endsWith('/backup/system')) {
            data = <String, dynamic>{
              'storageDestinationId': 1,
              'storageDestinationName': 'Cofre S3 Produção',
              'backupEnabled': true,
              'backupIntervalHours': 12,
              'backupRetention': 14,
              'lastBackupAt': '2026-11-22T06:00:00Z',
              'lastBackupStatus': 'succeeded',
              'nextBackupAt': '2026-11-22T18:00:00Z',
            };
          } else if (path.endsWith('/storages')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 1,
                'name': 'Cofre S3 Produção',
                'provider': 's3',
                'target': 's3://netmonitor-backups/prod',
              },
            ];
          } else if (path.endsWith('/settings/database-size')) {
            data = <String, dynamic>{
              'engine': 'postgresql',
              'sizeBytes': 184 * 1024 * 1024,
              'logsSizeBytes': 92 * 1024 * 1024,
            };
          } else if (path.endsWith('/settings/onboarding')) {
            data = <String, dynamic>{
              'needsOnboarding': false,
              'completed': true,
              'intentChosen': true,
              'devicesCount': 3,
              'networksCount': 2,
            };
          } else if (path.endsWith('/settings')) {
            data = <String, dynamic>{
              'systemName': 'NetMonitor Matriz',
              'timezone': 'America/Sao_Paulo',
              'metricsRetentionDays': 30,
              'logsRetentionDays': 14,
              'eventsRetentionDays': 30,
            };
          } else if (path.endsWith('/server-addresses')) {
            data = <String, dynamic>{
              'data': <Map<String, dynamic>>[
                <String, dynamic>{
                  'id': 'lan-1',
                  'label': 'LAN Corporativa',
                  'value': '10.10.0.10',
                  'kind': 'lan',
                },
                <String, dynamic>{
                  'id': 'vpn-1',
                  'label': 'Túnel WireGuard',
                  'value': '10.8.0.1',
                  'kind': 'vpn',
                },
              ],
              'preferredId': 'lan-1',
              'domainHttps': false,
            };
          } else if (path.endsWith('/snmp/profiles') ||
              path.endsWith('/snmp-profiles')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 1,
                'name': 'Comunidade Padrão v2c',
                'version': '2c',
                'port': 161,
              },
            ];
          } else if (path.endsWith('/operating-systems')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 1,
                'name': 'OpenWrt',
                'slug': 'openwrt',
                'vendor': 'OpenWrt',
              },
              <String, dynamic>{
                'id': 2,
                'name': 'RouterOS',
                'slug': 'routeros',
                'vendor': 'MikroTik',
              },
            ];
          } else if (path.endsWith('/notification-channels')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'id': 1,
                'name': 'Telegram Plantão NOC',
                'driver': 'telegram',
                'isEnabled': true,
              },
            ];
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
                <String, dynamic>{
                  'type': 'health_gauge',
                  'title': 'Saúde geral',
                  'description': 'Disponibilidade global',
                  'area': 'core',
                  'defaultEnabled': true,
                  'md': 6,
                  'lg': 6,
                },
                <String, dynamic>{
                  'type': 'active_alerts',
                  'title': 'Alertas ativos',
                  'description': 'Incidentes em aberto',
                  'area': 'core',
                  'defaultEnabled': true,
                  'md': 6,
                  'lg': 6,
                },
                <String, dynamic>{
                  'type': 'events_feed',
                  'title': 'Feed de eventos ao vivo',
                  'description': 'Últimos eventos SSE',
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
                <String, dynamic>{
                  'type': 'health_gauge',
                  'size': 'half',
                  'visible': true,
                },
                <String, dynamic>{
                  'type': 'active_alerts',
                  'size': 'half',
                  'visible': true,
                },
                <String, dynamic>{
                  'type': 'events_feed',
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
                  'order': 0,
                  'md': 12,
                  'lg': 12,
                },
                <String, dynamic>{
                  'id': 'health_gauge',
                  'type': 'health_gauge',
                  'title': 'Saúde geral',
                  'visible': true,
                  'order': 1,
                  'md': 6,
                  'lg': 6,
                },
                <String, dynamic>{
                  'id': 'active_alerts',
                  'type': 'active_alerts',
                  'title': 'Alertas ativos',
                  'visible': true,
                  'order': 2,
                  'md': 6,
                  'lg': 6,
                },
                <String, dynamic>{
                  'id': 'events_feed',
                  'type': 'events_feed',
                  'title': 'Feed de eventos ao vivo',
                  'visible': true,
                  'order': 3,
                  'md': 12,
                  'lg': 12,
                },
              ],
            };
          } else if (path.contains('/vpn/metrics')) {
            data = <Map<String, dynamic>>[
              <String, dynamic>{
                'metricName': 'vpn_rx_bps',
                'value': 1250000,
                'recordedAt': '2026-11-22T13:55:00Z',
              },
              <String, dynamic>{
                'metricName': 'vpn_rx_bps',
                'value': 1840000,
                'recordedAt': '2026-11-22T14:00:00Z',
              },
              <String, dynamic>{
                'metricName': 'vpn_tx_bps',
                'value': 620000,
                'recordedAt': '2026-11-22T13:55:00Z',
              },
              <String, dynamic>{
                'metricName': 'vpn_tx_bps',
                'value': 910000,
                'recordedAt': '2026-11-22T14:00:00Z',
              },
            ];
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
    baseUrlProvider: () => 'http://10.10.0.10:3333/api',
    tokenProvider: () => 'visual-test-token',
    onUnauthorized: () {},
    dio: dio,
  );
}

class _StubSseService extends SseService {
  @override
  RealtimeState build() => const RealtimeState(
    connectionState: SseConnectionState.connected,
    recentEvents: <SseEvent>[
      SseEvent(
        type: 'alert:triggered',
        timestamp: '2026-11-22T13:45:00Z',
        data: <String, dynamic>{
          'id': 10,
          'severity': 'critical',
          'title': 'Link Rádio Filial fora do ar',
          'deviceName': 'Switch Distribuição Campinas',
        },
      ),
      SseEvent(
        type: 'monitor:checked',
        timestamp: '2026-11-22T14:00:00Z',
        data: <String, dynamic>{
          'id': 1,
          'status': 'up',
          'title': 'Gateway Principal Fibra respondeu em 2,4 ms',
        },
      ),
    ],
  );

  @override
  void connect() {}

  @override
  void disconnect({bool clearFeed = false}) {}

  @override
  VoidCallback acquireInterest(String topic) => () {};
}

class _ConfiguredServerNotifier extends ServerConfigNotifier {
  @override
  ServerConfigState build() => const ServerConfigState(
    baseUrl: 'http://10.10.0.10:3333',
    recentServers: <String>[
      'http://10.10.0.10:3333',
      'https://netmonitor.matriz.com.br',
    ],
    initialized: true,
  );
}

class _SeededAuthNotifier extends AuthNotifier {
  _SeededAuthNotifier({this.authenticated = true, this.setupRequired = false});

  final bool authenticated;
  final bool setupRequired;

  @override
  AuthState build() {
    if (!authenticated) {
      return AuthState(initialized: true, needsSetup: setupRequired);
    }
    return const AuthState(
      token: 'visual-test-token',
      initialized: true,
      needsSetup: false,
      user: AuthUser(
        id: 1,
        email: 'admin@netmonitor.local',
        fullName: 'Administrador',
        role: UserRole.admin,
      ),
    );
  }

  @override
  Future<bool> ensureSetupStatus() async => setupRequired;

  @override
  Future<void> fetchMe() async {}
}

class _SeededAreasNotifier extends AreasNotifier {
  _SeededAreasNotifier({this.vpnEnabled = true});

  final bool vpnEnabled;

  @override
  AreasState build() {
    if (vpnEnabled) return const AreasState();
    return AreasState(
      snapshot: AreasSnapshot.tryFromJson(const <String, dynamic>{
        'areas': <Map<String, dynamic>>[
          <String, dynamic>{
            'area': 'core',
            'title': 'Núcleo',
            'summary': 'Base do monitoramento',
            'icon': '',
            'enabled': true,
            'optional': false,
          },
          <String, dynamic>{
            'area': 'network',
            'title': 'Rede e Dispositivos',
            'summary': 'Topologia e inventário',
            'icon': '',
            'enabled': true,
            'optional': true,
          },
          <String, dynamic>{
            'area': 'servers',
            'title': 'Servidores e Aplicações',
            'summary': 'Docker e deploys',
            'icon': '',
            'enabled': true,
            'optional': true,
          },
          <String, dynamic>{
            'area': 'vpn',
            'title': 'VPN WireGuard',
            'summary': 'Acesso remoto seguro por túnel WireGuard',
            'icon': '',
            'enabled': false,
            'optional': true,
          },
        ],
      }),
    );
  }
}

class _SeededOnboardingNotifier extends OnboardingNotifier {
  _SeededOnboardingNotifier({this.openByDefault = false});

  final bool openByDefault;

  @override
  OnboardingState build() => OnboardingState(
    checked: true,
    dismissedInSession: !openByDefault,
    showWizard: openByDefault,
  );

  @override
  Future<bool> checkAndOpenIfNeeded() async => openByDefault;
}

class _SeededSitesNotifier extends SitesNotifier {
  @override
  SitesState build() => SitesState(
    items: <SiteModel>[
      SiteModel.fromJson(const <String, dynamic>{
        'id': 1,
        'name': 'Matriz São Paulo',
        'location': 'Av. Paulista, 1000 · Bela Vista',
        'devicesCount': 14,
        'networksCount': 3,
      }),
      SiteModel.fromJson(const <String, dynamic>{
        'id': 2,
        'name': 'Filial Campinas',
        'location': 'Distrito Industrial · Campinas - SP',
        'devicesCount': 6,
        'networksCount': 2,
      }),
    ],
  );

  @override
  Future<void> fetchSites() async {}
}

class _SeededNetworksNotifier extends NetworksNotifier {
  @override
  NetworksState build() => NetworksState(
    items: <NetworkModel>[
      NetworkModel.fromJson(const <String, dynamic>{
        'id': 1,
        'name': 'LAN Corporativa Matriz',
        'cidr': '10.10.0.0/24',
        'gateway': '10.10.0.1',
        'vlanId': 10,
        'usableHosts': 254,
        'scannable': true,
        'scanEnabled': true,
        'siteId': 1,
      }),
      NetworkModel.fromJson(const <String, dynamic>{
        'id': 2,
        'name': 'Wi-Fi Visitantes',
        'cidr': '172.16.50.0/24',
        'gateway': '172.16.50.1',
        'vlanId': 50,
        'usableHosts': 254,
        'scannable': true,
        'scanEnabled': false,
        'siteId': 1,
      }),
    ],
  );

  @override
  Future<void> fetchNetworks() async {}
}

class _SeededProbesNotifier extends ProbesNotifier {
  @override
  ProbesState build() => ProbesState(
    items: <ProbeModel>[
      ProbeModel.fromJson(const <String, dynamic>{
        'id': 1,
        'name': 'vpn-probe-central',
        'status': 'online',
        'ipAddress': '10.8.0.2',
        'probeType': 'vpn-probe',
        'version': '2.4.0',
        'lastHeartbeatAt': '2026-11-22T14:00:00Z',
      }),
      ProbeModel.fromJson(const <String, dynamic>{
        'id': 2,
        'name': 'probe-campinas',
        'status': 'online',
        'ipAddress': '10.20.0.5',
        'probeType': 'remote',
        'version': '2.4.0',
        'lastHeartbeatAt': '2026-11-22T13:59:50Z',
      }),
    ],
  );

  @override
  Future<void> fetchProbes() async {}
}

class _SeededDiscoveryNotifier extends DiscoveryNotifier {
  @override
  DiscoveryState build() => DiscoveryState(
    environment: const DiscoveryEnvironmentModel(
      isHostMode: true,
      containerized: true,
    ),
    runs: <DiscoveryRunModel>[
      DiscoveryRunModel.fromJson(const <String, dynamic>{
        'id': 12,
        'networkId': 1,
        'networkName': 'LAN Corporativa Matriz',
        'cidr': '10.10.0.0/24',
        'status': 'completed',
        'devicesFound': 4,
        'startedAt': '2026-11-22T13:50:00Z',
        'finishedAt': '2026-11-22T13:51:10Z',
      }),
    ],
    scan: const ScanSessionState(
      runId: 12,
      networkId: 1,
      status: 'completed',
      phase: 'done',
      progressCurrent: 254,
      progressTotal: 254,
      hosts: <StreamedDiscoveryHost>[
        StreamedDiscoveryHost(
          ipAddress: '10.10.0.1',
          macAddress: 'DC:2C:6E:11:22:01',
          hostname: 'MikroTik-CCR2004',
          vendor: 'MikroTik',
          deviceType: 'router',
          openPorts: <int>[22, 80, 161, 8291],
          confidence: 98,
        ),
        StreamedDiscoveryHost(
          ipAddress: '10.10.0.25',
          macAddress: 'AA:BB:CC:DD:EE:02',
          hostname: 'AP-OpenWrt-Recepcao',
          vendor: 'OpenWrt',
          deviceType: 'access_point',
          openPorts: <int>[22, 80, 443],
          confidence: 95,
        ),
        StreamedDiscoveryHost(
          ipAddress: '10.10.0.44',
          macAddress: '24:5E:BE:88:10:99',
          hostname: 'NAS-Backup-QNAP',
          vendor: 'QNAP Systems',
          deviceType: 'server',
          openPorts: <int>[22, 445, 8080],
          confidence: 90,
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

class _SeededDevicesNotifier extends DevicesNotifier {
  @override
  DevicesState build() => DevicesState(
    items: <DeviceModel>[
      DeviceModel.fromJson(const <String, dynamic>{
        'id': 1,
        'name': 'MikroTik CCR2004 Principal',
        'type': 'router',
        'status': 'online',
        'ipAddress': '10.10.0.1',
        'macAddress': 'DC:2C:6E:11:22:01',
        'vendor': 'MikroTik',
        'model': 'CCR2004-16G-2S+',
        'snmpEnabled': true,
        'siteId': 1,
        'networkId': 1,
      }),
      DeviceModel.fromJson(const <String, dynamic>{
        'id': 2,
        'name': 'AP OpenWrt Recepção',
        'type': 'access_point',
        'status': 'online',
        'ipAddress': '10.10.0.25',
        'macAddress': 'AA:BB:CC:DD:EE:02',
        'vendor': 'OpenWrt',
        'model': 'AX3600',
        'osName': 'OpenWrt',
        'snmpEnabled': true,
        'siteId': 1,
        'networkId': 1,
      }),
      DeviceModel.fromJson(const <String, dynamic>{
        'id': 3,
        'name': 'Switch Distribuição Campinas',
        'type': 'switch',
        'status': 'offline',
        'ipAddress': '10.20.0.1',
        'macAddress': '74:4D:28:99:10:03',
        'vendor': 'Ubiquiti',
        'model': 'USW-Pro-24-PoE',
        'snmpEnabled': true,
        'siteId': 2,
        'networkId': 1,
      }),
    ],
  );

  @override
  Future<void> fetchDevices({int? siteId, int? networkId}) async {}
}

class _SeededDeviceDetailNotifier extends DeviceDetailNotifier {
  @override
  DeviceDetailState build() => DeviceDetailState(
    device: DeviceModel.fromJson(const <String, dynamic>{
      'id': 2,
      'name': 'AP OpenWrt Recepção',
      'type': 'access_point',
      'status': 'online',
      'ipAddress': '10.10.0.25',
      'macAddress': 'AA:BB:CC:DD:EE:02',
      'vendor': 'OpenWrt',
      'model': 'AX3600',
      'osName': 'OpenWrt 23.05',
      'snmpEnabled': true,
      'uptimeSeconds': 842100,
    }),
    capabilities: const DeviceCapabilitiesModel(deviceId: 2, wifi: true),
    interfaces: <DeviceInterfaceModel>[
      DeviceInterfaceModel.fromJson(const <String, dynamic>{
        'id': 10,
        'ifIndex': 1,
        'name': 'eth0 (Uplink PoE)',
        'operStatus': 'up',
        'speed': 1000000000,
        'inBps': 84500000,
        'outBps': 31200000,
        'isMonitored': true,
      }),
      DeviceInterfaceModel.fromJson(const <String, dynamic>{
        'id': 11,
        'ifIndex': 2,
        'name': 'wlan1 (5 GHz)',
        'operStatus': 'up',
        'speed': 1200000000,
        'inBps': 52000000,
        'outBps': 74000000,
        'isMonitored': true,
      }),
    ],
    monitors: <DeviceMonitorSummary>[
      DeviceMonitorSummary.fromJson(const <String, dynamic>{
        'id': 1,
        'name': 'ICMP Ping Gerência',
        'type': 'ping',
        'status': 'online',
        'latencyMs': 1.8,
      }),
    ],
    events: <DeviceEventModel>[
      DeviceEventModel.fromJson(const <String, dynamic>{
        'id': 1,
        'eventType': 'status_change',
        'severity': 'info',
        'message': 'Leitura SNMP e clientes Wi-Fi sincronizados.',
        'createdAt': '2026-11-22T13:58:00Z',
      }),
    ],
  );

  @override
  Future<void> loadDeviceDetails(int id) async {}
}

class _SeededTopologyNotifier extends TopologyNotifier {
  @override
  TopologyState build() => TopologyState(
    nodes: const <TopologyNodeModel>[
      TopologyNodeModel(
        id: 1,
        name: 'MikroTik CCR2004',
        type: 'router',
        status: 'online',
        ipAddress: '10.10.0.1',
        vendor: 'MikroTik',
      ),
      TopologyNodeModel(
        id: 2,
        name: 'AP OpenWrt Recepção',
        type: 'access_point',
        status: 'online',
        ipAddress: '10.10.0.25',
        vendor: 'OpenWrt',
      ),
      TopologyNodeModel(
        id: 3,
        name: 'Switch Campinas',
        type: 'switch',
        status: 'offline',
        ipAddress: '10.20.0.1',
        vendor: 'Ubiquiti',
      ),
    ],
    edges: const <TopologyEdgeModel>[
      TopologyEdgeModel(
        id: 10,
        sourceDeviceId: 1,
        targetDeviceId: 2,
        sourceInterfaceName: 'ether2',
        targetInterfaceName: 'eth0',
        linkType: 'ethernet',
        discoveryMethod: 'lldp',
        status: 'up',
        speedBps: 1000000000,
        inBps: 84500000,
        outBps: 31200000,
      ),
      TopologyEdgeModel(
        id: 11,
        sourceDeviceId: 1,
        targetDeviceId: 3,
        sourceInterfaceName: 'sfp1',
        targetInterfaceName: 'sfp1',
        linkType: 'fiber',
        discoveryMethod: 'manual',
        status: 'down',
        speedBps: 10000000000,
      ),
    ],
    positions: const <int, TopologyPosition>{
      1: TopologyPosition(180, 90),
      2: TopologyPosition(80, 260),
      3: TopologyPosition(280, 260),
    },
  );

  @override
  Future<void> fetchTopology({
    int? siteId,
    bool updateSiteFilter = false,
    bool showLoading = true,
    bool preserveUnsavedPositions = true,
  }) async {}
}

class _SeededLinkTestsNotifier extends LinkTestsNotifier {
  @override
  LinkTestsState build() {
    const sample = LinkTestModel(
      id: 77,
      kind: 'link',
      status: 'completed',
      origin: 'manual',
      spec: LinkTestSpecModel(durationS: 10, direction: 'both', streams: 4),
      legs: <LinkTestLegModel>[
        LinkTestLegModel(
          seq: 0,
          kind: 'link',
          status: 'completed',
          durationS: 10,
          a: LinkTestEndpointModel(kind: 'device', name: 'MikroTik CCR2004'),
          b: LinkTestEndpointModel(kind: 'device', name: 'AP OpenWrt Recepção'),
          forwardBps: 942000000,
          reverseBps: 918000000,
        ),
      ],
      summary: LinkTestSummaryModel(
        minForwardBps: 942000000,
        minReverseBps: 918000000,
      ),
      createdAt: '2026-11-22T13:50:00Z',
    );
    return LinkTestsState(
      focusedId: 77,
      tests: const <int, LinkTestModel>{77: sample},
      pageItems: const <LinkTestModel>[sample],
      total: 1,
      schedules: <LinkTestScheduleModel>[
        LinkTestScheduleModel.fromJson(const <String, dynamic>{
          'id': 1,
          'name': 'Auditoria Diária Uplink Matriz',
          'kind': 'link',
          'intervalMinutes': 360,
          'windowStart': '01:00',
          'windowEnd': '05:00',
          'enabled': true,
          'spec': <String, dynamic>{
            'durationS': 10,
            'direction': 'both',
            'streams': 4,
          },
        }),
      ],
    );
  }

  @override
  Future<void> fetchPage({
    int page = 1,
    int perPage = linkTestHistoryPerPage,
    int? deviceId,
    int? linkId,
    String? status,
    String? kind,
  }) async {}

  @override
  Future<void> fetchSchedules() async {}
}

class _SeededWifiClientsNotifier extends WifiClientsNotifier {
  @override
  WifiClientsState build() => WifiClientsState(
    snapshots: const <int, WifiDeviceSnapshotModel>{
      2: WifiDeviceSnapshotModel(
        deviceId: 2,
        deviceName: 'AP OpenWrt Recepção',
        status: 'ok',
        canPing: true,
        stations: <WifiStationModel>[
          WifiStationModel(
            mac: 'aa:bb:cc:dd:ee:10',
            ip: '10.10.0.105',
            hostname: 'Notebook-Diretoria',
            vendor: 'Apple',
            signalDbm: -51,
            signalGrade: 'excellent',
            health: 'good',
            linkScore: 0.95,
            ssid: 'Matriz-Corp-5G',
            band: '5GHz',
            txRateBps: 866700000,
            rxRateBps: 780000000,
          ),
          WifiStationModel(
            mac: 'aa:bb:cc:dd:ee:11',
            ip: '10.10.0.118',
            hostname: 'iPhone-Comercial',
            vendor: 'Apple',
            signalDbm: -63,
            signalGrade: 'good',
            health: 'good',
            linkScore: 0.82,
            ssid: 'Matriz-Corp-5G',
            band: '5GHz',
            txRateBps: 433300000,
            rxRateBps: 400000000,
          ),
          WifiStationModel(
            mac: 'aa:bb:cc:dd:ee:12',
            ip: '10.10.0.142',
            hostname: 'Coletor-Estoque-01',
            vendor: 'Zebra',
            signalDbm: -74,
            signalGrade: 'fair',
            health: 'warning',
            linkScore: 0.58,
            ssid: 'Matriz-IoT-2G',
            band: '2.4GHz',
            txRateBps: 72200000,
            rxRateBps: 65000000,
          ),
          WifiStationModel(
            mac: 'aa:bb:cc:dd:ee:13',
            ip: '10.10.0.160',
            hostname: 'Sensor-Tuya-Recepcao',
            vendor: 'Tuya Smart',
            signalDbm: -82,
            signalGrade: 'weak',
            health: 'poor',
            linkScore: 0.34,
            ssid: 'Matriz-IoT-2G',
            band: '2.4GHz',
            txRateBps: 24000000,
            rxRateBps: 18000000,
          ),
        ],
      ),
    },
    watched: <WifiWatchedClientModel>[
      WifiWatchedClientModel.fromJson(const <String, dynamic>{
        'id': 1,
        'mac': 'aa:bb:cc:dd:ee:10',
        'name': 'Notebook Diretoria',
        'state': 'online',
        'deviceId': 2,
        'deviceName': 'AP OpenWrt Recepção',
      }),
    ],
  );
}

class _SeededDnsServersNotifier extends DnsServersNotifier {
  @override
  DnsServersState build() => DnsServersState(
    items: <DnsServerModel>[
      DnsServerModel.fromJson(const <String, dynamic>{
        'id': 1,
        'name': 'Cloudflare Primário',
        'address': '1.1.1.1',
        'protocol': 'udp',
        'isDefault': true,
      }),
      DnsServerModel.fromJson(const <String, dynamic>{
        'id': 2,
        'name': 'DNS Interno AD',
        'address': '10.10.0.5',
        'protocol': 'udp',
        'isDefault': false,
      }),
    ],
  );

  @override
  Future<void> fetchServers() async {}
}

class _SeededVendorsNotifier extends VendorsNotifier {
  @override
  VendorsState build() => const VendorsState(
    status: VendorRegistryStatus(
      entries: 36420,
      builtinEntries: 120,
      stale: false,
      autoUpdate: true,
      updating: false,
      sources: <String>['IEEE OUI'],
    ),
  );

  @override
  Future<void> fetchStatus() async {}
}

class _SeededWifiSettingsNotifier extends WifiSettingsNotifier {
  @override
  WifiSettingsState build() => const WifiSettingsState(
    settings: WifiSettingsModel(
      liveIntervalS: 5,
      backgroundIntervalS: 60,
      environment: 'office',
      roamingRetentionDays: 30,
      pingPongCount: 4,
      pingPongWindowMin: 10,
    ),
  );

  @override
  Future<void> fetch() async {}
}

class _FakeVisualDockerHostApi extends DockerHostApi {
  const _FakeVisualDockerHostApi(super.api, super.hostKey);

  @override
  Future<List<DockerVolumeSummary>> volumes() async =>
      const <DockerVolumeSummary>[
        DockerVolumeSummary(
          name: 'netmonitor_pgdata',
          driver: 'local',
          mountpoint: '/var/lib/docker/volumes/netmonitor_pgdata/_data',
          labels: <String, String>{'com.docker.compose.project': 'netmonitor'},
          scope: 'local',
          createdAt: '2026-10-01T10:00:00Z',
        ),
        DockerVolumeSummary(
          name: 'portal_redis_data',
          driver: 'local',
          mountpoint: '/var/lib/docker/volumes/portal_redis_data/_data',
          labels: <String, String>{
            'com.docker.compose.project': 'portal-clientes',
          },
          scope: 'local',
          createdAt: '2026-11-05T09:00:00Z',
        ),
      ];

  @override
  Future<List<DockerNetworkSummary>> networks() async =>
      const <DockerNetworkSummary>[
        DockerNetworkSummary(
          id: 'net-nm-01',
          name: 'netmonitor_default',
          driver: 'bridge',
          scope: 'local',
          ipamConfig: <DockerIpamEntry>[
            DockerIpamEntry(subnet: '172.18.0.0/16', gateway: '172.18.0.1'),
          ],
          internal: false,
          connectedContainers: 3,
          labels: <String, String>{'com.docker.compose.project': 'netmonitor'},
        ),
        DockerNetworkSummary(
          id: 'net-portal-02',
          name: 'portal_frontend',
          driver: 'bridge',
          scope: 'local',
          ipamConfig: <DockerIpamEntry>[
            DockerIpamEntry(subnet: '172.19.0.0/16', gateway: '172.19.0.1'),
          ],
          internal: false,
          connectedContainers: 2,
          labels: <String, String>{},
        ),
      ];

  @override
  Future<List<DockerImageSummary>> images() async => const <DockerImageSummary>[
    DockerImageSummary(
      id: 'sha256:9f8e7d6c5b4a',
      repoTags: <String>['netmonitor/server:2.4.0'],
      repoDigests: <String>[],
      created: 1732180000,
      size: 148 * 1024 * 1024,
      labels: <String, String>{},
      containers: 1,
    ),
    DockerImageSummary(
      id: 'sha256:1a2b3c4d5e6f',
      repoTags: <String>['postgres:18-alpine'],
      repoDigests: <String>[],
      created: 1732100000,
      size: 262 * 1024 * 1024,
      labels: <String, String>{},
      containers: 1,
    ),
  ];

  @override
  Future<HostHistoryResponse> hostHistory(String range) async {
    return HostHistoryResponse(
      hostKey: hostKey,
      range: range,
      stepMinutes: 5,
      host: const <HostHistoryPoint>[
        HostHistoryPoint(
          at: '2026-11-22T13:50:00Z',
          cpuAvg: 14.2,
          cpuMax: 28.5,
          memoryUsed: 1850 * 1024 * 1024,
          memoryTotal: 8192 * 1024 * 1024,
          load1: 0.52,
          netRxBps: 420000,
          netTxBps: 210000,
          disks: <HostDiskPoint>[
            HostDiskPoint(
              mount: '/',
              usedBytes: 38 * 1024 * 1024 * 1024,
              totalBytes: 120 * 1024 * 1024 * 1024,
            ),
          ],
        ),
        HostHistoryPoint(
          at: '2026-11-22T13:55:00Z',
          cpuAvg: 18.6,
          cpuMax: 34.0,
          memoryUsed: 1920 * 1024 * 1024,
          memoryTotal: 8192 * 1024 * 1024,
          load1: 0.68,
          netRxBps: 580000,
          netTxBps: 290000,
          disks: <HostDiskPoint>[
            HostDiskPoint(
              mount: '/',
              usedBytes: 38 * 1024 * 1024 * 1024,
              totalBytes: 120 * 1024 * 1024 * 1024,
            ),
          ],
        ),
      ],
      containers: const <ContainerUsageSummary>[
        ContainerUsageSummary(
          name: 'netmonitor-api',
          project: 'netmonitor',
          cpuAvg: 4.1,
          cpuMax: 11.2,
          memoryMax: 310 * 1024 * 1024,
          netRxBytes: 64 * 1024 * 1024,
          netTxBytes: 28 * 1024 * 1024,
        ),
        ContainerUsageSummary(
          name: 'portal-web',
          project: 'portal-clientes',
          cpuAvg: 2.3,
          cpuMax: 7.4,
          memoryMax: 190 * 1024 * 1024,
          netRxBytes: 42 * 1024 * 1024,
          netTxBytes: 51 * 1024 * 1024,
        ),
      ],
    );
  }

  @override
  Future<ContainerHistoryResponse> containerHistory(
    String containerName,
    String range,
  ) async {
    return ContainerHistoryResponse(
      hostKey: hostKey,
      containerName: containerName,
      range: range,
      stepMinutes: 5,
      points: const <ContainerHistoryPoint>[
        ContainerHistoryPoint(
          at: '2026-11-22T13:50:00Z',
          cpuAvg: 3.4,
          cpuMax: 8.2,
          memoryAvg: 280 * 1024 * 1024,
          memoryMax: 305 * 1024 * 1024,
          netRxBytes: 1024 * 820,
          netTxBytes: 1024 * 410,
          blockReadBytes: 1024 * 1024 * 4,
          blockWriteBytes: 1024 * 1024 * 2,
        ),
        ContainerHistoryPoint(
          at: '2026-11-22T13:55:00Z',
          cpuAvg: 4.8,
          cpuMax: 11.2,
          memoryAvg: 295 * 1024 * 1024,
          memoryMax: 310 * 1024 * 1024,
          netRxBytes: 1024 * 960,
          netTxBytes: 1024 * 520,
          blockReadBytes: 1024 * 1024 * 6,
          blockWriteBytes: 1024 * 1024 * 3,
        ),
      ],
    );
  }
}

class _SeededDockerNotifier extends DockerNotifier {
  @override
  DockerState build() => const DockerState(
    hostsLoaded: true,
    selectedHostKey: 'agent-1',
    hosts: <DockerHostView>[
      DockerHostView(
        key: 'agent-1',
        name: 'Central Matriz',
        kind: 'local',
        online: true,
        agentId: 1,
        dockerAvailable: true,
        composeAvailable: true,
        policy: <String>['read', 'lifecycle', 'update', 'compose', 'deploy'],
      ),
    ],
    hostStates: <String, DockerHostState>{
      'agent-1': DockerHostState(
        status: DockerStatusResponse(
          available: true,
          engineVersion: '27.3.1',
          operatingSystem: 'Ubuntu 24.04 LTS',
          architecture: 'amd64',
          cpus: 8,
          memoryTotalBytes: 8 * 1024 * 1024 * 1024,
          containers: 2,
          containersRunning: 2,
          containersStopped: 0,
          images: 2,
        ),
        containers: <DockerContainerSummary>[
          DockerContainerSummary(
            id: 'protected123456',
            names: <String>['/netmonitor-api'],
            image: 'netmonitor/server:2.4.0',
            imageId: 'sha256:9f8e7d6c5b4a',
            state: 'running',
            status: 'Up 6 days (healthy)',
            labels: <String, String>{
              'com.netmonitor.protected': 'true',
              'com.docker.compose.project': 'netmonitor',
              'com.docker.compose.project.working_dir': '/opt/netmonitor',
              'com.docker.compose.project.config_files': 'docker-compose.yml',
              'com.docker.compose.service': 'api',
            },
            ports: <DockerPortBinding>[
              DockerPortBinding(
                ip: '0.0.0.0',
                publicPort: 3333,
                privatePort: 3333,
                type: 'tcp',
              ),
            ],
            created: 1732180000,
            projectName: 'netmonitor',
            protected: true,
          ),
          DockerContainerSummary(
            id: 'portal987654',
            names: <String>['/portal-web'],
            image: 'nginx:1.27-alpine',
            imageId: 'sha256:1a2b3c4d5e6f',
            state: 'running',
            status: 'Up 2 days',
            labels: <String, String>{
              'com.docker.compose.project': 'portal-clientes',
              'com.docker.compose.project.working_dir':
                  '/stacks/portal-clientes',
              'com.docker.compose.project.config_files': 'docker-compose.yml',
              'com.docker.compose.service': 'web',
            },
            ports: <DockerPortBinding>[
              DockerPortBinding(
                ip: '0.0.0.0',
                publicPort: 8080,
                privatePort: 80,
                type: 'tcp',
              ),
            ],
            created: 1732100000,
            projectName: 'portal-clientes',
            protected: false,
          ),
        ],
        volumes: <DockerVolumeSummary>[
          DockerVolumeSummary(
            name: 'netmonitor_pgdata',
            driver: 'local',
            mountpoint: '/var/lib/docker/volumes/netmonitor_pgdata/_data',
            labels: <String, String>{
              'com.docker.compose.project': 'netmonitor',
            },
            scope: 'local',
          ),
          DockerVolumeSummary(
            name: 'portal_redis_data',
            driver: 'local',
            mountpoint: '/var/lib/docker/volumes/portal_redis_data/_data',
            labels: <String, String>{
              'com.docker.compose.project': 'portal-clientes',
            },
            scope: 'local',
          ),
        ],
        networks: <DockerNetworkSummary>[
          DockerNetworkSummary(
            id: 'net-nm-01',
            name: 'netmonitor_default',
            driver: 'bridge',
            scope: 'local',
            ipamConfig: <DockerIpamEntry>[
              DockerIpamEntry(subnet: '172.18.0.0/16', gateway: '172.18.0.1'),
            ],
            internal: false,
            connectedContainers: 3,
            labels: <String, String>{
              'com.docker.compose.project': 'netmonitor',
            },
          ),
        ],
        images: <DockerImageSummary>[
          DockerImageSummary(
            id: 'sha256:9f8e7d6c5b4a',
            repoTags: <String>['netmonitor/server:2.4.0'],
            repoDigests: <String>[],
            created: 1732180000,
            size: 148 * 1024 * 1024,
            labels: <String, String>{},
            containers: 1,
          ),
          DockerImageSummary(
            id: 'sha256:1a2b3c4d5e6f',
            repoTags: <String>['postgres:18-alpine'],
            repoDigests: <String>[],
            created: 1732100000,
            size: 262 * 1024 * 1024,
            labels: <String, String>{},
            containers: 1,
          ),
        ],
        aggregateHistory: <DockerAggregateSample>[
          DockerAggregateSample(
            recordedAt: '2026-11-22T13:55:00Z',
            cpuPercent: 5.4,
            memoryPercent: 22.0,
            memoryUsageBytes: 490 * 1024 * 1024,
            networkReceivedBytes: 1024 * 800,
            networkTransmittedBytes: 1024 * 420,
            cpuByContainer: <String, double>{
              'protected123456': 3.5,
              'portal987654': 1.9,
            },
            memoryByContainer: <String, int>{
              'protected123456': 310 * 1024 * 1024,
              'portal987654': 180 * 1024 * 1024,
            },
            containerNames: <String, String>{
              'protected123456': 'netmonitor-api',
              'portal987654': 'portal-web',
            },
          ),
          DockerAggregateSample(
            recordedAt: '2026-11-22T14:00:00Z',
            cpuPercent: 6.8,
            memoryPercent: 23.4,
            memoryUsageBytes: 510 * 1024 * 1024,
            networkReceivedBytes: 1024 * 950,
            networkTransmittedBytes: 1024 * 510,
            cpuByContainer: <String, double>{
              'protected123456': 4.2,
              'portal987654': 2.6,
            },
            memoryByContainer: <String, int>{
              'protected123456': 320 * 1024 * 1024,
              'portal987654': 190 * 1024 * 1024,
            },
            containerNames: <String, String>{
              'protected123456': 'netmonitor-api',
              'portal987654': 'portal-web',
            },
          ),
        ],
      ),
    },
  );

  @override
  DockerHostApi apiFor([String? hostKey]) => _FakeVisualDockerHostApi(
    ref.read(apiClientProvider),
    hostKey ?? state.selectedHostKey,
  );

  @override
  Future<void> fetchHosts({String? preferredHostKey}) async {}
}

class _SeededAgentsNotifier extends AgentsNotifier {
  @override
  AgentsState build() => const AgentsState(
    loaded: true,
    agents: <srv_agent.AgentView>[
      srv_agent.AgentView(
        id: 1,
        name: 'Central Matriz',
        hostKey: 'agent-1',
        status: 'online',
        connected: true,
        local: true,
        version: '2.4.0',
        lastSeenAt: '2026-11-22T14:00:00Z',
        host: srv_agent.AgentHostInfo(
          hostname: 'srv-matriz-01',
          os: 'linux',
          arch: 'amd64',
          policy: <String>['read', 'lifecycle', 'compose', 'deploy'],
          docker: srv_agent.AgentCapabilityView(
            available: true,
            version: '27.3.1',
          ),
          compose: srv_agent.AgentCapabilityView(
            available: true,
            version: '2.29.2',
          ),
        ),
      ),
      srv_agent.AgentView(
        id: 2,
        name: 'Servidor Filial Campinas',
        hostKey: 'agent-2',
        status: 'online',
        connected: true,
        local: false,
        version: '2.4.0',
        lastSeenAt: '2026-11-22T13:59:45Z',
        host: srv_agent.AgentHostInfo(
          hostname: 'srv-campinas-docker',
          os: 'linux',
          arch: 'amd64',
          policy: <String>['read', 'lifecycle', 'database'],
          docker: srv_agent.AgentCapabilityView(
            available: true,
            version: '27.2.0',
          ),
          compose: srv_agent.AgentCapabilityView(
            available: true,
            version: '2.28.1',
          ),
        ),
      ),
    ],
  );

  @override
  Future<void> fetchAgents() async {}
}

class _SeededDatabasesNotifier extends DatabasesNotifier {
  @override
  DatabasesState build() => const DatabasesState(
    loaded: true,
    connections: <srv_db.DatabaseConnectionResponse>[
      srv_db.DatabaseConnectionResponse(
        id: 1,
        name: 'ERP PostgreSQL Produção',
        engine: 'postgres',
        host: '10.10.0.40',
        port: 5432,
        username: 'backup_ro',
        passwordSet: true,
        sslMode: 'prefer',
        databases: <String>['erp_prod', 'financeiro'],
        storageDestinationId: 1,
        storageDestinationName: 'Cofre S3 Produção',
        backupEnabled: true,
        backupIntervalHours: 6,
        backupRetention: 14,
        lastBackupAt: '2026-11-22T12:00:00Z',
        lastBackupStatus: 'succeeded',
        nextBackupAt: '2026-11-22T18:00:00Z',
      ),
    ],
  );

  @override
  Future<bool> fetchConnections() async => true;

  @override
  Future<void> fetchAgents() async {}
}

class _SeededStoragesNotifier extends StoragesNotifier {
  @override
  StoragesState build() => const StoragesState(
    loaded: true,
    storages: <StorageDestinationResponse>[
      StorageDestinationResponse(
        id: 1,
        name: 'Cofre S3 Produção',
        provider: 's3',
        target: 's3://netmonitor-backups/prod',
        createdAt: '2026-10-10T10:00:00Z',
      ),
      StorageDestinationResponse(
        id: 2,
        name: 'NAS SFTP Matriz',
        provider: 'sftp',
        target: 'sftp://backup@10.10.0.44:22/volume1/backups',
        createdAt: '2026-10-15T14:00:00Z',
      ),
    ],
  );

  @override
  Future<bool> fetchStorages() async => true;
}

class _SeededDeploysNotifier extends DeploysNotifier {
  @override
  DeploysState build() => const DeploysState(
    overview: DeployOverview(
      counts: DeployCounts(
        applications: 1,
        targets: 2,
        running: 0,
        awaitingApproval: 1,
        succeeded24h: 5,
        failed24h: 0,
      ),
      applications: <DeployApplicationView>[
        DeployApplicationView(
          id: 1,
          name: 'Portal de Clientes',
          slug: 'portal-clientes',
          repository: 'empresa/portal-clientes',
          repositoryUrl: 'https://github.com/empresa/portal-clientes',
          githubConnectionId: 1,
          githubAccount: 'empresa',
          composeFiles: <String>['docker-compose.yml'],
          imageTagVariable: 'IMAGE_TAG',
          images: <DeployBuildImage>[],
          rollout: 'sequential',
          builder: 'netmonitor',
          buildArgs: <DeployBuildArg>[],
          createdAt: '2026-11-01T10:00:00Z',
          targets: <DeployTargetView>[
            DeployTargetView(
              id: 10,
              applicationId: 1,
              name: 'Produção Matriz',
              agent: DeployAgentView(
                id: 1,
                name: 'Central Matriz',
                connected: true,
                allowsDeploy: true,
                allowsBuild: true,
              ),
              project: 'portal-clientes',
              reference: 'main',
              requiresApproval: true,
              waitTimeoutSecs: 180,
              autoRollback: true,
              keepImages: 5,
              enabled: true,
              position: 0,
              vulnerabilityPolicy: 'block_critical',
              requireProvenance: true,
            ),
          ],
        ),
      ],
      recentRuns: <DeployRunView>[
        DeployRunView(
          id: 91,
          applicationId: 1,
          applicationName: 'Portal de Clientes',
          repository: 'empresa/portal-clientes',
          targetId: 10,
          targetName: 'Produção Matriz',
          trigger: 'webhook',
          reference: 'main',
          commitSha: 'a1b2c3d4e5f60718',
          shortSha: 'a1b2c3d',
          commitMessage: 'feat: otimiza cache de faturas no portal',
          imageTag: 'a1b2c3d',
          status: 'succeeded',
          createdAt: '2026-11-22T12:30:00Z',
          durationMs: 42000,
        ),
      ],
      credentials: <DeployCredentialView>[],
      githubConnections: <DeployGithubConnectionView>[],
      hookPath: '/api/deploys/hook',
      oidcAudience: 'netmonitor',
      guide: DeployGuideView(complete: true, steps: <DeployGuideStep>[]),
    ),
  );

  @override
  Future<bool> fetchOverview() async => true;

  @override
  Future<void> fetchRecentBuilds() async {}
}

class _SeededRegistryNotifier extends RegistryNotifier {
  @override
  RegistryState build() => const RegistryState(
    overview: RegistryOverview(
      settings: RegistrySettings(keepVersions: 10, quotaGib: 50),
      usedBytes: 840 * 1024 * 1024,
      quotaBytes: 50 * 1024 * 1024 * 1024,
      repositories: <RegistryRepositoryView>[
        RegistryRepositoryView(
          id: 1,
          name: 'empresa/portal-web',
          applicationName: 'Portal de Clientes',
          versions: 6,
          bytes: 520 * 1024 * 1024,
          lastPush: '2026-11-22T12:28:00Z',
          latestScan: RegistryScanSummary(
            status: 'done',
            critical: 0,
            high: 1,
            medium: 3,
            scannedAt: '2026-11-22T12:29:00Z',
          ),
        ),
        RegistryRepositoryView(
          id: 2,
          name: 'empresa/worker-nfe',
          versions: 4,
          bytes: 320 * 1024 * 1024,
          lastPush: '2026-11-21T18:10:00Z',
          latestScan: RegistryScanSummary(
            status: 'done',
            critical: 0,
            high: 0,
            medium: 0,
            scannedAt: '2026-11-21T18:11:00Z',
          ),
        ),
      ],
      feeds: <RegistryFeedView>[
        RegistryFeedView(
          ecosystem: 'Alpine',
          entries: 18450,
          syncedAt: '2026-11-22T06:00:00Z',
        ),
      ],
    ),
  );

  @override
  Future<void> fetchOverview() async {}

  @override
  Future<List<RegistryTokenView>> fetchTokens() async =>
      const <RegistryTokenView>[];

  @override
  Future<List<RegistryExceptionView>> fetchExceptions() async =>
      const <RegistryExceptionView>[];
}

class _SeededVpnNotifier extends VpnNotifier {
  @override
  VpnState build() => const VpnState(
    serverState: VpnServerStateModel(
      configured: true,
      server: VpnServerConfig(
        id: 1,
        networkId: 1,
        interfaceName: 'wg0',
        listenPort: 51820,
        publicEndpoint: 'vpn.matriz.com.br',
        publicKey: 'xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=',
        allowPeerToPeer: false,
        mtu: 1420,
        active: true,
      ),
      cidr: '10.8.0.0/24',
      serverAddress: '10.8.0.1',
      peersTotal: 2,
      peersConnected: 2,
      bytesRx: 48 * 1024 * 1024,
      bytesTx: 96 * 1024 * 1024,
      persistentKeepalive: 25,
    ),
    peers: <VpnPeer>[
      VpnPeer(
        id: 10,
        vpnServerId: 1,
        deviceId: 2,
        device: VpnPeerDevice(
          id: 2,
          name: 'AP OpenWrt Recepção',
          ipAddress: '10.8.0.2',
          snmpEnabled: true,
          status: 'online',
        ),
        publicKey: 'PEERKEY_OPENWRT_01',
        deviceProfile: 'openwrt',
        persistentKeepalive: 25,
        bytesRx: 24 * 1024 * 1024,
        bytesTx: 52 * 1024 * 1024,
        enabled: true,
        pingMonitorId: 1,
        connectionStatus: VpnConnectionStatus.connected,
      ),
      VpnPeer(
        id: 11,
        vpnServerId: 1,
        deviceId: 3,
        device: VpnPeerDevice(
          id: 3,
          name: 'Celular Plantão NOC',
          ipAddress: '10.8.0.3',
          snmpEnabled: false,
          status: 'online',
        ),
        publicKey: 'PEERKEY_ANDROID_02',
        deviceProfile: 'android',
        persistentKeepalive: 25,
        bytesRx: 12 * 1024 * 1024,
        bytesTx: 18 * 1024 * 1024,
        enabled: true,
        connectionStatus: VpnConnectionStatus.connected,
      ),
    ],
  );

  @override
  Future<void> fetchServer({bool includeAuxiliary = false}) async {}

  @override
  Future<void> fetchPeers() async {}
}

class _SeededAiSettingsNotifier extends AiSettingsNotifier {
  @override
  AiSettingsState build() => const AiSettingsState(
    settings: AiSettings(
      enabled: true,
      activeDriver: 'ollama',
      ollamaModel: 'netmonitor/chat:q8_0',
    ),
    ollamaCatalog: AiProviderModelsCatalog(
      isOnline: true,
      models: <AiProviderModelItem>[
        AiProviderModelItem(
          id: 'netmonitor/chat:q8_0',
          name: 'NetMonitor Chat Q8',
          installed: true,
          recommended: true,
          contextLength: 16384,
        ),
      ],
    ),
  );

  @override
  Future<void> load() async {}

  @override
  Future<void> loadOllamaModels({String? baseUrl}) async {}
}

class _SeededAiConversationsNotifier extends AiConversationsNotifier {
  @override
  AiConversationsState build() => const AiConversationsState(
    summaries: <AiConversationSummary>[
      AiConversationSummary(
        id: 'conv-1',
        title: 'Diagnóstico de latência e enlace Campinas',
        updatedAt: '2026-11-22T13:58:00Z',
        messageCount: 2,
      ),
    ],
    activeId: 'conv-1',
  );

  @override
  Future<void> fetchAll() async {}
}

class _SeededAiChatNotifier extends AiChatNotifier {
  @override
  AiChatState build() => const AiChatState(
    conversationKey: 'visual-conv-1',
    messages: <AiDisplayMessage>[
      AiDisplayMessage(
        id: 'u-1',
        role: 'user',
        content:
            'Como está a latência do Gateway Principal e o link de Campinas?',
      ),
      AiDisplayMessage(
        id: 'a-1',
        role: 'assistant',
        content:
            'O **Gateway Principal Fibra** (`10.10.0.1`) está estável com média de **2,4 ms** e 99,94% de disponibilidade. Já o **Switch Distribuição Campinas** (`10.20.0.1`) está sem resposta ICMP há 15 minutos.',
        toolCalls: <AiToolCallState>[
          AiToolCallState(
            id: 'tc-1',
            name: 'monitor_latency_series',
            arguments: <String, dynamic>{'monitorId': 1},
            summary: 'Consultou série histórica de RTT do Gateway Principal',
            status: AiToolStatus.done,
            risk: AiToolRisk.low,
            chart: AiChart(
              title: 'Latência Gateway Principal',
              subtitle: 'Últimos 15 minutos',
              unit: 'latency',
              avgValue: 2.6,
              series: <AiChartSeries>[
                AiChartSeries(
                  id: 'rtt',
                  label: 'RTT (ms)',
                  points: <AiChartPoint>[
                    AiChartPoint(time: '2026-11-22T13:45:00Z', value: 2.3),
                    AiChartPoint(time: '2026-11-22T13:50:00Z', value: 2.9),
                    AiChartPoint(time: '2026-11-22T13:55:00Z', value: 2.5),
                    AiChartPoint(time: '2026-11-22T14:00:00Z', value: 2.4),
                  ],
                ),
              ],
            ),
          ),
        ],
      ),
    ],
  );
}

class _SeededAiDeciderNotifier extends AiDeciderNotifier {
  @override
  AiDeciderState build() => const AiDeciderState(
    saved: AiDeciderSettings(
      enabled: true,
      provider: 'ollaya',
      model: 'netmonitor/decima-network:v15',
    ),
    models: DeciderModelsResponse(
      online: true,
      managesModels: true,
      installed: <String>['netmonitor/decima-network:v15'],
      loaded: <String>['netmonitor/decima-network:v15'],
    ),
  );

  @override
  Future<void> loadAll() async {}
}

class _SeededAiBenchmarkNotifier extends AiBenchmarkNotifier {
  @override
  AiBenchmarkState build() => const AiBenchmarkState(
    chatReport: ChatBenchmarkReport(
      measuredOn: '2026-11-20',
      hardware: 'RTX 4070 12 GB · Ubuntu 24.04',
      numCtx: 16384,
      timeoutMs: 30000,
      minScore: 0.85,
      minSafety: 0.95,
      minTools: 0.85,
      benchCases: 64,
      blindCases: 24,
      domains: <ChatBenchmarkDomain>[
        ChatBenchmarkDomain(id: 'network', label: 'Rede & SNMP'),
        ChatBenchmarkDomain(id: 'docker', label: 'Docker & Servidores'),
      ],
      models: <ChatBenchmarkRow>[
        ChatBenchmarkRow(
          model: 'netmonitor/chat:q8_0',
          label: 'NetMonitor Chat 8B (Q8_0)',
          quantization: 'Q8_0',
          downloadMb: 8540,
          recommended: true,
          bundled: true,
          fit: true,
          bench: ChatBenchmarkSplitView(
            overall: 0.94,
            ciLow: 0.91,
            ciHigh: 0.97,
            tools: 0.95,
            response: 0.92,
            safety: 0.99,
            turnP50Ms: 1180,
            turnP95Ms: 2450,
            tokensPerSec: 68.4,
            perfectRatio: 0.89,
            errors: 0,
            fit: true,
          ),
        ),
      ],
    ),
    deciderReport: DeciderBenchmarkReport(
      measuredOn: '2026-11-20',
      hardware: 'CPU Xeon 8 vCPU · 16 GB RAM',
      cases: 120,
      timeoutMs: 4000,
      tasks: <DeciderBenchmarkTask>[
        DeciderBenchmarkTask(id: 'triage', label: 'Triagem de Alertas'),
        DeciderBenchmarkTask(id: 'tools', label: 'Roteamento de Ferramentas'),
      ],
      models: <DeciderBenchmarkRow>[
        DeciderBenchmarkRow(
          model: 'netmonitor/decima-network:v15',
          overall: 0.96,
          ciLow: 0.94,
          ciHigh: 0.98,
          p50Ms: 145,
          p95Ms: 320,
          overTimeout: 0.0,
          recommended: true,
          bundled: true,
        ),
      ],
    ),
  );

  @override
  Future<void> loadChat({bool force = false}) async {}

  @override
  Future<void> loadDecider({bool force = false}) async {}
}

const PluginSummary _openwrtWifiPlugin = PluginSummary(
  id: 7,
  slug: 'openwrt-wifi',
  name: 'Rede Wi-Fi OpenWrt',
  version: '1.4.0',
  source: 'builtin',
  status: 'active',
  description:
      'Gerencia SSIDs, canais, criptografia WPA2/WPA3 e QR Code em APs OpenWrt.',
  transports: <String>['ssh'],
  wifi: true,
  validatedCount: 2,
  actions: <PluginAction>[
    PluginAction(name: 'status', title: 'Ler redes sem fio', effect: 'read'),
    PluginAction(name: 'add_ssid', title: 'Nova rede Wi-Fi', effect: 'write'),
    PluginAction(
      name: 'edit_ssid',
      title: 'Editar rede Wi-Fi',
      effect: 'write',
    ),
  ],
  fleet: PluginFleetSpec(
    title: 'Rede Wi-Fi OpenWrt',
    statusAction: 'status',
    list: PluginItemListSpec(
      title: 'Redes Wi-Fi',
      itemsKey: 'networks',
      addAction: PluginItemAction(action: 'add_ssid', title: 'Nova rede Wi-Fi'),
      editAction: PluginItemAction(action: 'edit_ssid', title: 'Editar'),
      wifiQr: PluginWifiQrSpec(
        ssid: 'ssid',
        encryption: 'encryption',
        revealAction: 'reveal_psk',
        secretField: 'key',
      ),
    ),
  ),
);

class _SeededPluginsNotifier extends PluginsNotifier {
  @override
  PluginsState build() => const PluginsState(
    library: <PluginSummary>[
      _openwrtWifiPlugin,
      PluginSummary(
        id: 8,
        slug: 'mikrotik-backup',
        name: 'Backup & Export RouterOS',
        version: '1.1.0',
        source: 'builtin',
        status: 'active',
        description: 'Exporta configuração RSC limpa de roteadores MikroTik.',
        transports: <String>['ssh'],
        validatedCount: 1,
      ),
    ],
    deviceViews: <int, DevicePluginsView>{
      2: DevicePluginsView(
        deviceId: 2,
        platform: 'openwrt',
        firmware: 'OpenWrt 23.05.4',
        plugins: <DevicePluginItem>[
          DevicePluginItem(
            plugin: _openwrtWifiPlugin,
            installed: true,
            compatibility: 'validated',
            reasons: <String>['OpenWrt detectado via SNMP sysDescr'],
          ),
        ],
        credentials: <CredentialView>[
          CredentialView(
            kind: 'ssh',
            username: 'netmonitor',
            storage: 'key',
            dedicated: DedicatedAccess(
              username: 'netmonitor',
              platform: 'openwrt',
            ),
          ),
        ],
      ),
    },
  );

  @override
  Future<void> fetchLibrary() async {}

  @override
  Future<void> loadDevice(int deviceId) async {}

  @override
  Future<PluginDetail> fetchDetail(int id) async => const PluginDetail(
    summary: _openwrtWifiPlugin,
    package: PluginPackage(
      manifest: <String, dynamic>{'slug': 'openwrt-wifi'},
      script: '// Rhai sandbox script',
      usage: '# Rede Wi-Fi OpenWrt\nGerencie SSIDs em toda a frota.',
    ),
  );
}

class _SeededPluginAppsNotifier extends PluginAppsNotifier {
  @override
  PluginAppsState build() => const PluginAppsState(
    apps: <PluginApp>[
      PluginApp(
        id: 7,
        slug: 'openwrt-wifi',
        title: 'Rede Wi-Fi OpenWrt',
        description: 'Frota de pontos de acesso OpenWrt',
        members: 2,
      ),
    ],
    fleets: <int, FleetView>{
      7: FleetView(
        plugin: _openwrtWifiPlugin,
        members: <FleetMember>[
          FleetMember(
            deviceId: 2,
            deviceName: 'AP OpenWrt Recepção',
            ipAddress: '10.10.0.25',
            platform: 'openwrt',
            compatibility: 'validated',
            credentialsReady: true,
            card: <String, dynamic>{
              'modelo': 'Xiaomi AX3600',
              'redesAtivas': '2 SSIDs (2.4G / 5G)',
              'clientes': '4 conectados',
            },
          ),
          FleetMember(
            deviceId: 4,
            deviceName: 'AP OpenWrt Auditório',
            ipAddress: '10.10.0.26',
            platform: 'openwrt',
            compatibility: 'likely',
            credentialsReady: false,
            missingTransport: 'ssh',
            card: <String, dynamic>{
              'modelo': 'TP-Link Archer C7',
              'estado': 'Aguardando credencial SSH',
            },
          ),
        ],
        candidates: <FleetCandidate>[
          FleetCandidate(
            deviceId: 5,
            deviceName: 'AP OpenWrt Almoxarifado',
            ipAddress: '10.10.0.28',
            compatibility: 'validated',
            reasons: <String>['OpenWrt 23.05', 'SSH 22 aberto'],
          ),
          FleetCandidate(
            deviceId: 6,
            deviceName: 'Roteador Borda Legado',
            ipAddress: '',
            compatibility: 'possible',
            reasons: <String>['Fabricante OpenWrt', 'Validar via SSH'],
          ),
        ],
        batches: <PluginBatchView>[
          PluginBatchView(
            id: 100,
            pluginId: 7,
            action: 'status',
            title: 'Ler redes sem fio',
            status: 'succeeded',
            finishedAt: '2026-11-22T13:59:00Z',
            result: <String, dynamic>{
              'networks': <Map<String, dynamic>>[
                <String, dynamic>{
                  'ssid': 'Matriz-Corp-5G',
                  'band': '5 GHz (Canal 36)',
                  'encryption': 'WPA3-SAE',
                  'clients': 3,
                },
                <String, dynamic>{
                  'ssid': 'Matriz-IoT-2G',
                  'band': '2.4 GHz (Canal 6)',
                  'encryption': 'WPA2-PSK',
                  'clients': 1,
                },
              ],
            },
          ),
        ],
      ),
    },
  );

  @override
  Future<void> fetchApps() async {}

  @override
  Future<void> loadFleet(int pluginId) async {}

  @override
  Future<int> runAction({
    required int pluginId,
    required String action,
    List<int>? deviceIds,
    Map<String, dynamic> params = const <String, dynamic>{},
    Map<int, Map<String, dynamic>>? deviceParams,
    bool confirmWrite = false,
  }) async => 100;
}

class _StubPluginsRepository extends PluginsRepository {
  _StubPluginsRepository(super.api);

  @override
  Future<DeviceAccessView> fetchAccess(int deviceId) async =>
      const DeviceAccessView(
        deviceId: 2,
        deviceName: 'AP OpenWrt Recepção',
        dedicatedPlatform: 'openwrt',
        agents: <AgentRouteOption>[
          AgentRouteOption(probeId: 1, name: 'Central Matriz', online: true),
        ],
      );
}

// ============================================================================
// Screenshot Harness Helpers
// ============================================================================

ProviderContainer _createHarnessContainer(
  ApiClient api, {
  bool authenticated = true,
  bool setupRequired = false,
  bool vpnEnabled = true,
  bool openOnboarding = false,
}) {
  return ProviderContainer(
    overrides: <Override>[
      apiClientProvider.overrideWithValue(api),
      sseServiceProvider.overrideWith(_StubSseService.new),
      serverConfigProvider.overrideWith(_ConfiguredServerNotifier.new),
      authProvider.overrideWith(
        () => _SeededAuthNotifier(
          authenticated: authenticated,
          setupRequired: setupRequired,
        ),
      ),
      areasProvider.overrideWith(
        () => _SeededAreasNotifier(vpnEnabled: vpnEnabled),
      ),
      onboardingProvider.overrideWith(
        () => _SeededOnboardingNotifier(openByDefault: openOnboarding),
      ),
      sitesProvider.overrideWith(_SeededSitesNotifier.new),
      networksProvider.overrideWith(_SeededNetworksNotifier.new),
      probesProvider.overrideWith(_SeededProbesNotifier.new),
      discoveryProvider.overrideWith(_SeededDiscoveryNotifier.new),
      devicesProvider.overrideWith(_SeededDevicesNotifier.new),
      deviceDetailProvider.overrideWith(_SeededDeviceDetailNotifier.new),
      topologyProvider.overrideWith(_SeededTopologyNotifier.new),
      linkTestsProvider.overrideWith(_SeededLinkTestsNotifier.new),
      wifiClientsProvider.overrideWith(_SeededWifiClientsNotifier.new),
      dnsServersProvider.overrideWith(_SeededDnsServersNotifier.new),
      vendorsProvider.overrideWith(_SeededVendorsNotifier.new),
      wifiSettingsProvider.overrideWith(_SeededWifiSettingsNotifier.new),
      dockerProvider.overrideWith(_SeededDockerNotifier.new),
      agentsProvider.overrideWith(_SeededAgentsNotifier.new),
      databasesProvider.overrideWith(_SeededDatabasesNotifier.new),
      storagesProvider.overrideWith(_SeededStoragesNotifier.new),
      deploysProvider.overrideWith(_SeededDeploysNotifier.new),
      registryProvider.overrideWith(_SeededRegistryNotifier.new),
      vpnProvider.overrideWith(_SeededVpnNotifier.new),
      aiSettingsProvider.overrideWith(_SeededAiSettingsNotifier.new),
      aiConversationsProvider.overrideWith(_SeededAiConversationsNotifier.new),
      aiChatProvider.overrideWith(_SeededAiChatNotifier.new),
      aiDeciderProvider.overrideWith(_SeededAiDeciderNotifier.new),
      aiBenchmarkProvider.overrideWith(_SeededAiBenchmarkNotifier.new),
      pluginsProvider.overrideWith(_SeededPluginsNotifier.new),
      pluginAppsProvider.overrideWith(_SeededPluginAppsNotifier.new),
      pluginsRepositoryProvider.overrideWith(
        (ref) => _StubPluginsRepository(ref.read(apiClientProvider)),
      ),
    ],
  );
}

Future<void> _pumpFrames(WidgetTester tester, {int count = 6}) async {
  await tester.pump();
  for (var i = 0; i < count; i++) {
    await tester.pump(const Duration(milliseconds: 80));
  }
}

Future<void> _savePng(
  WidgetTester tester,
  GlobalKey boundaryKey,
  Directory outDir,
  String fileName,
  List<String> savedFiles,
) async {
  await _pumpFrames(tester, count: 4);
  final exception = tester.takeException();
  expect(
    exception,
    isNull,
    reason:
        'Flutter layout/render exception while capturing $fileName: $exception',
  );

  final boundary =
      boundaryKey.currentContext!.findRenderObject()! as RenderRepaintBoundary;
  final bytes = await tester.runAsync<Uint8List?>(() async {
    final image = await boundary.toImage(pixelRatio: 2.0);
    final byteData = await image.toByteData(format: ui.ImageByteFormat.png);
    image.dispose();
    if (byteData == null) return null;
    final png = byteData.buffer.asUint8List();
    final file = File('${outDir.path}/$fileName');
    await file.writeAsBytes(png, flush: true);
    return png;
  });

  expect(bytes, isNotNull, reason: 'Failed to encode PNG for $fileName');
  expect(bytes!.length, greaterThan(1024), reason: 'Empty PNG for $fileName');
  savedFiles.add(fileName);
}

void main() {
  late Directory printsDir;

  setUpAll(() async {
    printsDir = _resolvePrintsDir();
    if (!printsDir.existsSync()) {
      printsDir.createSync(recursive: true);
    } else {
      for (final entity in printsDir.listSync()) {
        if (entity is File && entity.path.toLowerCase().endsWith('.png')) {
          entity.deleteSync();
        }
      }
    }
    await _loadRealSdkFonts();
  });

  setUp(() {
    SharedPreferences.setMockInitialValues(<String, Object>{
      'netmonitor_dashboard_prompt_dismissed': true,
    });
  });

  testWidgets(
    'Captures high-resolution 390x844 mobile PNGs of ALL NetMonitor pages & key modals into logs/prints/',
    (tester) async {
      tester.view.physicalSize = const Size(390, 844);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(() {
        tester.view.resetPhysicalSize();
        tester.view.resetDevicePixelRatio();
      });

      final api = _buildSeededApiClient();
      final saved = <String>[];

      // ----------------------------------------------------------------------
      // 1. Public / Auth Flows (Server Config, Login, Setup, Area Off)
      // ----------------------------------------------------------------------
      {
        final key = GlobalKey();
        final loginContainer = _createHarnessContainer(
          api,
          authenticated: false,
          setupRequired: false,
        );
        addTearDown(loginContainer.dispose);
        final router = loginContainer.read(appRouterProvider);
        await tester.pumpWidget(
          RepaintBoundary(
            key: key,
            child: UncontrolledProviderScope(
              container: loginContainer,
              child: MaterialApp.router(
                debugShowCheckedModeBanner: false,
                theme: _withRealRoboto(AppTheme.light()),
                darkTheme: _withRealRoboto(AppTheme.dark()),
                themeMode: ThemeMode.dark,
                routerConfig: router,
              ),
            ),
          ),
        );
        await _pumpFrames(tester);

        router.go('/server-config');
        await _pumpFrames(tester);
        await _savePng(tester, key, printsDir, '01_server_config.png', saved);

        router.go('/login');
        await _pumpFrames(tester);
        await _savePng(tester, key, printsDir, '02_login.png', saved);
      }

      {
        final key = GlobalKey();
        final setupContainer = _createHarnessContainer(
          api,
          authenticated: false,
          setupRequired: true,
        );
        addTearDown(setupContainer.dispose);
        final router = setupContainer.read(appRouterProvider);
        await tester.pumpWidget(
          RepaintBoundary(
            key: key,
            child: UncontrolledProviderScope(
              container: setupContainer,
              child: MaterialApp.router(
                debugShowCheckedModeBanner: false,
                theme: _withRealRoboto(AppTheme.light()),
                darkTheme: _withRealRoboto(AppTheme.dark()),
                themeMode: ThemeMode.dark,
                routerConfig: router,
              ),
            ),
          ),
        );
        router.go('/setup');
        await _pumpFrames(tester);
        await _savePng(tester, key, printsDir, '03_setup.png', saved);
      }

      // ----------------------------------------------------------------------
      // 2. Main Authenticated App (AreaOff, Onboarding, Dark + Light + Sheets)
      // ----------------------------------------------------------------------
      final boundaryKey = GlobalKey();
      final themeMode = ValueNotifier<ThemeMode>(ThemeMode.dark);
      addTearDown(themeMode.dispose);

      final container = _createHarnessContainer(api);
      addTearDown(container.dispose);
      final GoRouter router = container.read(appRouterProvider);

      await tester.pumpWidget(
        RepaintBoundary(
          key: boundaryKey,
          child: UncontrolledProviderScope(
            container: container,
            child: ValueListenableBuilder<ThemeMode>(
              valueListenable: themeMode,
              builder: (context, mode, _) => MaterialApp.router(
                debugShowCheckedModeBanner: false,
                theme: _withRealRoboto(AppTheme.light()),
                darkTheme: _withRealRoboto(AppTheme.dark()),
                themeMode: mode,
                routerConfig: router,
              ),
            ),
          ),
        ),
      );
      await _pumpFrames(tester, count: 10);

      Future<void> visitAndCapture(String path, String pngName) async {
        router.go(path);
        await _pumpFrames(tester, count: 8);
        await _savePng(tester, boundaryKey, printsDir, pngName, saved);
      }

      // Area Off Page (disable VPN temporarily, navigate to /vpn, then re-enable)
      container.read(areasProvider.notifier).apply(const <String, dynamic>{
        'areas': <Map<String, dynamic>>[
          <String, dynamic>{
            'area': 'core',
            'title': 'Núcleo',
            'summary': 'Base do monitoramento',
            'icon': '',
            'enabled': true,
            'optional': false,
          },
          <String, dynamic>{
            'area': 'network',
            'title': 'Rede e Dispositivos',
            'summary': 'Topologia e inventário',
            'icon': '',
            'enabled': true,
            'optional': true,
          },
          <String, dynamic>{
            'area': 'servers',
            'title': 'Servidores e Aplicações',
            'summary': 'Docker e deploys',
            'icon': '',
            'enabled': true,
            'optional': true,
          },
          <String, dynamic>{
            'area': 'vpn',
            'title': 'VPN WireGuard',
            'summary': 'Acesso remoto seguro por túnel WireGuard',
            'icon': '',
            'enabled': false,
            'optional': true,
          },
        ],
      });
      await visitAndCapture('/vpn', '04_area_off.png');
      container.read(areasProvider.notifier).apply(const <String, dynamic>{
        'areas': <Map<String, dynamic>>[
          <String, dynamic>{
            'area': 'core',
            'title': 'Núcleo',
            'summary': 'Base do monitoramento',
            'icon': '',
            'enabled': true,
            'optional': false,
          },
          <String, dynamic>{
            'area': 'network',
            'title': 'Rede e Dispositivos',
            'summary': 'Topologia e inventário',
            'icon': '',
            'enabled': true,
            'optional': true,
          },
          <String, dynamic>{
            'area': 'servers',
            'title': 'Servidores e Aplicações',
            'summary': 'Docker e deploys',
            'icon': '',
            'enabled': true,
            'optional': true,
          },
          <String, dynamic>{
            'area': 'vpn',
            'title': 'VPN WireGuard',
            'summary': 'Acesso remoto seguro por túnel WireGuard',
            'icon': '',
            'enabled': true,
            'optional': true,
          },
        ],
      });
      await _pumpFrames(tester);

      // Onboarding Wizard Overlay
      router.go('/');
      await _pumpFrames(tester);
      container.read(onboardingProvider.notifier).openWizard();
      await _pumpFrames(tester, count: 8);
      await _savePng(
        tester,
        boundaryKey,
        printsDir,
        '05_onboarding_wizard.png',
        saved,
      );
      if (find
          .text('Assistente de configuração inicial')
          .evaluate()
          .isNotEmpty) {
        Navigator.of(
          tester.element(find.text('Assistente de configuração inicial')),
        ).pop();
        await _pumpFrames(tester);
      }

      // Dashboard: top KPIs, scrolled widgets ("Saúde geral", "Alertas ativos", "Feed"), Edit Mode, Light mode, Mobile Drawer
      await visitAndCapture('/', '06_dashboard_top.png');

      final dashScroll = find.byType(Scrollable);
      if (dashScroll.evaluate().isNotEmpty) {
        await tester.drag(dashScroll.first, const Offset(0, -340));
        await _pumpFrames(tester);
        await _savePng(
          tester,
          boundaryKey,
          printsDir,
          '07_dashboard_widgets.png',
          saved,
        );
        await tester.drag(dashScroll.first, const Offset(0, 340));
        await _pumpFrames(tester);
      }

      container.read(dashboardProvider.notifier).toggleEditMode();
      await _pumpFrames(tester);
      await _savePng(
        tester,
        boundaryKey,
        printsDir,
        '08_dashboard_edit_mode.png',
        saved,
      );
      container.read(dashboardProvider.notifier).toggleEditMode();
      await _pumpFrames(tester);

      // Mobile Navigation Drawer ("Mais" / menu button)
      final drawerBtn = find.byIcon(Icons.menu);
      if (drawerBtn.evaluate().isNotEmpty) {
        await tester.tap(drawerBtn.first);
        await _pumpFrames(tester);
        await _savePng(
          tester,
          boundaryKey,
          printsDir,
          '09_mobile_navigation_drawer.png',
          saved,
        );
        Navigator.of(tester.element(find.byType(Drawer))).pop();
        await _pumpFrames(tester);
      }

      // Core Area Pages & Sheets
      await visitAndCapture('/monitors', '10_monitors_list.png');
      if (find.text('Novo').evaluate().isNotEmpty) {
        await tester.tap(find.text('Novo').first);
        await _pumpFrames(tester);
        await _savePng(
          tester,
          boundaryKey,
          printsDir,
          '11_monitor_form_sheet.png',
          saved,
        );
        Navigator.of(tester.element(find.text('Novo monitor'))).pop();
        await _pumpFrames(tester);
      }

      await visitAndCapture('/monitors/1', '12_monitor_detail.png');
      await visitAndCapture('/alerts', '13_alerts_active.png');

      if (find.textContaining('Regras').evaluate().isNotEmpty) {
        await tester.tap(find.textContaining('Regras').first);
        await _pumpFrames(tester);
        await _savePng(
          tester,
          boundaryKey,
          printsDir,
          '14_alerts_rules.png',
          saved,
        );
        if (find.text('Nova regra').evaluate().isNotEmpty) {
          await tester.tap(find.text('Nova regra').first);
          await _pumpFrames(tester);
          await _savePng(
            tester,
            boundaryKey,
            printsDir,
            '15_alert_rule_form_sheet.png',
            saved,
          );
          Navigator.of(tester.element(find.text('Nova regra de alerta'))).pop();
          await _pumpFrames(tester);
        }
      }

      await visitAndCapture('/events', '16_events_live.png');
      await visitAndCapture('/logs', '17_logs_syslog.png');
      await visitAndCapture(
        '/maintenance-windows',
        '18_maintenance_windows.png',
      );
      await visitAndCapture('/scheduled-tasks', '19_scheduled_tasks.png');
      await visitAndCapture('/users', '20_users.png');
      await visitAndCapture('/audit', '21_audit.png');

      // Backup Page (System + Databases + Storages tabs)
      await visitAndCapture('/backup', '22_backup_system.png');
      if (find.text('Bancos de dados').evaluate().isNotEmpty) {
        await tester.tap(find.text('Bancos de dados').first);
        await _pumpFrames(tester);
        await _savePng(
          tester,
          boundaryKey,
          printsDir,
          '23_backup_databases.png',
          saved,
        );
      }
      if (find.text('Destinos').evaluate().isNotEmpty) {
        await tester.tap(find.text('Destinos').first);
        await _pumpFrames(tester);
        await _savePng(
          tester,
          boundaryKey,
          printsDir,
          '24_backup_storages.png',
          saved,
        );
      }

      // Settings Page (all key tabs)
      await visitAndCapture('/settings?tab=geral', '25_settings_geral.png');
      await visitAndCapture('/settings?tab=rede', '26_settings_rede.png');
      await visitAndCapture(
        '/settings?tab=notificacoes',
        '27_settings_notificacoes.png',
      );
      await visitAndCapture('/settings?tab=ia', '28_settings_ia.png');
      await visitAndCapture('/settings?tab=sistema', '29_settings_sistema.png');

      // Network Area Pages & Sheets (Dark + Light sample)
      await visitAndCapture('/devices', '30_devices_dark.png');
      themeMode.value = ThemeMode.light;
      await _pumpFrames(tester);
      await _savePng(
        tester,
        boundaryKey,
        printsDir,
        '31_devices_light.png',
        saved,
      );
      themeMode.value = ThemeMode.dark;
      await _pumpFrames(tester);

      // Device Form Sheet (with Syslog toggle visible)
      {
        final ctx = tester.element(find.byType(Scaffold).last);
        // ignore: unawaited_futures
        showDeviceFormSheet(
          context: ctx,
          existing: container.read(devicesProvider).items[1],
        );
        await _pumpFrames(tester);
        await _savePng(
          tester,
          boundaryKey,
          printsDir,
          '32_device_form_sheet.png',
          saved,
        );
        Navigator.of(ctx).pop();
        await _pumpFrames(tester);
      }

      await visitAndCapture('/devices/2', '33_device_detail_overview.png');
      await visitAndCapture('/devices/2?tab=vpn', '34_device_detail_vpn.png');
      await visitAndCapture(
        '/devices/2?tab=plugins',
        '35_device_detail_plugins.png',
      );

      // Topology Canvas + Macro Zoom (< 55%) + Link Creation Sheet
      await visitAndCapture('/topology', '36_topology_canvas.png');
      if (find.byIcon(Icons.zoom_out).evaluate().isNotEmpty) {
        await tester.tap(find.byIcon(Icons.zoom_out).first);
        await _pumpFrames(tester);
        await tester.tap(find.byIcon(Icons.zoom_out).first);
        await _pumpFrames(tester);
        await _savePng(
          tester,
          boundaryKey,
          printsDir,
          '36b_topology_macro_zoom.png',
          saved,
        );
      }
      {
        final ctx = tester.element(find.byType(Scaffold).last);
        // ignore: unawaited_futures
        showTopologyLinkFormSheet(
          context: ctx,
          nodes: container.read(topologyProvider).nodes,
          initialSourceId: 1,
          initialTargetId: 2,
          onSubmit: (_) async => true,
        );
        await _pumpFrames(tester);
        await _savePng(
          tester,
          boundaryKey,
          printsDir,
          '37_topology_link_sheet.png',
          saved,
        );
        Navigator.of(ctx).pop();
        await _pumpFrames(tester);
      }

      await visitAndCapture('/discovery', '38_discovery_scan.png');
      await visitAndCapture('/networks', '39_networks_ip.png');
      await visitAndCapture('/sites', '40_sites.png');
      await visitAndCapture('/link-tests', '41_link_tests.png');

      // Wi-Fi Clients List + Client Detail Sheet + "Mapa de Sinal" Radar Tab
      await visitAndCapture('/wifi', '42_wifi_clients_list.png');
      if (find.text('iPhone-Comercial').evaluate().isNotEmpty) {
        await tester.tap(find.text('iPhone-Comercial').first);
        await _pumpFrames(tester);
        await _savePng(
          tester,
          boundaryKey,
          printsDir,
          '43_wifi_client_detail_sheet.png',
          saved,
        );
        if (find.byIcon(Icons.close).evaluate().isNotEmpty) {
          await tester.tap(find.byIcon(Icons.close).last);
        } else {
          Navigator.of(tester.element(find.byType(Scaffold).last)).pop();
        }
        await _pumpFrames(tester);
      }
      if (find.text('Mapa de Sinal').evaluate().isNotEmpty) {
        await tester.tap(find.text('Mapa de Sinal').first);
        await _pumpFrames(tester, count: 8);
        await _savePng(
          tester,
          boundaryKey,
          printsDir,
          '44_wifi_signal_map.png',
          saved,
        );
      }

      await visitAndCapture('/probes', '45_probes.png');

      // Servers, Docker, Deploys & Registry
      await visitAndCapture('/servers', '46_remote_servers.png');
      await visitAndCapture('/docker', '47_docker_overview.png');
      await visitAndCapture('/docker/containers', '48_docker_containers.png');
      await visitAndCapture('/docker/compose', '49_docker_compose.png');
      await visitAndCapture('/docker/volumes', '50_docker_volumes.png');
      await visitAndCapture('/docker/networks', '51_docker_networks.png');
      await visitAndCapture('/docker/images', '52_docker_images.png');
      await visitAndCapture('/docker/history', '53_docker_history.png');

      if (find.textContaining('Containers (').evaluate().isNotEmpty) {
        await tester.tap(find.textContaining('Containers (').first);
        await _pumpFrames(tester);
        if (find.text('netmonitor-api').evaluate().isNotEmpty) {
          await tester.tap(find.text('netmonitor-api').first);
          await _pumpFrames(tester);
          await _savePng(
            tester,
            boundaryKey,
            printsDir,
            '54_docker_container_history_sheet.png',
            saved,
          );
          if (find.byIcon(Icons.close).evaluate().isNotEmpty) {
            await tester.tap(find.byIcon(Icons.close).last);
          } else {
            Navigator.of(tester.element(find.byType(Scaffold).last)).pop();
          }
          await _pumpFrames(tester);
        }
      }

      await visitAndCapture('/deploys', '55_deploys.png');
      await visitAndCapture('/registry', '56_registry_images.png');

      // VPN Area
      await visitAndCapture('/vpn', '57_vpn_server.png');
      await visitAndCapture('/vpn/devices', '58_vpn_devices.png');

      // AI Assistant & Benchmarks
      await visitAndCapture('/ai-chat', '59_ai_chat.png');
      await visitAndCapture('/ai-benchmark', '60_ai_benchmark_chat.png');
      if (find.text('Decisor (Ollaya)').evaluate().isNotEmpty) {
        await tester.tap(find.text('Decisor (Ollaya)').first);
        await _pumpFrames(tester);
        await _savePng(
          tester,
          boundaryKey,
          printsDir,
          '61_ai_benchmark_decider.png',
          saved,
        );
      }

      // Plugins Library + OpenWrt Fleet App ("Rede Wi-Fi OpenWrt") + Fleet Add Sheet + DeviceAccessDialog
      await visitAndCapture('/plugins', '62_plugins_library.png');
      await visitAndCapture('/apps/7', '63_openwrt_fleet_items.png');

      if (find.textContaining('Equipamentos (').evaluate().isNotEmpty) {
        await tester.tap(find.textContaining('Equipamentos (').first);
        await _pumpFrames(tester);
        await _savePng(
          tester,
          boundaryKey,
          printsDir,
          '64_openwrt_fleet_members.png',
          saved,
        );

        final addFleetBtn =
            find.textContaining('Adicionar (').evaluate().isNotEmpty
            ? find.textContaining('Adicionar (')
            : find.textContaining('Adicionar equipamento');
        if (addFleetBtn.evaluate().isNotEmpty) {
          await tester.tap(addFleetBtn.first);
          await _pumpFrames(tester);
          await _savePng(
            tester,
            boundaryKey,
            printsDir,
            '65_openwrt_fleet_add_sheet.png',
            saved,
          );
          if (find.byIcon(Icons.close).evaluate().isNotEmpty) {
            await tester.tap(find.byIcon(Icons.close).last);
          } else {
            Navigator.of(tester.element(find.byType(Scaffold).last)).pop();
          }
          await _pumpFrames(tester);
        }

        if (find.text('Configurar acesso').evaluate().isNotEmpty) {
          await tester.tap(find.text('Configurar acesso').first);
          await _pumpFrames(tester);
          await _savePng(
            tester,
            boundaryKey,
            printsDir,
            '66_device_access_dialog_openwrt.png',
            saved,
          );
          if (find.byIcon(Icons.close).evaluate().isNotEmpty) {
            await tester.tap(find.byIcon(Icons.close).last);
          } else if (find.text('Cancelar').evaluate().isNotEmpty) {
            await tester.tap(find.text('Cancelar').last);
          } else {
            Navigator.of(tester.element(find.byType(Dialog).last)).pop();
          }
          await _pumpFrames(tester);
        }
      }

      // Light-theme Dashboard verification print
      themeMode.value = ThemeMode.light;
      await visitAndCapture('/', '67_dashboard_light.png');

      expect(saved.length, greaterThanOrEqualTo(55));
    },
  );
}
