import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/composition_registries.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/models/link_test_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/link_tests_provider.dart';
import 'package:netmonitor_app/features/network/providers/wifi_provider.dart';
import 'package:netmonitor_app/features/network/widgets/device_access_host.dart';
import 'package:netmonitor_app/features/network/widgets/device_form_sheet.dart';
import 'package:netmonitor_app/features/network/widgets/diagnostics_dialogs.dart';
import 'package:netmonitor_app/features/network/widgets/wifi_qr_sheet.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class DeviceDetailPage extends ConsumerStatefulWidget {
  const DeviceDetailPage({
    super.key,
    required this.deviceId,
    this.initialTab,
    this.initialSection,
  });

  final int deviceId;
  final String? initialTab;
  final String? initialSection;

  @override
  ConsumerState<DeviceDetailPage> createState() => _DeviceDetailPageState();
}

class _DeviceDetailPageState extends ConsumerState<DeviceDetailPage> {
  String _selectedTabId = 'overview';

  @override
  void initState() {
    super.initState();
    if (widget.initialTab != null && widget.initialTab!.isNotEmpty) {
      _selectedTabId = widget.initialTab!;
    }
    Future<void>.microtask(
      () => ref
          .read(deviceDetailProvider.notifier)
          .loadDeviceDetails(widget.deviceId),
    );
  }

  @override
  void didUpdateWidget(covariant DeviceDetailPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.deviceId != widget.deviceId) {
      Future<void>.microtask(
        () => ref
            .read(deviceDetailProvider.notifier)
            .loadDeviceDetails(widget.deviceId),
      );
    }
    if (widget.initialTab != null &&
        widget.initialTab != oldWidget.initialTab) {
      _selectedTabId = widget.initialTab!;
    }
  }

  Future<void> _confirmDelete(DeviceModel device) async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Excluir dispositivo',
      message: 'Remover "${device.name}" e todos os dados monitorados dele?',
      confirmLabel: 'Excluir',
    );
    if (!ok) return;
    final deleted = await ref
        .read(devicesProvider.notifier)
        .deleteDevice(device.id);
    if (deleted && mounted) context.go('/devices');
  }

  @override
  Widget build(BuildContext context) {
    final detail = ref.watch(deviceDetailProvider);
    final auth = ref.watch(authProvider);
    final visibleAreas = ref.watch(visibleAreasProvider);
    final networkEnabled = visibleAreas.contains(ProductArea.network);
    final device = detail.device;
    final caps = detail.capabilities;

    if (detail.loading && device == null) {
      return const LoadingStateView(
        message: 'Carregando detalhes do equipamento...',
      );
    }
    if (device == null) {
      return ErrorStateView(
        message: detail.error ?? 'Equipamento não encontrado.',
        onRetry: () => ref
            .read(deviceDetailProvider.notifier)
            .loadDeviceDetails(widget.deviceId),
      );
    }

    // Composição declarativa: abas nativas + aba Wi-Fi (plugin teal) + abas de Core/VPN/Plugins via DeviceDetailTabRegistry.
    final registryTabs = DeviceDetailTabRegistry.resolve(
      deviceId: widget.deviceId,
      visibleAreas: visibleAreas,
    );

    final tabs = <_ResolvedTab>[
      _ResolvedTab(
        id: 'overview',
        title: 'Visão Geral',
        icon: Icons.dashboard_outlined,
        isPlugin: false,
        builder: (_) => _DeviceOverviewTab(detail: detail),
      ),
      _ResolvedTab(
        id: 'interfaces',
        title: 'Interfaces (${detail.interfaces.length})',
        icon: Icons.settings_ethernet,
        isPlugin: false,
        builder: (_) => _DeviceInterfacesTab(
          device: device,
          detail: detail,
          canWrite: auth.canWrite,
        ),
      ),
      _ResolvedTab(
        id: 'credentials',
        title: 'Credenciais',
        icon: Icons.vpn_key_outlined,
        isPlugin: false,
        builder: (_) => SectionCard(
          title: 'Credenciais e Acesso Dedicado',
          subtitle: 'Transporte SSH/HTTP/Telnet e chave dedicada (ADR 012/016)',
          icon: Icons.lock_outline,
          child: DeviceAccessPanel(deviceId: device.id),
        ),
      ),
      _ResolvedTab(
        id: 'events',
        title: 'Eventos (${detail.events.length})',
        icon: Icons.event_note_outlined,
        isPlugin: false,
        builder: (_) => _DeviceEventsTab(events: detail.events),
      ),
      if (networkEnabled &&
          (caps?.wifi == true ||
              ref.watch(wifiClientsProvider).snapshotFor(device.id) != null))
        _ResolvedTab(
          id: 'wifi',
          title: 'Clientes Wi-Fi',
          icon: Icons.wifi,
          isPlugin: true,
          builder: (_) => _DeviceWifiTab(device: device),
        ),
      for (final ext in registryTabs)
        _ResolvedTab(
          id: ext.id,
          title: ext.title,
          icon: ext.icon,
          isPlugin:
              ext.tone == SemanticTone.plugin ||
              ext.id == 'plugins' ||
              ext.id.startsWith('plugin-'),
          builder: (ctx) => ext.builder(ctx, device.id),
        ),
    ];

    final activeIndex = tabs
        .indexWhere((t) => t.id == _selectedTabId)
        .clamp(0, tabs.length - 1);
    final activeTab = tabs[activeIndex];
    final tabKeySignature = tabs.map((t) => t.id).join('|');

    // Telemetria rápida extraída das métricas recentes
    double? latestCpu;
    double? latestMem;
    for (final m in detail.metrics.reversed) {
      if (latestCpu == null &&
          (m.metricName == 'cpu_usage' || m.metricName == 'cpuUsagePercent')) {
        latestCpu = m.metricValue;
      }
      if (latestMem == null &&
          (m.metricName == 'memory_usage' ||
              m.metricName == 'memoryUsagePercent')) {
        latestMem = m.metricValue;
      }
      if (latestCpu != null && latestMem != null) break;
    }
    final upIfaces = detail.interfaces
        .where((i) => i.operStatus?.toLowerCase() == 'up')
        .length;
    final upMonitors = detail.monitors
        .where((m) => m.status == 'up' || m.status == 'online')
        .length;

    Future<void> editDevice() async {
      final saved = await showDeviceFormSheet(
        context: context,
        existing: device,
      );
      if (saved != null) {
        ref.read(deviceDetailProvider.notifier).loadDeviceDetails(device.id);
      }
    }

    void openLinkTest() {
      ref.read(pendingLinkTestDraftProvider.notifier).state =
          LinkTestRequestDraft(kind: 'server', targetDeviceId: device.id);
      context.go('/link-tests?deviceId=${device.id}');
    }

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 680;
        return RefreshIndicator(
          onRefresh: () => ref
              .read(deviceDetailProvider.notifier)
              .loadDeviceDetails(widget.deviceId),
          child: CustomScrollView(
            slivers: <Widget>[
              SliverToBoxAdapter(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    // ── Header do equipamento ──
                    SectionCard(
                      padding: EdgeInsets.all(isCompact ? 12 : 16),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: <Widget>[
                          Row(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: <Widget>[
                              IconButton(
                                visualDensity: VisualDensity.compact,
                                tooltip: 'Voltar para Dispositivos',
                                onPressed: () => context.go('/devices'),
                                icon: const Icon(Icons.arrow_back),
                              ),
                              const SizedBox(width: 4),
                              Container(
                                width: isCompact ? 40 : 48,
                                height: isCompact ? 40 : 48,
                                decoration: BoxDecoration(
                                  color: context.colorScheme.primaryContainer,
                                  borderRadius: BorderRadius.circular(12),
                                ),
                                child: Icon(
                                  deviceTypeIcon(device.type),
                                  size: isCompact ? 22 : 26,
                                  color: context.colorScheme.onPrimaryContainer,
                                ),
                              ),
                              const SizedBox(width: 10),
                              Expanded(
                                child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: <Widget>[
                                    Wrap(
                                      spacing: 8,
                                      runSpacing: 4,
                                      crossAxisAlignment:
                                          WrapCrossAlignment.center,
                                      children: <Widget>[
                                        Text(
                                          device.name,
                                          style:
                                              (isCompact
                                                      ? context
                                                            .textTheme
                                                            .titleMedium
                                                      : context
                                                            .textTheme
                                                            .headlineSmall)
                                                  ?.copyWith(
                                                    fontWeight: FontWeight.w800,
                                                  ),
                                        ),
                                        StatusIndicator.fromStatus(
                                          device.status,
                                          compact: isCompact,
                                        ),
                                      ],
                                    ),
                                    const SizedBox(height: 4),
                                    Wrap(
                                      spacing: 6,
                                      runSpacing: 4,
                                      children: <Widget>[
                                        if (device.hasIp)
                                          SemanticChip(
                                            label: device.ipAddress!,
                                            tone: SemanticTone.primary,
                                            compact: true,
                                            icon: Icons.lan_outlined,
                                          ),
                                        if (device.macAddress != null &&
                                            !isCompact)
                                          SemanticChip(
                                            label: device.macAddress!,
                                            tone: SemanticTone.secondary,
                                            compact: true,
                                          ),
                                        if (device.site != null)
                                          SemanticChip(
                                            label: device.site!.name,
                                            tone: SemanticTone.info,
                                            compact: true,
                                            icon: Icons.domain_outlined,
                                          ),
                                        if (!isCompact &&
                                            (device.vendor != null ||
                                                device.model != null))
                                          SemanticChip(
                                            label: [
                                              if (device.vendor != null)
                                                device.vendor!,
                                              if (device.model != null)
                                                device.model!,
                                            ].join(' '),
                                            tone: SemanticTone.info,
                                            compact: true,
                                          ),
                                      ],
                                    ),
                                  ],
                                ),
                              ),
                              if (isCompact) ...<Widget>[
                                if (auth.canWrite)
                                  IconButton(
                                    visualDensity: VisualDensity.compact,
                                    tooltip: 'Editar',
                                    onPressed: editDevice,
                                    icon: const Icon(Icons.edit_outlined),
                                  ),
                                PopupMenuButton<String>(
                                  tooltip: 'Mais ações',
                                  icon: const Icon(Icons.more_vert),
                                  onSelected: (action) {
                                    switch (action) {
                                      case 'ping':
                                        showQuickPingDialog(
                                          context,
                                          deviceId: device.id,
                                          deviceName: device.name,
                                        );
                                      case 'playbook':
                                        showDiagnosticPlaybookDialog(
                                          context,
                                          device: device,
                                        );
                                      case 'traceroute':
                                        showTracerouteDialog(
                                          context,
                                          initialHost: device.ipAddress,
                                        );
                                      case 'portscan':
                                        showPortScanDialog(
                                          context,
                                          initialHost: device.ipAddress,
                                        );
                                      case 'snmp_poll':
                                        ref
                                            .read(deviceDetailProvider.notifier)
                                            .triggerSnmpPoll(device.id);
                                      case 'snmp_scan':
                                        showSnmpScanDialog(
                                          context,
                                          deviceId: device.id,
                                          deviceName: device.name,
                                        );
                                      case 'link_test':
                                        openLinkTest();
                                      case 'delete':
                                        _confirmDelete(device);
                                    }
                                  },
                                  itemBuilder: (_) => <PopupMenuEntry<String>>[
                                    if (auth.canWrite &&
                                        !device.isSystem &&
                                        device.hasIp)
                                      const PopupMenuItem<String>(
                                        value: 'ping',
                                        child: ListTile(
                                          dense: true,
                                          contentPadding: EdgeInsets.zero,
                                          leading: Icon(Icons.network_ping),
                                          title: Text('Ping Rápido'),
                                        ),
                                      ),
                                    const PopupMenuItem<String>(
                                      value: 'playbook',
                                      child: ListTile(
                                        dense: true,
                                        contentPadding: EdgeInsets.zero,
                                        leading: Icon(Icons.playlist_play),
                                        title: Text('Playbook'),
                                      ),
                                    ),
                                    if (device
                                        .hasIp) ...<PopupMenuEntry<String>>[
                                      const PopupMenuItem<String>(
                                        value: 'traceroute',
                                        child: ListTile(
                                          dense: true,
                                          contentPadding: EdgeInsets.zero,
                                          leading: Icon(Icons.alt_route),
                                          title: Text('Traceroute'),
                                        ),
                                      ),
                                      const PopupMenuItem<String>(
                                        value: 'portscan',
                                        child: ListTile(
                                          dense: true,
                                          contentPadding: EdgeInsets.zero,
                                          leading: Icon(Icons.radar),
                                          title: Text('Port Scan'),
                                        ),
                                      ),
                                    ],
                                    if (auth.canWrite &&
                                        device.snmpEnabled) ...<
                                      PopupMenuEntry<String>
                                    >[
                                      PopupMenuItem<String>(
                                        value: 'snmp_poll',
                                        enabled: !detail.pollingSnmp,
                                        child: const ListTile(
                                          dense: true,
                                          contentPadding: EdgeInsets.zero,
                                          leading: Icon(Icons.sync),
                                          title: Text('Poll SNMP'),
                                        ),
                                      ),
                                      const PopupMenuItem<String>(
                                        value: 'snmp_scan',
                                        child: ListTile(
                                          dense: true,
                                          contentPadding: EdgeInsets.zero,
                                          leading: Icon(Icons.manage_search),
                                          title: Text('Escanear SNMP'),
                                        ),
                                      ),
                                    ],
                                    if (networkEnabled)
                                      const PopupMenuItem<String>(
                                        value: 'link_test',
                                        child: ListTile(
                                          dense: true,
                                          contentPadding: EdgeInsets.zero,
                                          leading: Icon(Icons.speed),
                                          title: Text('Testar Enlace'),
                                        ),
                                      ),
                                    if (auth.canWrite && !device.isSystem) ...<
                                      PopupMenuEntry<String>
                                    >[
                                      const PopupMenuDivider(),
                                      PopupMenuItem<String>(
                                        value: 'delete',
                                        child: ListTile(
                                          dense: true,
                                          contentPadding: EdgeInsets.zero,
                                          leading: Icon(
                                            Icons.delete_outline,
                                            color: context.colorScheme.error,
                                          ),
                                          title: Text(
                                            'Excluir',
                                            style: TextStyle(
                                              color: context.colorScheme.error,
                                            ),
                                          ),
                                        ),
                                      ),
                                    ],
                                  ],
                                ),
                              ],
                            ],
                          ),
                          if (!isCompact) ...<Widget>[
                            const SizedBox(height: 12),
                            const Divider(height: 1),
                            const SizedBox(height: 10),
                            Wrap(
                              spacing: 8,
                              runSpacing: 8,
                              children: <Widget>[
                                if (auth.canWrite)
                                  OutlinedButton.icon(
                                    onPressed: editDevice,
                                    icon: const Icon(
                                      Icons.edit_outlined,
                                      size: 18,
                                    ),
                                    label: const Text('Editar'),
                                  ),
                                if (auth.canWrite &&
                                    !device.isSystem &&
                                    device.hasIp)
                                  FilledButton.tonalIcon(
                                    onPressed: () => showQuickPingDialog(
                                      context,
                                      deviceId: device.id,
                                      deviceName: device.name,
                                    ),
                                    icon: const Icon(
                                      Icons.network_ping,
                                      size: 18,
                                    ),
                                    label: const Text('Ping Rápido'),
                                  ),
                                OutlinedButton.icon(
                                  onPressed: () => showDiagnosticPlaybookDialog(
                                    context,
                                    device: device,
                                  ),
                                  icon: const Icon(
                                    Icons.playlist_play,
                                    size: 18,
                                  ),
                                  label: const Text('Playbook'),
                                ),
                                if (device.hasIp) ...<Widget>[
                                  OutlinedButton.icon(
                                    onPressed: () => showTracerouteDialog(
                                      context,
                                      initialHost: device.ipAddress,
                                    ),
                                    icon: const Icon(Icons.alt_route, size: 18),
                                    label: const Text('Traceroute'),
                                  ),
                                  OutlinedButton.icon(
                                    onPressed: () => showPortScanDialog(
                                      context,
                                      initialHost: device.ipAddress,
                                    ),
                                    icon: const Icon(Icons.radar, size: 18),
                                    label: const Text('Port Scan'),
                                  ),
                                ],
                                if (auth.canWrite &&
                                    device.snmpEnabled) ...<Widget>[
                                  OutlinedButton.icon(
                                    onPressed: detail.pollingSnmp
                                        ? null
                                        : () => ref
                                              .read(
                                                deviceDetailProvider.notifier,
                                              )
                                              .triggerSnmpPoll(device.id),
                                    icon: detail.pollingSnmp
                                        ? const SizedBox(
                                            width: 15,
                                            height: 15,
                                            child: CircularProgressIndicator(
                                              strokeWidth: 2,
                                            ),
                                          )
                                        : const Icon(Icons.sync, size: 18),
                                    label: const Text('Poll SNMP'),
                                  ),
                                  OutlinedButton.icon(
                                    onPressed: () => showSnmpScanDialog(
                                      context,
                                      deviceId: device.id,
                                      deviceName: device.name,
                                    ),
                                    icon: const Icon(
                                      Icons.manage_search,
                                      size: 18,
                                    ),
                                    label: const Text('Escanear SNMP'),
                                  ),
                                ],
                                if (networkEnabled)
                                  OutlinedButton.icon(
                                    onPressed: openLinkTest,
                                    icon: const Icon(Icons.speed, size: 18),
                                    label: const Text('Testar Enlace'),
                                  ),
                                if (auth.canWrite && !device.isSystem)
                                  TextButton.icon(
                                    onPressed: () => _confirmDelete(device),
                                    icon: Icon(
                                      Icons.delete_outline,
                                      size: 18,
                                      color: context.colorScheme.error,
                                    ),
                                    label: Text(
                                      'Excluir',
                                      style: TextStyle(
                                        color: context.colorScheme.error,
                                      ),
                                    ),
                                  ),
                              ],
                            ),
                          ],
                        ],
                      ),
                    ),
                    const SizedBox(height: 10),
                    // ── Saúde e Telemetria (2x2 simétrico no mobile, 4 colunas no desktop) ──
                    ResponsiveGrid(
                      xs: 2,
                      sm: 2,
                      md: 4,
                      lg: 4,
                      xl: 4,
                      spacing: isCompact ? 8 : 12,
                      runSpacing: isCompact ? 8 : 12,
                      children: <Widget>[
                        KpiCard(
                          label: 'CPU',
                          value: latestCpu != null
                              ? formatPercent(latestCpu)
                              : '—',
                          icon: Icons.memory,
                          tone: latestCpu != null && latestCpu > 85
                              ? SemanticTone.error
                              : SemanticTone.primary,
                        ),
                        KpiCard(
                          label: 'Memória',
                          value: latestMem != null
                              ? formatPercent(latestMem)
                              : '—',
                          icon: Icons.sd_storage_outlined,
                          tone: latestMem != null && latestMem > 85
                              ? SemanticTone.warning
                              : SemanticTone.info,
                        ),
                        KpiCard(
                          label: 'Interfaces',
                          value: '$upIfaces / ${detail.interfaces.length}',
                          subtitle: 'Ativas',
                          icon: Icons.settings_ethernet,
                          tone: SemanticTone.success,
                        ),
                        KpiCard(
                          label: 'Monitores',
                          value: '$upMonitors / ${detail.monitors.length}',
                          subtitle: 'Saudáveis',
                          icon: Icons.monitor_heart_outlined,
                          tone: upMonitors == detail.monitors.length
                              ? SemanticTone.success
                              : SemanticTone.warning,
                        ),
                      ],
                    ),
                    const SizedBox(height: 10),
                  ],
                ),
              ),
              // ── Barra de abas fixa com TabBar rolável e distinção PLUGIN (Teal #00796B / #1DE9B6) ──
              SliverPersistentHeader(
                pinned: true,
                delegate: _DeviceDetailTabHeaderDelegate(
                  backgroundColor: Theme.of(context).scaffoldBackgroundColor,
                  child: _DeviceDetailTabBar(
                    key: ValueKey<String>(tabKeySignature),
                    tabs: tabs,
                    selectedIndex: activeIndex,
                    onSelected: (index) {
                      setState(() => _selectedTabId = tabs[index].id);
                    },
                  ),
                ),
              ),
              SliverPadding(
                padding: const EdgeInsets.only(top: 12, bottom: 24),
                sliver: SliverToBoxAdapter(child: activeTab.builder(context)),
              ),
            ],
          ),
        );
      },
    );
  }
}

class _DeviceDetailTabHeaderDelegate extends SliverPersistentHeaderDelegate {
  const _DeviceDetailTabHeaderDelegate({
    required this.child,
    required this.backgroundColor,
  });

  final Widget child;
  final Color backgroundColor;

  @override
  double get minExtent => 49;

  @override
  double get maxExtent => 49;

  @override
  Widget build(
    BuildContext context,
    double shrinkOffset,
    bool overlapsContent,
  ) {
    return ColoredBox(
      color: backgroundColor,
      child: Column(
        children: <Widget>[
          Expanded(child: child),
          const Divider(height: 1),
        ],
      ),
    );
  }

  @override
  bool shouldRebuild(covariant _DeviceDetailTabHeaderDelegate oldDelegate) {
    return oldDelegate.child != child ||
        oldDelegate.backgroundColor != backgroundColor;
  }
}

class _DeviceDetailTabBar extends StatefulWidget {
  const _DeviceDetailTabBar({
    super.key,
    required this.tabs,
    required this.selectedIndex,
    required this.onSelected,
  });

  final List<_ResolvedTab> tabs;
  final int selectedIndex;
  final ValueChanged<int> onSelected;

  @override
  State<_DeviceDetailTabBar> createState() => _DeviceDetailTabBarState();
}

class _DeviceDetailTabBarState extends State<_DeviceDetailTabBar>
    with SingleTickerProviderStateMixin {
  late final TabController _controller;

  @override
  void initState() {
    super.initState();
    _controller = TabController(
      length: widget.tabs.length,
      vsync: this,
      initialIndex: widget.selectedIndex.clamp(0, widget.tabs.length - 1),
    );
  }

  @override
  void didUpdateWidget(covariant _DeviceDetailTabBar oldWidget) {
    super.didUpdateWidget(oldWidget);
    final nextIdx = widget.selectedIndex.clamp(0, widget.tabs.length - 1);
    if (_controller.index != nextIdx) {
      _controller.animateTo(nextIdx);
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final pluginColors = context.semanticColors.resolve(
      SemanticTone.plugin,
      scheme,
    );
    final activeTab = widget.tabs[widget.selectedIndex];

    return TabBar(
      controller: _controller,
      isScrollable: true,
      tabAlignment: TabAlignment.start,
      dividerColor: Colors.transparent,
      indicatorColor: activeTab.isPlugin
          ? pluginColors.foreground
          : scheme.primary,
      labelPadding: const EdgeInsets.symmetric(horizontal: 12),
      onTap: widget.onSelected,
      tabs: <Widget>[
        for (var i = 0; i < widget.tabs.length; i++)
          Builder(
            builder: (_) {
              final tab = widget.tabs[i];
              final selected = i == widget.selectedIndex;
              final fg = tab.isPlugin
                  ? (selected
                        ? pluginColors.onContainer
                        : pluginColors.foreground)
                  : (selected ? scheme.primary : scheme.onSurfaceVariant);

              return Tab(
                child: Container(
                  padding: tab.isPlugin
                      ? const EdgeInsets.symmetric(horizontal: 8, vertical: 4)
                      : EdgeInsets.zero,
                  decoration: tab.isPlugin
                      ? BoxDecoration(
                          color: selected
                              ? pluginColors.container
                              : Colors.transparent,
                          borderRadius: BorderRadius.circular(8),
                          border: Border.all(
                            color: selected
                                ? pluginColors.foreground
                                : pluginColors.border,
                          ),
                        )
                      : null,
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      Icon(tab.icon, size: 17, color: fg),
                      const SizedBox(width: 6),
                      Text(
                        tab.title,
                        maxLines: 1,
                        softWrap: false,
                        style: TextStyle(
                          fontWeight: selected
                              ? FontWeight.w800
                              : FontWeight.w600,
                          color: fg,
                        ),
                      ),
                    ],
                  ),
                ),
              );
            },
          ),
      ],
    );
  }
}

class _ResolvedTab {
  const _ResolvedTab({
    required this.id,
    required this.title,
    required this.icon,
    required this.isPlugin,
    required this.builder,
  });

  final String id;
  final String title;
  final IconData icon;
  final bool isPlugin;
  final WidgetBuilder builder;
}

class _DeviceOverviewTab extends StatelessWidget {
  const _DeviceOverviewTab({required this.detail});

  final DeviceDetailState detail;

  @override
  Widget build(BuildContext context) {
    final d = detail.device!;
    final cpuPoints = <MetricSeriesPoint>[];
    final memPoints = <MetricSeriesPoint>[];
    for (final m in detail.metrics) {
      if (m.metricName == 'cpu_usage' || m.metricName == 'cpuUsagePercent') {
        cpuPoints.add(
          MetricSeriesPoint(x: cpuPoints.length.toDouble(), y: m.metricValue),
        );
      } else if (m.metricName == 'memory_usage' ||
          m.metricName == 'memoryUsagePercent') {
        memPoints.add(
          MetricSeriesPoint(x: memPoints.length.toDouble(), y: m.metricValue),
        );
      }
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        ResponsiveGrid(
          xs: 1,
          sm: 1,
          md: 2,
          lg: 2,
          xl: 2,
          children: <Widget>[
            SectionCard(
              title: 'Identidade e Rede',
              icon: Icons.info_outline,
              child: Column(
                children: <Widget>[
                  KeyValueRow(label: 'Tipo', value: deviceTypeLabel(d.type)),
                  KeyValueRow(
                    label: 'Endereço IP',
                    value: d.ipAddress ?? '—',
                    monospace: true,
                    copyable: d.hasIp,
                  ),
                  KeyValueRow(
                    label: 'Endereço MAC',
                    value: d.macAddress ?? '—',
                    monospace: true,
                    copyable: d.macAddress != null,
                  ),
                  KeyValueRow(
                    label: 'Fabricante / Modelo',
                    value: [
                      if (d.vendor != null) d.vendor!,
                      if (d.model != null) d.model!,
                    ].join(' · ').ifEmpty('—'),
                  ),
                  KeyValueRow(
                    label: 'Sistema Operacional',
                    value:
                        d.effectiveOperatingSystem ??
                        d.operatingSystem ??
                        'Automático',
                  ),
                  KeyValueRow(label: 'Site', value: d.site?.name ?? '—'),
                  KeyValueRow(
                    label: 'Rede IP',
                    value: d.network != null
                        ? '${d.network!.name} (${d.network!.cidr ?? ''})'
                        : '—',
                  ),
                  KeyValueRow(
                    label: 'Pai na hierarquia',
                    value: d.parent?.name ?? 'Raiz',
                  ),
                  KeyValueRow(
                    label: 'Última atividade',
                    value: formatRelativeTime(d.lastSeenAt),
                  ),
                ],
              ),
            ),
            SectionCard(
              title: 'Disponibilidade (24h)',
              subtitle: 'Resumo dos monitores associados',
              icon: Icons.monitor_heart_outlined,
              iconTone: SemanticTone.success,
              child: detail.monitors.isEmpty
                  ? const EmptyStateView(
                      title: 'Nenhum monitor cadastrado',
                      icon: Icons.monitor_heart_outlined,
                    )
                  : Column(
                      children: detail.monitors
                          .map((m) {
                            final uptime = detail.uptimeByMonitor[m.id];
                            return ListTile(
                              dense: true,
                              contentPadding: EdgeInsets.zero,
                              leading: StatusIndicator.fromStatus(
                                m.status,
                                compact: true,
                              ),
                              title: Text(
                                m.name,
                                style: const TextStyle(
                                  fontWeight: FontWeight.w700,
                                ),
                              ),
                              subtitle: Text(
                                uptime != null
                                    ? 'Uptime 24h: ${formatPercent(uptime.uptimePercentage)} · ${uptime.totalChecks} checks'
                                    : 'Latência: ${formatLatency(m.latencyMs)}',
                              ),
                              trailing: Text(
                                formatLatency(
                                  uptime?.avgLatencyMs ?? m.latencyMs,
                                ),
                                style: const TextStyle(
                                  fontWeight: FontWeight.w700,
                                ),
                              ),
                            );
                          })
                          .toList(growable: false),
                    ),
            ),
          ],
        ),
        if (cpuPoints.isNotEmpty || memPoints.isNotEmpty) ...<Widget>[
          const SizedBox(height: 12),
          SectionCard(
            title: 'Telemetria SNMP (CPU e Memória)',
            subtitle: 'Atualizada ao vivo pelo barramento SSE',
            icon: Icons.show_chart,
            child: MetricLineChart(
              height: 210,
              minY: 0,
              maxY: 100,
              yFormatter: (v) => '${v.round()}%',
              series: <MetricLineSeries>[
                MetricLineSeries(
                  name: 'CPU %',
                  points: cpuPoints,
                  tone: SemanticTone.primary,
                ),
                MetricLineSeries(
                  name: 'Memória %',
                  points: memPoints,
                  tone: SemanticTone.info,
                ),
              ],
            ),
          ),
        ],
      ],
    );
  }
}

class _DeviceInterfacesTab extends ConsumerWidget {
  const _DeviceInterfacesTab({
    required this.device,
    required this.detail,
    required this.canWrite,
  });

  final DeviceModel device;
  final DeviceDetailState detail;
  final bool canWrite;

  void _openTrafficChart(BuildContext context, DeviceInterfaceModel iface) {
    final rx = <MetricSeriesPoint>[];
    final tx = <MetricSeriesPoint>[];
    for (final m in detail.metrics) {
      if (m.interfaceId != iface.id) continue;
      if (m.metricName == 'inBps' || m.metricName == 'interface_in_bps') {
        rx.add(MetricSeriesPoint(x: rx.length.toDouble(), y: m.metricValue));
      } else if (m.metricName == 'outBps' ||
          m.metricName == 'interface_out_bps') {
        tx.add(MetricSeriesPoint(x: tx.length.toDouble(), y: m.metricValue));
      }
    }

    showAdaptiveFormSheet<void>(
      context: context,
      title: 'Tráfego · ${iface.displayName}',
      subtitle: 'RX/TX ao vivo (${formatLinkSpeed(iface.speed)})',
      maxWidth: 640,
      builder: (_) => MetricLineChart(
        height: 240,
        yFormatter: (v) => formatBps(v),
        series: <MetricLineSeries>[
          MetricLineSeries(
            name: 'Entrada (RX)',
            points: rx,
            tone: SemanticTone.success,
          ),
          MetricLineSeries(
            name: 'Saída (TX)',
            points: tx,
            tone: SemanticTone.info,
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    if (detail.interfaces.isEmpty) {
      return SectionCard(
        title: 'Interfaces SNMP',
        icon: Icons.settings_ethernet,
        child: EmptyStateView(
          title: 'Nenhuma interface descoberta',
          description:
              'Execute "Escanear SNMP" para descobrir e monitorar portas.',
          icon: Icons.settings_ethernet,
          action: canWrite && device.snmpEnabled
              ? FilledButton.icon(
                  onPressed: () => showSnmpScanDialog(
                    context,
                    deviceId: device.id,
                    deviceName: device.name,
                  ),
                  icon: const Icon(Icons.manage_search),
                  label: const Text('Escanear SNMP'),
                )
              : null,
        ),
      );
    }

    return SectionCard(
      title: 'Interfaces de Rede (${detail.interfaces.length})',
      subtitle: 'Tráfego ao vivo via SSE (interface:traffic)',
      icon: Icons.settings_ethernet,
      child: Column(
        children: detail.interfaces
            .map((iface) {
              final busy = detail.updatingInterfaceId == iface.id;
              return Column(
                children: <Widget>[
                  ListTile(
                    contentPadding: EdgeInsets.zero,
                    leading: StatusIndicator.fromStatus(
                      iface.operStatus ?? 'unknown',
                      compact: true,
                    ),
                    title: Text(
                      iface.displayName,
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(fontWeight: FontWeight.w700),
                    ),
                    subtitle: Text(
                      [
                        formatLinkSpeed(iface.speed),
                        if (iface.inBps != null) 'RX ${formatBps(iface.inBps)}',
                        if (iface.outBps != null)
                          'TX ${formatBps(iface.outBps)}',
                        if (iface.alias != null && iface.alias!.isNotEmpty)
                          iface.alias!,
                      ].join(' · '),
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                    ),
                    trailing: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        IconButton(
                          tooltip: 'Gráfico de tráfego',
                          icon: const Icon(Icons.show_chart),
                          onPressed: () => _openTrafficChart(context, iface),
                        ),
                        if (canWrite)
                          busy
                              ? const SizedBox(
                                  width: 24,
                                  height: 24,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : Switch(
                                  value: iface.isMonitored,
                                  onChanged: (v) => ref
                                      .read(deviceDetailProvider.notifier)
                                      .setInterfaceMonitoring(
                                        device.id,
                                        iface.id,
                                        v,
                                      ),
                                ),
                      ],
                    ),
                  ),
                  const Divider(height: 1),
                ],
              );
            })
            .toList(growable: false),
      ),
    );
  }
}

class _DeviceEventsTab extends StatelessWidget {
  const _DeviceEventsTab({required this.events});

  final List<DeviceEventModel> events;

  @override
  Widget build(BuildContext context) {
    if (events.isEmpty) {
      return const SectionCard(
        title: 'Eventos do Equipamento',
        icon: Icons.event_note_outlined,
        child: EmptyStateView(
          title: 'Nenhum evento recente',
          icon: Icons.event_available_outlined,
        ),
      );
    }

    return SectionCard(
      title: 'Linha do Tempo de Eventos',
      icon: Icons.event_note_outlined,
      child: Column(
        children: events
            .map((evt) {
              final tone = switch (evt.severity) {
                'critical' || 'error' => SemanticTone.error,
                'warning' => SemanticTone.warning,
                _ => SemanticTone.info,
              };
              return ListTile(
                dense: true,
                contentPadding: EdgeInsets.zero,
                leading: SemanticChip(
                  label: evt.severity.toUpperCase(),
                  tone: tone,
                  compact: true,
                ),
                title: Text(evt.message),
                subtitle: Text(
                  '${evt.eventType} · ${formatShortDateTime(evt.createdAt)}',
                ),
              );
            })
            .toList(growable: false),
      ),
    );
  }
}

class _DeviceWifiTab extends ConsumerWidget {
  const _DeviceWifiTab({required this.device});

  final DeviceModel device;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final wifi = ref.watch(wifiClientsProvider);
    final snapshot = wifi.snapshotFor(device.id);
    final canWrite = ref.watch(authProvider).canWrite;

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 540;
        final actions = Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            OutlinedButton.icon(
              onPressed: () => showWifiQrSheet(
                context,
                initialSsid: snapshot?.stations.firstOrNull?.ssid ?? '',
                deviceId: device.id,
              ),
              icon: const Icon(Icons.qr_code_2, size: 18),
              label: const Text('QR Wi-Fi'),
            ),
            if (canWrite)
              FilledButton.tonalIcon(
                onPressed: wifi.requesting
                    ? null
                    : () => ref
                          .read(wifiClientsProvider.notifier)
                          .refresh(deviceId: device.id),
                icon: const Icon(Icons.refresh, size: 18),
                label: const Text('Atualizar AP'),
              ),
          ],
        );
        return SectionCard(
          title: 'Clientes Wi-Fi · ${device.name}',
          subtitle: snapshot?.readAt != null
              ? 'Última leitura ${formatRelativeTime(snapshot!.readAt)}'
              : 'Dados em tempo real via plugin Wi-Fi',
          icon: Icons.wifi,
          iconTone: SemanticTone.plugin,
          trailing: isCompact ? null : actions,
          footer: isCompact ? actions : null,
          child: snapshot == null || snapshot.stations.isEmpty
              ? const EmptyStateView(
                  title: 'Nenhum cliente Wi-Fi conectado neste AP',
                  icon: Icons.wifi_off,
                  tone: SemanticTone.plugin,
                )
              : Column(
                  children: snapshot.stations
                      .map((st) {
                        return ListTile(
                          dense: true,
                          contentPadding: EdgeInsets.zero,
                          leading: SemanticChip(
                            label: '${st.signalDbm} dBm',
                            tone: st.signalDbm >= -60
                                ? SemanticTone.success
                                : st.signalDbm >= -75
                                ? SemanticTone.warning
                                : SemanticTone.error,
                            compact: true,
                          ),
                          title: Text(
                            st.hostname ?? st.vendor ?? st.mac,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(fontWeight: FontWeight.w700),
                          ),
                          subtitle: Text(
                            '${st.mac}${st.ip != null ? ' · ${st.ip}' : ''}'
                            '${st.ssid != null ? ' · ${st.ssid}' : ''}'
                            '${st.band != null ? ' (${st.band})' : ''}',
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                          ),
                          trailing: st.distance != null
                              ? Text(
                                  formatDistanceRange(
                                    st.distance!.minM,
                                    st.distance!.maxM,
                                  ),
                                )
                              : null,
                        );
                      })
                      .toList(growable: false),
                ),
        );
      },
    );
  }
}

extension _StringFallback on String {
  String ifEmpty(String fallback) => trim().isEmpty ? fallback : this;
}
