import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/form_validators.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/models/log_models.dart';
import 'package:netmonitor_app/features/core/providers/logs_provider.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

const List<(String, String)> deviceTypeOptions = <(String, String)>[
  ('router', 'Roteador'),
  ('switch', 'Switch'),
  ('access_point', 'Access Point'),
  ('firewall', 'Firewall'),
  ('server', 'Servidor'),
  ('workstation', 'Estação de trabalho'),
  ('nas', 'NAS / Storage'),
  ('printer', 'Impressora'),
  ('camera', 'Câmera'),
  ('iot', 'IoT'),
  ('vm', 'Máquina Virtual'),
  ('other', 'Outro'),
];

String deviceTypeLabel(String type) {
  for (final item in deviceTypeOptions) {
    if (item.$1 == type) return item.$2;
  }
  return type;
}

IconData deviceTypeIcon(String type) {
  return switch (type.toLowerCase()) {
    'router' => Icons.router_outlined,
    'switch' => Icons.hub_outlined,
    'access_point' => Icons.wifi_tethering,
    'firewall' => Icons.security_outlined,
    'server' => Icons.dns_outlined,
    'workstation' => Icons.computer_outlined,
    'nas' => Icons.storage_outlined,
    'printer' => Icons.print_outlined,
    'camera' => Icons.videocam_outlined,
    'iot' => Icons.sensors_outlined,
    'vm' => Icons.cloud_queue_outlined,
    _ => Icons.devices_other_outlined,
  };
}

/// Abre o formulário adaptativo (bottom sheet em mobile / dialog em desktop)
/// para criar, editar ou adotar um dispositivo descoberto.
Future<DeviceModel?> showDeviceFormSheet({
  required BuildContext context,
  DeviceModel? existing,
  Map<String, dynamic>? prefill,
}) {
  return showAdaptiveFormSheet<DeviceModel>(
    context: context,
    title: existing != null ? 'Editar dispositivo' : 'Novo dispositivo',
    subtitle: existing != null
        ? existing.name
        : 'Cadastre identidade, rede, sistema e telemetria SNMP.',
    maxWidth: 680,
    builder: (ctx) => _DeviceFormBody(
      existing: existing,
      prefill: prefill,
      onDone: (saved) => Navigator.of(ctx).pop(saved),
    ),
  );
}

class _DeviceFormBody extends ConsumerStatefulWidget {
  const _DeviceFormBody({this.existing, this.prefill, required this.onDone});

  final DeviceModel? existing;
  final Map<String, dynamic>? prefill;
  final ValueChanged<DeviceModel> onDone;

  @override
  ConsumerState<_DeviceFormBody> createState() => _DeviceFormBodyState();
}

class _DeviceFormBodyState extends ConsumerState<_DeviceFormBody> {
  final _formKey = GlobalKey<FormState>();

  late final TextEditingController _nameCtrl;
  late final TextEditingController _ipCtrl;
  late final TextEditingController _macCtrl;
  late final TextEditingController _vendorCtrl;
  late final TextEditingController _modelCtrl;
  late final TextEditingController _locationCtrl;
  late final TextEditingController _descCtrl;
  late final TextEditingController _communityCtrl;
  late final TextEditingController _pollIntervalCtrl;
  late final TextEditingController _linkIfaceCtrl;

  String _type = 'router';
  int? _siteId;
  int? _networkId;
  int? _parentId;
  String? _operatingSystem;
  String? _accessMode;
  bool _isMonitored = true;
  bool _snmpEnabled = false;
  bool _syslogEnabled = false;
  String _snmpVersion = 'v2c';

  bool _saving = false;
  bool _identifying = false;
  bool _autoIdentifying = false;
  bool _userPickedOs = false;
  bool _testingSnmp = false;
  bool _loadingIfaces = false;
  bool _loadingSyslogGuide = false;
  bool _ifacesQueried = false;
  bool _manualLinkIface = false;
  int? _manualSyslogSnippetIndex;
  String? _error;
  IdentifyResultModel? _identifyResult;
  SnmpTestResult? _snmpResult;
  MacVendorLookup? _macLookup;
  List<DeviceInterfaceModel> _discoveredIfaces = const <DeviceInterfaceModel>[];
  Timer? _autoIdentifyDebounce;
  String _lastAutoIdentifyKey = '';
  int _identifyRequestSeq = 0;

  @override
  void initState() {
    super.initState();
    final d = widget.existing;
    final p = widget.prefill ?? const <String, dynamic>{};

    _nameCtrl = TextEditingController(
      text: d?.name ?? (p['name'] as String?) ?? '',
    );
    _ipCtrl = TextEditingController(
      text: d?.ipAddress ?? (p['ipAddress'] as String?) ?? '',
    );
    _macCtrl = TextEditingController(
      text: d?.macAddress ?? (p['macAddress'] as String?) ?? '',
    );
    _vendorCtrl = TextEditingController(
      text: d?.vendor ?? (p['vendor'] as String?) ?? '',
    );
    _modelCtrl = TextEditingController(
      text: d?.model ?? (p['model'] as String?) ?? '',
    );
    _locationCtrl = TextEditingController(text: d?.location ?? '');
    _descCtrl = TextEditingController(text: d?.description ?? '');
    _communityCtrl = TextEditingController(text: d?.snmpCommunity ?? 'public');
    _pollIntervalCtrl = TextEditingController(
      text: (d?.snmpPollIntervalSeconds ?? 60).toString(),
    );
    _linkIfaceCtrl = TextEditingController(text: d?.linkInterfaceName ?? '');

    _type = d?.type ?? (p['type'] as String?) ?? 'router';
    if (!deviceTypeOptions.any((e) => e.$1 == _type)) _type = 'other';
    _siteId = d?.siteId ?? (p['siteId'] as int?);
    _networkId = d?.networkId ?? (p['networkId'] as int?);
    _parentId = d?.parentId;
    _operatingSystem = d?.operatingSystem;
    _userPickedOs =
        d?.operatingSystem != null && d!.operatingSystem!.trim().isNotEmpty;
    _accessMode = d?.accessMode;
    _isMonitored = d?.isMonitored ?? true;
    _snmpEnabled = d?.snmpEnabled ?? false;
    _snmpVersion = d?.snmpVersion ?? 'v2c';

    Future<void>.microtask(() {
      ref.read(sitesProvider.notifier).fetchSites();
      ref.read(networksProvider.notifier).fetchNetworks();
      ref.read(operatingSystemsProvider.notifier).fetchAll();
      ref.read(snmpProfilesProvider.notifier).fetchProfiles();
      if (_macCtrl.text.trim().length >= 6) _lookupMac(_macCtrl.text);
      if (widget.existing == null && !_userPickedOs && _hasIdentifySignals()) {
        _identifyDevice(automatic: true);
      }
    });
  }

  @override
  void dispose() {
    _autoIdentifyDebounce?.cancel();
    _nameCtrl.dispose();
    _ipCtrl.dispose();
    _macCtrl.dispose();
    _vendorCtrl.dispose();
    _modelCtrl.dispose();
    _locationCtrl.dispose();
    _descCtrl.dispose();
    _communityCtrl.dispose();
    _pollIntervalCtrl.dispose();
    _linkIfaceCtrl.dispose();
    super.dispose();
  }

  bool _hasIdentifySignals() {
    final ip = _ipCtrl.text.trim();
    final vendor = _vendorCtrl.text.trim();
    final model = _modelCtrl.text.trim();
    final macHex = _macCtrl.text.replaceAll(RegExp(r'[^0-9a-fA-F]'), '');
    return ip.isNotEmpty ||
        vendor.isNotEmpty ||
        model.isNotEmpty ||
        macHex.length >= 6;
  }

  String _identifySignature() {
    return <String>[
      _nameCtrl.text.trim().toLowerCase(),
      _ipCtrl.text.trim().toLowerCase(),
      _vendorCtrl.text.trim().toLowerCase(),
      _modelCtrl.text.trim().toLowerCase(),
      _snmpEnabled ? _snmpVersion : '',
      _snmpEnabled ? _communityCtrl.text.trim() : '',
    ].join('|');
  }

  void _scheduleAutoIdentify() {
    if (_userPickedOs) return;
    _autoIdentifyDebounce?.cancel();
    if (!_hasIdentifySignals()) return;
    _autoIdentifyDebounce = Timer(const Duration(milliseconds: 600), () {
      if (!mounted || _userPickedOs || !_hasIdentifySignals()) return;
      _identifyDevice(automatic: true);
    });
  }

  Future<void> _lookupMac(String mac) async {
    final result = await ref.read(vendorsProvider.notifier).lookup(mac);
    if (!mounted || result == null) return;
    setState(() {
      _macLookup = result;
      if (_vendorCtrl.text.trim().isEmpty && result.vendor != null) {
        _vendorCtrl.text = result.vendor!;
      }
      if (widget.existing == null &&
          result.hintDeviceType != null &&
          deviceTypeOptions.any((o) => o.$1 == result.hintDeviceType)) {
        _type = result.hintDeviceType!;
      }
    });
    _scheduleAutoIdentify();
  }

  Future<void> _identifyDevice({bool automatic = false}) async {
    if (automatic && _userPickedOs) return;
    final signature = _identifySignature();
    if (automatic && signature == _lastAutoIdentifyKey) return;
    if (automatic) _lastAutoIdentifyKey = signature;

    final requestSeq = ++_identifyRequestSeq;
    setState(() {
      if (automatic) {
        _autoIdentifying = true;
      } else {
        _identifying = true;
        _error = null;
      }
    });
    try {
      final res = await ref
          .read(operatingSystemsProvider.notifier)
          .identify(
            name: _nameCtrl.text.trim().isEmpty ? null : _nameCtrl.text.trim(),
            ipAddress: _ipCtrl.text.trim().isEmpty ? null : _ipCtrl.text.trim(),
            snmpVersion: _snmpEnabled ? _snmpVersion : null,
            snmpCommunity: _snmpEnabled ? _communityCtrl.text.trim() : null,
            vendor: _vendorCtrl.text.trim().isEmpty
                ? null
                : _vendorCtrl.text.trim(),
            model: _modelCtrl.text.trim().isEmpty
                ? null
                : _modelCtrl.text.trim(),
          );
      if (!mounted || requestSeq != _identifyRequestSeq) return;
      if (automatic && _userPickedOs) return;
      setState(() {
        _identifyResult = res;
        if (res?.operatingSystem != null) {
          _operatingSystem = res!.operatingSystem;
          _manualSyslogSnippetIndex = null;
        }
        if (_vendorCtrl.text.trim().isEmpty && res?.vendor != null) {
          _vendorCtrl.text = res!.vendor!;
        }
        if (_modelCtrl.text.trim().isEmpty && res?.model != null) {
          _modelCtrl.text = res!.model!;
        }
      });
    } catch (err) {
      if (!automatic && mounted && requestSeq == _identifyRequestSeq) {
        setState(() => _error = err.toString());
      }
    } finally {
      if (mounted && requestSeq == _identifyRequestSeq) {
        setState(() {
          if (automatic) {
            _autoIdentifying = false;
          } else {
            _identifying = false;
          }
        });
      }
    }
  }

  Future<void> _loadSyslogGuide({bool force = false}) async {
    if (!force && ref.read(logsProvider).setupGuide != null) return;
    setState(() => _loadingSyslogGuide = true);
    try {
      await ref.read(logsProvider.notifier).fetchSetupGuide();
    } finally {
      if (mounted) setState(() => _loadingSyslogGuide = false);
    }
  }

  Future<void> _testSnmp({required bool autoDetect}) async {
    final host = _ipCtrl.text.trim();
    if (host.isEmpty) {
      setState(() => _error = 'Informe o endereço IP antes de testar SNMP.');
      return;
    }
    setState(() {
      _testingSnmp = true;
      _error = null;
    });
    var shouldQueryInterfaces = false;
    try {
      final res = await ref
          .read(snmpProfilesProvider.notifier)
          .testSnmp(
            host: host,
            version: _snmpVersion,
            community: _communityCtrl.text.trim().isEmpty
                ? 'public'
                : _communityCtrl.text.trim(),
            autoDetect: autoDetect,
          );
      if (!mounted) return;
      setState(() {
        _snmpResult = res;
        if (res.responded) {
          _snmpEnabled = true;
          if (res.version != null) _snmpVersion = res.version!;
          if (res.community != null) _communityCtrl.text = res.community!;
          if (_nameCtrl.text.trim().isEmpty && res.sysName != null) {
            _nameCtrl.text = res.sysName!;
          }
        }
      });
      shouldQueryInterfaces = res.responded;
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _testingSnmp = false);
    }
    if (shouldQueryInterfaces && mounted) {
      await _queryLiveInterfaces();
      if (mounted && !_userPickedOs) {
        _lastAutoIdentifyKey = '';
        await _identifyDevice(automatic: true);
      }
    }
  }

  Future<void> _queryLiveInterfaces() async {
    final host = _ipCtrl.text.trim();
    if (host.isEmpty) {
      setState(
        () => _error = 'Informe o endereço IP antes de listar interfaces.',
      );
      return;
    }
    setState(() {
      _loadingIfaces = true;
      _error = null;
    });
    try {
      final list = await ref
          .read(snmpProfilesProvider.notifier)
          .queryInterfaces(
            host: host,
            version: _snmpVersion,
            community: _communityCtrl.text.trim().isEmpty
                ? 'public'
                : _communityCtrl.text.trim(),
          );
      if (mounted) {
        setState(() {
          _discoveredIfaces = list;
          _ifacesQueried = true;
          if (list.isNotEmpty) _manualLinkIface = false;
        });
      }
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _loadingIfaces = false);
    }
  }

  Future<void> _submit() async {
    if (!_formKey.currentState!.validate()) return;
    setState(() {
      _saving = true;
      _error = null;
    });

    final payload = <String, dynamic>{
      'name': _nameCtrl.text.trim(),
      'type': _type,
      if (_ipCtrl.text.trim().isNotEmpty) 'ipAddress': _ipCtrl.text.trim(),
      if (_macCtrl.text.trim().isNotEmpty) 'macAddress': _macCtrl.text.trim(),
      if (_vendorCtrl.text.trim().isNotEmpty) 'vendor': _vendorCtrl.text.trim(),
      if (_modelCtrl.text.trim().isNotEmpty) 'model': _modelCtrl.text.trim(),
      if (_locationCtrl.text.trim().isNotEmpty)
        'location': _locationCtrl.text.trim(),
      if (_descCtrl.text.trim().isNotEmpty)
        'description': _descCtrl.text.trim(),
      'siteId': _siteId,
      'networkId': _networkId,
      'parentId': _parentId,
      'operatingSystem': _operatingSystem,
      'accessMode': _accessMode,
      'isMonitored': _isMonitored,
      'snmpEnabled': _snmpEnabled,
      if (_snmpEnabled) ...<String, dynamic>{
        'snmpVersion': _snmpVersion,
        'snmpCommunity': _communityCtrl.text.trim().isEmpty
            ? 'public'
            : _communityCtrl.text.trim(),
        'snmpPollIntervalSeconds':
            int.tryParse(_pollIntervalCtrl.text.trim()) ?? 60,
      },
      if (_linkIfaceCtrl.text.trim().isNotEmpty)
        'linkInterfaceName': _linkIfaceCtrl.text.trim(),
    };

    final notifier = ref.read(devicesProvider.notifier);
    final saved = widget.existing != null
        ? await notifier.updateDevice(widget.existing!.id, payload)
        : await notifier.createDevice(payload);

    if (!mounted) return;
    setState(() => _saving = false);
    if (saved != null) {
      widget.onDone(saved);
    } else {
      setState(
        () => _error =
            ref.read(devicesProvider).error ?? 'Falha ao salvar dispositivo.',
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final sites = ref.watch(sitesProvider).items;
    final networks = ref.watch(networksProvider).items;
    final devices = ref
        .watch(devicesProvider)
        .items
        .where((d) => d.id != widget.existing?.id)
        .toList(growable: false);
    final systems = ref.watch(operatingSystemsProvider);
    final syslogGuide = ref.watch(logsProvider).setupGuide;
    final effectiveOsId =
        _operatingSystem ??
        _identifyResult?.operatingSystem ??
        widget.existing?.effectiveOperatingSystem;
    String? effectiveOsLabel;
    if (effectiveOsId != null) {
      for (final s in systems) {
        if (s.id == effectiveOsId) {
          effectiveOsLabel = s.label;
          break;
        }
      }
    }

    return Form(
      key: _formKey,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (_error != null) ...<Widget>[
            NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
            const SizedBox(height: 12),
          ],
          TextFormField(
            controller: _nameCtrl,
            decoration: const InputDecoration(
              labelText: 'Nome do dispositivo *',
            ),
            validator: FormValidators.required(
              'Informe o nome do dispositivo.',
            ),
          ),
          const SizedBox(height: 12),
          ResponsiveFieldRow(
            children: <Widget>[
              AppDropdownFormField<String>(
                initialValue: _type,
                decoration: const InputDecoration(labelText: 'Tipo'),
                items: deviceTypeOptions
                    .map(
                      (opt) => DropdownMenuItem(
                        value: opt.$1,
                        child: Text(opt.$2, overflow: TextOverflow.ellipsis),
                      ),
                    )
                    .toList(growable: false),
                onChanged: (v) {
                  if (v != null) setState(() => _type = v);
                },
              ),
              TextFormField(
                controller: _ipCtrl,
                decoration: const InputDecoration(labelText: 'Endereço IP'),
                validator: FormValidators.ipOrHost(),
                onChanged: (_) => _scheduleAutoIdentify(),
              ),
            ],
          ),
          const SizedBox(height: 12),
          TextFormField(
            controller: _macCtrl,
            decoration: InputDecoration(
              labelText: 'Endereço MAC',
              hintText: 'AA:BB:CC:DD:EE:FF',
              suffixIcon: IconButton(
                tooltip: 'Consultar fabricante (OUI)',
                icon: const Icon(Icons.search),
                onPressed: () => _lookupMac(_macCtrl.text),
              ),
            ),
            onChanged: (val) {
              final hex = val.replaceAll(RegExp(r'[^0-9a-fA-F]'), '');
              if (hex.length >= 6) _lookupMac(val);
            },
          ),
          if (_macLookup != null && _macLookup!.vendor != null) ...<Widget>[
            const SizedBox(height: 6),
            SemanticChip(
              label:
                  'OUI: ${_macLookup!.vendor} (${_macLookup!.source.toUpperCase()})',
              tone: SemanticTone.info,
              compact: true,
              icon: Icons.verified_outlined,
            ),
          ],
          const SizedBox(height: 12),
          ResponsiveFieldRow(
            children: <Widget>[
              AppDropdownFormField<int?>(
                initialValue: sites.any((s) => s.id == _siteId)
                    ? _siteId
                    : null,
                decoration: const InputDecoration(labelText: 'Site'),
                items: <DropdownMenuItem<int?>>[
                  const DropdownMenuItem<int?>(
                    value: null,
                    child: Text('Nenhum'),
                  ),
                  for (final s in sites)
                    DropdownMenuItem<int?>(
                      value: s.id,
                      child: Text(s.name, overflow: TextOverflow.ellipsis),
                    ),
                ],
                onChanged: (v) => setState(() => _siteId = v),
              ),
              AppDropdownFormField<int?>(
                initialValue: networks.any((n) => n.id == _networkId)
                    ? _networkId
                    : null,
                decoration: const InputDecoration(labelText: 'Rede IP'),
                items: <DropdownMenuItem<int?>>[
                  const DropdownMenuItem<int?>(
                    value: null,
                    child: Text('Nenhuma'),
                  ),
                  for (final n in networks)
                    DropdownMenuItem<int?>(
                      value: n.id,
                      child: Text(
                        '${n.name} (${n.cidr})',
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                ],
                onChanged: (v) => setState(() => _networkId = v),
              ),
            ],
          ),
          const SizedBox(height: 12),
          ResponsiveFieldRow(
            children: <Widget>[
              AppDropdownFormField<int?>(
                initialValue: devices.any((d) => d.id == _parentId)
                    ? _parentId
                    : null,
                decoration: const InputDecoration(
                  labelText: 'Dispositivo pai (hierarquia)',
                ),
                items: <DropdownMenuItem<int?>>[
                  const DropdownMenuItem<int?>(
                    value: null,
                    child: Text('Raiz / Nenhum'),
                  ),
                  for (final d in devices)
                    DropdownMenuItem<int?>(
                      value: d.id,
                      child: Text(d.name, overflow: TextOverflow.ellipsis),
                    ),
                ],
                onChanged: (v) => setState(() => _parentId = v),
              ),
              AppDropdownFormField<String?>(
                initialValue: _accessMode,
                decoration: const InputDecoration(labelText: 'Modo de acesso'),
                items: const <DropdownMenuItem<String?>>[
                  DropdownMenuItem<String?>(
                    value: null,
                    child: Text('Automático'),
                  ),
                  DropdownMenuItem<String?>(
                    value: 'local',
                    child: Text('Rede local'),
                  ),
                  DropdownMenuItem<String?>(value: 'vpn', child: Text('VPN')),
                  DropdownMenuItem<String?>(
                    value: 'remote',
                    child: Text('Sonda remota'),
                  ),
                ],
                onChanged: (v) => setState(() => _accessMode = v),
              ),
            ],
          ),
          const SizedBox(height: 12),
          ResponsiveFieldRow(
            children: <Widget>[
              TextFormField(
                controller: _vendorCtrl,
                decoration: const InputDecoration(labelText: 'Fabricante'),
                onChanged: (_) => _scheduleAutoIdentify(),
              ),
              TextFormField(
                controller: _modelCtrl,
                decoration: const InputDecoration(labelText: 'Modelo'),
                onChanged: (_) => _scheduleAutoIdentify(),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: <Widget>[
              Expanded(
                child: AppDropdownFormField<String?>(
                  key: ValueKey<String>(
                    'device-os-${_operatingSystem ?? 'auto'}-${systems.length}',
                  ),
                  initialValue: systems.any((s) => s.id == _operatingSystem)
                      ? _operatingSystem
                      : null,
                  decoration: const InputDecoration(
                    labelText: 'Sistema operacional',
                  ),
                  items: <DropdownMenuItem<String?>>[
                    const DropdownMenuItem<String?>(
                      value: null,
                      child: Text('Automático'),
                    ),
                    for (final s in systems)
                      DropdownMenuItem<String?>(
                        value: s.id,
                        child: Text(s.label, overflow: TextOverflow.ellipsis),
                      ),
                  ],
                  onChanged: (v) => setState(() {
                    _operatingSystem = v;
                    _userPickedOs = v != null;
                    _manualSyslogSnippetIndex = null;
                  }),
                ),
              ),
              const SizedBox(width: 8),
              OutlinedButton.icon(
                onPressed: _identifying ? null : () => _identifyDevice(),
                icon: _identifying
                    ? const SizedBox(
                        width: 14,
                        height: 14,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.auto_fix_high, size: 18),
                label: const Text('Identificar'),
              ),
            ],
          ),
          if (_autoIdentifying) ...<Widget>[
            const SizedBox(height: 6),
            Row(
              children: <Widget>[
                const SizedBox(
                  width: 12,
                  height: 12,
                  child: CircularProgressIndicator(strokeWidth: 2),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    'Identificando SO automaticamente...',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: context.textTheme.bodySmall?.copyWith(
                      color: context.colorScheme.onSurfaceVariant,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                ),
              ],
            ),
          ],
          if (_identifyResult != null) ...<Widget>[
            const SizedBox(height: 8),
            NestedPanel(
              tone: SemanticTone.info,
              padding: const EdgeInsets.all(10),
              child: Text(
                'Detectado: ${_identifyResult!.operatingSystem ?? 'desconhecido'}'
                '${_identifyResult!.source != null ? ' (fonte: ${_identifyResult!.source})' : ''}'
                '${_identifyResult!.evidence != null ? '\n${_identifyResult!.evidence}' : ''}',
                style: context.textTheme.bodySmall,
              ),
            ),
          ],
          const SizedBox(height: 12),
          NestedPanel(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Material(
                  type: MaterialType.transparency,
                  child: SwitchListTile(
                    contentPadding: EdgeInsets.zero,
                    title: const Text('Habilitar SNMP'),
                    subtitle: const Text(
                      'Coleta de interfaces, CPU, memória e sensores',
                    ),
                    value: _snmpEnabled,
                    onChanged: (v) {
                      setState(() => _snmpEnabled = v);
                      if (v) _scheduleAutoIdentify();
                    },
                  ),
                ),
                if (_snmpEnabled) ...<Widget>[
                  const SizedBox(height: 6),
                  ResponsiveFieldRow(
                    breakpoint: 480,
                    spacing: 12,
                    runSpacing: 12,
                    children: <Widget>[
                      AppDropdownFormField<String>(
                        initialValue: _snmpVersion,
                        decoration: const InputDecoration(
                          labelText: 'Versão SNMP',
                        ),
                        items: const <DropdownMenuItem<String>>[
                          DropdownMenuItem(value: 'v1', child: Text('SNMP v1')),
                          DropdownMenuItem(
                            value: 'v2c',
                            child: Text('SNMP v2c'),
                          ),
                          DropdownMenuItem(value: 'v3', child: Text('SNMP v3')),
                        ],
                        onChanged: (v) {
                          if (v != null) {
                            setState(() => _snmpVersion = v);
                            _scheduleAutoIdentify();
                          }
                        },
                      ),
                      TextFormField(
                        controller: _pollIntervalCtrl,
                        keyboardType: TextInputType.number,
                        decoration: const InputDecoration(
                          labelText: 'Intervalo (s)',
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 12),
                  TextFormField(
                    controller: _communityCtrl,
                    decoration: const InputDecoration(
                      labelText: 'Community SNMP',
                    ),
                    onChanged: (_) => _scheduleAutoIdentify(),
                  ),
                  const SizedBox(height: 12),
                  Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    children: <Widget>[
                      OutlinedButton.icon(
                        onPressed: _testingSnmp
                            ? null
                            : () => _testSnmp(autoDetect: false),
                        icon: _testingSnmp
                            ? const SizedBox(
                                width: 14,
                                height: 14,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : const Icon(Icons.network_check, size: 16),
                        label: const Text('Testar SNMP'),
                      ),
                      OutlinedButton.icon(
                        onPressed: _testingSnmp
                            ? null
                            : () => _testSnmp(autoDetect: true),
                        icon: const Icon(Icons.auto_awesome, size: 16),
                        label: const Text('Auto-detectar'),
                      ),
                      OutlinedButton.icon(
                        onPressed: _loadingIfaces ? null : _queryLiveInterfaces,
                        icon: _loadingIfaces
                            ? const SizedBox(
                                width: 14,
                                height: 14,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                ),
                              )
                            : const Icon(Icons.settings_ethernet, size: 16),
                        label: const Text('Listar interfaces'),
                      ),
                    ],
                  ),
                  if (_snmpResult != null) ...<Widget>[
                    const SizedBox(height: 8),
                    NestedPanel(
                      tone: _snmpResult!.responded
                          ? SemanticTone.success
                          : SemanticTone.error,
                      padding: const EdgeInsets.all(10),
                      child: Text(
                        _snmpResult!.responded
                            ? 'SNMP OK${_snmpResult!.sysName != null ? ' · ${_snmpResult!.sysName}' : ''}'
                                  '${_snmpResult!.sysDescr != null ? '\n${_snmpResult!.sysDescr}' : ''}'
                            : (_snmpResult!.message ?? 'Sem resposta SNMP.'),
                        style: context.textTheme.bodySmall,
                      ),
                    ),
                  ],
                  const SizedBox(height: 12),
                  SnmpUplinkInterfaceSelector(
                    controller: _linkIfaceCtrl,
                    interfaces: _discoveredIfaces,
                    loading: _loadingIfaces,
                    hasQueried: _ifacesQueried,
                    manualMode: _manualLinkIface,
                    onManualModeChanged: (manual) =>
                        setState(() => _manualLinkIface = manual),
                    onRequestInterfaces: _queryLiveInterfaces,
                    onSelected: (_) => setState(() {}),
                  ),
                ],
              ],
            ),
          ),
          const SizedBox(height: 12),
          DeviceSyslogSetupPanel(
            enabled: _syslogEnabled,
            loading: _loadingSyslogGuide,
            guide: syslogGuide,
            operatingSystemId: effectiveOsId,
            operatingSystemLabel: effectiveOsLabel,
            vendorHint: _vendorCtrl.text.trim().isEmpty
                ? null
                : _vendorCtrl.text.trim(),
            manualSnippetIndex: _manualSyslogSnippetIndex,
            onToggle: (v) {
              setState(() => _syslogEnabled = v);
              if (v) _loadSyslogGuide();
            },
            onSelectSnippetIndex: (idx) =>
                setState(() => _manualSyslogSnippetIndex = idx),
            onRetryLoad: () => _loadSyslogGuide(force: true),
          ),
          const SizedBox(height: 10),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Monitorar dispositivo'),
            value: _isMonitored,
            onChanged: (v) => setState(() => _isMonitored = v),
          ),
          TextFormField(
            controller: _locationCtrl,
            decoration: const InputDecoration(labelText: 'Localização física'),
          ),
          const SizedBox(height: 10),
          TextFormField(
            controller: _descCtrl,
            maxLines: 2,
            decoration: const InputDecoration(labelText: 'Observações'),
          ),
          const SizedBox(height: 16),
          Wrap(
            alignment: WrapAlignment.end,
            spacing: 8,
            runSpacing: 8,
            children: <Widget>[
              TextButton(
                onPressed: _saving ? null : () => Navigator.of(context).pop(),
                child: const Text('Cancelar'),
              ),
              FilledButton.icon(
                onPressed: _saving ? null : _submit,
                icon: _saving
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.check),
                label: Text(
                  widget.existing != null ? 'Salvar alterações' : 'Cadastrar',
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// Seletor coeso para a interface de uplink / entrada de link no cadastro SNMP.
///
/// Exibe um select Material 3 com status semântico (`UP`/`DOWN`) e velocidade
/// negociada (`formatLinkSpeed`) assim que as interfaces são descobertas,
/// preservando valores customizados existentes e permitindo alternância limpa
/// para digitação manual.
class SnmpUplinkInterfaceSelector extends StatelessWidget {
  const SnmpUplinkInterfaceSelector({
    super.key,
    required this.controller,
    required this.interfaces,
    required this.loading,
    required this.manualMode,
    required this.onManualModeChanged,
    this.hasQueried = false,
    this.onRequestInterfaces,
    this.onSelected,
  });

  static const String manualEntryValue = '__snmp_uplink_manual__';

  final TextEditingController controller;
  final List<DeviceInterfaceModel> interfaces;
  final bool loading;
  final bool hasQueried;
  final bool manualMode;
  final ValueChanged<bool> onManualModeChanged;
  final VoidCallback? onRequestInterfaces;
  final ValueChanged<String>? onSelected;

  /// Formata a velocidade negociada usando o formatador canônico `formatLinkSpeed`.
  static String formatNegotiatedSpeed(DeviceInterfaceModel iface) {
    final speed = iface.speed;
    if (speed == null || !speed.isFinite || speed <= 0) {
      return iface.isUp ? '—' : 'Sem link';
    }
    return formatLinkSpeed(speed);
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final sorted = DeviceInterfaceModel.sortForUplinkSelection(interfaces);
    final showSelect = sorted.isNotEmpty && !manualMode;
    final currentValue = controller.text.trim();
    final hasCustomValue =
        currentValue.isNotEmpty &&
        !sorted.any((iface) => iface.displayName == currentValue);
    final upCount = sorted.where((i) => i.isUp).length;
    final downCount = sorted.length - upCount;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (loading) ...<Widget>[
          Row(
            children: <Widget>[
              const SizedBox(
                width: 14,
                height: 14,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  'Consultando interfaces SNMP...',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: context.textTheme.bodySmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                    fontWeight: FontWeight.w500,
                  ),
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
        ],
        if (showSelect) ...<Widget>[
          AppDropdownFormField<String>(
            key: ValueKey<String>(
              'snmp-uplink-select-$currentValue-${sorted.length}',
            ),
            initialValue: currentValue,
            decoration: const InputDecoration(
              labelText: 'Interface de uplink / link',
            ),
            items: <DropdownMenuItem<String>>[
              DropdownMenuItem<String>(
                value: '',
                child: Text(
                  'Nenhuma (automático)',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(color: scheme.onSurfaceVariant),
                ),
              ),
              if (hasCustomValue)
                DropdownMenuItem<String>(
                  value: currentValue,
                  child: _CustomInterfaceOptionRow(name: currentValue),
                ),
              for (final iface in sorted)
                DropdownMenuItem<String>(
                  value: iface.displayName,
                  child: _SnmpInterfaceOptionRow(iface: iface),
                ),
              DropdownMenuItem<String>(
                value: manualEntryValue,
                child: Row(
                  children: <Widget>[
                    Icon(Icons.edit_outlined, size: 16, color: scheme.primary),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        'Informar manualmente...',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          color: scheme.primary,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ],
            onChanged: (value) {
              if (value == null) return;
              if (value == manualEntryValue) {
                onManualModeChanged(true);
                return;
              }
              controller.text = value;
              onSelected?.call(value);
            },
          ),
          const SizedBox(height: 6),
          Wrap(
            spacing: 10,
            runSpacing: 4,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              _InterfaceCountLegend(
                label: '$upCount UP',
                tone: SemanticTone.success,
              ),
              _InterfaceCountLegend(
                label: '$downCount DOWN',
                tone: SemanticTone.error,
              ),
            ],
          ),
        ] else ...<Widget>[
          TextFormField(
            controller: controller,
            decoration: InputDecoration(
              labelText: 'Interface de uplink / link',
              hintText: 'Ex.: ether1, wan0',
              suffixIcon:
                  sorted.isEmpty && onRequestInterfaces != null && !loading
                  ? IconButton(
                      tooltip: 'Listar interfaces SNMP',
                      icon: const Icon(Icons.settings_ethernet),
                      onPressed: onRequestInterfaces,
                    )
                  : null,
            ),
          ),
          const SizedBox(height: 6),
          if (manualMode && sorted.isNotEmpty)
            Align(
              alignment: AlignmentDirectional.centerStart,
              child: TextButton.icon(
                onPressed: () => onManualModeChanged(false),
                style: TextButton.styleFrom(
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 4,
                  ),
                  minimumSize: const Size(0, 32),
                  tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                ),
                icon: const Icon(Icons.list_alt, size: 16),
                label: Text('Selecionar da lista (${sorted.length})'),
              ),
            )
          else if (!loading)
            Text(
              hasQueried
                  ? 'Nenhuma interface SNMP retornada — informe manualmente.'
                  : 'Teste o SNMP ou clique em "Listar interfaces" para selecionar com status e velocidade.',
              style: context.textTheme.bodySmall?.copyWith(
                color: scheme.onSurfaceVariant,
              ),
            ),
        ],
      ],
    );
  }
}

class _SnmpInterfaceOptionRow extends StatelessWidget {
  const _SnmpInterfaceOptionRow({required this.iface});

  final DeviceInterfaceModel iface;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final subtitle = iface.subtitleText;
    final speedLabel = SnmpUplinkInterfaceSelector.formatNegotiatedSpeed(iface);

    return Row(
      children: <Widget>[
        _InterfaceStatusBadge(isUp: iface.isUp),
        const SizedBox(width: 8),
        Expanded(
          child: Text.rich(
            TextSpan(
              text: iface.displayName,
              style: TextStyle(
                fontWeight: FontWeight.w600,
                color: scheme.onSurface,
              ),
              children: subtitle != null
                  ? <InlineSpan>[
                      TextSpan(
                        text: ' · $subtitle',
                        style: TextStyle(
                          fontWeight: FontWeight.w400,
                          color: scheme.onSurfaceVariant,
                        ),
                      ),
                    ]
                  : null,
            ),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            softWrap: false,
          ),
        ),
        const SizedBox(width: 8),
        _InterfaceSpeedTag(
          label: speedLabel,
          isUp: iface.isUp,
          hasSpeed: iface.negotiatedSpeedBps > 0,
        ),
      ],
    );
  }
}

class _CustomInterfaceOptionRow extends StatelessWidget {
  const _CustomInterfaceOptionRow({required this.name});

  final String name;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final infoColors = context.semanticColors.resolve(
      SemanticTone.info,
      scheme,
    );

    return Row(
      children: <Widget>[
        Container(
          padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
          decoration: BoxDecoration(
            color: infoColors.container,
            borderRadius: BorderRadius.circular(6),
            border: Border.all(color: infoColors.border),
          ),
          child: Text(
            'ATUAL',
            style: TextStyle(
              fontSize: 10,
              fontWeight: FontWeight.w700,
              color: infoColors.onContainer,
              height: 1.1,
            ),
          ),
        ),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            name,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            softWrap: false,
            style: TextStyle(
              fontWeight: FontWeight.w600,
              color: scheme.onSurface,
            ),
          ),
        ),
      ],
    );
  }
}

class _InterfaceStatusBadge extends StatelessWidget {
  const _InterfaceStatusBadge({required this.isUp});

  final bool isUp;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(
      isUp ? SemanticTone.success : SemanticTone.error,
      scheme,
    );

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: colors.container,
        borderRadius: BorderRadius.circular(6),
        border: Border.all(color: colors.border),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Container(
            width: 6,
            height: 6,
            decoration: BoxDecoration(
              color: colors.foreground,
              shape: BoxShape.circle,
            ),
          ),
          const SizedBox(width: 4),
          Text(
            isUp ? 'UP' : 'DOWN',
            style: TextStyle(
              fontSize: 10.5,
              fontWeight: FontWeight.w800,
              color: colors.onContainer,
              height: 1.1,
            ),
          ),
        ],
      ),
    );
  }
}

class _InterfaceSpeedTag extends StatelessWidget {
  const _InterfaceSpeedTag({
    required this.label,
    required this.isUp,
    required this.hasSpeed,
  });

  final String label;
  final bool isUp;
  final bool hasSpeed;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final activeColors = context.semanticColors.resolve(
      SemanticTone.info,
      scheme,
    );
    final highlight = isUp && hasSpeed;

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
      decoration: BoxDecoration(
        color: highlight ? activeColors.container : scheme.surfaceContainerHigh,
        borderRadius: BorderRadius.circular(6),
        border: Border.all(
          color: highlight ? activeColors.border : scheme.outlineVariant,
        ),
      ),
      child: Text(
        label,
        maxLines: 1,
        softWrap: false,
        style: TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w700,
          color: highlight ? activeColors.onContainer : scheme.onSurfaceVariant,
          height: 1.1,
        ),
      ),
    );
  }
}

class _InterfaceCountLegend extends StatelessWidget {
  const _InterfaceCountLegend({required this.label, required this.tone});

  final String label;
  final SemanticTone tone;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final colors = context.semanticColors.resolve(tone, scheme);
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: <Widget>[
        Container(
          width: 7,
          height: 7,
          decoration: BoxDecoration(
            color: colors.foreground,
            shape: BoxShape.circle,
          ),
        ),
        const SizedBox(width: 5),
        Text(
          label,
          style: context.textTheme.labelSmall?.copyWith(
            color: scheme.onSurfaceVariant,
            fontWeight: FontWeight.w600,
          ),
        ),
      ],
    );
  }
}

/// Painel coeso "Receber logs (Syslog)" no cadastro/edição de dispositivo.
///
/// Reaproveita o guia canônico do servidor (`/api/logs/setup-snippet` via
/// `logsProvider`), exibe o destino Syslog (IP/host + porta) e seleciona
/// dinamicamente a receita de comandos para o sistema operacional detectado ou
/// escolhido pelo operador.
class DeviceSyslogSetupPanel extends StatelessWidget {
  const DeviceSyslogSetupPanel({
    super.key,
    required this.enabled,
    required this.loading,
    required this.onToggle,
    this.guide,
    this.operatingSystemId,
    this.operatingSystemLabel,
    this.vendorHint,
    this.manualSnippetIndex,
    this.onSelectSnippetIndex,
    this.onRetryLoad,
  });

  final bool enabled;
  final bool loading;
  final SetupGuide? guide;
  final String? operatingSystemId;
  final String? operatingSystemLabel;
  final String? vendorHint;
  final int? manualSnippetIndex;
  final ValueChanged<bool> onToggle;
  final ValueChanged<int?>? onSelectSnippetIndex;
  final VoidCallback? onRetryLoad;

  /// Resolve o índice da receita Syslog mais adequada ao SO / fabricante atual.
  static int? resolveSnippetIndex(
    List<SetupSnippet> snippets, {
    String? osId,
    String? osLabel,
    String? vendorHint,
  }) {
    if (snippets.isEmpty) return null;

    final idNeedle = osId?.trim().toLowerCase() ?? '';
    final labelNeedle = osLabel?.trim().toLowerCase() ?? '';
    final vendorNeedle = vendorHint?.trim().toLowerCase() ?? '';

    if (idNeedle.isNotEmpty) {
      for (var i = 0; i < snippets.length; i++) {
        if (snippets[i].id.trim().toLowerCase() == idNeedle) return i;
      }
      for (var i = 0; i < snippets.length; i++) {
        final combined = '${snippets[i].id} ${snippets[i].label}'.toLowerCase();
        if (combined.contains(idNeedle) ||
            _matchesOsAlias(idNeedle, combined)) {
          return i;
        }
      }
    }

    if (labelNeedle.isNotEmpty) {
      for (var i = 0; i < snippets.length; i++) {
        final label = snippets[i].label.toLowerCase();
        if (label.contains(labelNeedle) || labelNeedle.contains(label)) {
          return i;
        }
      }
    }

    if (vendorNeedle.isNotEmpty) {
      for (var i = 0; i < snippets.length; i++) {
        final combined = '${snippets[i].id} ${snippets[i].label}'.toLowerCase();
        if (combined.contains(vendorNeedle) ||
            _matchesOsAlias(vendorNeedle, combined)) {
          return i;
        }
      }
    }

    return 0;
  }

  static bool _matchesOsAlias(String needle, String haystack) {
    const aliasGroups = <List<String>>[
      <String>['mikrotik', 'routeros'],
      <String>['ubiquiti', 'unifi', 'edgeos', 'edgerouter', 'airos'],
      <String>['cisco', 'ios', 'nxos'],
      <String>['openwrt', 'lede'],
      <String>['linux', 'rsyslog', 'debian', 'ubuntu', 'systemd'],
      <String>['pfsense', 'opnsense', 'freebsd'],
      <String>['fortinet', 'fortigate', 'fortios'],
      <String>['huawei', 'vrp'],
      <String>['juniper', 'junos'],
    ];
    for (final group in aliasGroups) {
      final needleInGroup = group.any(needle.contains);
      if (needleInGroup && group.any(haystack.contains)) {
        return true;
      }
    }
    return false;
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final snippets = guide?.snippets ?? const <SetupSnippet>[];
    final autoIdx = resolveSnippetIndex(
      snippets,
      osId: operatingSystemId,
      osLabel: operatingSystemLabel,
      vendorHint: vendorHint,
    );
    final selectedIdx =
        (manualSnippetIndex != null &&
            manualSnippetIndex! >= 0 &&
            manualSnippetIndex! < snippets.length)
        ? manualSnippetIndex!
        : autoIdx;
    final activeSnippet = (selectedIdx != null && selectedIdx < snippets.length)
        ? snippets[selectedIdx]
        : null;

    return NestedPanel(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          Material(
            type: MaterialType.transparency,
            child: SwitchListTile(
              contentPadding: EdgeInsets.zero,
              title: const Text('Receber logs (Syslog)'),
              subtitle: const Text(
                'Endereço do servidor e comandos de envio por sistema operacional',
              ),
              value: enabled,
              onChanged: onToggle,
            ),
          ),
          if (enabled) ...<Widget>[
            const SizedBox(height: 8),
            if (loading && guide == null)
              Row(
                children: <Widget>[
                  const SizedBox(
                    width: 14,
                    height: 14,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      'Carregando endereço e comandos Syslog...',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.bodySmall?.copyWith(
                        color: scheme.onSurfaceVariant,
                      ),
                    ),
                  ),
                ],
              )
            else if (guide == null)
              Wrap(
                spacing: 8,
                runSpacing: 8,
                crossAxisAlignment: WrapCrossAlignment.center,
                children: <Widget>[
                  Text(
                    'Não foi possível carregar o guia Syslog do servidor.',
                    style: context.textTheme.bodySmall?.copyWith(
                      color: scheme.onSurfaceVariant,
                    ),
                  ),
                  if (onRetryLoad != null)
                    OutlinedButton.icon(
                      onPressed: onRetryLoad,
                      icon: const Icon(Icons.refresh, size: 16),
                      label: const Text('Tentar novamente'),
                    ),
                ],
              )
            else ...<Widget>[
              KeyValueRow(
                label: 'Destino Syslog (UDP)',
                value: '${guide!.serverAddress}:${guide!.port}',
                monospace: true,
                copyable: true,
              ),
              if (snippets.length > 1) ...<Widget>[
                const SizedBox(height: 10),
                AppDropdownFormField<int>(
                  key: ValueKey<String>(
                    'syslog-recipe-$selectedIdx-${snippets.length}',
                  ),
                  initialValue: selectedIdx ?? 0,
                  decoration: const InputDecoration(
                    labelText: 'Receita de configuração (SO)',
                  ),
                  items: <DropdownMenuItem<int>>[
                    for (var i = 0; i < snippets.length; i++)
                      DropdownMenuItem<int>(
                        value: i,
                        child: Text(
                          snippets[i].label,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                  ],
                  onChanged: (idx) => onSelectSnippetIndex?.call(idx),
                ),
              ],
              if (activeSnippet != null) ...<Widget>[
                const SizedBox(height: 10),
                CodeViewer(
                  title: activeSnippet.label,
                  code: activeSnippet.commands,
                  maxHeight: 200,
                ),
                if (activeSnippet.notes != null &&
                    activeSnippet.notes!.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 6),
                  Text(
                    activeSnippet.notes!,
                    style: context.textTheme.bodySmall?.copyWith(
                      color: scheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ] else ...<Widget>[
                const SizedBox(height: 8),
                Text(
                  'Configure o equipamento para enviar Syslog UDP para '
                  '${guide!.serverAddress}:${guide!.port}.',
                  style: context.textTheme.bodySmall?.copyWith(
                    color: scheme.onSurfaceVariant,
                  ),
                ),
              ],
            ],
          ],
        ],
      ),
    );
  }
}
