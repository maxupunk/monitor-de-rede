import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/form_validators.dart';
import 'package:netmonitor_app/core/utils/json_parsers.dart';
import 'package:netmonitor_app/features/core/models/monitor_models.dart';
import 'package:netmonitor_app/features/core/providers/monitors_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

const List<(String, String)> kMonitorKinds = <(String, String)>[
  ('ping', 'Ping (ICMP)'),
  ('tcp', 'TCP (Porta)'),
  ('http', 'HTTP'),
  ('https', 'HTTPS'),
  ('dns', 'DNS'),
  ('snmp', 'SNMP'),
  ('ssl', 'SSL / Certificado'),
  ('port_scan', 'Varredura de Portas'),
  ('container', 'Container Docker'),
  ('host_resources', 'Recursos do Host'),
];

const List<(String, String)> kSnmpModes = <(String, String)>[
  ('oid', 'OID customizado'),
  ('cpu_usage', 'Uso de CPU'),
  ('memory_usage', 'Uso de memória'),
  ('interface', 'Estado de interface'),
  ('interface_traffic', 'Tráfego de interface (bps)'),
  ('sensor', 'Sensor SNMP'),
  ('temperature', 'Temperatura'),
  ('disk', 'Disco / Armazenamento'),
];

String _normalizeSnmpMode(Object? raw) {
  final mode = (raw ?? 'oid').toString().trim();
  return switch (mode) {
    'cpu' => 'cpu_usage',
    'memory' => 'memory_usage',
    'traffic' => 'interface_traffic',
    '' => 'oid',
    _ => mode,
  };
}

String _interfaceChoiceKey(DeviceInterfaceRef intf) =>
    intf.ifIndex != null ? 'idx:${intf.ifIndex}' : 'name:${intf.name}';

/// Abre o formulário completo de criação/edição de monitor (todos os tipos e modos SNMP).
Future<bool> showMonitorFormSheet({
  required BuildContext context,
  required WidgetRef ref,
  MonitorModel? existing,
  int? presetDeviceId,
}) async {
  final state = ref.read(monitorsProvider);
  if (state.devices.isEmpty) {
    await ref.read(monitorsProvider.notifier).fetchReferenceCatalogs();
  }
  if (!context.mounted) return false;

  final result = await showAdaptiveFormSheet<bool>(
    context: context,
    title: existing == null ? 'Novo monitor' : 'Editar monitor',
    subtitle: 'Configure tipo, destino, intervalo e parâmetros específicos.',
    maxWidth: 640,
    builder: (sheetCtx) =>
        _MonitorFormBody(existing: existing, presetDeviceId: presetDeviceId),
  );
  return result == true;
}

class _MonitorFormBody extends ConsumerStatefulWidget {
  const _MonitorFormBody({this.existing, this.presetDeviceId});

  final MonitorModel? existing;
  final int? presetDeviceId;

  @override
  ConsumerState<_MonitorFormBody> createState() => _MonitorFormBodyState();
}

class _MonitorFormBodyState extends ConsumerState<_MonitorFormBody> {
  final GlobalKey<FormState> _formKey = GlobalKey<FormState>();
  late final TextEditingController _nameCtrl;
  late final TextEditingController _targetCtrl;
  late final TextEditingController _portCtrl;
  late final TextEditingController _intervalCtrl;
  late final TextEditingController _timeoutCtrl;
  late final TextEditingController _retryCtrl;
  late final TextEditingController _oidCtrl;
  late final TextEditingController _expectedStatusCtrl;
  late final TextEditingController _dnsDomainCtrl;

  String _type = 'ping';
  String _snmpMode = 'oid';
  int? _deviceId;
  int? _probeId;
  bool _enabled = true;
  bool _saving = false;
  bool _loadingIfs = false;
  List<DeviceInterfaceRef> _interfaces = const <DeviceInterfaceRef>[];
  String? _selectedInterfaceKey;
  String? _error;

  @override
  void initState() {
    super.initState();
    final e = widget.existing;
    _type = e?.type == 'tls' ? 'ssl' : (e?.type ?? 'ping');
    _deviceId = e?.deviceId ?? widget.presetDeviceId;
    _probeId = e?.probeId;
    _enabled = e?.isEnabled ?? true;
    final cfg = e?.configuration ?? const <String, dynamic>{};
    _snmpMode = _normalizeSnmpMode(cfg['metric'] ?? cfg['mode']);
    final savedIfIndex = parseIntOrNull(cfg['ifIndex']);
    final savedIfName = (cfg['ifName'] as String?)?.trim();
    if (savedIfIndex != null) {
      _selectedInterfaceKey = 'idx:$savedIfIndex';
    } else if (savedIfName != null && savedIfName.isNotEmpty) {
      _selectedInterfaceKey = 'name:$savedIfName';
    }

    _nameCtrl = TextEditingController(text: e?.name ?? '');
    _targetCtrl = TextEditingController(text: e?.target ?? '');
    _portCtrl = TextEditingController(text: e?.port?.toString() ?? '');
    _intervalCtrl = TextEditingController(
      text: (e?.intervalSeconds ?? 60).toString(),
    );
    _timeoutCtrl = TextEditingController(
      text: (e?.timeoutSeconds ?? 5).toString(),
    );
    _retryCtrl = TextEditingController(text: (e?.retryCount ?? 1).toString());
    _oidCtrl = TextEditingController(text: (cfg['oid'] ?? '').toString());
    _expectedStatusCtrl = TextEditingController(
      text: (cfg['expectedStatus'] ?? '200').toString(),
    );
    _dnsDomainCtrl = TextEditingController(
      text: (cfg['domain'] ?? 'google.com').toString(),
    );

    if (_deviceId != null && _type == 'snmp') {
      _loadInterfaces(_deviceId!);
    }
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _targetCtrl.dispose();
    _portCtrl.dispose();
    _intervalCtrl.dispose();
    _timeoutCtrl.dispose();
    _retryCtrl.dispose();
    _oidCtrl.dispose();
    _expectedStatusCtrl.dispose();
    _dnsDomainCtrl.dispose();
    super.dispose();
  }

  Future<void> _loadInterfaces(int deviceId, {bool scan = false}) async {
    setState(() => _loadingIfs = true);
    final list = scan
        ? await ref
              .read(monitorsProvider.notifier)
              .scanDeviceInterfaces(deviceId)
        : await ref
              .read(monitorsProvider.notifier)
              .fetchDeviceInterfaces(deviceId);
    if (!mounted) return;
    setState(() {
      _interfaces = list;
      _loadingIfs = false;
    });
  }

  Future<void> _submit() async {
    if (_saving) return;
    if (!_formKey.currentState!.validate()) return;
    setState(() {
      _saving = true;
      _error = null;
    });

    final target = _targetCtrl.text.trim();
    final port = int.tryParse(_portCtrl.text.trim());
    final cfg = <String, dynamic>{
      if (widget.existing != null) ...widget.existing!.configuration,
    };
    if (_type == 'ping' ||
        _type == 'tcp' ||
        _type == 'snmp' ||
        _type == 'ssl') {
      if (target.isNotEmpty) cfg['host'] = target;
    } else if (_type == 'http' || _type == 'https') {
      if (target.isNotEmpty) {
        cfg['url'] = target.startsWith('http') ? target : '$_type://$target';
      }
      final code = int.tryParse(_expectedStatusCtrl.text.trim());
      if (code != null) cfg['expectedStatus'] = code;
    } else if (_type == 'dns') {
      if (_dnsDomainCtrl.text.trim().isNotEmpty) {
        cfg['domain'] = _dnsDomainCtrl.text.trim();
      } else if (target.isNotEmpty) {
        cfg['domain'] = target;
      }
    } else if (_type == 'container' && target.isNotEmpty) {
      cfg['container'] = target;
    }
    if (port != null) {
      cfg['port'] = port;
    }
    if (_type == 'snmp') {
      cfg['metric'] = _snmpMode;
      cfg['mode'] = _snmpMode;
      if (_snmpMode == 'oid' && _oidCtrl.text.trim().isNotEmpty) {
        cfg['oid'] = _oidCtrl.text.trim();
      }
      if ((_snmpMode == 'interface' || _snmpMode == 'interface_traffic') &&
          _selectedInterfaceKey != null) {
        final selected = _interfaces
            .where((i) => _interfaceChoiceKey(i) == _selectedInterfaceKey)
            .firstOrNull;
        if (selected != null) {
          cfg['ifName'] = selected.name;
          if (selected.ifIndex != null) {
            cfg['ifIndex'] = selected.ifIndex;
          }
        }
      }
    }

    final payload = <String, dynamic>{
      'name': _nameCtrl.text.trim(),
      'type': _type,
      'target': target,
      'deviceId': _deviceId,
      'probeId': _probeId,
      'port': ?port,
      'intervalSeconds': int.tryParse(_intervalCtrl.text.trim()) ?? 60,
      'timeoutSeconds': int.tryParse(_timeoutCtrl.text.trim()) ?? 5,
      'retryCount': int.tryParse(_retryCtrl.text.trim()) ?? 1,
      'enabled': _enabled,
      'isEnabled': _enabled,
      if (cfg.isNotEmpty) 'configuration': cfg,
    };

    final notifier = ref.read(monitorsProvider.notifier);
    final ok = widget.existing == null
        ? await notifier.createMonitor(payload)
        : await notifier.updateMonitor(widget.existing!.id, payload);

    if (!mounted) return;
    if (ok) {
      Navigator.of(context).pop(true);
    } else {
      setState(() {
        _saving = false;
        _error = ref.read(monitorsProvider).error ?? 'Erro ao salvar monitor.';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final monitorsState = ref.watch(monitorsProvider);
    final needsPort = _type == 'tcp' || _type == 'ssl';
    final safeDeviceId = monitorsState.devices.any((d) => d.id == _deviceId)
        ? _deviceId
        : null;
    final safeProbeId = monitorsState.probes.any((p) => p.id == _probeId)
        ? _probeId
        : null;
    final safeInterfaceKey =
        _interfaces.any((i) => _interfaceChoiceKey(i) == _selectedInterfaceKey)
        ? _selectedInterfaceKey
        : null;

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
            decoration: const InputDecoration(labelText: 'Nome do monitor'),
            validator: FormValidators.required('Informe o nome do monitor.'),
          ),
          const SizedBox(height: 12),
          AppDropdownFormField<String>(
            initialValue: kMonitorKinds.any((k) => k.$1 == _type)
                ? _type
                : 'ping',
            decoration: const InputDecoration(labelText: 'Tipo de verificação'),
            items: <DropdownMenuItem<String>>[
              for (final (value, label) in kMonitorKinds)
                DropdownMenuItem<String>(
                  value: value,
                  child: Text(label, overflow: TextOverflow.ellipsis),
                ),
            ],
            onChanged: (v) {
              if (v == null) return;
              setState(() => _type = v);
              if (v == 'snmp' && _deviceId != null) {
                _loadInterfaces(_deviceId!);
              }
            },
          ),
          const SizedBox(height: 12),
          AppDropdownFormField<int?>(
            initialValue: safeDeviceId,
            decoration: const InputDecoration(
              labelText: 'Dispositivo vinculado',
            ),
            items: <DropdownMenuItem<int?>>[
              const DropdownMenuItem<int?>(
                value: null,
                child: Text('Sem vínculo', overflow: TextOverflow.ellipsis),
              ),
              for (final d in monitorsState.devices)
                DropdownMenuItem<int?>(
                  value: d.id,
                  child: Text(
                    d.ipAddress != null ? '${d.name} (${d.ipAddress})' : d.name,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
            ],
            onChanged: (v) {
              setState(() => _deviceId = v);
              if (v != null) {
                final dev = monitorsState.devices
                    .where((d) => d.id == v)
                    .firstOrNull;
                if (dev != null &&
                    _targetCtrl.text.trim().isEmpty &&
                    dev.ipAddress != null) {
                  _targetCtrl.text = dev.ipAddress!;
                }
                if (_type == 'snmp') _loadInterfaces(v);
              }
            },
          ),
          const SizedBox(height: 12),
          TextFormField(
            controller: _targetCtrl,
            decoration: InputDecoration(
              labelText: _type == 'http' || _type == 'https'
                  ? 'URL de destino'
                  : 'Alvo (IP ou hostname)',
              hintText: _type == 'https'
                  ? 'https://exemplo.com/health'
                  : '192.168.1.1',
            ),
            validator: FormValidators.required('Informe o alvo do monitor.'),
          ),
          if (needsPort) ...<Widget>[
            const SizedBox(height: 12),
            TextFormField(
              controller: _portCtrl,
              keyboardType: TextInputType.number,
              decoration: InputDecoration(
                labelText: 'Porta',
                hintText: _type == 'ssl' ? '443' : '80',
              ),
              validator: FormValidators.port(),
            ),
          ],
          if (_type == 'http' || _type == 'https') ...<Widget>[
            const SizedBox(height: 12),
            TextFormField(
              controller: _expectedStatusCtrl,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(
                labelText: 'Código HTTP esperado',
              ),
            ),
          ],
          if (_type == 'dns') ...<Widget>[
            const SizedBox(height: 12),
            TextFormField(
              controller: _dnsDomainCtrl,
              decoration: const InputDecoration(
                labelText: 'Domínio a consultar',
              ),
            ),
          ],
          if (_type == 'snmp') ...<Widget>[
            const SizedBox(height: 12),
            AppDropdownFormField<String>(
              initialValue: kSnmpModes.any((m) => m.$1 == _snmpMode)
                  ? _snmpMode
                  : 'oid',
              decoration: const InputDecoration(labelText: 'Modo SNMP'),
              items: <DropdownMenuItem<String>>[
                for (final (value, label) in kSnmpModes)
                  DropdownMenuItem<String>(
                    value: value,
                    child: Text(label, overflow: TextOverflow.ellipsis),
                  ),
              ],
              onChanged: (v) {
                if (v != null) setState(() => _snmpMode = v);
              },
            ),
            if (_snmpMode == 'oid') ...<Widget>[
              const SizedBox(height: 12),
              TextFormField(
                controller: _oidCtrl,
                style: const TextStyle(fontFamily: 'monospace'),
                decoration: const InputDecoration(
                  labelText: 'OID SNMP',
                  hintText: '1.3.6.1.2.1.1.3.0',
                ),
              ),
            ],
            if ((_snmpMode == 'interface' ||
                    _snmpMode == 'interface_traffic') &&
                _deviceId != null) ...<Widget>[
              const SizedBox(height: 12),
              LayoutBuilder(
                builder: (context, constraints) {
                  final dropdown = AppDropdownFormField<String?>(
                    initialValue: safeInterfaceKey,
                    decoration: const InputDecoration(labelText: 'Interface'),
                    items: <DropdownMenuItem<String?>>[
                      const DropdownMenuItem<String?>(
                        value: null,
                        child: Text(
                          'Selecione a interface',
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                      for (final intf in _interfaces)
                        DropdownMenuItem<String?>(
                          value: _interfaceChoiceKey(intf),
                          child: Text(
                            intf.name,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                    ],
                    onChanged: (v) => setState(() => _selectedInterfaceKey = v),
                  );
                  final discoverBtn = OutlinedButton.icon(
                    onPressed: _loadingIfs
                        ? null
                        : () => _loadInterfaces(_deviceId!, scan: true),
                    icon: const Icon(Icons.radar, size: 18),
                    label: const Text('Descobrir'),
                  );
                  if (constraints.maxWidth < 460) {
                    return Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: <Widget>[
                        dropdown,
                        const SizedBox(height: 8),
                        discoverBtn,
                      ],
                    );
                  }
                  return Row(
                    children: <Widget>[
                      Expanded(child: dropdown),
                      const SizedBox(width: 8),
                      discoverBtn,
                    ],
                  );
                },
              ),
            ],
          ],
          if (monitorsState.probes.isNotEmpty) ...<Widget>[
            const SizedBox(height: 12),
            AppDropdownFormField<int?>(
              initialValue: safeProbeId,
              decoration: const InputDecoration(
                labelText: 'Sonda / Agente (opcional)',
              ),
              items: <DropdownMenuItem<int?>>[
                const DropdownMenuItem<int?>(
                  value: null,
                  child: Text(
                    'Execução local (central)',
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                for (final p in monitorsState.probes)
                  DropdownMenuItem<int?>(
                    value: p.id,
                    child: Text(p.name, overflow: TextOverflow.ellipsis),
                  ),
              ],
              onChanged: (v) => setState(() => _probeId = v),
            ),
          ],
          const SizedBox(height: 12),
          ResponsiveFieldRow(
            breakpoint: 640,
            children: <Widget>[
              TextFormField(
                controller: _intervalCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(labelText: 'Intervalo (s)'),
                validator: FormValidators.integerRange(5, 86400),
              ),
              TextFormField(
                controller: _timeoutCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(labelText: 'Timeout (s)'),
                validator: FormValidators.integerRange(1, 120),
              ),
              TextFormField(
                controller: _retryCtrl,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(labelText: 'Tentativas'),
                validator: FormValidators.integerRange(0, 10),
              ),
            ],
          ),
          const SizedBox(height: 8),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            value: _enabled,
            onChanged: (v) => setState(() => _enabled = v),
            title: const Text('Monitor ativo'),
          ),
          const SizedBox(height: 14),
          FilledButton.icon(
            onPressed: _saving ? null : _submit,
            icon: _saving
                ? const SizedBox(
                    width: 18,
                    height: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.save_outlined),
            label: Text(
              widget.existing == null ? 'Criar monitor' : 'Salvar alterações',
            ),
          ),
        ],
      ),
    );
  }
}

/// Diálogo de provisionamento rápido de serviços SaaS (`SaasPresetsDialog`).
Future<void> showSaasPresetsSheet(BuildContext context, WidgetRef ref) async {
  await showAdaptiveFormSheet<void>(
    context: context,
    title: 'Serviços SaaS pré-configurados',
    subtitle:
        'Selecione serviços externos para monitorar disponibilidade e latência.',
    maxWidth: 640,
    builder: (_) => const _SaasPresetsBody(),
  );
}

class _SaasPresetsBody extends ConsumerStatefulWidget {
  const _SaasPresetsBody();

  @override
  ConsumerState<_SaasPresetsBody> createState() => _SaasPresetsBodyState();
}

class _SaasPresetsBodyState extends ConsumerState<_SaasPresetsBody> {
  List<SaasPresetItem> _presets = const <SaasPresetItem>[];
  final Set<String> _selected = <String>{};
  String _category = 'Todos';
  bool _loading = true;
  bool _saving = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final list = await ref.read(monitorsProvider.notifier).fetchSaasPresets();
      if (!mounted) return;
      setState(() {
        _presets = list;
        _loading = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = describeError(err);
        _loading = false;
      });
    }
  }

  Future<void> _provision() async {
    if (_selected.isEmpty) return;
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      await ref
          .read(monitorsProvider.notifier)
          .provisionSaasPresets(_selected.toList());
      if (!mounted) return;
      Navigator.of(context).pop();
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('${_selected.length} serviço(s) SaaS provisionado(s).'),
        ),
      );
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _saving = false;
        _error = describeError(err);
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) {
      return const LoadingStateView(message: 'Carregando catálogo SaaS...');
    }
    final categories = <String>{
      'Todos',
      ..._presets.map((p) => p.category),
    }.toList();
    final filtered = _category == 'Todos'
        ? _presets
        : _presets
              .where((p) => p.category == _category)
              .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        if (_error != null) ...<Widget>[
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
          const SizedBox(height: 10),
        ],
        Wrap(
          spacing: 6,
          runSpacing: 6,
          children: <Widget>[
            for (final cat in categories)
              FilterChip(
                selected: _category == cat,
                label: Text(cat),
                onSelected: (_) => setState(() => _category = cat),
              ),
          ],
        ),
        const SizedBox(height: 12),
        for (final preset in filtered)
          CheckboxListTile(
            contentPadding: EdgeInsets.zero,
            value: preset.provisioned || _selected.contains(preset.id),
            onChanged: preset.provisioned
                ? null
                : (checked) {
                    setState(() {
                      if (checked == true) {
                        _selected.add(preset.id);
                      } else {
                        _selected.remove(preset.id);
                      }
                    });
                  },
            title: Text(
              preset.name,
              style: const TextStyle(fontWeight: FontWeight.w700),
            ),
            subtitle: Text('${preset.category} · ${preset.target}'),
            secondary: preset.provisioned
                ? const SemanticChip(
                    label: 'Ativo',
                    tone: SemanticTone.success,
                    compact: true,
                  )
                : null,
          ),
        const SizedBox(height: 16),
        FilledButton.icon(
          onPressed: _selected.isEmpty || _saving ? null : _provision,
          icon: _saving
              ? const SizedBox(
                  width: 18,
                  height: 18,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.cloud_done_outlined),
          label: Text('Provisionar (${_selected.length})'),
        ),
      ],
    );
  }
}
