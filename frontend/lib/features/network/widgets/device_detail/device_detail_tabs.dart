import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/device_presentation.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/device_models.dart';
import 'package:netmonitor_app/features/network/providers/devices_provider.dart';
import 'package:netmonitor_app/features/network/providers/wifi_provider.dart';
import 'package:netmonitor_app/features/network/widgets/diagnostics_dialogs.dart';
import 'package:netmonitor_app/features/network/widgets/wifi_qr_sheet.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class DeviceOverviewTab extends StatelessWidget {
  const DeviceOverviewTab({super.key, required this.detail});

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

    final vendorModel = <String>[
      if (d.vendor != null) d.vendor!,
      if (d.model != null) d.model!,
    ].join(' · ');

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
                    value: vendorModel.trim().isEmpty ? '—' : vendorModel,
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
              yFormatter: (v) => formatPercent(v, digits: 0),
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

class DeviceInterfacesTab extends ConsumerWidget {
  const DeviceInterfacesTab({
    super.key,
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

class DeviceEventsTab extends StatelessWidget {
  const DeviceEventsTab({super.key, required this.events});

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

class DeviceWifiTab extends ConsumerWidget {
  const DeviceWifiTab({super.key, required this.device});

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
                            label: formatSignalDbm(st.signalDbm),
                            tone: wifiSignalTone(st.signalDbm),
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
