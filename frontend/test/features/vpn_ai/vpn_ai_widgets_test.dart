import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/areas/composition_registries.dart';
import 'package:netmonitor_app/areas/vpn_area.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/features/ai/models/ai_models.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_tool_card.dart';
import 'package:netmonitor_app/features/ai/widgets/ai_tool_chart.dart';
import 'package:netmonitor_app/features/plugins/models/plugin_models.dart';
import 'package:netmonitor_app/features/plugins/providers/plugins_provider.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_approval_dialog.dart';
import 'package:netmonitor_app/features/plugins/widgets/plugin_review_dialog.dart';
import 'package:netmonitor_app/features/vpn/models/vpn_models.dart';
import 'package:netmonitor_app/features/vpn/widgets/vpn_script_viewer.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

Widget _wrap(Widget child, {List<Override> overrides = const <Override>[]}) {
  return ProviderScope(
    overrides: overrides,
    child: MaterialApp(
      theme: AppTheme.light(),
      darkTheme: AppTheme.dark(),
      home: Scaffold(body: child),
    ),
  );
}

void main() {
  group('ADR 017/019 & Plugin Color Token Contracts (vpnArea)', () {
    test('preserves exact routeMeta areas for VPN, AI (core) and Plugins (network)', () {
      expect(vpnArea.routeMeta['/vpn']?.area, ProductArea.vpn);
      expect(vpnArea.routeMeta['/ai-chat']?.area, ProductArea.core);
      expect(vpnArea.routeMeta['/ai-chat']?.fullBleed, isTrue);
      expect(vpnArea.routeMeta['/ai-benchmark']?.area, ProductArea.core);
      expect(vpnArea.routeMeta['/plugins']?.area, ProductArea.network);
      expect(vpnArea.routeMeta['/plugins']?.requiresAdmin, isTrue);
      expect(vpnArea.routeMeta['/apps']?.area, ProductArea.network);
    });

    test('AI settings cards stay visible even when VPN area is disabled', () {
      final onlyCore = <ProductArea>{ProductArea.core};
      final tabs = SettingsSectionRegistry.visibleTabs(onlyCore);
      final iaTab = tabs.where((t) => t.tab.value == 'ia').firstOrNull;
      expect(iaTab, isNotNull);
      expect(iaTab!.cards.length, 3);
    });

    test('DevicePluginsTab uses SemanticTone.plugin (teal) and never purple/primary', () {
      final pluginsTab = vpnArea.deviceDetailTabs.firstWhere(
        (tab) => tab.id == 'plugins',
      );
      expect(pluginsTab.area, ProductArea.network);
      expect(pluginsTab.tone, SemanticTone.plugin);
      expect(AppSemanticColors.light.plugin, const Color(0xFF00796B));
      expect(AppSemanticColors.dark.plugin, const Color(0xFF1DE9B6));
    });

    test(
      'registers extraSseBuilders and extraShellOverlays per ADR 017 area',
      () {
        expect(vpnArea.sseBuilder, isNotNull);
        expect(vpnArea.extraSseBuilders.containsKey(ProductArea.core), isTrue);
        expect(
          vpnArea.extraSseBuilders.containsKey(ProductArea.network),
          isTrue,
        );

        final overlaysCoreOnly = visibleShellOverlays(<ProductArea>{
          ProductArea.core,
        });
        expect(overlaysCoreOnly.length, greaterThanOrEqualTo(2));
      },
    );
  });

  group('Security & Interactive Widget Parity Tests', () {
    testWidgets(
      'AiToolCard shows DeviceAccess write hallucination warning & Approve/Reject',
      (tester) async {
        const tool = AiToolCallState(
          id: 'call-dev-1',
          name: 'device_ssh_exec',
          arguments: <String, dynamic>{'command': 'uci commit'},
          summary: 'ALTERA O EQUIPAMENTO: aplicar uci commit',
          status: AiToolStatus.awaiting,
          risk: AiToolRisk.high,
          reason: 'Modifica configuração ativa do roteador.',
        );

        await tester.pumpWidget(
          _wrap(
            const SingleChildScrollView(
              child: AiToolCard(messageId: 'm-1', tool: tool),
            ),
          ),
        );

        expect(
          find.textContaining('ALTERA a configuração do equipamento'),
          findsOneWidget,
        );
        expect(find.text('Risco alto'), findsOneWidget);
        expect(find.text('Confirmar'), findsOneWidget);
        expect(find.text('Cancelar'), findsOneWidget);
      },
    );

    testWidgets('AiToolChart renders fl_chart series and legend', (
      tester,
    ) async {
      const chart = AiChart(
        title: 'Latência do Gateway',
        subtitle: 'Última hora',
        unit: 'latency',
        avgValue: 12.5,
        series: <AiChartSeries>[
          AiChartSeries(
            id: 'rtt',
            label: 'RTT',
            points: <AiChartPoint>[
              AiChartPoint(time: '2026-11-20T10:00:00Z', value: 10),
              AiChartPoint(time: '2026-11-20T10:05:00Z', value: 15),
            ],
          ),
        ],
      );

      await tester.pumpWidget(
        _wrap(
          const Padding(
            padding: EdgeInsets.all(16),
            child: AiToolChart(chart: chart),
          ),
        ),
      );

      expect(find.text('Latência do Gateway'), findsOneWidget);
      expect(find.textContaining('Média:'), findsOneWidget);
    });

    testWidgets('PluginApprovalOverlay shows write access confirmation modal', (
      tester,
    ) async {
      final container = ProviderContainer();
      addTearDown(container.dispose);

      container.read(pluginsProvider.notifier).applyApprovalRequired(
        const <String, dynamic>{
          'key': 'appr-write-1',
          'runId': 22,
          'deviceId': 4,
          'deviceName': 'SW-Core',
          'pluginName': 'VLAN Manager',
          'action': 'create_vlan',
          'request': <String, dynamic>{
            'transport': 'ssh',
            'effect': 'write',
            'summary': 'interface vlan 50',
          },
          'expiresAt': '2026-11-20T12:30:00Z',
        },
      );

      await tester.pumpWidget(
        UncontrolledProviderScope(
          container: container,
          child: MaterialApp(
            theme: AppTheme.light(),
            home: const Scaffold(
              body: Stack(
                children: <Widget>[SizedBox.expand(), PluginApprovalOverlay()],
              ),
            ),
          ),
        ),
      );

      expect(find.text('Aprovar acesso ao equipamento'), findsOneWidget);
      expect(find.text('Altera o equipamento'), findsOneWidget);
      expect(find.text('interface vlan 50'), findsOneWidget);
      expect(find.text('Aprovar'), findsOneWidget);
      expect(find.text('Negar'), findsOneWidget);
    });

    testWidgets('PluginReviewDialog blocks installation on critical risk', (
      tester,
    ) async {
      const detail = PluginDetail(
        summary: PluginSummary(
          id: 99,
          slug: 'evil-plugin',
          name: 'Plugin Suspeito',
          version: '1.0.0',
          status: 'quarantine',
        ),
        package: PluginPackage(manifest: <String, dynamic>{}, script: ''),
        review: ReviewReport(
          risk: 'critical',
          staticFindings: <ReviewFinding>[
            ReviewFinding(
              severity: 'critical',
              message: 'Tentativa de exfiltração',
              line: 4,
            ),
          ],
        ),
      );

      await tester.pumpWidget(
        _wrap(
          Builder(
            builder: (context) => FilledButton(
              onPressed: () =>
                  showPluginReviewDialog(context: context, detail: detail),
              child: const Text('Abrir revisão'),
            ),
          ),
        ),
      );

      await tester.tap(find.text('Abrir revisão'));
      await tester.pumpAndSettle();

      expect(find.textContaining('BLOQUEADO'), findsOneWidget);
      final installBtn = tester.widget<FilledButton>(
        find.widgetWithText(FilledButton, 'Instalar como rascunho'),
      );
      expect(installBtn.onPressed, isNull);
    });

    testWidgets(
      'VpnScriptViewer shows one-time key warning and hides script when consumed',
      (tester) async {
        const consumed = VpnArtifact(
          peerId: 10,
          profile: 'android',
          label: 'Android',
          delivery: 'qr',
          fileName: 'wg0.conf',
          language: 'ini',
          content: '[Interface]\nPrivateKey = CHAVE-PRIVADA-INDISPONIVEL',
          supportsQrCode: true,
          summary: <VpnArtifactSummaryItem>[
            VpnArtifactSummaryItem(label: 'IP VPN', value: '10.8.0.2/32'),
          ],
        );

        await tester.pumpWidget(
          _wrap(
            Consumer(
              builder: (context, ref, _) => FilledButton(
                onPressed: () => showVpnScriptViewer(
                  context: context,
                  ref: ref,
                  artifact: consumed,
                ),
                child: const Text('Abrir cofre'),
              ),
            ),
          ),
        );

        await tester.tap(find.text('Abrir cofre'));
        await tester.pumpAndSettle();

        expect(find.text('Chave privada já entregue'), findsOneWidget);
        expect(find.text('Gerar novas chaves'), findsOneWidget);
        expect(find.text('Copiar tudo'), findsNothing);
      },
    );
  });
}
