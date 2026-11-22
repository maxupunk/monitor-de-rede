import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/providers/ai_providers.dart';
import 'package:netmonitor_app/features/ai/utils/ai_chat_logic.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugin_apps_provider.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/features/plugins/utils/plugin_settings_logic.dart';
import 'package:netmonitor_app/features/plugins/utils/wifi_qr_logic.dart';
import 'package:netmonitor_app/features/vpn/models/vpn_models.dart';
import 'package:netmonitor_app/features/vpn/providers/vpn_provider.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

void main() {
  group('VPN models, CIDR helpers & SSE realtime', () {
    test('detects one-time private key availability in VpnArtifact', () {
      const fresh = VpnArtifact(
        peerId: 1,
        profile: 'android',
        label: 'Android',
        delivery: 'qr',
        fileName: 'wg0.conf',
        language: 'ini',
        content: '[Interface]\nPrivateKey = secret-key-123\n',
        supportsQrCode: true,
      );
      const consumed = VpnArtifact(
        peerId: 1,
        profile: 'android',
        label: 'Android',
        delivery: 'qr',
        fileName: 'wg0.conf',
        language: 'ini',
        content: '[Interface]\nPrivateKey = CHAVE-PRIVADA-INDISPONIVEL\n',
        supportsQrCode: true,
      );

      expect(fresh.hasPrivateKey, isTrue);
      expect(consumed.hasPrivateKey, isFalse);
    });

    test('parses IPv4 CIDR, detects overlap and suggests free ranges', () {
      expect(parseIpv4Cidr('10.8.0.0/24'), isNotNull);
      expect(parseIpv4Cidr('999.8.0.0/24'), isNull);
      expect(cidrOverlaps('10.8.0.0/24', '10.8.0.128/25'), isTrue);
      expect(cidrOverlaps('10.8.0.0/24', '10.9.0.0/24'), isFalse);

      final free = freeVpnCidrs(const <String>['10.8.0.0/24', '10.9.0.0/24']);
      expect(free, isNot(contains('10.8.0.0/24')));
      expect(free, isNot(contains('10.9.0.0/24')));
      expect(free, contains('10.10.0.0/24'));
    });

    test('VpnNotifier applies vpn:peers_updated SSE payload in memory', () {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      final notifier = container.read(vpnProvider.notifier);
      notifier.state = const VpnState(
        peers: <VpnPeer>[
          VpnPeer(
            id: 42,
            vpnServerId: 1,
            deviceId: 7,
            publicKey: 'pk-1',
            deviceProfile: 'linux',
            persistentKeepalive: 25,
            bytesRx: 100,
            bytesTx: 200,
            enabled: true,
            connectionStatus: VpnConnectionStatus.awaiting,
          ),
        ],
      );

      notifier.applyRealtimePeers(<String, dynamic>{
        'peers': <Map<String, dynamic>>[
          <String, dynamic>{
            'id': 42,
            'connectionStatus': 'connected',
            'lastHandshakeAt': '2026-11-20T12:00:00Z',
            'lastSeenAt': '2026-11-20T12:01:00Z',
            'bytesRx': 4096,
            'bytesTx': 8192,
            'needsFirewallHint': true,
            'pingOutsideTunnel': false,
            'pingMonitorId': 99,
          },
        ],
      });

      final updated = container.read(vpnProvider).peers.first;
      expect(updated.connectionStatus, VpnConnectionStatus.connected);
      expect(updated.bytesRx, 4096);
      expect(updated.bytesTx, 8192);
      expect(updated.needsFirewallHint, isTrue);
      expect(updated.pingMonitorId, 99);
    });
  });

  group(
    'AI SSE-over-POST stream logic, mentions, screen disambiguation & Decider',
    () {
      test('handles all 10 SSE-over-POST frame types accurately', () {
        var msg = const AiDisplayMessage(
          id: 'a-1',
          role: 'assistant',
          content: '',
          isStreaming: true,
        );

        msg = applyChatEvent(msg, const <String, dynamic>{
          'type': 'textDelta',
          'content': 'Verificando ',
        });
        msg = applyChatEvent(msg, const <String, dynamic>{
          'type': 'toolCall',
          'id': 'c1',
          'name': 'list_alerts',
          'arguments': <String, dynamic>{'active': true},
        });
        msg = applyChatEvent(msg, const <String, dynamic>{
          'type': 'toolResult',
          'id': 'c1',
          'result': <String, dynamic>{'count': 2},
        });
        msg = applyChatEvent(msg, const <String, dynamic>{
          'type': 'textDelta',
          'content': 'Texto alucinado',
        });
        msg = applyChatEvent(msg, const <String, dynamic>{'type': 'retract'});
        expect(msg.content, 'Verificando ');

        msg = applyChatEvent(msg, const <String, dynamic>{
          'type': 'textDelta',
          'content': 'há 2 alertas ativos.',
        });
        msg = applyChatEvent(msg, const <String, dynamic>{
          'type': 'confirmationRequired',
          'id': 'c2',
          'name': 'device_ssh_exec',
          'arguments': <String, dynamic>{'cmd': 'reboot'},
          'summary': 'ALTERA O EQUIPAMENTO: reiniciar roteador',
          'risk': 'high',
          'reason': 'Reinício interrompe tráfego',
        });
        msg = applyChatEvent(msg, const <String, dynamic>{
          'type': 'notice',
          'key': 'ctx',
          'message': 'Janela pequena',
          'settings': true,
        });
        msg = applyChatEvent(msg, const <String, dynamic>{
          'type': 'usage',
          'promptTokens': 500,
          'completionTokens': 100,
          'contextTokens': 600,
          'contextWindow': 16384,
          'generationMs': 2000,
          'model': 'netmonitor/chat:q8_0',
        });
        msg = applyChatEvent(msg, const <String, dynamic>{'type': 'done'});

        expect(msg.isStreaming, isFalse);
        expect(msg.content, 'Verificando há 2 alertas ativos.');
        expect(msg.toolCalls.length, 2);
        expect(msg.toolCalls[0].status, AiToolStatus.done);
        expect(msg.toolCalls[1].status, AiToolStatus.awaiting);
        expect(msg.toolCalls[1].risk, AiToolRisk.high);
        expect(isDeviceAccessTool(msg.toolCalls[1].name), isTrue);
        expect(msg.notices.length, 1);
        expect(msg.notices.first.settings, isTrue);
        expect(tokensPerSecond(msg.usage), closeTo(50.0, 0.1));
      });

      test(
        'applies contextCompacted and builds trimmed history with decision notes',
        () {
          const m1 = AiDisplayMessage(id: 'u1', role: 'user', content: 'Oi');
          const m2 = AiDisplayMessage(
            id: 'a1',
            role: 'assistant',
            content: 'Olá',
          );
          const m3 = AiDisplayMessage(
            id: 'a2',
            role: 'assistant',
            content: 'Pronto',
            toolCalls: <AiToolCallState>[
              AiToolCallState(
                id: 'c9',
                name: 'silence_alert',
                summary: 'Silenciar alerta #12',
                status: AiToolStatus.cancelled,
              ),
            ],
          );

          final compacted = applyCompaction(
            const <AiDisplayMessage>[m1, m2, m3],
            const <String>['u1', 'a1', 'a2'],
            const <String, dynamic>{
              'foldedMessages': 2,
              'summary': 'Resumo inicial',
              'tokensBefore': 4000,
              'tokensAfter': 900,
            },
          );

          expect(compacted[1].compaction?.summary, 'Resumo inicial');
          final history = buildChatHistory(compacted);
          expect(history.summary, 'Resumo inicial');
          expect(history.messages.length, 1);
          expect(
            history.messages.first['content'],
            contains('[Ação cancelada pelo usuário: Silenciar alerta #12]'),
          );
        },
      );

      test(
        'mentions parser and ambiguous Portuguese screen disambiguation',
        () {
          expect(AiMentionKind.source.tone, SemanticTone.info);
          expect(AiMentionKind.source.tone, isNot(SemanticTone.plugin));

          final q = findMentionQuery('verifique @core-sw', 18);
          expect(q, isNotNull);
          expect(q!.query, 'core-sw');
          final inserted = insertMention(
            'verifique @core-sw',
            q,
            18,
            'Core-SW-01',
          );
          expect(inserted.text, 'verifique @Core-SW-01 ');

          const prevScreen = AskedScreen(
            id: 'alerts',
            label: 'Alertas',
            icon: 'mdi-bell',
          );
          const currScreen = AskedScreen(
            id: 'vpn',
            label: 'VPN',
            icon: 'mdi-shield',
          );
          const messages = <AiDisplayMessage>[
            AiDisplayMessage(
              id: 'u1',
              role: 'user',
              content: 'Ver alertas',
              screen: prevScreen,
            ),
          ];

          expect(
            screenToConfirm(
              currentScreen: currScreen,
              messages: messages,
              text: 'e isso, é grave?',
              hasMentions: false,
            )?.length,
            2,
          );
          expect(
            screenToConfirm(
              currentScreen: currScreen,
              messages: messages,
              text: 'e isso aqui, é grave?',
              hasMentions: false,
            ),
            isNull,
          );
        },
      );

      test('Decider local host, model install matching and SSE approvals', () {
        expect(isLocalOllaya('http://ollaya:11435'), isTrue);
        expect(isLocalOllaya('http://127.0.0.1:11435'), isTrue);
        expect(isLocalOllaya('https://ollaya.remote.example'), isFalse);
        expect(isModelInstalled(const <String>['laya:latest'], 'laya'), isTrue);
        expect(
          isModelInstalled(const <String>['laya:q4_k_m'], 'laya:q8_0'),
          isFalse,
        );

        final container = ProviderContainer();
        addTearDown(container.dispose);
        final approvals = container.read(aiApprovalsProvider.notifier);
        approvals.applyPending(const <String, dynamic>{
          'callId': 'call-1',
          'userId': 5,
          'tool': 'device_ssh_exec',
          'summary': 'Executar comando',
          'risk': 'high',
        });
        expect(container.read(aiApprovalsProvider).mine(5).length, 1);
        expect(container.read(aiApprovalsProvider).mine(9).length, 0);
        approvals.applyResolved(const <String, dynamic>{
          'callId': 'call-1',
          'decision': 'confirmed',
        });
        expect(container.read(aiApprovalsProvider).pending, isEmpty);
      });
    },
  );

  group('Plugins Wi-Fi QR, Schema Forms, Quarantine Review & SSE', () {
    test(
      'builds ZXing Wi-Fi QR payload with escaping and OWE/nopass rules',
      () {
        expect(wifiQrSecurity('owe'), 'nopass');
        expect(wifiNeedsPassword('owe-mixed'), isFalse);
        expect(wifiQrSecurity('psk2'), 'WPA');
        expect(
          wifiQrPayload(
            ssid: 'Escritório;Lab',
            encryption: 'psk2',
            password: 'a:b"c',
            hidden: true,
          ),
          r'WIFI:T:WPA;S:Escritório\;Lab;P:a\:b\"c;H:true;;',
        );
      },
    );

    test('validates plugin schema widgets, visibleWhen and normalization', () {
      expect(validatePluginWidget('ip', '10.0.0.1'), isNull);
      expect(validatePluginWidget('ip', 'nao-e-ip'), isNotNull);
      expect(validatePluginWidget('cidr', '192.168.1.0/24'), isNull);
      expect(validatePluginWidget('mac', 'AA:BB:CC:DD:EE:FF'), isNull);
      expect(validatePluginWidget('port', '443'), isNull);
      expect(validatePluginWidget('port', '70000'), isNotNull);
      expect(validatePluginWidget('url', 'https://netmonitor.local'), isNull);

      final schema = <String, dynamic>{
        'order': <String>['mode', 'password', 'port'],
        'required': <String>['mode'],
        'properties': <String, dynamic>{
          'port': <String, dynamic>{'type': 'integer', 'widget': 'port'},
          'mode': <String, dynamic>{'type': 'string', 'default': 'open'},
          'password': <String, dynamic>{
            'type': 'string',
            'secret': true,
            'visibleWhen': <String, dynamic>{'field': 'mode', 'equals': 'wpa'},
          },
        },
      };

      final fields = parsePluginSchemaFields(schema);
      expect(fields.map((f) => f.name).toList(), <String>[
        'mode',
        'password',
        'port',
      ]);

      final normalizedOpen = normalizePluginFormParams(
        schema,
        <String, dynamic>{
          'mode': 'open',
          'password': 'should-be-stripped',
          'port': '22',
        },
      );
      expect(normalizedOpen.containsKey('password'), isFalse);
      expect(normalizedOpen['port'], 22);
    });

    test(
      'enforces quarantine security gates & SSE plugin approvals/batches',
      () {
        const critical = ReviewReport(risk: 'critical');
        const highNoAi = ReviewReport(risk: 'high', ai: null);
        const lowWithAi = ReviewReport(
          risk: 'low',
          ai: PluginAiReview(summary: 'Seguro'),
        );
        expect(critical.isBlocked, isTrue);
        expect(highNoAi.requiresAcknowledgement, isTrue);
        expect(lowWithAi.isBlocked, isFalse);
        expect(lowWithAi.requiresAcknowledgement, isFalse);

        final container = ProviderContainer();
        addTearDown(container.dispose);

        final plugins = container.read(pluginsProvider.notifier);
        plugins.applyApprovalRequired(const <String, dynamic>{
          'key': 'k-1',
          'runId': 11,
          'deviceId': 3,
          'deviceName': 'AP-Sala',
          'pluginName': 'OpenWrt Wi-Fi',
          'action': 'set_ssid',
          'request': <String, dynamic>{
            'transport': 'ssh',
            'effect': 'write',
            'summary': 'uci set wireless.default.ssid=Corp',
          },
          'expiresAt': '2026-11-20T12:10:00Z',
        });
        expect(container.read(pluginsProvider).nextApproval?.isWrite, isTrue);
        plugins.applyApprovalResolved(const <String, dynamic>{'key': 'k-1'});
        expect(container.read(pluginsProvider).nextApproval, isNull);

        final apps = container.read(pluginAppsProvider.notifier);
        apps.applyRunStep(const <String, dynamic>{
          'runId': 11,
          'entry': <String, dynamic>{
            'seq': 1,
            'kind': 'step:ssh',
            'request': 'Aplicando SSID',
            'durationMs': 45,
          },
        });
        expect(
          container.read(pluginAppsProvider).lastSteps[11]?.request,
          'Aplicando SSID',
        );
      },
    );
  });
}
