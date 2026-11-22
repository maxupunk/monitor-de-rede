import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/wifi_models.dart';
import 'package:netmonitor_app/features/network/providers/wifi_provider.dart';
import 'package:netmonitor_app/features/network/topology/canvas/topology_canvas_view.dart'
    show extractCanvasScale;
import 'package:netmonitor_app/features/network/topology/engine/topology_layouts.dart'
    show computeFitMatrix;
import 'package:netmonitor_app/features/network/widgets/wifi_qr_sheet.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

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

  Future<void> _openWatchDialog(
    WifiStationRow row,
    WifiWatchedClientModel? existing,
  ) async {
    final nameCtrl = TextEditingController(
      text: existing?.name ?? row.displayName,
    );
    await showAdaptiveFormSheet<void>(
      context: context,
      title: existing != null
          ? 'Renomear aparelho vigiado'
          : 'Vigiar aparelho Wi-Fi',
      subtitle: row.station.mac,
      builder: (ctx) => Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          TextField(
            controller: nameCtrl,
            decoration: const InputDecoration(labelText: 'Apelido do aparelho'),
          ),
          const SizedBox(height: 14),
          SymmetricalActionBar(
            children: <Widget>[
              if (existing != null)
                OutlinedButton(
                  onPressed: () async {
                    await ref
                        .read(wifiClientsProvider.notifier)
                        .unwatch(existing.id);
                    if (ctx.mounted) Navigator.of(ctx).pop();
                  },
                  child: const Text(
                    'Deixar de vigiar',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                )
              else
                OutlinedButton(
                  onPressed: () => Navigator.of(ctx).pop(),
                  child: const Text('Cancelar'),
                ),
              FilledButton.icon(
                onPressed: () async {
                  if (nameCtrl.text.trim().isEmpty) return;
                  final ok = existing != null
                      ? await ref
                            .read(wifiClientsProvider.notifier)
                            .renameWatched(existing.id, nameCtrl.text.trim())
                      : await ref
                            .read(wifiClientsProvider.notifier)
                            .watch(row.station.mac, nameCtrl.text.trim());
                  if (ok && ctx.mounted) Navigator.of(ctx).pop();
                },
                icon: const Icon(Icons.star_outline),
                label: const Text('Salvar'),
              ),
            ],
          ),
        ],
      ),
    );
    nameCtrl.dispose();
  }

  void _openStationInspector(WifiStationRow row) {
    final canWrite = ref.read(authProvider).canWrite;
    ref.read(wifiClientsProvider.notifier).fetchRoaming(row.station.mac);

    showAdaptiveFormSheet<void>(
      context: context,
      title: row.displayName,
      subtitle: '${row.station.mac} · AP: ${row.deviceName}',
      maxWidth: 580,
      builder: (_) => Consumer(
        builder: (context, ref, _) {
          final wifi = ref.watch(wifiClientsProvider);
          final watched = wifi.watchedByMac[row.station.mac.toLowerCase()];
          final pingState = wifi.pingFor(row.deviceId, row.station.mac);
          final roamHist = wifi.roamHistory[row.station.mac];
          final st = row.station;

          return Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              KeyValueRow(
                label: 'MAC',
                value: st.mac,
                monospace: true,
                copyable: true,
              ),
              KeyValueRow(label: 'IP', value: st.ip ?? '—', monospace: true),
              KeyValueRow(
                label: 'Fabricante',
                value: st.vendor ?? (st.randomizedMac ? 'MAC privado' : '—'),
              ),
              KeyValueRow(
                label: 'SSID / Banda',
                value: '${st.ssid ?? '—'} (${st.band ?? '—'})',
              ),
              KeyValueRow(
                label: 'Sinal (RSSI)',
                value:
                    '${st.signalDbm} dBm (${_signalGradeLabel(st.signalGrade)})',
              ),
              if (st.snrDb != null)
                KeyValueRow(label: 'SNR', value: '${st.snrDb} dB'),
              if (st.txRateBps != null || st.rxRateBps != null)
                KeyValueRow(
                  label: 'Taxa PHY (TX / RX)',
                  value:
                      '${formatBps(st.txRateBps)} / ${formatBps(st.rxRateBps)}',
                ),
              if (st.distance != null)
                KeyValueRow(
                  label: 'Distância estimada',
                  value: formatDistanceRange(
                    st.distance!.minM,
                    st.distance!.maxM,
                  ),
                ),
              if (st.healthReason != null) ...<Widget>[
                const SizedBox(height: 8),
                NestedPanel(
                  tone: SemanticTone.warning,
                  child: Text(st.healthReason!),
                ),
              ],
              const SizedBox(height: 12),
              SymmetricalActionBar(
                children: <Widget>[
                  if (canWrite && row.canPing && st.ip != null)
                    FilledButton.tonalIcon(
                      onPressed: pingState?.loading == true
                          ? null
                          : () => ref
                                .read(wifiClientsProvider.notifier)
                                .pingStation(row),
                      icon: const Icon(Icons.network_ping, size: 18),
                      label: Text(
                        pingState?.loading == true
                            ? 'Medindo...'
                            : 'Ping pelo AP',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  if (canWrite)
                    OutlinedButton.icon(
                      onPressed: () => _openWatchDialog(row, watched),
                      icon: Icon(
                        watched != null ? Icons.star : Icons.star_border,
                        size: 18,
                      ),
                      label: Text(
                        watched != null
                            ? 'Vigiado (${watched.name})'
                            : 'Vigiar aparelho',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                  OutlinedButton.icon(
                    onPressed: () => showWifiQrSheet(
                      context,
                      initialSsid: st.ssid ?? '',
                      deviceId: row.deviceId,
                    ),
                    icon: const Icon(Icons.qr_code_2, size: 18),
                    label: const Text(
                      'QR Wi-Fi',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ],
              ),
              if (pingState?.result != null) ...<Widget>[
                const SizedBox(height: 10),
                NestedPanel(
                  tone: pingState!.result!.lossPct == 0
                      ? SemanticTone.success
                      : SemanticTone.warning,
                  child: Text(
                    'Ping do AP até ${pingState.result!.target}: '
                    '${formatLatency(pingState.result!.avgMs)} · '
                    '${pingState.result!.received}/${pingState.result!.sent} recebidos',
                  ),
                ),
              ],
              const Divider(height: 24),
              Text(
                'Histórico de Roaming',
                style: context.textTheme.titleSmall?.copyWith(
                  fontWeight: FontWeight.w700,
                ),
              ),
              const SizedBox(height: 6),
              if (roamHist?.loading == true)
                const LinearProgressIndicator()
              else if (roamHist == null || roamHist.events.isEmpty)
                Text(
                  'Nenhuma troca de AP registrada.',
                  style: context.textTheme.bodySmall,
                )
              else
                for (final roam in roamHist.events.take(10))
                  ListTile(
                    dense: true,
                    contentPadding: EdgeInsets.zero,
                    leading: Icon(
                      roam.pingPong ? Icons.warning_amber : Icons.swap_horiz,
                      color: roam.pingPong
                          ? context.semanticColors.warning
                          : context.semanticColors.plugin,
                    ),
                    title: Text(
                      '${roam.fromDeviceName} -> ${roam.toDeviceName}',
                    ),
                    subtitle: Text(
                      '${formatShortDateTime(roam.occurredAt)}'
                      '${roam.signalBeforeDbm != null ? ' · ${roam.signalBeforeDbm} -> ${roam.signalAfterDbm ?? '?'} dBm' : ''}',
                    ),
                  ),
            ],
          );
        },
      ),
    );
  }

  String _signalGradeLabel(String grade) =>
      switch (grade.trim().toLowerCase()) {
        'excellent' => 'excelente',
        'good' => 'bom',
        'fair' => 'regular',
        'poor' || 'weak' => 'fraco',
        _ => grade,
      };

  SemanticTone _signalTone(int rssi) {
    if (rssi >= -60) return SemanticTone.success;
    if (rssi >= -72) return SemanticTone.info;
    if (rssi >= -82) return SemanticTone.warning;
    return SemanticTone.error;
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
          label: 'Sinal: ${_signalGradeLabel(_gradeFilter)}',
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
              delegate: _WifiTabBarDelegate(
                backgroundColor: Theme.of(context).scaffoldBackgroundColor,
                tabBar: TabBar(
                  controller: _tabController,
                  isScrollable: isCompact,
                  tabAlignment: isCompact
                      ? TabAlignment.start
                      : TabAlignment.fill,
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
                ),
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
                                      value: '2.4GHz',
                                      child: Text(
                                        '2.4GHz',
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                    ),
                                    DropdownMenuItem(
                                      value: '5GHz',
                                      child: Text(
                                        '5GHz',
                                        overflow: TextOverflow.ellipsis,
                                      ),
                                    ),
                                    DropdownMenuItem(
                                      value: '6GHz',
                                      child: Text(
                                        '6GHz',
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
                                label: '${st.signalDbm} dBm',
                                tone: _signalTone(st.signalDbm),
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
                child: _WifiSignalMapView(
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

class _WifiTabBarDelegate extends SliverPersistentHeaderDelegate {
  const _WifiTabBarDelegate({
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
  bool shouldRebuild(covariant _WifiTabBarDelegate oldDelegate) {
    return oldDelegate.tabBar != tabBar ||
        oldDelegate.backgroundColor != backgroundColor;
  }
}

@immutable
class _WifiSignalRingSpec {
  const _WifiSignalRingSpec({required this.dbm, required this.radius});

  final int dbm;
  final double radius;

  String get label => '$dbm dBm';
}

const List<_WifiSignalRingSpec> _kWifiSignalRings = <_WifiSignalRingSpec>[
  _WifiSignalRingSpec(dbm: -50, radius: 96),
  _WifiSignalRingSpec(dbm: -65, radius: 162),
  _WifiSignalRingSpec(dbm: -75, radius: 228),
  _WifiSignalRingSpec(dbm: -85, radius: 294),
];

const Size _kWifiStationBadgeSize = Size(108, 44);
const Size _kWifiApBadgeSize = Size(124, 54);

@immutable
class _WifiPlacedStation {
  const _WifiPlacedStation({
    required this.row,
    required this.apCenter,
    required this.center,
    this.watchedName,
  });

  final WifiStationRow row;
  final Offset apCenter;
  final Offset center;
  final String? watchedName;
}

@immutable
class _WifiRadarScene {
  const _WifiRadarScene({
    required this.apCenters,
    required this.stations,
    required this.bounds,
    required this.worldSize,
  });

  final Map<int, Offset> apCenters;
  final List<_WifiPlacedStation> stations;
  final Rect bounds;
  final Size worldSize;
}

double _radiusForWifiSignalDbm(int signalDbm) {
  final clamped = signalDbm.clamp(-96, -30);
  if (clamped >= -50) {
    final t = (-30 - clamped) / 20.0;
    return 66.0 + t * (96.0 - 66.0);
  }
  if (clamped >= -65) {
    final t = (-50 - clamped) / 15.0;
    return 96.0 + t * (162.0 - 96.0);
  }
  if (clamped >= -75) {
    final t = (-65 - clamped) / 10.0;
    return 162.0 + t * (228.0 - 162.0);
  }
  if (clamped >= -85) {
    final t = (-75 - clamped) / 10.0;
    return 228.0 + t * (294.0 - 228.0);
  }
  final t = (-85 - clamped) / 11.0;
  return 294.0 + t * 32.0;
}

_WifiRadarScene _computeWifiRadarScene(
  List<WifiDeviceSnapshotModel> snapshots,
  Map<String, WifiWatchedClientModel> watchedByMac,
) {
  if (snapshots.isEmpty) {
    return const _WifiRadarScene(
      apCenters: <int, Offset>{},
      stations: <_WifiPlacedStation>[],
      bounds: Rect.zero,
      worldSize: Size(1200, 900),
    );
  }

  final apCenters = <int, Offset>{};
  final cols = math.max(1, math.sqrt(snapshots.length).ceil());
  const double originX = 480;
  const double originY = 440;
  const double stepX = 760;
  const double stepY = 720;

  for (var i = 0; i < snapshots.length; i++) {
    final col = i % cols;
    final row = i ~/ cols;
    apCenters[snapshots[i].deviceId] = Offset(
      originX + col * stepX,
      originY + row * stepY,
    );
  }

  final allStations = <_WifiPlacedStation>[];
  var minX = double.infinity;
  var minY = double.infinity;
  var maxX = double.negativeInfinity;
  var maxY = double.negativeInfinity;

  void includeRect(Rect rect) {
    if (rect.left < minX) minX = rect.left;
    if (rect.top < minY) minY = rect.top;
    if (rect.right > maxX) maxX = rect.right;
    if (rect.bottom > maxY) maxY = rect.bottom;
  }

  final outerRingRadius = _kWifiSignalRings.last.radius + 26.0;

  for (final ap in snapshots) {
    final apCenter = apCenters[ap.deviceId]!;
    includeRect(Rect.fromCircle(center: apCenter, radius: outerRingRadius));

    final sortedStations = List<WifiStationModel>.from(ap.stations)
      ..sort((a, b) {
        final sigCmp = b.signalDbm.compareTo(a.signalDbm);
        if (sigCmp != 0) return sigCmp;
        return a.mac.toLowerCase().compareTo(b.mac.toLowerCase());
      });

    final count = sortedStations.length;
    final coords = <Offset>[];
    for (var i = 0; i < count; i++) {
      final st = sortedStations[i];
      // Evita o topo exato (-pi/2) onde ficam os rótulos dos anéis de dBm.
      final baseAngle = -math.pi / 3 + (2 * math.pi * i) / math.max(1, count);
      final r = _radiusForWifiSignalDbm(st.signalDbm);
      coords.add(
        Offset(
          apCenter.dx + r * math.cos(baseAngle),
          apCenter.dy + r * math.sin(baseAngle),
        ),
      );
    }

    // Relaxamento determinístico anti-colisão para evitar badges empilhados.
    final minDx = _kWifiStationBadgeSize.width + 12.0;
    final minDy = _kWifiStationBadgeSize.height + 10.0;
    const minHubDist = 74.0;
    for (var iter = 0; iter < 40 && coords.length > 1; iter++) {
      var moved = false;
      for (var i = 0; i < coords.length; i++) {
        for (var j = i + 1; j < coords.length; j++) {
          final a = coords[i];
          final b = coords[j];
          final dx = b.dx - a.dx;
          final dy = b.dy - a.dy;
          final overlapX = minDx - dx.abs();
          final overlapY = minDy - dy.abs();
          if (overlapX > 0 && overlapY > 0) {
            moved = true;
            if (overlapX <= overlapY) {
              final dir = dx >= 0 ? 1.0 : -1.0;
              final shift = overlapX / 2 + 0.5;
              coords[i] = Offset(a.dx - dir * shift, a.dy);
              coords[j] = Offset(b.dx + dir * shift, b.dy);
            } else {
              final dir = dy >= 0 ? 1.0 : -1.0;
              final shift = overlapY / 2 + 0.5;
              coords[i] = Offset(a.dx, a.dy - dir * shift);
              coords[j] = Offset(b.dx, b.dy + dir * shift);
            }
          }
        }
      }
      for (var i = 0; i < coords.length; i++) {
        final rel = coords[i] - apCenter;
        final dist = rel.distance;
        if (dist < minHubDist) {
          moved = true;
          final safeDir = dist > 1e-3
              ? Offset(rel.dx / dist, rel.dy / dist)
              : const Offset(1, 0);
          coords[i] = apCenter + safeDir * minHubDist;
        }
      }
      if (!moved) break;
    }

    for (var i = 0; i < count; i++) {
      final st = sortedStations[i];
      final pos = coords[i];
      final watched = watchedByMac[st.mac.toLowerCase()];
      includeRect(
        Rect.fromCenter(
          center: pos,
          width: _kWifiStationBadgeSize.width + 8,
          height: _kWifiStationBadgeSize.height + 8,
        ),
      );
      allStations.add(
        _WifiPlacedStation(
          row: WifiStationRow(
            deviceId: ap.deviceId,
            deviceName: ap.deviceName,
            canPing: ap.canPing,
            station: st,
          ),
          apCenter: apCenter,
          center: pos,
          watchedName: watched?.name,
        ),
      );
    }
  }

  final bounds = Rect.fromLTRB(minX, minY, maxX, maxY);
  final worldSize = Size(
    math.max(1400.0, bounds.right + 420.0),
    math.max(1100.0, bounds.bottom + 420.0),
  );
  return _WifiRadarScene(
    apCenters: Map<int, Offset>.unmodifiable(apCenters),
    stations: List<_WifiPlacedStation>.unmodifiable(allStations),
    bounds: bounds,
    worldSize: worldSize,
  );
}

class _WifiSignalMapView extends StatefulWidget {
  const _WifiSignalMapView({
    required this.snapshots,
    required this.recentRoams,
    required this.watchedByMac,
    required this.onTapStation,
  });

  final List<WifiDeviceSnapshotModel> snapshots;
  final List<WifiRoamEventModel> recentRoams;
  final Map<String, WifiWatchedClientModel> watchedByMac;
  final ValueChanged<WifiStationRow> onTapStation;

  @override
  State<_WifiSignalMapView> createState() => _WifiSignalMapViewState();
}

class _WifiSignalMapViewState extends State<_WifiSignalMapView> {
  final TransformationController _mapCtrl = TransformationController();
  int? _selectedApId;
  Size _viewportSize = Size.zero;
  double _currentScale = 1.0;
  String _lastFittedSignature = '';
  Rect _lastBounds = Rect.zero;
  EdgeInsets _lastFitPadding = const EdgeInsets.all(24);

  @override
  void initState() {
    super.initState();
    _mapCtrl.addListener(_handleTransformChanged);
  }

  @override
  void dispose() {
    _mapCtrl.removeListener(_handleTransformChanged);
    _mapCtrl.dispose();
    super.dispose();
  }

  void _handleTransformChanged() {
    final nextScale = extractCanvasScale(_mapCtrl.value);
    if (!nextScale.isFinite || nextScale <= 0) return;
    if ((nextScale - _currentScale).abs() >= 0.015 && mounted) {
      setState(() => _currentScale = nextScale);
    }
  }

  void _fitToScreen() {
    if (_viewportSize == Size.zero || _lastBounds.isEmpty) return;
    final matrix = computeFitMatrix(
      _lastBounds,
      _viewportSize,
      padding: _lastFitPadding,
      minScale: 0.25,
      maxScale: 1.15,
    );
    _mapCtrl.value = matrix;
    final nextScale = extractCanvasScale(matrix);
    if ((nextScale - _currentScale).abs() >= 0.005 && mounted) {
      setState(() => _currentScale = nextScale);
    }
  }

  void _zoomBy(double factor) {
    if (_viewportSize == Size.zero) return;
    final current = _mapCtrl.value;
    final currentScale = extractCanvasScale(current);
    final clamped = (currentScale * factor).clamp(0.25, 2.6).toDouble();
    final focal = Offset(_viewportSize.width / 2, _viewportSize.height / 2);
    final inv = Matrix4.copy(current);
    if (inv.invert() == 0.0) return;
    final worldFocal = MatrixUtils.transformPoint(inv, focal);
    final tx = focal.dx - worldFocal.dx * clamped;
    final ty = focal.dy - worldFocal.dy * clamped;
    _mapCtrl.value = Matrix4.identity()
      ..translateByDouble(tx, ty, 0.0, 1.0)
      ..scaleByDouble(clamped, clamped, clamped, 1.0);
  }

  @override
  Widget build(BuildContext context) {
    if (widget.snapshots.isEmpty) {
      return const EmptyStateView(
        title: 'Aguardando snapshots Wi-Fi ao vivo...',
        description:
            'Interesse SSE "wifi" ativo: os APs reportam em cadência ao vivo.',
        icon: Icons.radar,
        tone: SemanticTone.plugin,
      );
    }

    final validApId = widget.snapshots.any((ap) => ap.deviceId == _selectedApId)
        ? _selectedApId
        : null;
    final visibleSnapshots = validApId == null
        ? widget.snapshots
        : widget.snapshots
              .where((ap) => ap.deviceId == validApId)
              .toList(growable: false);

    final scene = _computeWifiRadarScene(visibleSnapshots, widget.watchedByMac);
    final hasApBar = widget.snapshots.length > 1;
    final fitPadding = EdgeInsets.fromLTRB(24, hasApBar ? 58 : 24, 24, 28);
    _lastBounds = scene.bounds;
    _lastFitPadding = fitPadding;

    final scheme = context.colorScheme;
    final semantics = context.semanticColors;

    return Card(
      elevation: 2,
      clipBehavior: Clip.antiAlias,
      child: LayoutBuilder(
        builder: (context, constraints) {
          final nextSize = Size(constraints.maxWidth, constraints.maxHeight);
          final fitSignature =
              '${visibleSnapshots.map((s) => s.deviceId).join(",")}|'
              '${nextSize.width.round()}x${nextSize.height.round()}';
          if (nextSize.width.isFinite &&
              nextSize.height.isFinite &&
              nextSize.width > 0 &&
              nextSize.height > 0) {
            _viewportSize = nextSize;
            if (_lastFittedSignature != fitSignature && !scene.bounds.isEmpty) {
              _lastFittedSignature = fitSignature;
              WidgetsBinding.instance.addPostFrameCallback((_) {
                if (!mounted) return;
                _fitToScreen();
              });
            }
          }

          final zoomPercent = (_currentScale * 100).round();

          return Stack(
            children: <Widget>[
              // Fundo de grade sutil de alto contraste.
              Positioned.fill(
                child: ColoredBox(color: scheme.surfaceContainerLowest),
              ),

              // Área interativa com pan livre em todas as direções e pinch-zoom.
              Positioned.fill(
                child: InteractiveViewer(
                  key: const ValueKey<String>('wifi-signal-interactive-viewer'),
                  transformationController: _mapCtrl,
                  constrained: false,
                  panEnabled: true,
                  scaleEnabled: true,
                  boundaryMargin: const EdgeInsets.all(1200),
                  minScale: 0.25,
                  maxScale: 2.6,
                  child: SizedBox(
                    width: scene.worldSize.width,
                    height: scene.worldSize.height,
                    child: Stack(
                      clipBehavior: Clip.none,
                      children: <Widget>[
                        Positioned.fill(
                          child: IgnorePointer(
                            child: CustomPaint(
                              painter: _WifiRadialPainter(
                                apCenters: scene.apCenters,
                                stations: scene.stations,
                                recentRoams: widget.recentRoams,
                                scheme: scheme,
                                semantics: semantics,
                                textTheme: context.textTheme,
                              ),
                            ),
                          ),
                        ),
                        for (final ap in visibleSnapshots)
                          if (scene.apCenters[ap.deviceId] != null)
                            Positioned(
                              left:
                                  scene.apCenters[ap.deviceId]!.dx -
                                  _kWifiApBadgeSize.width / 2,
                              top:
                                  scene.apCenters[ap.deviceId]!.dy -
                                  _kWifiApBadgeSize.height / 2,
                              child: Container(
                                width: _kWifiApBadgeSize.width,
                                height: _kWifiApBadgeSize.height,
                                padding: const EdgeInsets.symmetric(
                                  horizontal: 8,
                                  vertical: 5,
                                ),
                                decoration: BoxDecoration(
                                  color: semantics.pluginContainer,
                                  borderRadius: BorderRadius.circular(12),
                                  border: Border.all(
                                    color: semantics.plugin,
                                    width: 2,
                                  ),
                                  boxShadow: <BoxShadow>[
                                    BoxShadow(
                                      color: scheme.shadow.withValues(
                                        alpha: 0.2,
                                      ),
                                      blurRadius: 8,
                                      offset: const Offset(0, 2),
                                    ),
                                  ],
                                ),
                                child: Column(
                                  mainAxisAlignment: MainAxisAlignment.center,
                                  mainAxisSize: MainAxisSize.min,
                                  children: <Widget>[
                                    Row(
                                      mainAxisAlignment:
                                          MainAxisAlignment.center,
                                      mainAxisSize: MainAxisSize.min,
                                      children: <Widget>[
                                        Icon(
                                          Icons.wifi_tethering,
                                          size: 16,
                                          color: semantics.onPluginContainer,
                                        ),
                                        const SizedBox(width: 4),
                                        Text(
                                          '${ap.stations.length}',
                                          style: context.textTheme.labelSmall
                                              ?.copyWith(
                                                fontWeight: FontWeight.w800,
                                                color:
                                                    semantics.onPluginContainer,
                                              ),
                                        ),
                                      ],
                                    ),
                                    const SizedBox(height: 2),
                                    Text(
                                      ap.deviceName,
                                      maxLines: 1,
                                      overflow: TextOverflow.ellipsis,
                                      style: context.textTheme.labelSmall
                                          ?.copyWith(
                                            fontWeight: FontWeight.w800,
                                            color: semantics.onPluginContainer,
                                          ),
                                    ),
                                  ],
                                ),
                              ),
                            ),
                        for (final placed in scene.stations)
                          Positioned(
                            key: ValueKey<String>(
                              'wifi-map-station-${placed.row.deviceId}-${placed.row.station.mac}',
                            ),
                            left:
                                placed.center.dx -
                                _kWifiStationBadgeSize.width / 2,
                            top:
                                placed.center.dy -
                                _kWifiStationBadgeSize.height / 2,
                            child: _WifiStationMapNode(
                              placed: placed,
                              onTap: () => widget.onTapStation(placed.row),
                            ),
                          ),
                      ],
                    ),
                  ),
                ),
              ),

              // Seletor rápido de AP no topo quando há múltiplos APs.
              if (hasApBar)
                Positioned(
                  top: 10,
                  left: 10,
                  right: 10,
                  child: SingleChildScrollView(
                    scrollDirection: Axis.horizontal,
                    child: Row(
                      children: <Widget>[
                        ChoiceChip(
                          label: Text('Todos APs (${widget.snapshots.length})'),
                          selected: validApId == null,
                          onSelected: (_) =>
                              setState(() => _selectedApId = null),
                        ),
                        for (final ap in widget.snapshots) ...<Widget>[
                          const SizedBox(width: 6),
                          ChoiceChip(
                            label: Text(
                              '${ap.deviceName} (${ap.stations.length})',
                            ),
                            selected: validApId == ap.deviceId,
                            onSelected: (_) => setState(
                              () => _selectedApId = validApId == ap.deviceId
                                  ? null
                                  : ap.deviceId,
                            ),
                          ),
                        ],
                      ],
                    ),
                  ),
                ),

              // Dica compacta de arraste/toque no canto inferior esquerdo.
              Positioned(
                left: 10,
                bottom: 10,
                child: IgnorePointer(
                  child: DecoratedBox(
                    decoration: BoxDecoration(
                      color: scheme.surface.withValues(alpha: 0.92),
                      borderRadius: BorderRadius.circular(20),
                      border: Border.all(color: scheme.outlineVariant),
                    ),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 10,
                        vertical: 5,
                      ),
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: <Widget>[
                          Icon(
                            Icons.open_with,
                            size: 14,
                            color: semantics.plugin,
                          ),
                          const SizedBox(width: 6),
                          Text(
                            'Arraste para mover · Toque no cliente',
                            style: context.textTheme.labelSmall?.copyWith(
                              fontWeight: FontWeight.w600,
                              color: scheme.onSurface,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              ),

              // Controles ergonômicos flutuantes no canto inferior direito (padrão Topologia).
              Positioned(
                right: 10,
                bottom: 10,
                child: Material(
                  elevation: 3,
                  color: scheme.surface.withValues(alpha: 0.96),
                  shadowColor: scheme.shadow.withValues(alpha: 0.24),
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(16),
                    side: BorderSide(color: scheme.outlineVariant),
                  ),
                  clipBehavior: Clip.antiAlias,
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      IconButton(
                        tooltip: 'Aproximar mapa',
                        visualDensity: VisualDensity.compact,
                        icon: const Icon(Icons.zoom_in, size: 20),
                        onPressed: () => _zoomBy(1.28),
                      ),
                      Tooltip(
                        message:
                            'Zoom atual ($zoomPercent%) — toque para ajustar à tela',
                        child: InkWell(
                          onTap: _fitToScreen,
                          child: SizedBox(
                            width: 44,
                            height: 22,
                            child: Center(
                              child: Text(
                                '$zoomPercent%',
                                maxLines: 1,
                                softWrap: false,
                                style: context.textTheme.labelSmall?.copyWith(
                                  fontSize: 10,
                                  fontWeight: FontWeight.w800,
                                  color: scheme.onSurfaceVariant,
                                  fontFeatures: const <FontFeature>[
                                    FontFeature.tabularFigures(),
                                  ],
                                ),
                              ),
                            ),
                          ),
                        ),
                      ),
                      IconButton(
                        tooltip: 'Afastar mapa',
                        visualDensity: VisualDensity.compact,
                        icon: const Icon(Icons.zoom_out, size: 20),
                        onPressed: () => _zoomBy(1.0 / 1.28),
                      ),
                      IconButton(
                        tooltip: 'Ajustar mapa à tela',
                        visualDensity: VisualDensity.compact,
                        icon: const Icon(Icons.fit_screen_outlined, size: 20),
                        onPressed: _fitToScreen,
                      ),
                    ],
                  ),
                ),
              ),
            ],
          );
        },
      ),
    );
  }
}

class _WifiStationMapNode extends StatelessWidget {
  const _WifiStationMapNode({required this.placed, required this.onTap});

  final _WifiPlacedStation placed;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final scheme = context.colorScheme;
    final semantics = context.semanticColors;
    final st = placed.row.station;
    final tone = st.signalDbm >= -60
        ? SemanticTone.success
        : st.signalDbm >= -72
        ? SemanticTone.info
        : st.signalDbm >= -82
        ? SemanticTone.warning
        : SemanticTone.error;
    final toneColors = semantics.resolve(tone, scheme);
    final label = placed.watchedName ?? placed.row.displayName;

    return Material(
      color: scheme.surface,
      elevation: 2,
      shadowColor: scheme.shadow.withValues(alpha: 0.18),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(10),
        side: BorderSide(color: toneColors.border, width: 1.4),
      ),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onTap,
        child: SizedBox(
          width: _kWifiStationBadgeSize.width,
          height: _kWifiStationBadgeSize.height,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 4),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: <Widget>[
                Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: <Widget>[
                    if (placed.watchedName != null) ...<Widget>[
                      Icon(Icons.star, size: 11, color: semantics.warning),
                      const SizedBox(width: 3),
                    ],
                    Flexible(
                      child: Text(
                        label,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: context.textTheme.labelSmall?.copyWith(
                          fontWeight: FontWeight.w700,
                          color: scheme.onSurface,
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 1),
                Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    Container(
                      width: 6,
                      height: 6,
                      decoration: BoxDecoration(
                        color: toneColors.foreground,
                        shape: BoxShape.circle,
                      ),
                    ),
                    const SizedBox(width: 4),
                    Text(
                      '${st.signalDbm} dBm',
                      maxLines: 1,
                      style: TextStyle(
                        fontSize: 10,
                        fontWeight: FontWeight.w800,
                        color: toneColors.foreground,
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _WifiRadialPainter extends CustomPainter {
  const _WifiRadialPainter({
    required this.apCenters,
    required this.stations,
    required this.recentRoams,
    required this.scheme,
    required this.semantics,
    required this.textTheme,
  });

  final Map<int, Offset> apCenters;
  final List<_WifiPlacedStation> stations;
  final List<WifiRoamEventModel> recentRoams;
  final ColorScheme scheme;
  final AppSemanticColors semantics;
  final TextTheme textTheme;

  @override
  void paint(Canvas canvas, Size size) {
    final ringPaint = Paint()
      ..color = scheme.outlineVariant.withValues(alpha: 0.65)
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1.2;

    final labelBgPaint = Paint()
      ..color = scheme.surfaceContainerHigh.withValues(alpha: 0.92);

    for (final center in apCenters.values) {
      for (final ring in _kWifiSignalRings) {
        canvas.drawCircle(center, ring.radius, ringPaint);
        final tp = TextPainter(
          text: TextSpan(
            text: ring.label,
            style: (textTheme.labelSmall ?? const TextStyle(fontSize: 10))
                .copyWith(
                  fontSize: 10,
                  fontWeight: FontWeight.w700,
                  color: scheme.onSurfaceVariant,
                ),
          ),
          textDirection: TextDirection.ltr,
        )..layout();
        final pillRect = RRect.fromRectAndRadius(
          Rect.fromCenter(
            center: Offset(center.dx, center.dy - ring.radius),
            width: tp.width + 10,
            height: tp.height + 4,
          ),
          const Radius.circular(6),
        );
        canvas.drawRRect(pillRect, labelBgPaint);
        tp.paint(
          canvas,
          Offset(
            center.dx - tp.width / 2,
            center.dy - ring.radius - tp.height / 2,
          ),
        );
      }
    }

    for (final roam in recentRoams.take(8)) {
      final fromCenter = roam.fromDeviceId != null
          ? apCenters[roam.fromDeviceId!]
          : null;
      final toCenter = roam.toDeviceId != null
          ? apCenters[roam.toDeviceId!]
          : null;
      if (fromCenter == null || toCenter == null || fromCenter == toCenter) {
        continue;
      }
      final roamPaint = Paint()
        ..color = (roam.pingPong ? semantics.warning : semantics.plugin)
            .withValues(alpha: 0.55)
        ..strokeWidth = 2.0
        ..style = PaintingStyle.stroke;
      final mid = Offset(
        (fromCenter.dx + toCenter.dx) / 2,
        (fromCenter.dy + toCenter.dy) / 2 - 44,
      );
      final path = Path()
        ..moveTo(fromCenter.dx, fromCenter.dy)
        ..quadraticBezierTo(mid.dx, mid.dy, toCenter.dx, toCenter.dy);
      canvas.drawPath(path, roamPaint);
    }

    for (final placed in stations) {
      final dbm = placed.row.station.signalDbm;
      final color = dbm >= -60
          ? semantics.success
          : dbm >= -72
          ? semantics.info
          : dbm >= -82
          ? semantics.warning
          : semantics.error;
      final linePaint = Paint()
        ..color = color.withValues(alpha: 0.78)
        ..strokeWidth = 2.0;
      canvas.drawLine(placed.apCenter, placed.center, linePaint);
    }
  }

  @override
  bool shouldRepaint(covariant _WifiRadialPainter oldDelegate) =>
      oldDelegate.stations != stations ||
      oldDelegate.apCenters != apCenters ||
      oldDelegate.recentRoams != recentRoams ||
      oldDelegate.scheme != scheme;
}
