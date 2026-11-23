import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/features/network/widgets/device_form_sheet.dart';
import 'package:netmonitor_app/features/network/widgets/discovery/discovery_widgets.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

export 'package:netmonitor_app/features/network/widgets/discovery/discovery_widgets.dart';

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
  bool _bootstrapped = false;
  bool _autoStarted = false;
  bool _envBannerDismissed = false;
  bool _completedBannerDismissed = false;

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 3, vsync: this);
    _selectedNetworkId = widget.initialNetworkId;
    Future<void>.microtask(_bootstrap);
  }

  @override
  void didUpdateWidget(covariant DiscoveryPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    final networkChanged =
        oldWidget.initialNetworkId != widget.initialNetworkId;
    if (networkChanged && widget.initialNetworkId != null) {
      _selectedNetworkId = widget.initialNetworkId;
    }
    final shouldTriggerScan =
        widget.autoStartScan && (!oldWidget.autoStartScan || networkChanged);
    if (shouldTriggerScan && _bootstrapped) {
      WidgetsBinding.instance.addPostFrameCallback((_) async {
        if (!mounted) return;
        _autoStarted = true;
        await _startScan();
      });
    }
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
    _bootstrapped = true;
    final networks = ref.read(networksProvider).items;
    final hasSelected = networks.any((n) => n.id == _selectedNetworkId);
    if (!hasSelected && networks.isNotEmpty) {
      setState(() => _selectedNetworkId = networks.first.id);
    }
    if (widget.autoStartScan && !_autoStarted && _selectedNetworkId != null) {
      _autoStarted = true;
      await _startScan();
    }
  }

  @override
  void dispose() {
    _tabController.dispose();
    _hostSearchCtrl.dispose();
    super.dispose();
  }

  Future<void> _startScan() async {
    final networks = ref.read(networksProvider).items;
    final effectiveId = networks.any((n) => n.id == _selectedNetworkId)
        ? _selectedNetworkId
        : (networks.isNotEmpty ? networks.first.id : null);
    if (effectiveId == null) return;
    if (_selectedNetworkId != effectiveId) {
      setState(() => _selectedNetworkId = effectiveId);
    }
    setState(() => _completedBannerDismissed = false);
    await ref.read(discoveryProvider.notifier).startScan(effectiveId);
  }

  Future<void> _checkConflicts() async {
    await ref.read(discoveryProvider.notifier).checkConflicts();
    if (!mounted) return;
    _tabController.animateTo(2);
  }

  Future<void> _cleanupRuns() async {
    final removed = await ref
        .read(discoveryProvider.notifier)
        .cleanup(olderThanDays: 7);
    if (mounted && removed != null) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('$removed execuções antigas removidas.')),
      );
    }
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
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('"${saved.name}" adicionado ao inventário.')),
      );
    }
  }

  Future<void> _adoptSelectedBatch(
    List<StreamedDiscoveryHost> hosts,
    Set<String> selectedIps,
  ) async {
    if (selectedIps.isEmpty) return;
    final netId =
        ref.read(discoveryProvider).scan.networkId ?? _selectedNetworkId;
    final net = ref
        .read(networksProvider)
        .items
        .where((n) => n.id == netId)
        .firstOrNull;
    final created = await ref
        .read(discoveryProvider.notifier)
        .adoptHostsBatch(
          hosts: hosts,
          selectedIps: selectedIps,
          networkId: netId,
          siteId: net?.siteId,
        );
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          '${countNoun(created, 'host adotado', 'hosts adotados')} no inventário.',
        ),
      ),
    );
  }

  Future<void> _openBatchAdoptSheet(
    List<StreamedDiscoveryHost> newHosts,
  ) async {
    if (newHosts.isEmpty) return;
    final selectedIps = await showAdaptiveFormSheet<Set<String>>(
      context: context,
      title: 'Adotar em lote',
      subtitle:
          'Selecione os equipamentos descobertos para adicionar ao inventário.',
      maxWidth: 580,
      builder: (ctx) => BatchAdoptSheetBody(
        hosts: newHosts,
        onCancel: () => Navigator.of(ctx).pop(),
        onConfirm: (picked) => Navigator.of(ctx).pop(picked),
      ),
    );
    if (selectedIps != null && selectedIps.isNotEmpty && mounted) {
      await _adoptSelectedBatch(newHosts, selectedIps);
    }
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
          KeyValueRow(
            label: 'Confiança',
            value: formatPercent(host.confidence, digits: 0),
          ),
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

  void _showEnvironmentNoticeDialog() {
    showDialog<void>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Row(
          children: <Widget>[
            Icon(Icons.warning_amber_rounded),
            SizedBox(width: 8),
            Expanded(child: Text('Modo de rede do container')),
          ],
        ),
        content: const Text(
          'O servidor está em container fora do modo host: a leitura ARP/MAC em Camada 2 fica limitada à rede bridge do container. A descoberta por ICMP, DNS e SNMP continua funcionando normalmente.',
        ),
        actions: <Widget>[
          FilledButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('Entendi'),
          ),
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

    final allHosts = discovery.scan.hosts;
    final newHosts = allHosts
        .where((h) => !managedIps.contains(h.ipAddress))
        .toList(growable: false);
    final newCount = newHosts.length;
    final managedCount = allHosts.length - newCount;

    final query = _hostSearchCtrl.text.trim().toLowerCase();
    final hosts = allHosts
        .where((h) {
          final managed = managedIps.contains(h.ipAddress);
          if (_hostFilter == 'new' && managed) return false;
          if (_hostFilter == 'managed' && !managed) return false;
          if (query.isNotEmpty) {
            final text =
                '${h.ipAddress} ${h.macAddress ?? ''} ${h.hostname ?? ''} ${h.mdnsName ?? ''} ${h.systemOrVendorLabel ?? ''}'
                    .toLowerCase();
            if (!text.contains(query)) return false;
          }
          return true;
        })
        .toList(growable: false);

    final showEnvWarning =
        discovery.environment != null &&
        discovery.environment!.containerized &&
        !discovery.environment!.isHostMode;

    final showStatusStrip =
        discovery.isScanning ||
        (discovery.scan.status != 'idle' &&
            (!_completedBannerDismissed || discovery.scan.status == 'failed'));

    final effectiveNetworkId = networks.any((n) => n.id == _selectedNetworkId)
        ? _selectedNetworkId
        : (networks.isNotEmpty ? networks.first.id : null);

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 680;
        final tabBar = TabBar(
          controller: _tabController,
          isScrollable: isCompact,
          tabAlignment: isCompact ? TabAlignment.start : TabAlignment.fill,
          labelPadding: const EdgeInsets.symmetric(horizontal: 14),
          tabs: <Widget>[
            Tab(text: 'Resultados (${allHosts.length})'),
            Tab(text: 'Histórico (${discovery.runs.length})'),
            Tab(text: 'Conflitos (${discovery.conflicts.length})'),
          ],
        );

        return NestedScrollView(
          headerSliverBuilder: (context, innerBoxIsScrolled) => <Widget>[
            SliverToBoxAdapter(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  DiscoveryTopBar(
                    isCompact: isCompact,
                    canWrite: canWrite,
                    loadingConflicts: discovery.loadingConflicts,
                    showEnvBadge: showEnvWarning && _envBannerDismissed,
                    onShowEnvNotice: _showEnvironmentNoticeDialog,
                    onCheckConflicts: _checkConflicts,
                    onCleanup: _cleanupRuns,
                  ),
                  if (showEnvWarning && !_envBannerDismissed) ...<Widget>[
                    const SizedBox(height: 8),
                    CompactEnvBanner(
                      onInfo: _showEnvironmentNoticeDialog,
                      onDismiss: () =>
                          setState(() => _envBannerDismissed = true),
                    ),
                  ],
                  if (discovery.error != null) ...<Widget>[
                    const SizedBox(height: 8),
                    NestedPanel(
                      tone: SemanticTone.error,
                      child: Text(discovery.error!),
                    ),
                  ],
                  const SizedBox(height: 8),
                  DiscoveryControlCard(
                    isCompact: isCompact,
                    canWrite: canWrite,
                    networks: networks,
                    selectedNetworkId: effectiveNetworkId,
                    discovery: discovery,
                    showStatusStrip: showStatusStrip,
                    onNetworkChanged: (v) =>
                        setState(() => _selectedNetworkId = v),
                    onStartScan: _startScan,
                    onCancelScan: () =>
                        ref.read(discoveryProvider.notifier).cancelScan(),
                    onDismissStatus: () =>
                        setState(() => _completedBannerDismissed = true),
                  ),
                  const SizedBox(height: 8),
                ],
              ),
            ),
            SliverPersistentHeader(
              pinned: true,
              delegate: PinnedSliverHeaderDelegate(
                height: tabBar.preferredSize.height + 1,
                backgroundColor: Theme.of(context).scaffoldBackgroundColor,
                bottomBorder: true,
                child: tabBar,
              ),
            ),
          ],
          body: TabBarView(
            controller: _tabController,
            children: <Widget>[
              DiscoveryResultsTab(
                hosts: hosts,
                totalCount: allHosts.length,
                newCount: newCount,
                managedCount: managedCount,
                managedIps: managedIps,
                canWrite: canWrite,
                hostFilter: _hostFilter,
                searchController: _hostSearchCtrl,
                onSearchChanged: () => setState(() {}),
                onFilterChanged: (f) => setState(() => _hostFilter = f),
                onOpenBatchAdopt: () => _openBatchAdoptSheet(newHosts),
                onAdoptSingle: _adoptHost,
                onOpenDetails: _showHostDetails,
                onRefresh: _bootstrap,
              ),
              DiscoveryHistoryTab(
                runs: discovery.runs,
                onRefresh: () =>
                    ref.read(discoveryProvider.notifier).fetchDiscoveryRuns(),
              ),
              DiscoveryConflictsTab(
                conflicts: discovery.conflicts,
                onRefresh: () =>
                    ref.read(discoveryProvider.notifier).fetchConflicts(),
              ),
            ],
          ),
        );
      },
    );
  }
}
