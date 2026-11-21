import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/form_validators.dart';
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
  String _snmpVersion = 'v2c';

  bool _saving = false;
  bool _identifying = false;
  bool _testingSnmp = false;
  bool _loadingIfaces = false;
  String? _error;
  IdentifyResultModel? _identifyResult;
  SnmpTestResult? _snmpResult;
  MacVendorLookup? _macLookup;
  List<DeviceInterfaceModel> _discoveredIfaces = const <DeviceInterfaceModel>[];

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
    });
  }

  @override
  void dispose() {
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
  }

  Future<void> _identifyDevice() async {
    setState(() {
      _identifying = true;
      _error = null;
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
      if (!mounted) return;
      setState(() {
        _identifyResult = res;
        if (res?.operatingSystem != null) {
          _operatingSystem = res!.operatingSystem;
        }
        if (_vendorCtrl.text.trim().isEmpty && res?.vendor != null) {
          _vendorCtrl.text = res!.vendor!;
        }
        if (_modelCtrl.text.trim().isEmpty && res?.model != null) {
          _modelCtrl.text = res!.model!;
        }
      });
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _identifying = false);
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
    } catch (err) {
      if (mounted) setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _testingSnmp = false);
    }
  }

  Future<void> _queryLiveInterfaces() async {
    final host = _ipCtrl.text.trim();
    if (host.isEmpty) return;
    setState(() => _loadingIfaces = true);
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
      if (mounted) setState(() => _discoveredIfaces = list);
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
          Row(
            children: <Widget>[
              Expanded(
                child: DropdownButtonFormField<String>(
                  initialValue: _type,
                  decoration: const InputDecoration(labelText: 'Tipo'),
                  items: deviceTypeOptions
                      .map(
                        (opt) => DropdownMenuItem(
                          value: opt.$1,
                          child: Text(opt.$2),
                        ),
                      )
                      .toList(growable: false),
                  onChanged: (v) {
                    if (v != null) setState(() => _type = v);
                  },
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: TextFormField(
                  controller: _ipCtrl,
                  decoration: const InputDecoration(labelText: 'Endereço IP'),
                  validator: FormValidators.ipOrHost(),
                ),
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
          Row(
            children: <Widget>[
              Expanded(
                child: DropdownButtonFormField<int?>(
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
                      DropdownMenuItem<int?>(value: s.id, child: Text(s.name)),
                  ],
                  onChanged: (v) => setState(() => _siteId = v),
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: DropdownButtonFormField<int?>(
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
                        child: Text('${n.name} (${n.cidr})'),
                      ),
                  ],
                  onChanged: (v) => setState(() => _networkId = v),
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Row(
            children: <Widget>[
              Expanded(
                child: DropdownButtonFormField<int?>(
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
                      DropdownMenuItem<int?>(value: d.id, child: Text(d.name)),
                  ],
                  onChanged: (v) => setState(() => _parentId = v),
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: DropdownButtonFormField<String?>(
                  initialValue: _accessMode,
                  decoration: const InputDecoration(
                    labelText: 'Modo de acesso',
                  ),
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
              ),
            ],
          ),
          const SizedBox(height: 12),
          Row(
            children: <Widget>[
              Expanded(
                child: TextFormField(
                  controller: _vendorCtrl,
                  decoration: const InputDecoration(labelText: 'Fabricante'),
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: TextFormField(
                  controller: _modelCtrl,
                  decoration: const InputDecoration(labelText: 'Modelo'),
                ),
              ),
            ],
          ),
          const SizedBox(height: 12),
          Row(
            children: <Widget>[
              Expanded(
                child: DropdownButtonFormField<String?>(
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
                        child: Text(s.label),
                      ),
                  ],
                  onChanged: (v) => setState(() => _operatingSystem = v),
                ),
              ),
              const SizedBox(width: 8),
              OutlinedButton.icon(
                onPressed: _identifying ? null : _identifyDevice,
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
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: const Text('Habilitar SNMP'),
                  subtitle: const Text(
                    'Coleta de interfaces, CPU, memória e sensores',
                  ),
                  value: _snmpEnabled,
                  onChanged: (v) => setState(() => _snmpEnabled = v),
                ),
                if (_snmpEnabled) ...<Widget>[
                  Row(
                    children: <Widget>[
                      Expanded(
                        child: DropdownButtonFormField<String>(
                          initialValue: _snmpVersion,
                          decoration: const InputDecoration(
                            labelText: 'Versão',
                          ),
                          items: const <DropdownMenuItem<String>>[
                            DropdownMenuItem(
                              value: 'v1',
                              child: Text('SNMP v1'),
                            ),
                            DropdownMenuItem(
                              value: 'v2c',
                              child: Text('SNMP v2c'),
                            ),
                            DropdownMenuItem(
                              value: 'v3',
                              child: Text('SNMP v3'),
                            ),
                          ],
                          onChanged: (v) {
                            if (v != null) setState(() => _snmpVersion = v);
                          },
                        ),
                      ),
                      const SizedBox(width: 10),
                      Expanded(
                        child: TextFormField(
                          controller: _communityCtrl,
                          decoration: const InputDecoration(
                            labelText: 'Community',
                          ),
                        ),
                      ),
                      const SizedBox(width: 10),
                      Expanded(
                        child: TextFormField(
                          controller: _pollIntervalCtrl,
                          keyboardType: TextInputType.number,
                          decoration: const InputDecoration(
                            labelText: 'Intervalo (s)',
                          ),
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 10),
                  Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    children: <Widget>[
                      OutlinedButton.icon(
                        onPressed: _testingSnmp
                            ? null
                            : () => _testSnmp(autoDetect: false),
                        icon: const Icon(Icons.network_check, size: 16),
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
                        icon: const Icon(Icons.settings_ethernet, size: 16),
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
                  const SizedBox(height: 10),
                  TextFormField(
                    controller: _linkIfaceCtrl,
                    decoration: const InputDecoration(
                      labelText: 'Interface de uplink / entrada de link',
                      hintText: 'Ex.: ether1, wan0',
                    ),
                  ),
                  if (_discoveredIfaces.isNotEmpty) ...<Widget>[
                    const SizedBox(height: 8),
                    Wrap(
                      spacing: 6,
                      runSpacing: 6,
                      children: _discoveredIfaces
                          .take(12)
                          .map((iface) {
                            return ActionChip(
                              label: Text(iface.displayName),
                              onPressed: () => setState(
                                () => _linkIfaceCtrl.text = iface.displayName,
                              ),
                            );
                          })
                          .toList(growable: false),
                    ),
                  ],
                ],
              ],
            ),
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
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: <Widget>[
              TextButton(
                onPressed: _saving ? null : () => Navigator.of(context).pop(),
                child: const Text('Cancelar'),
              ),
              const SizedBox(width: 8),
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
