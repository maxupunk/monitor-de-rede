import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/core/presentation/widgets/snmp_credential_dropdown.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/diagnostics_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

// ── SNMP Test Dialog ─────────────────────────────────────────────────────────

Future<void> showSnmpTestDialog(
  BuildContext context, {
  String? initialHost,
  int? initialProfileId,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Teste SNMP',
    subtitle: 'Valide conectividade, credencial e walk de interfaces',
    maxWidth: 600,
    builder: (_) => _SnmpTestBody(
      initialHost: initialHost,
      initialProfileId: initialProfileId,
    ),
  );
}

class _SnmpTestBody extends ConsumerStatefulWidget {
  const _SnmpTestBody({this.initialHost, this.initialProfileId});

  final String? initialHost;
  final int? initialProfileId;

  @override
  ConsumerState<_SnmpTestBody> createState() => _SnmpTestBodyState();
}

class _SnmpTestBodyState extends ConsumerState<_SnmpTestBody> {
  late final TextEditingController _hostCtrl;
  final TextEditingController _portCtrl = TextEditingController(text: '161');
  int? _profileId;
  bool _testing = false;
  SnmpTestResult? _result;
  List<DeviceInterfaceModel> _interfaces = const <DeviceInterfaceModel>[];
  String? _error;

  @override
  void initState() {
    super.initState();
    _hostCtrl = TextEditingController(text: widget.initialHost ?? '');
    _profileId = widget.initialProfileId;
  }

  @override
  void dispose() {
    _hostCtrl.dispose();
    _portCtrl.dispose();
    super.dispose();
  }

  Future<void> _test({required bool autoDetect}) async {
    if (_hostCtrl.text.trim().isEmpty) return;
    setState(() {
      _testing = true;
      _error = null;
      _interfaces = const <DeviceInterfaceModel>[];
    });
    try {
      final res = await ref
          .read(snmpProfilesProvider.notifier)
          .testSnmp(
            host: _hostCtrl.text.trim(),
            port: int.tryParse(_portCtrl.text.trim()) ?? 161,
            profileId: _profileId,
            autoDetect: autoDetect,
          );
      if (!mounted) return;
      setState(() {
        _result = res;
        if (res.profileId != null) _profileId = res.profileId;
      });
      if (res.responded) {
        final ifaces = await ref
            .read(snmpProfilesProvider.notifier)
            .queryInterfaces(
              host: _hostCtrl.text.trim(),
              port: int.tryParse(_portCtrl.text.trim()) ?? 161,
              profileId: res.profileId ?? _profileId,
            );
        if (mounted) setState(() => _interfaces = ifaces);
      }
    } catch (err) {
      if (mounted) setState(() => _error = describeError(err));
    } finally {
      if (mounted) setState(() => _testing = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        ResponsiveFieldRow(
          breakpoint: 480,
          children: <Widget>[
            TextField(
              controller: _hostCtrl,
              decoration: const InputDecoration(labelText: 'Host / IP'),
            ),
            TextField(
              controller: _portCtrl,
              keyboardType: TextInputType.number,
              decoration: const InputDecoration(labelText: 'Porta'),
            ),
          ],
        ),
        const SizedBox(height: 10),
        SnmpCredentialDropdown(
          value: _profileId,
          onChanged: (id) => setState(() => _profileId = id),
        ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            FilledButton.icon(
              onPressed: _testing ? null : () => _test(autoDetect: false),
              icon: const Icon(Icons.play_arrow),
              label: const Text('Testar SNMP'),
            ),
            OutlinedButton.icon(
              onPressed: _testing ? null : () => _test(autoDetect: true),
              icon: const Icon(Icons.auto_awesome),
              label: const Text('Auto-detectar'),
            ),
          ],
        ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        if (_result != null) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            tone: _result!.responded
                ? SemanticTone.success
                : SemanticTone.error,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  _result!.responded
                      ? 'Agente SNMP respondeu'
                      : 'Sem resposta SNMP',
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                if (_result!.sysName != null)
                  KeyValueRow(label: 'sysName', value: _result!.sysName!),
                if (_result!.sysDescr != null)
                  KeyValueRow(label: 'sysDescr', value: _result!.sysDescr!),
                if (_result!.message != null) Text(_result!.message!),
              ],
            ),
          ),
        ],
        if (_interfaces.isNotEmpty) ...<Widget>[
          const SizedBox(height: 12),
          Text(
            'Interfaces descobertas (${_interfaces.length})',
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(height: 6),
          for (final iface in _interfaces.take(20))
            ListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              title: Text(iface.displayName),
              subtitle: Text(formatLinkSpeed(iface.speed)),
              trailing: StatusIndicator.fromStatus(
                iface.operStatus ?? 'unknown',
                compact: true,
              ),
            ),
        ],
      ],
    );
  }
}

// ── SNMP Scan & Apply Monitors Dialog ────────────────────────────────────────

Future<void> showSnmpScanDialog(
  BuildContext context, {
  required int deviceId,
  required String deviceName,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Escanear SNMP · $deviceName',
    subtitle:
        'Descubra CPU, memória, interfaces e sensores e aplique monitores em lote',
    maxWidth: 680,
    builder: (ctx) => _SnmpScanBody(
      deviceId: deviceId,
      onApplied: () => Navigator.of(ctx).pop(),
    ),
  );
}

class _SnmpScanBody extends ConsumerStatefulWidget {
  const _SnmpScanBody({required this.deviceId, required this.onApplied});

  final int deviceId;
  final VoidCallback onApplied;

  @override
  ConsumerState<_SnmpScanBody> createState() => _SnmpScanBodyState();
}

class _SnmpScanBodyState extends ConsumerState<_SnmpScanBody> {
  SnmpScanResultModel? _scan;
  bool _scanning = true;
  bool _applying = false;
  bool _enableCpu = false;
  bool _enableMem = false;
  bool _clearRemoved = false;
  final Set<int> _selectedIfIndexes = <int>{};
  final Set<String> _selectedSensors = <String>{};

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_runScan);
  }

  Future<void> _runScan() async {
    setState(() => _scanning = true);
    final res = await ref
        .read(deviceDetailProvider.notifier)
        .scanDeviceSnmp(widget.deviceId);
    if (!mounted) return;
    if (res != null) {
      _enableCpu = res.hasCpuMonitor || res.cpuUsagePercent != null;
      _enableMem = res.hasMemoryMonitor || res.memoryUsedPercent != null;
      _selectedIfIndexes
        ..clear()
        ..addAll(
          res.interfaces.where((i) => i.isMonitored).map((i) => i.ifIndex),
        );
      _selectedSensors
        ..clear()
        ..addAll(res.sensors.where((s) => s.isMonitored).map((s) => s.key));
    }
    setState(() {
      _scan = res;
      _scanning = false;
    });
  }

  Future<void> _apply() async {
    setState(() => _applying = true);
    final ok = await ref
        .read(deviceDetailProvider.notifier)
        .applySnmpMonitors(
          widget.deviceId,
          enableCpuMonitor: _enableCpu,
          enableMemoryMonitor: _enableMem,
          monitoredIfIndexes: _selectedIfIndexes.toList(growable: false),
          monitoredSensors: _selectedSensors.toList(growable: false),
          clearRemovedHistory: _clearRemoved,
        );
    if (!mounted) return;
    setState(() => _applying = false);
    if (ok) widget.onApplied();
  }

  @override
  Widget build(BuildContext context) {
    if (_scanning) {
      return const LoadingStateView(
        message: 'Consultando MIBs SNMP no equipamento (até 120s)...',
      );
    }
    if (_scan == null || !_scan!.snmpResponded) {
      return ErrorStateView(
        message:
            ref.watch(deviceDetailProvider).error ??
            'O equipamento não respondeu via SNMP.',
        onRetry: _runScan,
      );
    }
    final scan = _scan!;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        NestedPanel(
          tone: SemanticTone.success,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                scan.sysName ?? 'SNMP conectado',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              if (scan.sysDescr != null)
                Text(scan.sysDescr!, style: context.textTheme.bodySmall),
              if (scan.matchedProfileName != null)
                Padding(
                  padding: const EdgeInsets.only(top: 6),
                  child: SemanticChip(
                    label: 'Perfil: ${scan.matchedProfileName}',
                    tone: SemanticTone.info,
                    compact: true,
                  ),
                ),
            ],
          ),
        ),
        const SizedBox(height: 12),
        CheckboxListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('Monitorar uso de CPU'),
          subtitle: Text(
            scan.cpuUsagePercent != null
                ? 'Atual: ${formatPercent(scan.cpuUsagePercent)}'
                : 'Não reportado',
          ),
          value: _enableCpu,
          onChanged: (v) => setState(() => _enableCpu = v ?? false),
        ),
        CheckboxListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('Monitorar uso de Memória'),
          subtitle: Text(
            scan.memoryUsedPercent != null
                ? 'Atual: ${formatPercent(scan.memoryUsedPercent)}'
                : 'Não reportado',
          ),
          value: _enableMem,
          onChanged: (v) => setState(() => _enableMem = v ?? false),
        ),
        if (scan.interfaces.isNotEmpty) ...<Widget>[
          const Divider(),
          Text(
            'Interfaces (${_selectedIfIndexes.length}/${scan.interfaces.length} selecionadas)',
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          for (final iface in scan.interfaces)
            CheckboxListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              title: Text(iface.ifName),
              subtitle: Text(
                '${iface.ifOperStatus ?? '—'} · ${formatLinkSpeed(iface.ifSpeed)}'
                '${iface.ifAlias != null && iface.ifAlias!.isNotEmpty ? ' · ${iface.ifAlias}' : ''}',
              ),
              value: _selectedIfIndexes.contains(iface.ifIndex),
              onChanged: (v) {
                setState(() {
                  if (v == true) {
                    _selectedIfIndexes.add(iface.ifIndex);
                  } else {
                    _selectedIfIndexes.remove(iface.ifIndex);
                  }
                });
              },
            ),
        ],
        if (scan.sensors.isNotEmpty) ...<Widget>[
          const Divider(),
          Text(
            'Sensores (${_selectedSensors.length}/${scan.sensors.length})',
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
          for (final sensor in scan.sensors)
            CheckboxListTile(
              dense: true,
              contentPadding: EdgeInsets.zero,
              title: Text(sensor.label),
              subtitle: Text('${sensor.formattedValue} · ${sensor.oid}'),
              value: _selectedSensors.contains(sensor.key),
              onChanged: (v) {
                setState(() {
                  if (v == true) {
                    _selectedSensors.add(sensor.key);
                  } else {
                    _selectedSensors.remove(sensor.key);
                  }
                });
              },
            ),
        ],
        const Divider(),
        SwitchListTile(
          contentPadding: EdgeInsets.zero,
          title: const Text('Limpar histórico de itens removidos'),
          value: _clearRemoved,
          onChanged: (v) => setState(() => _clearRemoved = v),
        ),
        const SizedBox(height: 12),
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton.icon(
            onPressed: _applying ? null : _apply,
            icon: _applying
                ? const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.check),
            label: const Text('Aplicar monitores SNMP'),
          ),
        ),
      ],
    );
  }
}

// ── SNMP Profiles Dialog ─────────────────────────────────────────────────────

Future<void> showSnmpProfilesDialog(BuildContext context) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Perfis SNMP',
    subtitle: 'Catálogo de perfis embutidos e personalizados de sensores SNMP',
    maxWidth: 640,
    builder: (_) => const _SnmpProfilesBody(),
  );
}

class _SnmpProfilesBody extends ConsumerStatefulWidget {
  const _SnmpProfilesBody();

  @override
  ConsumerState<_SnmpProfilesBody> createState() => _SnmpProfilesBodyState();
}

class _SnmpProfilesBodyState extends ConsumerState<_SnmpProfilesBody> {
  final _idCtrl = TextEditingController();
  final _nameCtrl = TextEditingController();
  final _vendorCtrl = TextEditingController();
  final _oidPrefixCtrl = TextEditingController();
  bool _creating = false;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(
      () => ref.read(snmpProfilesProvider.notifier).fetchProfiles(),
    );
  }

  @override
  void dispose() {
    _idCtrl.dispose();
    _nameCtrl.dispose();
    _vendorCtrl.dispose();
    _oidPrefixCtrl.dispose();
    super.dispose();
  }

  Future<void> _saveNew() async {
    if (_idCtrl.text.trim().isEmpty || _nameCtrl.text.trim().isEmpty) return;
    await ref.read(snmpProfilesProvider.notifier).saveProfile(<String, dynamic>{
      'id': _idCtrl.text.trim(),
      'name': _nameCtrl.text.trim(),
      'vendor': _vendorCtrl.text.trim().isEmpty
          ? 'Custom'
          : _vendorCtrl.text.trim(),
      'category': 'network',
      if (_oidPrefixCtrl.text.trim().isNotEmpty)
        'sysObjectIdPrefix': _oidPrefixCtrl.text.trim(),
      'sensors': const <Map<String, dynamic>>[],
    });
    if (!mounted) return;
    _idCtrl.clear();
    _nameCtrl.clear();
    _vendorCtrl.clear();
    _oidPrefixCtrl.clear();
    setState(() => _creating = false);
  }

  @override
  Widget build(BuildContext context) {
    final profiles = ref.watch(snmpProfilesProvider);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Wrap(
          alignment: WrapAlignment.spaceBetween,
          crossAxisAlignment: WrapCrossAlignment.center,
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            Text(
              '${profiles.length} perfis disponíveis',
              style: context.textTheme.labelLarge,
            ),
            OutlinedButton.icon(
              onPressed: () => setState(() => _creating = !_creating),
              icon: Icon(_creating ? Icons.close : Icons.add, size: 18),
              label: Text(_creating ? 'Fechar formulário' : 'Novo perfil'),
            ),
          ],
        ),
        if (_creating) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                ResponsiveFieldRow(
                  breakpoint: 480,
                  children: <Widget>[
                    TextField(
                      controller: _idCtrl,
                      decoration: const InputDecoration(labelText: 'ID (slug)'),
                    ),
                    TextField(
                      controller: _nameCtrl,
                      decoration: const InputDecoration(labelText: 'Nome'),
                    ),
                  ],
                ),
                const SizedBox(height: 10),
                ResponsiveFieldRow(
                  breakpoint: 480,
                  children: <Widget>[
                    TextField(
                      controller: _vendorCtrl,
                      decoration: const InputDecoration(
                        labelText: 'Fabricante',
                      ),
                    ),
                    TextField(
                      controller: _oidPrefixCtrl,
                      decoration: const InputDecoration(
                        labelText: 'Prefixo sysObjectID',
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 10),
                Align(
                  alignment: Alignment.centerRight,
                  child: FilledButton(
                    onPressed: _saveNew,
                    child: const Text('Salvar perfil'),
                  ),
                ),
              ],
            ),
          ),
        ],
        const SizedBox(height: 12),
        if (profiles.isEmpty)
          const EmptyStateView(
            title: 'Nenhum perfil SNMP carregado',
            icon: Icons.tune_outlined,
          )
        else
          for (final p in profiles) ...<Widget>[
            NestedPanel(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
              child: Row(
                children: <Widget>[
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          p.name,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: context.textTheme.titleSmall?.copyWith(
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                        Text(
                          '${p.vendor} · ${countNoun(p.sensorCount, 'sensor', 'sensores')}'
                          '${p.sysObjectIdPrefix != null ? ' · ${p.sysObjectIdPrefix}' : ''}',
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: context.textTheme.bodySmall,
                        ),
                      ],
                    ),
                  ),
                  const SizedBox(width: 8),
                  SemanticChip(
                    label: p.isBuiltin ? 'Embutido' : 'Personalizado',
                    tone: p.isBuiltin
                        ? SemanticTone.info
                        : SemanticTone.primary,
                    compact: true,
                  ),
                  if (!p.isBuiltin)
                    IconButton(
                      tooltip: 'Excluir perfil',
                      icon: const Icon(Icons.delete_outline, size: 18),
                      onPressed: () => ref
                          .read(snmpProfilesProvider.notifier)
                          .deleteProfile(p.id),
                    ),
                ],
              ),
            ),
            const SizedBox(height: 8),
          ],
      ],
    );
  }
}
