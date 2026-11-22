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
import 'package:netmonitor_app/features/network/widgets/device_detail/device_detail_tabs.dart';
import 'package:netmonitor_app/features/network/widgets/device_form_sheet.dart';
import 'package:netmonitor_app/features/network/widgets/diagnostics_dialogs.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

export 'package:netmonitor_app/features/network/widgets/device_detail/device_detail_tabs.dart';

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
        builder: (_) => DeviceOverviewTab(detail: detail),
      ),
      _ResolvedTab(
        id: 'interfaces',
        title: 'Interfaces (${detail.interfaces.length})',
        icon: Icons.settings_ethernet,
        isPlugin: false,
        builder: (_) => DeviceInterfacesTab(
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
        builder: (_) => DeviceEventsTab(events: detail.events),
      ),
      if (networkEnabled &&
          (caps?.wifi == true ||
              ref.watch(wifiClientsProvider).snapshotFor(device.id) != null))
        _ResolvedTab(
          id: 'wifi',
          title: 'Clientes Wi-Fi',
          icon: Icons.wifi,
          isPlugin: true,
          builder: (_) => DeviceWifiTab(device: device),
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
              SliverPersistentHeader(
                pinned: true,
                delegate: PinnedSliverHeaderDelegate(
                  height: 49,
                  backgroundColor: Theme.of(context).scaffoldBackgroundColor,
                  bottomBorder: true,
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
