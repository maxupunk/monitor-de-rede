import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/features/network/widgets/device_form_sheet.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class DiscoveryPage extends ConsumerStatefulWidget {
  const DiscoveryPage({
    super.key,
    this.initialNetworkId,
    this.autoStartScan = false,
  });

  final int? initialNetworkId;
  final bool autoStartScan;

  @override
  ConsumerState<DiscoveryPage> createState() => _DiscoveryPageState();
}

class _DiscoveryPageState extends ConsumerState<DiscoveryPage>
    with SingleTickerProviderStateMixin {
  late final TabController _tabController;
  final TextEditingController _hostSearchCtrl = TextEditingController();
  int? _selectedNetworkId;
  String _hostFilter = 'all';
  final Set<String> _selectedHostIps = <String>{};
  bool _autoStarted = false;

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 3, vsync: this);
    _selectedNetworkId = widget.initialNetworkId;
    Future<void>.microtask(_bootstrap);
  }

  Future<void> _bootstrap() async {
    await Future.wait(<Future<void>>[
      ref.read(networksProvider.notifier).fetchNetworks(),
      ref.read(devicesProvider.notifier).fetchDevices(),
      ref.read(discoveryProvider.notifier).fetchDiscoveryRuns(),
      ref.read(discoveryProvider.notifier).fetchEnvironment(),
      ref.read(discoveryProvider.notifier).fetchConflicts(),
    ]);
    if (!mounted) return;
    final networks = ref.read(networksProvider).items;
    if (_selectedNetworkId == null && networks.isNotEmpty) {
      setState(() => _selectedNetworkId = networks.first.id);
    }
    if (widget.autoStartScan && !_autoStarted && _selectedNetworkId != null) {
      _autoStarted = true;
      await ref.read(discoveryProvider.notifier).startScan(_selectedNetworkId!);
    }
  }

  @override
  void dispose() {
    _tabController.dispose();
    _hostSearchCtrl.dispose();
    super.dispose();
  }

  Future<void> _adoptHost(StreamedDiscoveryHost host) async {
    final netId =
        ref.read(discoveryProvider).scan.networkId ?? _selectedNetworkId;
    final networks = ref.read(networksProvider).items;
    final net = networks.where((n) => n.id == netId).firstOrNull;
    final saved = await showDeviceFormSheet(
      context: context,
      prefill: <String, dynamic>{
        'name': host.bestName,
        'ipAddress': host.ipAddress,
        'macAddress': host.macAddress,
        'vendor': host.vendor,
        'type': host.deviceType ?? 'other',
        'networkId': netId,
        'siteId': net?.siteId,
      },
    );
    if (saved != null && mounted) {
      _selectedHostIps.remove(host.ipAddress);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('"${saved.name}" adicionado ao inventário.')),
      );
    }
  }

  Future<void> _adoptSelectedBatch(List<StreamedDiscoveryHost> hosts) async {
    final netId =
        ref.read(discoveryProvider).scan.networkId ?? _selectedNetworkId;
    final net = ref
        .read(networksProvider)
        .items
        .where((n) => n.id == netId)
        .firstOrNull;
    var created = 0;
    for (final host in hosts) {
      if (!_selectedHostIps.contains(host.ipAddress)) continue;
      final res = await ref.read(devicesProvider.notifier).createDevice(
        <String, dynamic>{
          'name': host.bestName,
          'ipAddress': host.ipAddress,
          if (host.macAddress != null) 'macAddress': host.macAddress,
          if (host.vendor != null) 'vendor': host.vendor,
          'type': host.deviceType ?? 'other',
          'networkId': netId,
          'siteId': net?.siteId,
          'isMonitored': true,
        },
      );
      if (res != null) created++;
    }
    if (!mounted) return;
    setState(() => _selectedHostIps.clear());
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          '${countNoun(created, 'host adotado', 'hosts adotados')} no inventário.',
        ),
      ),
    );
  }

  void _showHostDetails(
    StreamedDiscoveryHost host,
    bool isManaged,
    bool canWrite,
  ) {
    showAdaptiveFormSheet<void>(
      context: context,
      title: host.bestName,
      subtitle: host.ipAddress,
      builder: (ctx) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          KeyValueRow(
            label: 'IP',
            value: host.ipAddress,
            monospace: true,
            copyable: true,
          ),
          KeyValueRow(
            label: 'MAC',
            value: host.macAddress ?? '—',
            monospace: true,
          ),
          KeyValueRow(label: 'Hostname', value: host.hostname ?? '—'),
          KeyValueRow(label: 'mDNS', value: host.mdnsName ?? '—'),
          KeyValueRow(label: 'Fabricante', value: host.vendor ?? '—'),
          KeyValueRow(
            label: 'Tipo sugerido',
            value: deviceTypeLabel(host.deviceType ?? 'other'),
          ),
          KeyValueRow(label: 'Confiança', value: '${host.confidence}%'),
          if (host.openPorts.isNotEmpty) ...<Widget>[
            const SizedBox(height: 10),
            Text('Portas abertas', style: context.textTheme.labelLarge),
            const SizedBox(height: 6),
            Wrap(
              spacing: 6,
              runSpacing: 6,
              children: host.openPorts
                  .map(
                    (p) => SemanticChip(
                      label: '$p',
                      tone: SemanticTone.info,
                      compact: true,
                    ),
                  )
                  .toList(growable: false),
            ),
          ],
          if (!isManaged && canWrite) ...<Widget>[
            const SizedBox(height: 16),
            FilledButton.icon(
              onPressed: () {
                Navigator.of(ctx).pop();
                _adoptHost(host);
              },
              icon: const Icon(Icons.add_circle_outline),
              label: const Text('Adotar no inventário'),
            ),
          ],
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final discovery = ref.watch(discoveryProvider);
    final networks = ref.watch(networksProvider).items;
    final managedIps = ref
        .watch(devicesProvider)
        .items
        .map((d) => d.ipAddress?.trim())
        .whereType<String>()
        .toSet();
    final canWrite = ref.watch(authProvider).canWrite;

    final query = _hostSearchCtrl.text.trim().toLowerCase();
    final hosts = discovery.scan.hosts
        .where((h) {
          final managed = managedIps.contains(h.ipAddress);
          if (_hostFilter == 'new' && managed) return false;
          if (_hostFilter == 'managed' && !managed) return false;
          if (query.isNotEmpty) {
            final text =
                '${h.ipAddress} ${h.macAddress ?? ''} ${h.hostname ?? ''} ${h.vendor ?? ''}'
                    .toLowerCase();
            if (!text.contains(query)) return false;
          }
          return true;
        })
        .toList(growable: false);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SearchFilterHeader(
          title: 'Descoberta de Rede',
          subtitle: 'Varredura ARP/ICMP/SNMP ao vivo via SSE e adoção para o inventário',
          actions: <Widget>[
            if (canWrite) ...<Widget>[
              OutlinedButton.icon(
                onPressed: discovery.loadingConflicts
                    ? null
                    : () =>
                          ref.read(discoveryProvider.notifier).checkConflicts(),
                icon: const Icon(Icons.report_problem_outlined, size: 18),
                label: const Text('Verificar conflitos'),
              ),
              OutlinedButton.icon(
                onPressed: () async {
                  final removed = await ref
                      .read(discoveryProvider.notifier)
                      .cleanup(olderThanDays: 7);
                  if (context.mounted && removed != null) {
                    ScaffoldMessenger.of(context).showSnackBar(
                      SnackBar(
                        content: Text('$removed execuções antigas removidas.'),
                      ),
                    );
                  }
                },
                icon: const Icon(Icons.cleaning_services_outlined, size: 18),
                label: const Text('Limpar (+7d)'),
              ),
            ],
          ],
        ),
        const SizedBox(height: 12),
        if (discovery.environment != null &&
            discovery.environment!.containerized &&
            !discovery.environment!.isHostMode) ...<Widget>[
          const NestedPanel(
            tone: SemanticTone.warning,
            child: Row(
              children: <Widget>[
                Icon(Icons.warning_amber_rounded),
                SizedBox(width: 10),
                Expanded(
                  child: Text(
                    'Container fora do modo host: a leitura ARP/MAC em Camada 2 fica limitada à rede bridge do container.',
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 12),
        ],
        SectionCard(
          child: Wrap(
            spacing: 12,
            runSpacing: 10,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: <Widget>[
              ConstrainedBox(
                constraints: const BoxConstraints(minWidth: 220, maxWidth: 360),
                child: DropdownButtonFormField<int>(
                  initialValue: networks.any((n) => n.id == _selectedNetworkId)
                      ? _selectedNetworkId
                      : (networks.isNotEmpty ? networks.first.id : null),
                  decoration: const InputDecoration(labelText: 'Rede alvo'),
                  items: networks
                      .map(
                        (n) => DropdownMenuItem<int>(
                          value: n.id,
                          child: Text('${n.name} (${n.cidr})'),
                        ),
                      )
                      .toList(growable: false),
                  onChanged: discovery.isScanning
                      ? null
                      : (v) => setState(() => _selectedNetworkId = v),
                ),
              ),
              if (canWrite) ...<Widget>[
                FilledButton.icon(
                  onPressed: discovery.isScanning || _selectedNetworkId == null
                      ? null
                      : () => ref
                            .read(discoveryProvider.notifier)
                            .startScan(_selectedNetworkId!),
                  icon: const Icon(Icons.radar),
                  label: Text(
                    discovery.isScanning ? 'Varrendo...' : 'Iniciar varredura',
                  ),
                ),
                if (discovery.isScanning)
                  OutlinedButton.icon(
                    onPressed: () =>
                        ref.read(discoveryProvider.notifier).cancelScan(),
                    icon: const Icon(Icons.stop),
                    label: const Text('Cancelar'),
                  ),
              ],
            ],
          ),
        ),
        if (discovery.isScanning ||
            discovery.scan.status != 'idle') ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(
            tone: discovery.scan.status == 'failed'
                ? SemanticTone.error
                : discovery.scan.status == 'completed'
                ? SemanticTone.success
                : SemanticTone.info,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: <Widget>[
                    Text(
                      'Status: ${discovery.scan.status.toUpperCase()} · Fase: ${discovery.scan.phase}',
                      style: context.textTheme.titleSmall?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    Text(
                      '${discovery.scan.progressCurrent}/${discovery.scan.progressTotal} · ${discovery.scan.hosts.length} hosts',
                      style: context.textTheme.labelMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                LinearProgressIndicator(
                  value: discovery.scan.progressPercent / 100.0,
                ),
                if (discovery.scan.error != null) ...<Widget>[
                  const SizedBox(height: 6),
                  Text(
                    discovery.scan.error!,
                    style: TextStyle(color: context.colorScheme.error),
                  ),
                ],
              ],
            ),
          ),
        ],
        const SizedBox(height: 10),
        TabBar(
          controller: _tabController,
          tabs: <Widget>[
            Tab(text: 'Resultados (${discovery.scan.hosts.length})'),
            Tab(text: 'Histórico (${discovery.runs.length})'),
            Tab(text: 'Conflitos (${discovery.conflicts.length})'),
          ],
        ),
        const SizedBox(height: 10),
        Expanded(
          child: TabBarView(
            controller: _tabController,
            children: <Widget>[
              // Tab 1: Resultados
              ListView(
                children: <Widget>[
                  Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: <Widget>[
                      SizedBox(
                        width: 260,
                        child: TextField(
                          controller: _hostSearchCtrl,
                          decoration: const InputDecoration(
                            hintText: 'Filtrar IP, MAC, hostname...',
                            prefixIcon: Icon(Icons.search, size: 18),
                            isDense: true,
                          ),
                          onChanged: (_) => setState(() {}),
                        ),
                      ),
                      ChoiceChip(
                        label: const Text('Todos'),
                        selected: _hostFilter == 'all',
                        onSelected: (_) => setState(() => _hostFilter = 'all'),
                      ),
                      ChoiceChip(
                        label: const Text('Novos'),
                        selected: _hostFilter == 'new',
                        onSelected: (_) => setState(() => _hostFilter = 'new'),
                      ),
                      ChoiceChip(
                        label: const Text('Já gerenciados'),
                        selected: _hostFilter == 'managed',
                        onSelected: (_) =>
                            setState(() => _hostFilter = 'managed'),
                      ),
                      if (canWrite && _selectedHostIps.isNotEmpty)
                        FilledButton.icon(
                          onPressed: () => _adoptSelectedBatch(hosts),
                          icon: const Icon(Icons.playlist_add, size: 18),
                          label: Text(
                            'Adotar selecionados (${_selectedHostIps.length})',
                          ),
                        ),
                    ],
                  ),
                  const SizedBox(height: 10),
                  if (hosts.isEmpty)
                    const EmptyStateView(
                      title: 'Nenhum host descoberto nesta sessão',
                      description: 'Selecione uma rede acima e clique em "Iniciar varredura".',
                      icon: Icons.radar,
                    )
                  else
                    for (final host in hosts) ...<Widget>[
                      Builder(
                        builder: (context) {
                          final managed = managedIps.contains(host.ipAddress);
                          final checked = _selectedHostIps.contains(
                            host.ipAddress,
                          );
                          return Card(
                            elevation: 2,
                            margin: const EdgeInsets.only(bottom: 8),
                            child: ListTile(
                              onTap: () =>
                                  _showHostDetails(host, managed, canWrite),
                              leading: !managed && canWrite
                                  ? Checkbox(
                                      value: checked,
                                      onChanged: (v) {
                                        setState(() {
                                          if (v == true) {
                                            _selectedHostIps.add(
                                              host.ipAddress,
                                            );
                                          } else {
                                            _selectedHostIps.remove(
                                              host.ipAddress,
                                            );
                                          }
                                        });
                                      },
                                    )
                                  : Icon(
                                      deviceTypeIcon(
                                        host.deviceType ?? 'other',
                                      ),
                                    ),
                              title: Text(
                                '${host.bestName} (${host.ipAddress})',
                                style: const TextStyle(
                                  fontWeight: FontWeight.w700,
                                ),
                              ),
                              subtitle: Text(
                                [
                                  if (host.macAddress != null) host.macAddress!,
                                  if (host.vendor != null) host.vendor!,
                                  if (host.openPorts.isNotEmpty)
                                    'Portas: ${host.openPorts.take(6).join(', ')}',
                                ].join(' · '),
                              ),
                              trailing: managed
                                  ? const SemanticChip(
                                      label: 'Gerenciado',
                                      tone: SemanticTone.success,
                                      compact: true,
                                    )
                                  : canWrite
                                  ? FilledButton.tonal(
                                      onPressed: () => _adoptHost(host),
                                      child: const Text('Adotar'),
                                    )
                                  : const SemanticChip(
                                      label: 'Novo',
                                      tone: SemanticTone.info,
                                      compact: true,
                                    ),
                            ),
                          );
                        },
                      ),
                    ],
                ],
              ),
              // Tab 2: Histórico
              discovery.runs.isEmpty
                  ? const EmptyStateView(
                      title: 'Nenhuma varredura registrada',
                      icon: Icons.history,
                    )
                  : ListView.builder(
                      itemCount: discovery.runs.length,
                      itemBuilder: (context, index) {
                        final run = discovery.runs[index];
                        return Card(
                          elevation: 2,
                          margin: const EdgeInsets.only(bottom: 8),
                          child: ListTile(
                            leading: StatusIndicator.fromStatus(
                              run.status,
                              compact: true,
                            ),
                            title: Text(
                              '${run.networkName ?? 'Rede #${run.networkId}'}${run.cidr != null ? ' (${run.cidr})' : ''}',
                              style: const TextStyle(
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                            subtitle: Text(
                              '${countNoun(run.devicesFound, 'host encontrado', 'hosts encontrados')} · '
                              'Iniciada: ${formatShortDateTime(run.startedAt)}',
                            ),
                            trailing: run.finishedAt != null
                                ? Text(
                                    formatTimeSpan(
                                      run.startedAt,
                                      run.finishedAt,
                                    ),
                                  )
                                : null,
                          ),
                        );
                      },
                    ),
              // Tab 3: Conflitos
              discovery.conflicts.isEmpty
                  ? const EmptyStateView(
                      title: 'Nenhum conflito de IP ou MAC detectado',
                      icon: Icons.verified_user_outlined,
                      tone: SemanticTone.success,
                    )
                  : ListView.builder(
                      itemCount: discovery.conflicts.length,
                      itemBuilder: (context, index) {
                        final c = discovery.conflicts[index];
                        return Card(
                          elevation: 2,
                          margin: const EdgeInsets.only(bottom: 8),
                          child: ListTile(
                            leading: SemanticChip(
                              label: c.conflictType,
                              tone: SemanticTone.error,
                              compact: true,
                            ),
                            title: Text(
                              c.ipAddress,
                              style: const TextStyle(
                                fontFamily: 'monospace',
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                            subtitle: Text(
                              '${c.description}\nMACs: ${c.macAddresses.join(', ')}',
                            ),
                            isThreeLine: true,
                          ),
                        );
                      },
                    ),
            ],
          ),
        ),
      ],
    );
  }
}
