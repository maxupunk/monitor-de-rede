import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/wifi_models.dart';
import 'package:netmonitor_app/features/network/providers/wifi_provider.dart';
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
  VoidCallback? _releaseWifiInterest;

  @override
  void initState() {
    super.initState();
    _tabController = TabController(
      length: 2,
      vsync: this,
      initialIndex: widget.initialTab == 'map' ? 1 : 0,
    );
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
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: <Widget>[
              if (existing != null)
                TextButton(
                  onPressed: () async {
                    await ref
                        .read(wifiClientsProvider.notifier)
                        .unwatch(existing.id);
                    if (ctx.mounted) Navigator.of(ctx).pop();
                  },
                  child: const Text('Deixar de vigiar'),
                ),
              const SizedBox(width: 8),
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
                value: '${st.signalDbm} dBm (${st.signalGrade})',
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
              Wrap(
                spacing: 8,
                runSpacing: 8,
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
                      ),
                    ),
                  OutlinedButton.icon(
                    onPressed: () => showWifiQrSheet(
                      context,
                      initialSsid: st.ssid ?? '',
                      deviceId: row.deviceId,
                    ),
                    icon: const Icon(Icons.qr_code_2, size: 18),
                    label: const Text('QR Wi-Fi'),
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
                      '${roam.fromDeviceName} → ${roam.toDeviceName}',
                    ),
                    subtitle: Text(
                      '${formatShortDateTime(roam.occurredAt)}'
                      '${roam.signalBeforeDbm != null ? ' · ${roam.signalBeforeDbm} → ${roam.signalAfterDbm ?? '?'} dBm' : ''}',
                    ),
                  ),
            ],
          );
        },
      ),
    );
  }

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

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SearchFilterHeader(
          title: 'Sinais Wi-Fi',
          subtitle: 'Estações conectadas, roaming ao vivo e mapa radial de sinal via SSE',
          searchController: _searchCtrl,
          searchHint: 'Buscar por MAC, IP, hostname, SSID ou apelido...',
          onSearchChanged: (_) => setState(() {}),
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
                    : () => ref.read(wifiClientsProvider.notifier).refresh(),
                icon: wifi.requesting || wifi.isReading
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.refresh, size: 18),
                label: const Text('Atualizar APs'),
              ),
          ],
          filters: <Widget>[
            if (wifi.devices.isNotEmpty)
              ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 180),
                child: DropdownButtonFormField<int?>(
                  initialValue: _apFilter,
                  isDense: true,
                  decoration: const InputDecoration(labelText: 'AP'),
                  items: <DropdownMenuItem<int?>>[
                    const DropdownMenuItem<int?>(
                      value: null,
                      child: Text('Todos os APs'),
                    ),
                    for (final ap in wifi.devices)
                      DropdownMenuItem<int?>(
                        value: ap.deviceId,
                        child: Text(ap.deviceName),
                      ),
                  ],
                  onChanged: (v) => setState(() => _apFilter = v),
                ),
              ),
            if (ssids.isNotEmpty)
              ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 160),
                child: DropdownButtonFormField<String>(
                  initialValue: _ssidFilter,
                  isDense: true,
                  decoration: const InputDecoration(labelText: 'SSID'),
                  items: <DropdownMenuItem<String>>[
                    const DropdownMenuItem(
                      value: 'all',
                      child: Text('Todos SSIDs'),
                    ),
                    for (final s in ssids)
                      DropdownMenuItem(value: s, child: Text(s)),
                  ],
                  onChanged: (v) => setState(() => _ssidFilter = v ?? 'all'),
                ),
              ),
            ChoiceChip(
              label: const Text('Vigiados'),
              selected: _watchedOnly,
              onSelected: (v) => setState(() => _watchedOnly = v),
            ),
            ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 150),
              child: DropdownButtonFormField<String>(
                initialValue: _gradeFilter,
                isDense: true,
                decoration: const InputDecoration(labelText: 'Sinal'),
                items: const <DropdownMenuItem<String>>[
                  DropdownMenuItem(value: 'all', child: Text('Todos')),
                  DropdownMenuItem(
                    value: 'excellent',
                    child: Text('Excelente'),
                  ),
                  DropdownMenuItem(value: 'good', child: Text('Bom')),
                  DropdownMenuItem(value: 'fair', child: Text('Regular')),
                  DropdownMenuItem(value: 'poor', child: Text('Fraco')),
                ],
                onChanged: (v) => setState(() => _gradeFilter = v ?? 'all'),
              ),
            ),
            for (final b in const <String>['all', '2.4GHz', '5GHz', '6GHz'])
              ChoiceChip(
                label: Text(b == 'all' ? 'Todas bandas' : b),
                selected: _bandFilter == b,
                onSelected: (_) => setState(() => _bandFilter = b),
              ),
          ],
        ),
        if (wifi.watched.isNotEmpty) ...<Widget>[
          const SizedBox(height: 10),
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
        const SizedBox(height: 10),
        TabBar(
          controller: _tabController,
          tabs: <Widget>[
            Tab(
              icon: const Icon(Icons.list),
              text: 'Lista (${filtered.length})',
            ),
            const Tab(icon: Icon(Icons.radar), text: 'Mapa de Sinal (Ao vivo)'),
          ],
        ),
        const SizedBox(height: 10),
        Expanded(
          child: TabBarView(
            controller: _tabController,
            children: <Widget>[
              // ── Tab 1: Lista ──
              filtered.isEmpty
                  ? EmptyStateView(
                      title: 'Nenhum cliente Wi-Fi conectado',
                      description: 'Os dados chegam automaticamente via SSE (wifi:snapshot) dos APs com plugin Wi-Fi.',
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
                    )
                  : ListView.builder(
                      itemCount: filtered.length,
                      itemBuilder: (context, index) {
                        final row = filtered[index];
                        final st = row.station;
                        final watched = watchedByMac[st.mac.toLowerCase()];
                        return Card(
                          elevation: 2,
                          margin: const EdgeInsets.only(bottom: 8),
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
                            ),
                            trailing: Column(
                              mainAxisAlignment: MainAxisAlignment.center,
                              crossAxisAlignment: CrossAxisAlignment.end,
                              children: <Widget>[
                                if (st.txRateBps != null)
                                  Text(
                                    formatBps(st.txRateBps),
                                    style: context.textTheme.labelMedium
                                        ?.copyWith(fontWeight: FontWeight.w700),
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
                            ),
                          ),
                        );
                      },
                    ),
              // ── Tab 2: Mapa Radial Interativo (com `interest=wifi`) ──
              _WifiSignalMapView(
                snapshots: wifi.devices,
                recentRoams: wifi.roams,
                onTapStation: _openStationInspector,
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _WifiSignalMapView extends StatelessWidget {
  const _WifiSignalMapView({
    required this.snapshots,
    required this.recentRoams,
    required this.onTapStation,
  });

  final List<WifiDeviceSnapshotModel> snapshots;
  final List<WifiRoamEventModel> recentRoams;
  final ValueChanged<WifiStationRow> onTapStation;

  @override
  Widget build(BuildContext context) {
    if (snapshots.isEmpty) {
      return const EmptyStateView(
        title: 'Aguardando snapshots Wi-Fi ao vivo...',
        description:
            'Interesse SSE "wifi" ativo: os APs reportam em cadência ao vivo.',
        icon: Icons.radar,
        tone: SemanticTone.plugin,
      );
    }

    const double canvasW = 1400;
    const double canvasH = 900;
    final apCenters = <int, Offset>{};
    final cols = math.max(1, math.sqrt(snapshots.length).ceil());
    for (var i = 0; i < snapshots.length; i++) {
      final col = i % cols;
      final row = i ~/ cols;
      apCenters[snapshots[i].deviceId] = Offset(
        350.0 + col * 560.0,
        350.0 + row * 480.0,
      );
    }

    final stationOffsets = <(WifiStationRow, Offset, Offset)>[];
    for (final ap in snapshots) {
      final center = apCenters[ap.deviceId]!;
      final count = ap.stations.length;
      for (var i = 0; i < count; i++) {
        final st = ap.stations[i];
        final angle = (2 * math.pi * i) / math.max(1, count);
        // Raio proporcional ao sinal RSSI (-35 dBm perto -> -90 dBm longe)
        final normalizedDist = ((-st.signalDbm - 35) / 55.0).clamp(0.25, 1.0);
        final radius = 85.0 + normalizedDist * 155.0;
        final pos = Offset(
          center.dx + radius * math.cos(angle),
          center.dy + radius * math.sin(angle),
        );
        stationOffsets.add((
          WifiStationRow(
            deviceId: ap.deviceId,
            deviceName: ap.deviceName,
            canPing: ap.canPing,
            station: st,
          ),
          center,
          pos,
        ));
      }
    }

    return Card(
      elevation: 2,
      clipBehavior: Clip.antiAlias,
      child: InteractiveViewer(
        constrained: false,
        boundaryMargin: const EdgeInsets.all(300),
        minScale: 0.4,
        maxScale: 2.2,
        child: SizedBox(
          width: canvasW,
          height: canvasH,
          child: Stack(
            children: <Widget>[
              Positioned.fill(
                child: CustomPaint(
                  painter: _WifiRadialPainter(
                    apCenters: apCenters.values.toList(growable: false),
                    links: stationOffsets,
                    scheme: context.colorScheme,
                    semantics: context.semanticColors,
                  ),
                ),
              ),
              for (final ap in snapshots)
                Positioned(
                  left: apCenters[ap.deviceId]!.dx - 54,
                  top: apCenters[ap.deviceId]!.dy - 28,
                  child: Container(
                    width: 108,
                    padding: const EdgeInsets.symmetric(
                      horizontal: 8,
                      vertical: 6,
                    ),
                    decoration: BoxDecoration(
                      color: context.semanticColors.pluginContainer,
                      borderRadius: BorderRadius.circular(12),
                      border: Border.all(
                        color: context.semanticColors.plugin,
                        width: 2,
                      ),
                    ),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        Icon(
                          Icons.router,
                          size: 18,
                          color: context.semanticColors.onPluginContainer,
                        ),
                        Text(
                          ap.deviceName,
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: context.textTheme.labelSmall?.copyWith(
                            fontWeight: FontWeight.w800,
                            color: context.semanticColors.onPluginContainer,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              for (final entry in stationOffsets)
                Positioned(
                  left: entry.$3.dx - 46,
                  top: entry.$3.dy - 18,
                  child: InkWell(
                    onTap: () => onTapStation(entry.$1),
                    borderRadius: BorderRadius.circular(10),
                    child: Container(
                      width: 92,
                      padding: const EdgeInsets.symmetric(
                        horizontal: 6,
                        vertical: 4,
                      ),
                      decoration: BoxDecoration(
                        color: context.colorScheme.surface,
                        borderRadius: BorderRadius.circular(10),
                        border: Border.all(
                          color: context.colorScheme.outlineVariant,
                        ),
                      ),
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: <Widget>[
                          Text(
                            entry.$1.displayName,
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            style: context.textTheme.labelSmall?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                          Text(
                            '${entry.$1.station.signalDbm} dBm',
                            style: TextStyle(
                              fontSize: 10,
                              fontWeight: FontWeight.w700,
                              color: entry.$1.station.signalDbm >= -65
                                  ? context.semanticColors.success
                                  : context.semanticColors.warning,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}

class _WifiRadialPainter extends CustomPainter {
  const _WifiRadialPainter({
    required this.apCenters,
    required this.links,
    required this.scheme,
    required this.semantics,
  });

  final List<Offset> apCenters;
  final List<(WifiStationRow, Offset, Offset)> links;
  final ColorScheme scheme;
  final AppSemanticColors semantics;

  @override
  void paint(Canvas canvas, Size size) {
    final ringPaint = Paint()
      ..color = scheme.outlineVariant.withValues(alpha: 0.4)
      ..style = PaintingStyle.stroke
      ..strokeWidth = 1;

    for (final center in apCenters) {
      for (final r in const <double>[110, 175, 240]) {
        canvas.drawCircle(center, r, ringPaint);
      }
    }

    for (final entry in links) {
      final score = entry.$1.station.linkScore;
      final color = score >= 0.7
          ? semantics.success
          : score >= 0.4
          ? semantics.warning
          : semantics.error;
      final linePaint = Paint()
        ..color = color.withValues(alpha: 0.75)
        ..strokeWidth = 2;
      canvas.drawLine(entry.$2, entry.$3, linePaint);
    }
  }

  @override
  bool shouldRepaint(covariant _WifiRadialPainter oldDelegate) =>
      oldDelegate.links != links || oldDelegate.apCenters != apCenters;
}
