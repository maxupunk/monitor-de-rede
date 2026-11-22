import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/device_presentation.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/wifi_models.dart';
import 'package:netmonitor_app/features/network/providers/wifi_provider.dart';
import 'package:netmonitor_app/features/network/widgets/wifi_qr_sheet.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

Future<void> showWifiWatchClientSheet(
  BuildContext context, {
  required WifiStationRow row,
  WifiWatchedClientModel? existing,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: existing != null
        ? 'Renomear aparelho vigiado'
        : 'Vigiar aparelho Wi-Fi',
    subtitle: row.station.mac,
    builder: (ctx) => _WifiWatchClientFormBody(
      row: row,
      existing: existing,
      onDone: () => Navigator.of(ctx).pop(),
    ),
  );
}

class _WifiWatchClientFormBody extends ConsumerStatefulWidget {
  const _WifiWatchClientFormBody({
    required this.row,
    this.existing,
    required this.onDone,
  });

  final WifiStationRow row;
  final WifiWatchedClientModel? existing;
  final VoidCallback onDone;

  @override
  ConsumerState<_WifiWatchClientFormBody> createState() =>
      _WifiWatchClientFormBodyState();
}

class _WifiWatchClientFormBodyState
    extends ConsumerState<_WifiWatchClientFormBody> {
  late final TextEditingController _nameCtrl;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    _nameCtrl = TextEditingController(
      text: widget.existing?.name ?? widget.row.displayName,
    );
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    super.dispose();
  }

  Future<void> _unwatch() async {
    final existing = widget.existing;
    if (existing == null || _saving) return;
    setState(() => _saving = true);
    await ref.read(wifiClientsProvider.notifier).unwatch(existing.id);
    if (mounted) widget.onDone();
  }

  Future<void> _save() async {
    final trimmed = _nameCtrl.text.trim();
    if (trimmed.isEmpty || _saving) return;
    setState(() => _saving = true);
    final existing = widget.existing;
    final ok = existing != null
        ? await ref
              .read(wifiClientsProvider.notifier)
              .renameWatched(existing.id, trimmed)
        : await ref
              .read(wifiClientsProvider.notifier)
              .watch(widget.row.station.mac, trimmed);
    if (!mounted) return;
    if (ok) {
      widget.onDone();
    } else {
      setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final existing = widget.existing;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        TextField(
          controller: _nameCtrl,
          decoration: const InputDecoration(labelText: 'Apelido do aparelho'),
        ),
        const SizedBox(height: 14),
        SymmetricalActionBar(
          children: <Widget>[
            if (existing != null)
              OutlinedButton(
                onPressed: _saving ? null : _unwatch,
                child: const Text(
                  'Deixar de vigiar',
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              )
            else
              OutlinedButton(
                onPressed: _saving ? null : widget.onDone,
                child: const Text('Cancelar'),
              ),
            FilledButton.icon(
              onPressed: _saving ? null : _save,
              icon: const Icon(Icons.star_outline),
              label: const Text('Salvar'),
            ),
          ],
        ),
      ],
    );
  }
}

void showWifiStationInspectorSheet(
  BuildContext context, {
  required WidgetRef ref,
  required WifiStationRow row,
}) {
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
                  '${formatSignalDbm(st.signalDbm)} (${wifiSignalGradeLabel(st.signalGrade)})',
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
                    onPressed: () => showWifiWatchClientSheet(
                      context,
                      row: row,
                      existing: watched,
                    ),
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
                  title: Text('${roam.fromDeviceName} -> ${roam.toDeviceName}'),
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
