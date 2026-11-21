import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:netmonitor_app/features/core/models/log_models.dart';
import 'package:netmonitor_app/features/core/providers/logs_provider.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/features/network/widgets/device_form_sheet.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class _StubSitesNotifier extends SitesNotifier {
  @override
  SitesState build() => const SitesState();

  @override
  Future<void> fetchSites() async {}
}

class _StubNetworksNotifier extends NetworksNotifier {
  @override
  NetworksState build() => const NetworksState();

  @override
  Future<void> fetchNetworks() async {}
}

class _StubDevicesNotifier extends DevicesNotifier {
  @override
  DevicesState build() => const DevicesState();

  @override
  Future<void> fetchDevices() async {}
}

class _StubOperatingSystemsNotifier extends OperatingSystemsNotifier {
  _StubOperatingSystemsNotifier({this.nextIdentifyResult});

  static const List<OperatingSystemOption> _defaultOptions =
      <OperatingSystemOption>[
        OperatingSystemOption(
          id: 'routeros',
          label: 'MikroTik RouterOS',
          supportsSyslog: true,
        ),
        OperatingSystemOption(
          id: 'linux',
          label: 'Linux (rsyslog)',
          supportsSyslog: true,
        ),
        OperatingSystemOption(
          id: 'cisco_ios',
          label: 'Cisco IOS',
          supportsSyslog: true,
        ),
      ];

  IdentifyResultModel? nextIdentifyResult;
  int identifyCalls = 0;

  @override
  List<OperatingSystemOption> build() => _defaultOptions;

  @override
  Future<void> fetchAll({bool force = false}) async {}

  @override
  Future<IdentifyResultModel?> identify({
    String? name,
    required String? ipAddress,
    String? snmpVersion,
    String? snmpCommunity,
    String? vendor,
    String? model,
  }) async {
    identifyCalls++;
    return nextIdentifyResult;
  }
}

class _StubLogsNotifier extends LogsNotifier {
  _StubLogsNotifier(this.guide);

  final SetupGuide guide;
  int fetchGuideCalls = 0;

  @override
  LogsState build() => const LogsState();

  @override
  Future<SetupGuide?> fetchSetupGuide({String? address}) async {
    fetchGuideCalls++;
    state = state.copyWith(setupGuide: guide);
    return guide;
  }
}

class _StubSnmpProfilesNotifier extends SnmpProfilesNotifier {
  _StubSnmpProfilesNotifier({
    required this.testResult,
    required this.queriedInterfaces,
  });

  final SnmpTestResult testResult;
  final List<DeviceInterfaceModel> queriedInterfaces;

  int testCalls = 0;
  int queryCalls = 0;
  String? lastQueryVersion;
  String? lastQueryCommunity;

  @override
  List<SnmpProfileModel> build() => const <SnmpProfileModel>[];

  @override
  Future<void> fetchProfiles() async {}

  @override
  Future<SnmpTestResult> testSnmp({
    required String host,
    int port = 161,
    String version = 'v2c',
    String community = 'public',
    bool autoDetect = false,
  }) async {
    testCalls++;
    return testResult;
  }

  @override
  Future<List<DeviceInterfaceModel>> queryInterfaces({
    required String host,
    int port = 161,
    String version = 'v2c',
    String community = 'public',
  }) async {
    queryCalls++;
    lastQueryVersion = version;
    lastQueryCommunity = community;
    return queriedInterfaces;
  }
}

const SetupGuide _sampleSyslogGuide = SetupGuide(
  serverAddress: '10.0.0.10',
  port: 514,
  snippets: <SetupSnippet>[
    SetupSnippet(
      id: 'routeros',
      label: 'MikroTik RouterOS',
      commands:
          '/system logging action add name=netmonitor target=remote remote=10.0.0.10 remote-port=514 bsd-syslog=yes',
      notes: 'Cole no Terminal do RouterOS.',
    ),
    SetupSnippet(
      id: 'linux',
      label: 'Linux (rsyslog)',
      commands: '*.* @10.0.0.10:514',
      notes: 'Adicione em /etc/rsyslog.d/90-netmonitor.conf.',
    ),
    SetupSnippet(
      id: 'cisco_ios',
      label: 'Cisco IOS',
      commands: 'logging host 10.0.0.10 transport udp port 514',
    ),
  ],
);

void main() {
  group('DeviceInterfaceModel SNMP parsing & sorting', () {
    test(
      'parses backend SnmpInterface JSON (camelCase, bps speed, string status)',
      () {
        final up10g = DeviceInterfaceModel.fromJson(const <String, dynamic>{
          'ifIndex': 1,
          'ifName': 'sfp-sfpplus1',
          'ifDescr': 'sfp-sfpplus1',
          'ifAlias': 'WAN Fibra Principal',
          'ifType': 6,
          'ifSpeed': 10000000000,
          'ifAdminStatus': 'up',
          'ifOperStatus': 'up',
          'macAddress': 'aa:bb:cc:dd:ee:01',
          'isMonitored': false,
        });

        expect(up10g.ifIndex, 1);
        expect(up10g.displayName, 'sfp-sfpplus1');
        expect(up10g.subtitleText, 'WAN Fibra Principal');
        expect(up10g.isUp, isTrue);
        expect(up10g.operStatus, 'up');
        expect(up10g.adminStatus, 'up');
        expect(up10g.speed, 10000000000);
        expect(
          SnmpUplinkInterfaceSelector.formatNegotiatedSpeed(up10g),
          '10 Gbps',
        );

        final downPort = DeviceInterfaceModel.fromJson(const <String, dynamic>{
          'ifIndex': 2,
          'ifName': 'ether2',
          'ifDescr': 'ether2',
          'ifSpeed': 0,
          'ifAdminStatus': 'up',
          'ifOperStatus': 'down',
        });

        expect(downPort.isUp, isFalse);
        expect(downPort.operStatus, 'down');
        expect(downPort.subtitleText, isNull);
        expect(
          SnmpUplinkInterfaceSelector.formatNegotiatedSpeed(downPort),
          'Sem link',
        );
      },
    );

    test(
      'normalizes numeric MIB status (1=up, 2=down) and highSpeed/speedMbps fallback',
      () {
        final numericUp = DeviceInterfaceModel.fromJson(const <String, dynamic>{
          'ifIndex': 5,
          'ifName': 'eth0',
          'ifOperStatus': 1,
          'ifAdminStatus': 1,
          'ifHighSpeed': 2500,
        });
        expect(numericUp.operStatus, 'up');
        expect(numericUp.adminStatus, 'up');
        expect(numericUp.isUp, isTrue);
        expect(numericUp.speed, 2500000000);
        expect(
          SnmpUplinkInterfaceSelector.formatNegotiatedSpeed(numericUp),
          '2.5 Gbps',
        );

        final numericDown = DeviceInterfaceModel.fromJson(
          const <String, dynamic>{
            'ifIndex': 6,
            'name': 'eth1',
            'operStatus': '2',
            'speedMbps': 100,
          },
        );
        expect(numericDown.operStatus, 'down');
        expect(numericDown.isUp, isFalse);
        expect(numericDown.speed, 100000000);
        expect(
          SnmpUplinkInterfaceSelector.formatNegotiatedSpeed(numericDown),
          '100 Mbps',
        );
      },
    );

    test(
      'sortForUplinkSelection places UP + highest speed first and deduplicates names',
      () {
        final unsorted = <DeviceInterfaceModel>[
          const DeviceInterfaceModel(
            id: 0,
            deviceId: 0,
            ifIndex: 1,
            ifName: 'ether2',
            operStatus: 'down',
            speed: 1000000000,
          ),
          const DeviceInterfaceModel(
            id: 0,
            deviceId: 0,
            ifIndex: 2,
            ifName: 'ether3',
            operStatus: 'up',
            speed: 100000000,
          ),
          const DeviceInterfaceModel(
            id: 0,
            deviceId: 0,
            ifIndex: 3,
            ifName: 'ether1',
            operStatus: 'up',
            speed: 1000000000,
          ),
          const DeviceInterfaceModel(
            id: 0,
            deviceId: 0,
            ifIndex: 4,
            ifName: 'sfp1',
            operStatus: 'up',
            speed: 10000000000,
          ),
          const DeviceInterfaceModel(
            id: 0,
            deviceId: 0,
            ifIndex: 5,
            ifName: 'ether1',
            operStatus: 'down',
            speed: 0,
          ),
          const DeviceInterfaceModel(
            id: 0,
            deviceId: 0,
            ifIndex: 6,
            ifName: 'ether4',
            operStatus: 'down',
            speed: 0,
          ),
        ];

        final sorted = DeviceInterfaceModel.sortForUplinkSelection(unsorted);
        expect(
          sorted.map((i) => i.displayName).toList(),
          equals(<String>['sfp1', 'ether1', 'ether3', 'ether2', 'ether4']),
        );
        expect(sorted[1].isUp, isTrue);
      },
    );
  });

  group('DeviceSyslogSetupPanel OS recipe resolution', () {
    test('matches snippet by OS id, label, or vendor alias', () {
      expect(
        DeviceSyslogSetupPanel.resolveSnippetIndex(
          _sampleSyslogGuide.snippets,
          osId: 'linux',
        ),
        1,
      );
      expect(
        DeviceSyslogSetupPanel.resolveSnippetIndex(
          _sampleSyslogGuide.snippets,
          osId: 'cisco_ios',
        ),
        2,
      );
      expect(
        DeviceSyslogSetupPanel.resolveSnippetIndex(
          _sampleSyslogGuide.snippets,
          vendorHint: 'MikroTik',
        ),
        0,
      );
    });
  });

  group('SnmpUplinkInterfaceSelector widget (360px mobile & custom value)', () {
    testWidgets(
      'renders Material 3 select with UP/DOWN badges & canonical speeds at 360px with zero overflow',
      (tester) async {
        tester.view.physicalSize = const Size(360, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        final ctrl = TextEditingController(text: 'custom-wan0');
        addTearDown(ctrl.dispose);

        final interfaces = <DeviceInterfaceModel>[
          const DeviceInterfaceModel(
            id: 0,
            deviceId: 0,
            ifIndex: 1,
            ifName: 'ether1',
            alias: 'Link Fibra Corporativa Operadora Principal',
            operStatus: 'up',
            speed: 1000000000,
          ),
          const DeviceInterfaceModel(
            id: 0,
            deviceId: 0,
            ifIndex: 2,
            ifName: 'ether2',
            operStatus: 'up',
            speed: 100000000,
          ),
          const DeviceInterfaceModel(
            id: 0,
            deviceId: 0,
            ifIndex: 3,
            ifName: 'ether3',
            operStatus: 'down',
            speed: 0,
          ),
        ];

        var manualMode = false;

        await tester.pumpWidget(
          MaterialApp(
            theme: AppTheme.dark(),
            home: StatefulBuilder(
              builder: (context, setState) {
                return Scaffold(
                  body: Padding(
                    padding: const EdgeInsets.all(20),
                    child: NestedPanel(
                      child: SnmpUplinkInterfaceSelector(
                        controller: ctrl,
                        interfaces: interfaces,
                        loading: false,
                        manualMode: manualMode,
                        onManualModeChanged: (v) =>
                            setState(() => manualMode = v),
                        onSelected: (_) => setState(() {}),
                      ),
                    ),
                  ),
                );
              },
            ),
          ),
        );
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(find.text('custom-wan0'), findsOneWidget);
        expect(find.text('ATUAL'), findsOneWidget);
        expect(find.text('2 UP'), findsOneWidget);
        expect(find.text('1 DOWN'), findsOneWidget);

        await tester.tap(find.byType(AppDropdownFormField<String>));
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(find.text('UP'), findsAtLeastNWidgets(2));
        expect(find.text('DOWN'), findsAtLeastNWidgets(1));
        expect(find.text('1 Gbps'), findsAtLeastNWidgets(1));
        expect(find.text('100 Mbps'), findsAtLeastNWidgets(1));
        expect(find.text('Sem link'), findsAtLeastNWidgets(1));
        expect(find.text('Informar manualmente...'), findsAtLeastNWidgets(1));

        await tester.tap(find.text('1 Gbps').last);
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(ctrl.text, 'ether1');

        await tester.tap(find.byType(AppDropdownFormField<String>));
        await tester.pumpAndSettle();
        await tester.tap(find.text('Informar manualmente...').last);
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(manualMode, isTrue);
        expect(find.byType(TextFormField), findsOneWidget);
        expect(find.text('Selecionar da lista (3)'), findsOneWidget);
      },
    );
  });

  group('DeviceFormSheet SNMP, Syslog & smart OS auto-identify (360px)', () {
    testWidgets(
      'auto-identifies OS on open, auto-lists SNMP interfaces after test, and shows OS-reactive Syslog snippet with zero overflow',
      (tester) async {
        tester.view.physicalSize = const Size(360, 844);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        final osStub = _StubOperatingSystemsNotifier(
          nextIdentifyResult: const IdentifyResultModel(
            operatingSystem: 'routeros',
            source: 'snmp',
            vendor: 'MikroTik',
            model: 'RB5009',
            evidence: 'RouterOS v7',
          ),
        );
        final logsStub = _StubLogsNotifier(_sampleSyslogGuide);
        final snmpStub = _StubSnmpProfilesNotifier(
          testResult: const SnmpTestResult(
            responded: true,
            version: 'v2c',
            community: 'public',
            sysName: 'MikroTik-Core',
            sysDescr: 'RouterOS RB5009',
          ),
          queriedInterfaces: const <DeviceInterfaceModel>[
            DeviceInterfaceModel(
              id: 0,
              deviceId: 0,
              ifIndex: 1,
              ifName: 'ether2',
              operStatus: 'down',
              speed: 0,
            ),
            DeviceInterfaceModel(
              id: 0,
              deviceId: 0,
              ifIndex: 2,
              ifName: 'ether1',
              alias: 'WAN Fibra',
              operStatus: 'up',
              speed: 1000000000,
            ),
            DeviceInterfaceModel(
              id: 0,
              deviceId: 0,
              ifIndex: 3,
              ifName: 'ether3',
              operStatus: 'up',
              speed: 100000000,
            ),
          ],
        );

        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              sitesProvider.overrideWith(_StubSitesNotifier.new),
              networksProvider.overrideWith(_StubNetworksNotifier.new),
              devicesProvider.overrideWith(_StubDevicesNotifier.new),
              operatingSystemsProvider.overrideWith(() => osStub),
              logsProvider.overrideWith(() => logsStub),
              snmpProfilesProvider.overrideWith(() => snmpStub),
            ],
            child: MaterialApp(
              theme: AppTheme.dark(),
              home: Builder(
                builder: (context) => Scaffold(
                  body: Center(
                    child: FilledButton(
                      onPressed: () => showDeviceFormSheet(
                        context: context,
                        prefill: const <String, dynamic>{
                          'name': 'Core-GW',
                          'ipAddress': '10.0.0.1',
                        },
                      ),
                      child: const Text('Abrir'),
                    ),
                  ),
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();

        await tester.tap(find.text('Abrir'));
        await tester.pumpAndSettle();

        // Identificação automática de SO rodou ao abrir com prefill.
        expect(tester.takeException(), isNull);
        expect(osStub.identifyCalls, 1);
        expect(find.textContaining('Detectado: routeros'), findsOneWidget);

        // Habilita SNMP
        final snmpSwitch = find.widgetWithText(
          SwitchListTile,
          'Habilitar SNMP',
        );
        await tester.ensureVisible(snmpSwitch);
        await tester.tap(snmpSwitch);
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(find.text('Versão SNMP'), findsOneWidget);
        expect(find.text('Community SNMP'), findsOneWidget);
        expect(find.text('Intervalo (s)'), findsOneWidget);
        expect(find.text('Testar SNMP'), findsOneWidget);
        expect(find.text('Listar interfaces'), findsOneWidget);

        expect(
          find.widgetWithText(TextFormField, 'Interface de uplink / link'),
          findsOneWidget,
        );

        // Clica em "Testar SNMP" -> lista interfaces automaticamente + re-identifica SO
        final testBtn = find.text('Testar SNMP');
        await tester.ensureVisible(testBtn);
        await tester.tap(testBtn);
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(snmpStub.testCalls, 1);
        expect(snmpStub.queryCalls, 1);
        expect(snmpStub.lastQueryVersion, 'v2c');
        expect(snmpStub.lastQueryCommunity, 'public');
        expect(osStub.identifyCalls, greaterThanOrEqualTo(2));

        // Campo de uplink agora é select com UP/DOWN e velocidades canônicas
        final uplinkSelect = find.byWidgetPredicate(
          (w) =>
              w is AppDropdownFormField<String> &&
              w.decoration.labelText == 'Interface de uplink / link',
        );
        await tester.ensureVisible(uplinkSelect);
        expect(uplinkSelect, findsOneWidget);
        expect(find.text('2 UP'), findsOneWidget);
        expect(find.text('1 DOWN'), findsOneWidget);

        await tester.tap(uplinkSelect);
        await tester.pumpAndSettle();
        expect(tester.takeException(), isNull);
        expect(find.text('1 Gbps'), findsAtLeastNWidgets(1));
        expect(find.text('100 Mbps'), findsAtLeastNWidgets(1));
        expect(find.text('Sem link'), findsAtLeastNWidgets(1));
        await tester.tap(find.text('1 Gbps').last);
        await tester.pumpAndSettle();

        // Habilita "Receber logs (Syslog)" -> exibe endereço + receita RouterOS via CodeViewer
        final syslogSwitch = find.widgetWithText(
          SwitchListTile,
          'Receber logs (Syslog)',
        );
        await tester.ensureVisible(syslogSwitch);
        await tester.tap(syslogSwitch);
        await tester.pumpAndSettle();

        expect(tester.takeException(), isNull);
        expect(logsStub.fetchGuideCalls, 1);
        expect(find.text('10.0.0.10:514'), findsOneWidget);
        expect(find.byType(CodeViewer), findsOneWidget);
        expect(find.textContaining('bsd-syslog=yes'), findsOneWidget);
      },
    );

    testWidgets(
      'debounced auto-identify respects manual OS selection and never overwrites it',
      (tester) async {
        tester.view.physicalSize = const Size(390, 844);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.resetPhysicalSize);
        addTearDown(tester.view.resetDevicePixelRatio);

        final osStub = _StubOperatingSystemsNotifier(
          nextIdentifyResult: const IdentifyResultModel(
            operatingSystem: 'routeros',
            source: 'heuristic',
          ),
        );
        final logsStub = _StubLogsNotifier(_sampleSyslogGuide);
        final snmpStub = _StubSnmpProfilesNotifier(
          testResult: const SnmpTestResult(responded: true),
          queriedInterfaces: const <DeviceInterfaceModel>[],
        );

        await tester.pumpWidget(
          ProviderScope(
            overrides: <Override>[
              sitesProvider.overrideWith(_StubSitesNotifier.new),
              networksProvider.overrideWith(_StubNetworksNotifier.new),
              devicesProvider.overrideWith(_StubDevicesNotifier.new),
              operatingSystemsProvider.overrideWith(() => osStub),
              logsProvider.overrideWith(() => logsStub),
              snmpProfilesProvider.overrideWith(() => snmpStub),
            ],
            child: MaterialApp(
              theme: AppTheme.dark(),
              home: Builder(
                builder: (context) => Scaffold(
                  body: Center(
                    child: FilledButton(
                      onPressed: () => showDeviceFormSheet(context: context),
                      child: const Text('Abrir'),
                    ),
                  ),
                ),
              ),
            ),
          ),
        );
        await tester.pumpAndSettle();
        await tester.tap(find.text('Abrir'));
        await tester.pumpAndSettle();

        // Sem prefill, não chama identify no open.
        expect(osStub.identifyCalls, 0);

        // Seleciona manualmente Linux (rsyslog)
        final osDropdown = find.byWidgetPredicate(
          (w) =>
              w is AppDropdownFormField<String?> &&
              w.decoration.labelText == 'Sistema operacional',
        );
        await tester.ensureVisible(osDropdown);
        await tester.tap(osDropdown);
        await tester.pumpAndSettle();
        await tester.tap(find.text('Linux (rsyslog)').last);
        await tester.pumpAndSettle();

        // Digita IP e aguarda debounce (> 600ms): NÃO deve disparar auto-identify!
        final ipField = find.widgetWithText(TextFormField, 'Endereço IP');
        await tester.ensureVisible(ipField);
        await tester.enterText(ipField, '192.168.1.1');
        await tester.pump(const Duration(milliseconds: 700));
        await tester.pumpAndSettle();

        expect(osStub.identifyCalls, 0);
        expect(find.text('Linux (rsyslog)'), findsOneWidget);
      },
    );
  });
}
