import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/device_presentation.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/wifi_models.dart';
import 'package:netmonitor_app/features/network/providers/wifi_provider.dart';
import 'package:netmonitor_app/features/network/widgets/wifi/wifi_signal_map_view.dart';
import 'package:netmonitor_app/features/network/widgets/wifi/wifi_station_sheets.dart';
import 'package:netmonitor_app/features/network/widgets/wifi_qr_sheet.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

export 'package:netmonitor_app/features/network/widgets/wifi/wifi_signal_map_view.dart';
export 'package:netmonitor_app/features/network/widgets/wifi/wifi_station_sheets.dart';

class WifiClientsPage extends ConsumerStatefulWidget {
  const WifiClientsPage({super.key, this.initialTab});

  final String? initialTab;

  @override
  ConsumerState<WifiClientsPage> createState() => _WifiClientsPageState();
}

class _WifiClientsPageState extends ConsumerState<WifiClientsPage>
    with SingleTickerProviderStateMixin {
  late final TabController _tabController;
  final TextEditingController _searchCtrl = TextEditingController();
  int? _apFilter;
  String _ssidFilter = 'all';
  String _bandFilter = 'all';
  String _gradeFilter = 'all';
  bool _watchedOnly = false;
  bool _isMapTab = false;
  VoidCallback? _releaseWifiInterest;

  @override
  void initState() {
    super.initState();
    _tabController = TabController(
      length: 2,
      vsync: this,
      initialIndex: widget.initialTab == 'map' ? 1 : 0,
    );
    _isMapTab = _tabController.index == 1;
    _tabController.addListener(_syncWifiInterest);
    // Adquire interesse SSE `'wifi'` (ZERO polling — AGENTS.md §9).
    Future<void>.microtask(_syncWifiInterest);
  }

  void _syncWifiInterest() {
    if (!mounted) return;
    final onMapTab = _tabController.index == 1;
    if (onMapTab && _releaseWifiInterest == null) {
      _releaseWifiInterest = ref
          .read(sseServiceProvider.notifier)
          .acquireInterest('wifi');
    } else if (!onMapTab && _releaseWifiInterest != null) {
      _releaseWifiInterest?.call();
      _releaseWifiInterest = null;
    }
    if (_isMapTab != onMapTab) {
      setState(() => _isMapTab = onMapTab);
    }
  }

  @override
  void dispose() {
    _releaseWifiInterest?.call();
    _releaseWifiInterest = null;
    _tabController.removeListener(_syncWifiInterest);
    _tabController.dispose();
    _searchCtrl.dispose();
    super.dispose();
  }

  void _openStationInspector(WifiStationRow row) {
    showWifiStationInspectorSheet(context, ref: ref, row: row);
  }

  @override
  Widget build(BuildContext context) {
    final wifi = ref.watch(wifiClientsProvider);
    final canWrite = ref.watch(authProvider).canWrite;
    final allRows = wifi.rows;
    final watchedByMac = wifi.watchedByMac;

    final ssids =
        allRows
            .map((r) => r.station.ssid)
            .whereType<String>()
            .where((s) => s.isNotEmpty)
            .toSet()
            .toList()
          ..sort();

    final query = _searchCtrl.text.trim().toLowerCase();
    final filtered = allRows
        .where((r) {
          if (_apFilter != null && r.deviceId != _apFilter) return false;
          if (_ssidFilter != 'all' && r.station.ssid != _ssidFilter) {
            return false;
          }
          if (_bandFilter != 'all' && r.station.band != _bandFilter) {
            return false;
          }
          if (_gradeFilter != 'all' && r.station.signalGrade != _gradeFilter) {
            return false;
          }
          final watched = watchedByMac[r.station.mac.toLowerCase()];
          if (_watchedOnly && watched == null) return false;
          if (query.isNotEmpty) {
            final hay =
                '${r.displayName} ${r.station.mac} ${r.station.ip ?? ''} ${r.station.ssid ?? ''} ${r.deviceName} ${watched?.name ?? ''}'
                    .toLowerCase();
            if (!hay.contains(query)) return false;
          }
          return true;
        })
        .toList(growable: false);

    final activeFilterCount =
        (_apFilter != null ? 1 : 0) +
        (_ssidFilter != 'all' ? 1 : 0) +
        (_bandFilter != 'all' ? 1 : 0) +
        (_gradeFilter != 'all' ? 1 : 0) +
        (_watchedOnly ? 1 : 0);

    final activeApName = _apFilter != null
        ? wifi.devices
              .where((d) => d.deviceId == _apFilter)
              .firstOrNull
              ?.deviceName
        : null;

    final activeChips = <ActiveFilterChipData>[
      if (activeApName != null)
        ActiveFilterChipData(
          label: 'AP: $activeApName',
          onDeleted: () => setState(() => _apFilter = null),
        ),
      if (_ssidFilter != 'all')
        ActiveFilterChipData(
          label: 'SSID: $_ssidFilter',
          onDeleted: () => setState(() => _ssidFilter = 'all'),
        ),
      if (_bandFilter != 'all')
        ActiveFilterChipData(
          label: _bandFilter,
          onDeleted: () => setState(() => _bandFilter = 'all'),
        ),
      if (_gradeFilter != 'all')
        ActiveFilterChipData(
          label: 'Sinal: ${wifiSignalGradeLabel(_gradeFilter)}',
          onDeleted: () => setState(() => _gradeFilter = 'all'),
        ),
      if (_watchedOnly)
        ActiveFilterChipData(
          label: 'Vigiados',
          onDeleted: () => setState(() => _watchedOnly = false),
        ),
    ];

    return LayoutBuilder(
      builder: (context, constraints) {
        final isCompact = constraints.maxWidth < 680;
        final pluginColor = context.semanticColors.plugin;
        final tabBar = TabBar(
          controller: _tabController,
          isScrollable: isCompact,
          tabAlignment: isCompact ? TabAlignment.start : TabAlignment.fill,
          indicatorColor: pluginColor,
          labelColor: pluginColor,
          labelPadding: const EdgeInsets.symmetric(horizontal: 14),
          tabs: <Widget>[
            Tab(
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  const Icon(Icons.list, size: 18),
                  const SizedBox(width: 6),
                  Text('Lista (${filtered.length})'),
                ],
              ),
            ),
            const Tab(
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  Icon(Icons.radar, size: 18),
                  SizedBox(width: 6),
                  Text('Mapa de Sinal'),
                ],
              ),
            ),
          ],
        );

        return NestedScrollView(
          physics: _isMapTab ? const NeverScrollableScrollPhysics() : null,
          headerSliverBuilder: (context, innerBoxIsScrolled) => <Widget>[
            SliverToBoxAdapter(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: <Widget>[
                  if (isCompact)
                    Row(
                      children: <Widget>[
                        Expanded(
                          child: Text(
                            'Sinais Wi-Fi',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: context.textTheme.titleLarge?.copyWith(
                              fontWeight: FontWeight.w800,
                            ),
                          ),
                        ),
                        if (canWrite)
                          IconButton(
                            tooltip: 'Atualizar APs',
                            onPressed: wifi.requesting
                                ? null
                                : () => ref
                                      .read(wifiClientsProvider.notifier)
                                      .refresh(),
                            icon: wifi.requesting || wifi.isReading
                                ? const SizedBox(
                                    width: 18,
                                    height: 18,
                                    child: CircularProgressIndicator(
                                      strokeWidth: 2,
                                    ),
                                  )
                                : const Icon(Icons.refresh),
                          ),
                        PopupMenuButton<String>(
                          tooltip: 'Mais ações',
                          icon: const Icon(Icons.more_vert),
                          onSelected: (value) {
                            if (value == 'qr') {
                              showWifiQrSheet(
                                context,
                                initialSsid: ssids.firstOrNull ?? '',
                              );
                            } else if (value == 'settings') {
                              context.go('/settings?tab=wifi');
                            }
                          },
                          itemBuilder: (_) => const <PopupMenuEntry<String>>[
                            PopupMenuItem<String>(
                              value: 'qr',
                              child: ListTile(
                                dense: true,
                                contentPadding: EdgeInsets.zero,
                                leading: Icon(Icons.qr_code_2),
                                title: Text('QR Wi-Fi'),
                              ),
                            ),
                            PopupMenuItem<String>(
                              value: 'settings',
                              child: ListTile(
                                dense: true,
                                contentPadding: EdgeInsets.zero,
                                leading: Icon(Icons.tune),
                                title: Text('Configurações Wi-Fi'),
                              ),
                            ),
                          ],
                        ),
                      ],
                    )
                  else
                    SearchFilterHeader(
                      title: 'Sinais Wi-Fi',
                      subtitle:
                          'Estações conectadas, roaming ao vivo e mapa radial de sinal via SSE',
                      actions: <Widget>[
                        OutlinedButton.icon(
                          onPressed: () => showWifiQrSheet(
                            context,
                            initialSsid: ssids.firstOrNull ?? '',
                          ),
                          icon: const Icon(Icons.qr_code_2, size: 18),
                          label: const Text('QR Wi-Fi'),
                        ),
                        OutlinedButton.icon(
                          onPressed: () => context.go('/settings?tab=wifi'),
                          icon: const Icon(Icons.tune, size: 18),
                          label: const Text('Config.'),
                        ),
                        if (canWrite)
                          FilledButton.icon(
                            onPressed: wifi.requesting
                                ? null
                                : () => ref
                                      .read(wifiClientsProvider.notifier)
                                      .refresh(),
                            icon: wifi.requesting || wifi.isReading
                                ? const SizedBox(
                                    width: 16,
                                    height: 16,
                                    child: CircularProgressIndicator(
                                      strokeWidth: 2,
                                    ),
                                  )
                                : const Icon(Icons.refresh, size: 18),
                            label: const Text('Atualizar APs'),
                          ),
                      ],
                    ),
                  if (wifi.watched.isNotEmpty) ...<Widget>[
                    const SizedBox(height: 8),
                    SingleChildScrollView(
                      scrollDirection: Axis.horizontal,
                      child: Row(
                        children: wifi.watched
                            .map((w) {
                              final online =
                                  w.state == 'online' || w.state == 'connected';
                              return Padding(
                                padding: const EdgeInsets.only(right: 8),
                                child: SemanticChip(
                                  label:
                                      '${w.name}${w.deviceName != null ? ' · ${w.deviceName}' : ''}',
                                  tone: online
                                      ? SemanticTone.success
                                      : SemanticTone.error,
                                  icon: Icons.star,
                                  compact: true,
                                ),
                              );
                            })
                            .toList(growable: false),
                      ),
                    ),
                  ],
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
            physics: const NeverScrollableScrollPhysics(),
            children: <Widget>[
              // ── Tab 1: Lista ──
              CustomScrollView(
                slivers: <Widget>[
                  SliverPadding(
                    padding: const EdgeInsets.only(top: 10, bottom: 10),
                    sliver: SliverToBoxAdapter(
                      child: UnifiedFilterBar(
                        searchController: _searchCtrl,
                        searchHint: 'Buscar MAC, IP, hostname ou SSID...',
                        onSearchChanged: (_) => setState(() {}),
                        activeFilterCount: activeFilterCount,
                        filterSheetTitle: 'Filtros de Sinais Wi-Fi',
                        filterSheetBuilder: (sheetCtx, setSheet) => Column(
                          crossAxisAlignment: CrossAxisAlignment.stretch,
                          children: <Widget>[
                            if (wifi.devices.isNotEmpty ||
                                ssids.isNotEmpty) ...<Widget>[
                              ResponsiveFieldRow(
                                breakpoint: 480,
                                children: <Widget>[
                                  if (wifi.devices.isNotEmpty)
                                    AppDropdownFormField<int?>(
                                      initialValue:
                                          wifi.devices.any(
                                            (ap) => ap.deviceId == _apFilter,
                                          )
                                          ? _apFilter
                                          : null,
                                      decoration: const InputDecoration(
                                        labelText: 'AP',
                                      ),
                                      items: <DropdownMenuItem<int?>>[
                                        const DropdownMenuItem<int?>(
                                          value: null,
                                          child: Text(
                                            'Todos os APs',
                                            overflow: TextOverflow.ellipsis,
                                          ),
                                        ),
                                        for (final ap in wifi.devices)
                                          DropdownMenuItem<int?>(
                                            value: ap.deviceId,
                                            child: Text(
                                              ap.deviceName,
                                              overflow: TextOverflow.ellipsis,
                                            ),
                                          ),
                                      ],
                                      onChanged: (v) {
                                        setState(() => _apFilter = v);
                                        setSheet(() {});
                                      },
                                    ),
                                  if (ssids.isNotEmpty)
                                    AppDropdownFormField<String>(
                                      initialValue: ssids.contains(_ssidFilter)
                                          ? _ssidFilter
                                          : 'all',
                                      decoration: const InputDecoration(
                                        labelText: 'SSID',
                                      ),
                                      items: <DropdownMenuItem<String>>[
                                        const DropdownMenuItem<String>(
                                          value: 'all',
                                          child: Text(
                                            'Todos SSIDs',
                                            overflow: TextOverflow.ellipsis,
                                          ),
                                        ),
                                        for (final s in ssids)
                                          DropdownMenuItem<String>(
                                            value: s,
                                            child: Text(
                                              s,
                                              overflow: TextOverflow.ellipsis,
                                            ),
                                          ),
                                      ],
                                      onChanged: (v) {
                                        setState(
                                          () => _ssidFilter = v ?? 'all',
                                        );
                                        setSheet(() {});
                                      },
                                    ),
                                ],
                              ),
                              const SizedBox(height: 12),
                            ],
                            ResponsiveFieldRow(
                              breakpoint: 480,
                              children: <Widget>[
                                AppDropdownFormField<String>(
                                  initialValue: _gradeFilter,
                                  decoration: const InputDecoration(
                                    labelText: 'Sinal',
                                  ),
                                  items: const <DropdownMenuItem<String>>[
                                    DropdownMenuItem(
                                      value: 'all',
                                      child: Text(
                                        'Todos',
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                    ),
                                    DropdownMenuItem(
                                      value: 'excellent',
                                      child: Text(
                                        'Excelente',
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                    ),
                                    DropdownMenuItem(
                                      value: 'good',
                                      child: Text(
                                        'Bom',
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                    ),
                                    DropdownMenuItem(
                                      value: 'fair',
                                      child: Text(
                                        'Regular',
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                    ),
                                    DropdownMenuItem(
                                      value: 'poor',
                                      child: Text(
                                        'Fraco',
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                    ),
                                  ],
                                  onChanged: (v) {
                                    setState(() => _gradeFilter = v ?? 'all');
                                    setSheet(() {});
                                  },
                                ),
                                AppDropdownFormField<String>(
                                  initialValue: _bandFilter,
                                  decoration: const InputDecoration(
                                    labelText: 'Banda',
                                  ),
                                  items: const <DropdownMenuItem<String>>[
                                    DropdownMenuItem(
                                      value: 'all',
                                      child: Text(
                                        'Todas bandas',
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                    ),
                                    DropdownMenuItem(
                                      value: '2g',
                                      child: Text(
                                        '2.4 GHz',
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                    ),
                                    DropdownMenuItem(
                                      value: '5g',
                                      child: Text(
                                        '5 GHz',
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                    ),
                                    DropdownMenuItem(
                                      value: '6g',
                                      child: Text(
                                        '6 GHz',
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                    ),
                                  ],
                                  onChanged: (v) {
                                    setState(() => _bandFilter = v ?? 'all');
                                    setSheet(() {});
                                  },
                                ),
                              ],
                            ),
                            const SizedBox(height: 8),
                            SwitchListTile(
                              contentPadding: EdgeInsets.zero,
                              title: const Text('Apenas aparelhos vigiados'),
                              value: _watchedOnly,
                              onChanged: (v) {
                                setState(() => _watchedOnly = v);
                                setSheet(() {});
                              },
                            ),
                          ],
                        ),
                        onClearFilters: () => setState(() {
                          _apFilter = null;
                          _ssidFilter = 'all';
                          _bandFilter = 'all';
                          _gradeFilter = 'all';
                          _watchedOnly = false;
                        }),
                        activeChips: activeChips,
                      ),
                    ),
                  ),
                  if (filtered.isEmpty)
                    SliverFillRemaining(
                      hasScrollBody: false,
                      child: EmptyStateView(
                        title: 'Nenhum cliente Wi-Fi conectado',
                        description:
                            'Os dados chegam automaticamente via SSE (wifi:snapshot) dos APs com plugin Wi-Fi.',
                        icon: Icons.wifi_find,
                        tone: SemanticTone.plugin,
                        action: canWrite
                            ? FilledButton.icon(
                                onPressed: () => ref
                                    .read(wifiClientsProvider.notifier)
                                    .refresh(),
                                icon: const Icon(Icons.refresh),
                                label: const Text('Solicitar leitura agora'),
                              )
                            : null,
                      ),
                    )
                  else
                    SliverPadding(
                      padding: const EdgeInsets.only(bottom: 16),
                      sliver: SliverList.separated(
                        itemCount: filtered.length,
                        separatorBuilder: (_, _) => const SizedBox(height: 8),
                        itemBuilder: (context, index) {
                          final row = filtered[index];
                          final st = row.station;
                          final watched = watchedByMac[st.mac.toLowerCase()];
                          return Card(
                            elevation: 2,
                            child: ListTile(
                              onTap: () => _openStationInspector(row),
                              leading: SemanticChip(
                                label: formatSignalDbm(st.signalDbm),
                                tone: wifiSignalTone(st.signalDbm),
                                compact: true,
                              ),
                              title: Row(
                                children: <Widget>[
                                  if (watched != null) ...<Widget>[
                                    Icon(
                                      Icons.star,
                                      size: 16,
                                      color: context.semanticColors.warning,
                                    ),
                                    const SizedBox(width: 4),
                                  ],
                                  Expanded(
                                    child: Text(
                                      watched?.name ?? row.displayName,
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: const TextStyle(
                                        fontWeight: FontWeight.w700,
                                      ),
                                    ),
                                  ),
                                ],
                              ),
                              subtitle: Text(
                                '${row.deviceName} · ${st.ssid ?? '—'}'
                                '${st.band != null ? ' (${st.band})' : ''}'
                                '${st.ip != null ? ' · ${st.ip}' : ''}',
                                maxLines: 2,
                                overflow: TextOverflow.ellipsis,
                              ),
                              trailing:
                                  (st.txRateBps != null || st.distance != null)
                                  ? Column(
                                      mainAxisSize: MainAxisSize.min,
                                      mainAxisAlignment:
                                          MainAxisAlignment.center,
                                      crossAxisAlignment:
                                          CrossAxisAlignment.end,
                                      children: <Widget>[
                                        if (st.txRateBps != null)
                                          Text(
                                            formatBps(st.txRateBps),
                                            style: context.textTheme.labelMedium
                                                ?.copyWith(
                                                  fontWeight: FontWeight.w700,
                                                ),
                                          ),
                                        if (st.distance != null)
                                          Text(
                                            formatDistanceRange(
                                              st.distance!.minM,
                                              st.distance!.maxM,
                                            ),
                                            style: context.textTheme.labelSmall,
                                          ),
                                      ],
                                    )
                                  : null,
                            ),
                          );
                        },
                      ),
                    ),
                ],
              ),
              // ── Tab 2: Mapa Radial Interativo (com `interest=wifi`) ──
              Padding(
                padding: const EdgeInsets.only(top: 10, bottom: 10),
                child: WifiSignalMapView(
                  snapshots: wifi.devices,
                  recentRoams: wifi.roams,
                  watchedByMac: watchedByMac,
                  onTapStation: _openStationInspector,
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}
