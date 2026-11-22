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
    if (_selectedNetworkId == null) return;
    setState(() => _completedBannerDismissed = false);
    await ref.read(discoveryProvider.notifier).startScan(_selectedNetworkId!);
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
    var created = 0;
    for (final host in hosts) {
      if (!selectedIps.contains(host.ipAddress)) continue;
      final res = await ref
          .read(devicesProvider.notifier)
          .createDevice(<String, dynamic>{
            'name': host.bestName,
            'ipAddress': host.ipAddress,
            if (host.macAddress != null) 'macAddress': host.macAddress,
            if (host.vendor != null) 'vendor': host.vendor,
            'type': host.deviceType ?? 'other',
            'networkId': netId,
            'siteId': net?.siteId,
            'isMonitored': true,
          });
      if (res != null) created++;
    }
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
      builder: (ctx) => _BatchAdoptSheetBody(
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
        return NestedScrollView(
          headerSliverBuilder: (context, innerBoxIsScrolled) => <Widget>[
            SliverToBoxAdapter(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  _DiscoveryTopBar(
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
                    _CompactEnvBanner(
                      onInfo: _showEnvironmentNoticeDialog,
                      onDismiss: () =>
                          setState(() => _envBannerDismissed = true),
                    ),
                  ],
                  const SizedBox(height: 8),
                  _DiscoveryControlCard(
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
              delegate: _StickyTabBarDelegate(
                backgroundColor: Theme.of(context).scaffoldBackgroundColor,
                tabBar: TabBar(
                  controller: _tabController,
                  isScrollable: isCompact,
                  tabAlignment: isCompact
                      ? TabAlignment.start
                      : TabAlignment.fill,
                  labelPadding: const EdgeInsets.symmetric(horizontal: 14),
                  tabs: <Widget>[
                    Tab(text: 'Resultados (${allHosts.length})'),
                    Tab(text: 'Histórico (${discovery.runs.length})'),
                    Tab(text: 'Conflitos (${discovery.conflicts.length})'),
                  ],
                ),
              ),
            ),
          ],
          body: TabBarView(
            controller: _tabController,
            children: <Widget>[
              _DiscoveryResultsTab(
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
              _DiscoveryHistoryTab(
                runs: discovery.runs,
                onRefresh: () =>
                    ref.read(discoveryProvider.notifier).fetchDiscoveryRuns(),
              ),
              _DiscoveryConflictsTab(
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

class _DiscoveryTopBar extends StatelessWidget {
  const _DiscoveryTopBar({
    required this.isCompact,
    required this.canWrite,
    required this.loadingConflicts,
    required this.showEnvBadge,
    required this.onShowEnvNotice,
    required this.onCheckConflicts,
    required this.onCleanup,
  });

  final bool isCompact;
  final bool canWrite;
  final bool loadingConflicts;
  final bool showEnvBadge;
  final VoidCallback onShowEnvNotice;
  final VoidCallback onCheckConflicts;
  final VoidCallback onCleanup;

  @override
  Widget build(BuildContext context) {
    final warningPair = context.semanticColors.resolve(
      SemanticTone.warning,
      context.colorScheme,
    );
    return Row(
      crossAxisAlignment: CrossAxisAlignment.center,
      children: <Widget>[
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Text(
                'Descoberta de Rede',
                style:
                    (isCompact
                            ? context.textTheme.titleLarge
                            : context.textTheme.headlineSmall)
                        ?.copyWith(fontWeight: FontWeight.w700),
              ),
              if (!isCompact) ...<Widget>[
                const SizedBox(height: 2),
                Text(
                  'Varredura ARP/ICMP/SNMP ao vivo via SSE e adoção para o inventário',
                  style: context.textTheme.bodySmall?.copyWith(
                    color: context.colorScheme.onSurfaceVariant,
                  ),
                ),
              ],
            ],
          ),
        ),
        if (showEnvBadge)
          IconButton(
            tooltip: 'Aviso de modo bridge do container',
            onPressed: onShowEnvNotice,
            icon: Icon(
              Icons.warning_amber_rounded,
              color: warningPair.foreground,
            ),
          ),
        if (canWrite && !isCompact) ...<Widget>[
          OutlinedButton.icon(
            onPressed: loadingConflicts ? null : onCheckConflicts,
            icon: const Icon(Icons.report_problem_outlined, size: 18),
            label: const Text('Conflitos'),
          ),
          const SizedBox(width: 8),
          OutlinedButton.icon(
            onPressed: onCleanup,
            icon: const Icon(Icons.cleaning_services_outlined, size: 18),
            label: const Text('Limpar (+7d)'),
          ),
        ],
        if (canWrite && isCompact)
          PopupMenuButton<String>(
            tooltip: 'Mais ações',
            icon: const Icon(Icons.more_vert),
            onSelected: (value) {
              if (value == 'conflicts') onCheckConflicts();
              if (value == 'cleanup') onCleanup();
            },
            itemBuilder: (context) => <PopupMenuEntry<String>>[
              PopupMenuItem<String>(
                value: 'conflicts',
                enabled: !loadingConflicts,
                child: const ListTile(
                  dense: true,
                  contentPadding: EdgeInsets.zero,
                  leading: Icon(Icons.report_problem_outlined),
                  title: Text('Verificar conflitos'),
                ),
              ),
              const PopupMenuItem<String>(
                value: 'cleanup',
                child: ListTile(
                  dense: true,
                  contentPadding: EdgeInsets.zero,
                  leading: Icon(Icons.cleaning_services_outlined),
                  title: Text('Limpar execuções (+7d)'),
                ),
              ),
            ],
          ),
      ],
    );
  }
}

class _CompactEnvBanner extends StatelessWidget {
  const _CompactEnvBanner({required this.onInfo, required this.onDismiss});

  final VoidCallback onInfo;
  final VoidCallback onDismiss;

  @override
  Widget build(BuildContext context) {
    final pair = context.semanticColors.resolve(
      SemanticTone.warning,
      context.colorScheme,
    );
    return NestedPanel(
      tone: SemanticTone.warning,
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      child: Row(
        children: <Widget>[
          Icon(Icons.warning_amber_rounded, size: 18, color: pair.onContainer),
          const SizedBox(width: 8),
          Expanded(
            child: InkWell(
              onTap: onInfo,
              child: Text(
                'Container fora do modo host: ARP/MAC limitado à bridge.',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: context.textTheme.bodySmall?.copyWith(
                  color: pair.onContainer,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
          ),
          IconButton(
            visualDensity: VisualDensity.compact,
            padding: EdgeInsets.zero,
            constraints: const BoxConstraints(minWidth: 32, minHeight: 32),
            tooltip: 'Ocultar aviso',
            icon: Icon(Icons.close, size: 16, color: pair.onContainer),
            onPressed: onDismiss,
          ),
        ],
      ),
    );
  }
}

class _DiscoveryControlCard extends StatelessWidget {
  const _DiscoveryControlCard({
    required this.isCompact,
    required this.canWrite,
    required this.networks,
    required this.selectedNetworkId,
    required this.discovery,
    required this.showStatusStrip,
    required this.onNetworkChanged,
    required this.onStartScan,
    required this.onCancelScan,
    required this.onDismissStatus,
  });

  final bool isCompact;
  final bool canWrite;
  final List<NetworkModel> networks;
  final int? selectedNetworkId;
  final DiscoveryState discovery;
  final bool showStatusStrip;
  final ValueChanged<int?> onNetworkChanged;
  final VoidCallback onStartScan;
  final VoidCallback onCancelScan;
  final VoidCallback onDismissStatus;

  String _statusLabel(String raw) {
    return switch (raw.toLowerCase()) {
      'running' || 'scanning' => 'Varrendo',
      'completed' => 'Concluída',
      'failed' => 'Falhou',
      'cancelled' || 'canceled' => 'Cancelada',
      _ => raw,
    };
  }

  @override
  Widget build(BuildContext context) {
    final scan = discovery.scan;
    final statusTone = scan.status == 'failed'
        ? SemanticTone.error
        : scan.status == 'completed'
        ? SemanticTone.success
        : SemanticTone.info;
    final statusPair = context.semanticColors.resolve(
      statusTone,
      context.colorScheme,
    );

    return Card(
      elevation: 2,
      child: Padding(
        padding: EdgeInsets.all(isCompact ? 10 : 12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Row(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: <Widget>[
                Expanded(
                  child: AppDropdownFormField<int>(
                    initialValue: selectedNetworkId,
                    decoration: const InputDecoration(
                      labelText: 'Rede alvo',
                      isDense: true,
                      contentPadding: EdgeInsets.symmetric(
                        horizontal: 12,
                        vertical: 10,
                      ),
                    ),
                    items: networks
                        .map(
                          (n) => DropdownMenuItem<int>(
                            value: n.id,
                            child: Text(
                              '${n.name} (${n.cidr})',
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                        )
                        .toList(growable: false),
                    onChanged: discovery.isScanning ? null : onNetworkChanged,
                  ),
                ),
                if (canWrite) ...<Widget>[
                  const SizedBox(width: 8),
                  if (discovery.isScanning)
                    FilledButton.tonalIcon(
                      onPressed: onCancelScan,
                      icon: const Icon(Icons.stop, size: 18),
                      label: Text(isCompact ? 'Parar' : 'Cancelar'),
                    )
                  else
                    FilledButton.icon(
                      onPressed: selectedNetworkId == null ? null : onStartScan,
                      icon: const Icon(Icons.radar, size: 18),
                      label: Text(isCompact ? 'Varrer' : 'Iniciar varredura'),
                    ),
                ],
              ],
            ),
            if (showStatusStrip) ...<Widget>[
              const SizedBox(height: 8),
              Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: 10,
                  vertical: 6,
                ),
                decoration: BoxDecoration(
                  color: statusPair.container,
                  borderRadius: BorderRadius.circular(8),
                  border: Border.all(color: statusPair.border),
                ),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    Row(
                      children: <Widget>[
                        Icon(
                          scan.status == 'failed'
                              ? Icons.error_outline
                              : scan.status == 'completed'
                              ? Icons.check_circle_outline
                              : Icons.radar,
                          size: 16,
                          color: statusPair.onContainer,
                        ),
                        const SizedBox(width: 6),
                        Expanded(
                          child: Text(
                            discovery.isScanning
                                ? '${_statusLabel(scan.status)} · ${scan.phase}'
                                : '${_statusLabel(scan.status)} · ${countNoun(scan.hosts.length, 'host', 'hosts')}',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: context.textTheme.labelMedium?.copyWith(
                              color: statusPair.onContainer,
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                        ),
                        const SizedBox(width: 6),
                        Text(
                          '${scan.progressCurrent}/${scan.progressTotal}',
                          style: context.textTheme.labelSmall?.copyWith(
                            color: statusPair.onContainer,
                            fontWeight: FontWeight.w700,
                            fontFamily: 'monospace',
                          ),
                        ),
                        if (!discovery.isScanning && scan.status != 'failed')
                          IconButton(
                            visualDensity: VisualDensity.compact,
                            padding: EdgeInsets.zero,
                            constraints: const BoxConstraints(
                              minWidth: 28,
                              minHeight: 28,
                            ),
                            tooltip: 'Ocultar resumo',
                            icon: Icon(
                              Icons.close,
                              size: 15,
                              color: statusPair.onContainer,
                            ),
                            onPressed: onDismissStatus,
                          ),
                      ],
                    ),
                    if (discovery.isScanning) ...<Widget>[
                      const SizedBox(height: 6),
                      ClipRRect(
                        borderRadius: BorderRadius.circular(4),
                        child: LinearProgressIndicator(
                          minHeight: 4,
                          value: scan.progressTotal > 0
                              ? (scan.progressPercent / 100.0).clamp(0.0, 1.0)
                              : null,
                        ),
                      ),
                    ],
                    if (scan.error != null) ...<Widget>[
                      const SizedBox(height: 4),
                      Text(
                        scan.error!,
                        style: context.textTheme.bodySmall?.copyWith(
                          color: statusPair.onContainer,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _StickyTabBarDelegate extends SliverPersistentHeaderDelegate {
  const _StickyTabBarDelegate({
    required this.tabBar,
    required this.backgroundColor,
  });

  final TabBar tabBar;
  final Color backgroundColor;

  @override
  double get minExtent => tabBar.preferredSize.height + 1;

  @override
  double get maxExtent => tabBar.preferredSize.height + 1;

  @override
  Widget build(
    BuildContext context,
    double shrinkOffset,
    bool overlapsContent,
  ) {
    return ColoredBox(
      color: backgroundColor,
      child: Column(children: <Widget>[tabBar, const Divider(height: 1)]),
    );
  }

  @override
  bool shouldRebuild(covariant _StickyTabBarDelegate oldDelegate) {
    return oldDelegate.tabBar != tabBar ||
        oldDelegate.backgroundColor != backgroundColor;
  }
}

class _DiscoveryResultsTab extends StatelessWidget {
  const _DiscoveryResultsTab({
    required this.hosts,
    required this.totalCount,
    required this.newCount,
    required this.managedCount,
    required this.managedIps,
    required this.canWrite,
    required this.hostFilter,
    required this.searchController,
    required this.onSearchChanged,
    required this.onFilterChanged,
    required this.onOpenBatchAdopt,
    required this.onAdoptSingle,
    required this.onOpenDetails,
    required this.onRefresh,
  });

  final List<StreamedDiscoveryHost> hosts;
  final int totalCount;
  final int newCount;
  final int managedCount;
  final Set<String> managedIps;
  final bool canWrite;
  final String hostFilter;
  final TextEditingController searchController;
  final VoidCallback onSearchChanged;
  final ValueChanged<String> onFilterChanged;
  final VoidCallback onOpenBatchAdopt;
  final ValueChanged<StreamedDiscoveryHost> onAdoptSingle;
  final void Function(StreamedDiscoveryHost, bool isManaged, bool canWrite)
  onOpenDetails;
  final Future<void> Function() onRefresh;

  @override
  Widget build(BuildContext context) {
    return RefreshIndicator(
      onRefresh: onRefresh,
      child: CustomScrollView(
        slivers: <Widget>[
          SliverPadding(
            padding: const EdgeInsets.only(top: 10, bottom: 8),
            sliver: SliverToBoxAdapter(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  UnifiedFilterBar(
                    searchController: searchController,
                    searchHint: 'Filtrar IP, MAC, hostname ou fabricante...',
                    onSearchChanged: (_) => onSearchChanged(),
                    quickChips: <Widget>[
                      FilterChip(
                        label: Text('Todos ($totalCount)'),
                        selected: hostFilter == 'all',
                        onSelected: (_) => onFilterChanged('all'),
                      ),
                      FilterChip(
                        label: Text('Novos ($newCount)'),
                        selected: hostFilter == 'new',
                        onSelected: (_) => onFilterChanged('new'),
                      ),
                      FilterChip(
                        label: Text('Gerenciados ($managedCount)'),
                        selected: hostFilter == 'managed',
                        onSelected: (_) => onFilterChanged('managed'),
                      ),
                    ],
                  ),
                  if (canWrite && newCount > 1) ...<Widget>[
                    const SizedBox(height: 8),
                    Align(
                      alignment: Alignment.centerRight,
                      child: OutlinedButton.icon(
                        style: OutlinedButton.styleFrom(
                          visualDensity: VisualDensity.compact,
                          minimumSize: const Size(0, 36),
                          padding: const EdgeInsets.symmetric(horizontal: 12),
                        ),
                        onPressed: onOpenBatchAdopt,
                        icon: const Icon(
                          Icons.playlist_add_check_outlined,
                          size: 18,
                        ),
                        label: Text('Adotar em lote ($newCount)'),
                      ),
                    ),
                  ],
                ],
              ),
            ),
          ),
          if (hosts.isEmpty)
            const SliverFillRemaining(
              hasScrollBody: false,
              child: EmptyStateView(
                title: 'Nenhum host encontrado',
                description:
                    'Selecione uma rede acima e toque em "Varrer", ou ajuste os filtros.',
                icon: Icons.radar,
              ),
            )
          else
            SliverPadding(
              padding: const EdgeInsets.only(bottom: 16),
              sliver: SliverList.separated(
                itemCount: hosts.length,
                separatorBuilder: (context, index) => const SizedBox(height: 8),
                itemBuilder: (context, index) {
                  final host = hosts[index];
                  final managed = managedIps.contains(host.ipAddress);
                  return _DiscoveryHostCard(
                    host: host,
                    managed: managed,
                    canWrite: canWrite,
                    onTap: () => onOpenDetails(host, managed, canWrite),
                    onAdopt: () => onAdoptSingle(host),
                  );
                },
              ),
            ),
        ],
      ),
    );
  }
}

class _DiscoveryHostCard extends StatelessWidget {
  const _DiscoveryHostCard({
    required this.host,
    required this.managed,
    required this.canWrite,
    required this.onTap,
    required this.onAdopt,
  });

  final StreamedDiscoveryHost host;
  final bool managed;
  final bool canWrite;
  final VoidCallback onTap;
  final VoidCallback onAdopt;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final addressLine = host.addressSubtitle;
    final systemLine = host.systemOrVendorLabel;
    final portsLine = host.openPortsSummary;

    return Card(
      elevation: 2,
      shape: RoundedRectangleBorder(borderRadius: AppTheme.borderRadius),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        borderRadius: AppTheme.borderRadius,
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 12),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              Container(
                width: 38,
                height: 38,
                decoration: BoxDecoration(
                  color: scheme.primaryContainer,
                  borderRadius: BorderRadius.circular(10),
                ),
                child: Icon(
                  deviceTypeIcon(host.deviceType ?? 'other'),
                  size: 20,
                  color: scheme.onPrimaryContainer,
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      host.bestName,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.titleSmall?.copyWith(
                        fontWeight: FontWeight.w700,
                        color: scheme.onSurface,
                      ),
                    ),
                    if (addressLine != null) ...<Widget>[
                      const SizedBox(height: 3),
                      Text(
                        addressLine,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.labelSmall?.copyWith(
                          fontFamily: 'monospace',
                          fontWeight: FontWeight.w600,
                          color: scheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                    if (systemLine != null) ...<Widget>[
                      const SizedBox(height: 3),
                      Text(
                        systemLine,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.bodySmall?.copyWith(
                          fontWeight: FontWeight.w600,
                          color: scheme.primary,
                        ),
                      ),
                    ],
                    if (portsLine != null) ...<Widget>[
                      const SizedBox(height: 3),
                      Text(
                        portsLine,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.labelSmall?.copyWith(
                          color: scheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              const SizedBox(width: 8),
              if (managed)
                const SemanticChip(
                  label: 'Gerenciado',
                  tone: SemanticTone.success,
                  compact: true,
                )
              else if (canWrite)
                FilledButton.tonal(
                  style: FilledButton.styleFrom(
                    minimumSize: const Size(0, 36),
                    visualDensity: VisualDensity.compact,
                    tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    padding: const EdgeInsets.symmetric(horizontal: 12),
                  ),
                  onPressed: onAdopt,
                  child: const Text('Adotar'),
                )
              else
                const SemanticChip(
                  label: 'Novo',
                  tone: SemanticTone.info,
                  compact: true,
                ),
            ],
          ),
        ),
      ),
    );
  }
}

class _BatchAdoptSheetBody extends StatefulWidget {
  const _BatchAdoptSheetBody({
    required this.hosts,
    required this.onCancel,
    required this.onConfirm,
  });

  final List<StreamedDiscoveryHost> hosts;
  final VoidCallback onCancel;
  final ValueChanged<Set<String>> onConfirm;

  @override
  State<_BatchAdoptSheetBody> createState() => _BatchAdoptSheetBodyState();
}

class _BatchAdoptSheetBodyState extends State<_BatchAdoptSheetBody> {
  final Set<String> _selectedIps = <String>{};

  void _selectAll() {
    setState(() {
      for (final host in widget.hosts) {
        _selectedIps.add(host.ipAddress);
      }
    });
  }

  void _clearSelection() {
    setState(_selectedIps.clear);
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final allSelected = _selectedIps.length == widget.hosts.length;

    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Wrap(
          alignment: WrapAlignment.spaceBetween,
          crossAxisAlignment: WrapCrossAlignment.center,
          spacing: 8,
          runSpacing: 4,
          children: <Widget>[
            Text(
              countNoun(_selectedIps.length, 'selecionado', 'selecionados'),
              style: context.textTheme.labelLarge?.copyWith(
                fontWeight: FontWeight.w700,
                color: scheme.onSurface,
              ),
            ),
            Row(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                TextButton(
                  style: TextButton.styleFrom(
                    visualDensity: VisualDensity.compact,
                    tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    padding: const EdgeInsets.symmetric(horizontal: 8),
                  ),
                  onPressed: allSelected ? null : _selectAll,
                  child: const Text('Selecionar todos'),
                ),
                const SizedBox(width: 4),
                TextButton(
                  style: TextButton.styleFrom(
                    visualDensity: VisualDensity.compact,
                    tapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    padding: const EdgeInsets.symmetric(horizontal: 8),
                  ),
                  onPressed: _selectedIps.isEmpty ? null : _clearSelection,
                  child: const Text('Limpar'),
                ),
              ],
            ),
          ],
        ),
        const SizedBox(height: 8),
        ConstrainedBox(
          constraints: BoxConstraints(
            maxHeight: MediaQuery.sizeOf(context).height * 0.45,
          ),
          child: NestedPanel(
            padding: EdgeInsets.zero,
            child: Material(
              type: MaterialType.transparency,
              clipBehavior: Clip.antiAlias,
              borderRadius: AppTheme.borderRadius,
              child: ListView.separated(
                shrinkWrap: true,
                itemCount: widget.hosts.length,
                separatorBuilder: (context, index) => const Divider(height: 1),
                itemBuilder: (context, index) {
                  final host = widget.hosts[index];
                  final selected = _selectedIps.contains(host.ipAddress);
                  final subtitleParts = <String>[
                    ?host.addressSubtitle,
                    ?host.systemOrVendorLabel,
                  ];
                  return CheckboxListTile(
                    dense: true,
                    controlAffinity: ListTileControlAffinity.leading,
                    value: selected,
                    onChanged: (checked) {
                      setState(() {
                        if (checked ?? false) {
                          _selectedIps.add(host.ipAddress);
                        } else {
                          _selectedIps.remove(host.ipAddress);
                        }
                      });
                    },
                    secondary: Icon(
                      deviceTypeIcon(host.deviceType ?? 'other'),
                      size: 20,
                      color: scheme.primary,
                    ),
                    title: Text(
                      host.bestName,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: context.textTheme.bodyMedium?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    subtitle: subtitleParts.isNotEmpty
                        ? Text(
                            subtitleParts.join(' · '),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: context.textTheme.labelSmall?.copyWith(
                              fontFamily: 'monospace',
                              color: scheme.onSurfaceVariant,
                            ),
                          )
                        : null,
                  );
                },
              ),
            ),
          ),
        ),
        const SizedBox(height: 16),
        Wrap(
          alignment: WrapAlignment.end,
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            TextButton(
              onPressed: widget.onCancel,
              child: const Text('Cancelar'),
            ),
            FilledButton.icon(
              onPressed: _selectedIps.isEmpty
                  ? null
                  : () => widget.onConfirm(Set<String>.from(_selectedIps)),
              icon: const Icon(Icons.playlist_add, size: 18),
              label: const Text('Adotar selecionados'),
            ),
          ],
        ),
      ],
    );
  }
}

class _DiscoveryHistoryTab extends StatelessWidget {
  const _DiscoveryHistoryTab({required this.runs, required this.onRefresh});

  final List<DiscoveryRunModel> runs;
  final Future<void> Function() onRefresh;

  @override
  Widget build(BuildContext context) {
    return RefreshIndicator(
      onRefresh: onRefresh,
      child: runs.isEmpty
          ? ListView(
              children: const <Widget>[
                SizedBox(height: 32),
                EmptyStateView(
                  title: 'Nenhuma varredura registrada',
                  icon: Icons.history,
                ),
              ],
            )
          : ListView.separated(
              padding: const EdgeInsets.symmetric(vertical: 10),
              itemCount: runs.length,
              separatorBuilder: (context, index) => const SizedBox(height: 8),
              itemBuilder: (context, index) {
                final run = runs[index];
                return Card(
                  elevation: 2,
                  child: ListTile(
                    leading: StatusIndicator.fromStatus(
                      run.status,
                      compact: true,
                    ),
                    title: Text(
                      '${run.networkName ?? 'Rede #${run.networkId}'}${run.cidr != null ? ' (${run.cidr})' : ''}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(fontWeight: FontWeight.w700),
                    ),
                    subtitle: Text(
                      '${countNoun(run.devicesFound, 'host encontrado', 'hosts encontrados')} · '
                      'Iniciada: ${formatShortDateTime(run.startedAt)}',
                    ),
                    trailing: run.finishedAt != null
                        ? Text(formatTimeSpan(run.startedAt, run.finishedAt))
                        : null,
                  ),
                );
              },
            ),
    );
  }
}

class _DiscoveryConflictsTab extends StatelessWidget {
  const _DiscoveryConflictsTab({
    required this.conflicts,
    required this.onRefresh,
  });

  final List<NetworkConflictModel> conflicts;
  final Future<void> Function() onRefresh;

  @override
  Widget build(BuildContext context) {
    return RefreshIndicator(
      onRefresh: onRefresh,
      child: conflicts.isEmpty
          ? ListView(
              children: const <Widget>[
                SizedBox(height: 32),
                EmptyStateView(
                  title: 'Nenhum conflito de IP ou MAC detectado',
                  icon: Icons.verified_user_outlined,
                  tone: SemanticTone.success,
                ),
              ],
            )
          : ListView.separated(
              padding: const EdgeInsets.symmetric(vertical: 10),
              itemCount: conflicts.length,
              separatorBuilder: (context, index) => const SizedBox(height: 8),
              itemBuilder: (context, index) {
                final c = conflicts[index];
                return Card(
                  elevation: 2,
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
    );
  }
}
