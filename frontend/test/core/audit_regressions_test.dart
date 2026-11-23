import 'dart:convert';
import 'dart:typed_data';
import 'dart:ui';

import 'package:dio/dio.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/core/auth/auth_storage.dart';
import 'package:netmonitor_app/core/network/api_client.dart';
import 'package:netmonitor_app/core/network/sse_client.dart';
import 'package:netmonitor_app/core/network/sse_transport.dart';
import 'package:netmonitor_app/features/ai/models/ai_benchmark_models.dart';
import 'package:netmonitor_app/features/ai/models/ai_chat_models.dart';
import 'package:netmonitor_app/features/ai/models/ai_decider_models.dart';
import 'package:netmonitor_app/features/core/models/alert_models.dart';
import 'package:netmonitor_app/features/core/models/dashboard_models.dart';
import 'package:netmonitor_app/features/core/models/monitor_models.dart';
import 'package:netmonitor_app/features/core/models/settings_models.dart';
import 'package:netmonitor_app/features/network/models/link_test_models.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/providers/link_tests_provider.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_manifest_models.dart';
import 'package:netmonitor_app/features/servers/models/agent_models.dart';
import 'package:netmonitor_app/features/servers/models/database_models.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:shared_preferences/shared_preferences.dart';

class _MockAdapter implements HttpClientAdapter {
  _MockAdapter(this.handler);

  final ResponseBody Function(RequestOptions options) handler;

  @override
  Future<ResponseBody> fetch(
    RequestOptions options,
    Stream<Uint8List>? requestStream,
    Future<void>? cancelFuture,
  ) async => handler(options);

  @override
  void close({bool force = false}) {}
}

class _StubSseTransport implements SseTransport {
  void Function(Object)? _onError;
  int connectCalls = 0;

  @override
  void connect({
    required Uri url,
    Map<String, String> headers = const <String, String>{},
    required VoidCallback onOpen,
    required void Function(String rawData) onMessage,
    required void Function(Object error) onError,
  }) {
    connectCalls++;
    _onError = onError;
  }

  void triggerError(Object err) => _onError?.call(err);

  @override
  void close() {}
}

void main() {
  group('SSE & ApiClient regressions', () {
    test(
      'SseEvent.tryParseJson preserves flat JSON payloads (Syslog live tail)',
      () {
        final evt = SseEvent.tryParseJson(
          '{"id":42,"sourceIp":"192.168.1.1","message":"link up","severity":6}',
        );
        expect(evt, isNotNull);
        expect(evt!.data['id'], 42);
        expect(evt.data['message'], 'link up');
      },
    );

    test(
      'SseClient stops reconnecting and calls onUnauthorized on HTTP 401',
      () async {
        final transport = _StubSseTransport();
        var unauthorized = 0;
        final client = SseClient(
          transportFactory: () => transport,
          backoffCalculator: (_) => const Duration(milliseconds: 5),
          onUnauthorized: () => unauthorized++,
        );
        client.connect(
          urlBuilder: () => Uri.parse('http://127.0.0.1/api/events/stream'),
        );
        expect(transport.connectCalls, 1);

        transport.triggerError(const SseHttpException(401));
        await Future<void>.delayed(const Duration(milliseconds: 25));

        expect(unauthorized, 1);
        expect(client.state, SseConnectionState.disconnected);
        expect(transport.connectCalls, 1);
        client.dispose();
      },
    );

    test(
      'ApiClient handles 204 for List<dynamic> and ignores malformed NDJSON lines',
      () async {
        final dio = Dio(BaseOptions(validateStatus: (_) => true))
          ..httpClientAdapter = _MockAdapter(
            (_) => ResponseBody.fromString('', 204),
          );
        final client = ApiClient(
          baseUrlProvider: () => 'http://127.0.0.1/api',
          tokenProvider: () => 'tok',
          onUnauthorized: () {},
          dio: dio,
        );
        final list = await client.get<List<dynamic>>('/plugins');
        expect(list, isEmpty);

        final drained = drainNdjson(
          '{"ok":true}\nnot-valid-json\n{"step":2}\n{"partial":',
        );
        expect(drained.events.length, 2);
        expect(drained.events.first['ok'], isTrue);
        expect(drained.remainder, '{"partial":');
      },
    );

    test(
      'ApiClient expires session on NetMonitor JWT 401 even under /plugins',
      () async {
        var unauthorized = 0;
        final dio = Dio(BaseOptions(validateStatus: (_) => true))
          ..httpClientAdapter = _MockAdapter(
            (_) => ResponseBody.fromString(
              jsonEncode(<String, dynamic>{'message': 'Não autenticado'}),
              401,
              headers: <String, List<String>>{
                Headers.contentTypeHeader: <String>[Headers.jsonContentType],
              },
            ),
          );
        final client = ApiClient(
          baseUrlProvider: () => 'http://127.0.0.1/api',
          tokenProvider: () => 'expired',
          onUnauthorized: () => unauthorized++,
          dio: dio,
        );
        await expectLater(
          () => client.get<List<dynamic>>('/plugins'),
          throwsA(isA<ApiException>()),
        );
        expect(unauthorized, 1);
      },
    );
  });

  group('Core, Network, Servers, AI & Plugins DTO contracts', () {
    test(
      'AlertRuleCondition normalizes operators and preserves log_pattern metadata',
      () {
        final cond = AlertRuleCondition.fromJson(<String, dynamic>{
          'field': 'logMatchCount',
          'operator': '==',
          'value': 3,
          'pattern': '(?i)auth failure',
          'windowSeconds': 300,
        });
        expect(cond.operator, 'eq');
        expect(cond.pattern, '(?i)auth failure');
        expect(cond.windowSeconds, 300);
        expect(cond.toJson()['operator'], 'eq');
        expect(cond.toJson()['pattern'], '(?i)auth failure');
      },
    );

    test(
      'MonitorModel keeps null deviceId and reads lastRunAt; SaasPresetItem reads isProvisioned',
      () {
        final mon = MonitorModel.fromJson(<String, dynamic>{
          'id': 7,
          'deviceId': null,
          'name': 'Google',
          'type': 'https',
          'target': 'https://google.com',
          'lastRunAt': '2026-11-23T12:00:00Z',
        });
        expect(mon.deviceId, isNull);
        expect(mon.lastCheckedAt, '2026-11-23T12:00:00Z');

        final preset = SaasPresetItem.fromJson(<String, dynamic>{
          'id': 'google-http',
          'name': 'Google',
          'checkType': 'http',
          'isProvisioned': true,
        });
        expect(preset.type, 'http');
        expect(preset.provisioned, isTrue);

        final bucket = HourlyEventBucket.fromJson(<String, dynamic>{
          'hour': 14,
          'label': '14:00',
          'total': 5,
        });
        expect(bucket.hour, '14:00');

        final push = PushStatusModel.fromJson(<String, dynamic>{
          'enabled': true,
          'configured': true,
          'userSubscriptions': 2,
          'totalSubscriptions': 4,
        });
        expect(push.subscriptionCount, 2);
      },
    );

    test(
      'Docker & Deploy models match Rust backend JSON shapes without TypeError',
      () {
        final detail = DockerContainerDetail.fromJson(<String, dynamic>{
          'id': 'abc',
          'name': '/nginx',
          'image': 'nginx:latest',
          'hostConfig': <String, dynamic>{
            'restartPolicy': <String, dynamic>{
              'name': 'unless-stopped',
              'maximumRetryCount': 3,
            },
          },
          'mounts': <Map<String, dynamic>>[
            <String, dynamic>{
              'type': 'bind',
              'source': '/a',
              'destination': '/b',
              'readWrite': false,
            },
          ],
        });
        expect(detail.restartPolicy, 'unless-stopped (máx. 3)');
        expect(detail.mounts.single.rw, isFalse);

        final port = DockerPortBinding.fromJson(<String, dynamic>{
          'privatePort': 53,
          'protocol': 'udp',
        });
        expect(port.type, 'udp');

        final gh = DeployGithubConnectionView.fromJson(<String, dynamic>{
          'id': 1,
          'account': 'acme',
          'organization': true,
          'appSlug': 'netmonitor-acme',
        });
        expect(gh.organization, isTrue);

        const img = DeployBuildImage(
          name: 'web',
          context: '.',
          dockerfile: 'Dockerfile',
          image: 'registry.local/web:latest',
        );
        expect(img.toJson()['reference'], 'registry.local/web:latest');
        expect(
          DeployBuildImage.fromJson(<String, dynamic>{
            'name': 'web',
            'reference': 'registry.local/web:1',
          }).image,
          'registry.local/web:1',
        );

        expect(
          DeployGuideView.fromJson(<String, dynamic>{
            'done': true,
            'steps': const <dynamic>[],
          }).complete,
          isTrue,
        );
        expect(
          DeployReadinessCheck.fromJson(<String, dynamic>{
            'level': 'ok',
            'label': 'Builder pronto',
          }).title,
          'Builder pronto',
        );

        final addr = ServerAddressOption.fromJson(<String, dynamic>{
          'id': 'lan',
          'label': 'Rede local',
          'kind': 'lan',
          'value': '192.168.1.10',
        });
        expect(addr.host, '192.168.1.10');

        final dbCont = DatabaseContainer.fromJson(<String, dynamic>{
          'hostKey': 'agent-2',
          'viaProbeId': 9,
          'containerId': 'c1',
          'containerName': 'pg',
          'engine': 'postgres',
        });
        expect(dbCont.probeId, 9);
      },
    );

    test(
      'Network, AI & Plugins DTO contracts parse backend shapes accurately',
      () {
        final runRes = LinkTestPluginRunResult.fromJson(<String, dynamic>{
          'run': <String, dynamic>{'id': 77, 'status': 'succeeded'},
        });
        expect(runRes.id, 77);
        expect(runRes.isFinished, isTrue);

        final spec = LinkTestSpecModel.fromJson(<String, dynamic>{
          'limitMbps': 100.0,
        });
        expect(spec.limitMbps, 100);
        expect(spec.toJson()['limitMbps'], isA<int>());

        final net = NetworkModel.fromJson(<String, dynamic>{
          'id': 1,
          'name': 'VLAN 10',
          'cidr': '10.0.10.0/24',
          'vlan': 10,
        });
        expect(net.vlanId, 10);

        final triage = DeciderTriage.fromJson(<String, dynamic>{
          'actionable': 12.0,
          'symptom': 84.0,
          'transient': 4.0,
          'suppressed': true,
        });
        expect(triage.classification, 'symptom');
        expect(triage.confidence, closeTo(0.84, 0.001));

        final group = DeciderGroupScore.fromJson(<String, dynamic>{
          'id': 'network',
          'probability': 0.91,
          'selected': true,
        });
        expect(group.group, 'network');
        expect(group.confidence, 91);

        final provider = DeciderProviderView.fromJson(<String, dynamic>{
          'id': 'ollama',
          'models': <Map<String, dynamic>>[
            <String, dynamic>{'name': 'qwen2.5:3b', 'description': 'Fast'},
          ],
        });
        expect(provider.models, <String>['qwen2.5:3b']);

        final split = ChatBenchmarkSplitView.fromJson(<String, dynamic>{
          'overall': 82.0,
          'overallLow': 76.0,
          'overallHigh': 88.0,
          'answer': 85.0,
          'tokensPerS': 42.5,
          'perfect': 0.7,
        });
        expect(split.ciLow, 76.0);
        expect(split.ciHigh, 88.0);
        expect(split.response, 85.0);
        expect(split.tokensPerSec, 42.5);
        expect(split.perfectRatio, 0.7);

        final digest = AiDigest.fromJson(<String, dynamic>{
          'text': 'Tudo estável.',
          'generatedAt': '2026-11-23T10:00:00Z',
          'periodHours': 24,
        });
        expect(digest.summary, 'Tudo estável.');
        expect(digest.period, '24h');
        expect(digest.createdAt, '2026-11-23T10:00:00Z');

        final report = PluginTestReport.fromJson(<String, dynamic>{
          'passed': false,
          'problems': <String>['Falta função detect'],
          'cases': <Map<String, dynamic>>[
            <String, dynamic>{'name': 'case1', 'passed': true},
            <String, dynamic>{
              'name': 'case2',
              'passed': false,
              'message': 'Falhou assert',
            },
          ],
        });
        expect(report.ok, isFalse);
        expect(report.passed, 1);
        expect(report.failed, 1);
        expect(report.total, 2);
        expect(report.cases[1].error, 'Falhou assert');
      },
    );

    test(
      'AuthStorage respects rememberMe=false without persisting to SharedPreferences',
      () async {
        SharedPreferences.setMockInitialValues(<String, Object>{});
        final storage = AuthStorage();
        await storage.save(
          token: 'ephemeral-tok',
          user: <String, dynamic>{'id': 1, 'email': 'a@b.com'},
          rememberMe: false,
        );
        final prefs = await SharedPreferences.getInstance();
        expect(prefs.getString('netmonitor_auth_token'), isNull);
        expect(prefs.getString('netmonitor_auth_user'), isNull);

        final loaded = await storage.load();
        expect(loaded.token, 'ephemeral-tok');
        expect(loaded.rememberMe, isFalse);
        await storage.clear();
      },
    );
  });
}
