import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/vpn/models/vpn_models.dart';
import 'package:netmonitor_app/features/vpn/providers/vpn_provider.dart';
import 'package:netmonitor_app/features/vpn/widgets/vpn_firewall_hints_dialog.dart';
import 'package:netmonitor_app/features/vpn/widgets/vpn_script_viewer.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Aba VPN embutida em `/devices/:id` via `DeviceDetailTabRegistry`.
class DeviceVpnTab extends ConsumerStatefulWidget {
  const DeviceVpnTab({super.key, required this.deviceId});

  final int deviceId;

  @override
  ConsumerState<DeviceVpnTab> createState() => _DeviceVpnTabState();
}

class _DeviceVpnTabState extends ConsumerState<DeviceVpnTab> {
  List<VpnMetricSample> _metrics = const <VpnMetricSample>[];
  bool _loadingMetrics = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _load());
  }

  Future<void> _load() async {
    final notifier = ref.read(vpnProvider.notifier);
    if (ref.read(vpnProvider).serverState == null) {
      await notifier.fetchServer();
    }
    if (ref.read(vpnProvider).peers.isEmpty) {
      await notifier.fetchPeers();
    }
    if (!mounted) return;
    setState(() => _loadingMetrics = true);
    final metrics = await notifier.fetchDeviceMetrics(widget.deviceId);
    if (mounted) {
      setState(() {
        _metrics = metrics;
        _loadingMetrics = false;
      });
    }
  }

  Future<void> _openConfig(VpnPeer peer) async {
    final artifact = await ref.read(vpnProvider.notifier).fetchConfig(peer.id);
    if (artifact != null && mounted) {
      await showVpnScriptViewer(
        context: context,
        ref: ref,
        artifact: artifact,
        peerId: peer.id,
        deviceName: peer.displayName,
      );
    }
  }

  Future<void> _rotateKeys(VpnPeer peer) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Rotacionar chaves de ${peer.displayName}?',
      message:
          'A chave atual será revogada na hora e o dispositivo só volta a conectar após aplicar a nova configuração.',
      confirmLabel: 'Rotacionar',
      tone: SemanticTone.warning,
      icon: Icons.key_outlined,
    );
    if (!confirmed || !mounted) return;
    final artifact = await ref.read(vpnProvider.notifier).rotateKeys(peer.id);
    if (artifact != null && mounted) {
      await showVpnScriptViewer(
        context: context,
        ref: ref,
        artifact: artifact,
        peerId: peer.id,
        deviceName: peer.displayName,
      );
    }
  }

  Future<void> _revoke(VpnPeer peer) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Revogar acesso VPN?',
      message:
          'O peer ${peer.displayName} será removido do WireGuard e o IP ${peer.ipAddress} voltará a ficar livre.',
      confirmLabel: 'Revogar acesso',
      tone: SemanticTone.error,
      icon: Icons.delete_outline,
    );
    if (!confirmed || !mounted) return;
    final ok = await ref.read(vpnProvider.notifier).revokePeer(peer.id);
    if (ok && mounted) {
      context.go('/vpn/devices');
    }
  }

  Future<void> _openFirewallHints(VpnPeer peer) async {
    final rules = await ref
        .read(vpnProvider.notifier)
        .fetchFirewallHints(peer.id);
    if (rules != null && mounted) {
      await showVpnFirewallHintsDialog(
        context: context,
        deviceName: peer.displayName,
        rulesContent: rules,
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final vpn = ref.watch(vpnProvider);
    final auth = ref.watch(authProvider);
    final canWrite = auth.canWriteIn(ProductArea.vpn);
    final peer = vpn.peerForDevice(widget.deviceId);

    if (vpn.loading && peer == null) {
      return const LoadingStateView(
        message: 'Carregando dados do túnel VPN...',
      );
    }

    if (peer == null) {
      return EmptyStateView(
        title: 'Este dispositivo não possui túnel VPN',
        description:
            'Crie um peer na tela VPN WireGuard para conectar este equipamento pelo túnel.',
        icon: Icons.vpn_lock_outlined,
        action: FilledButton.icon(
          onPressed: () => context.go('/vpn/devices'),
          icon: const Icon(Icons.open_in_new),
          label: const Text('Abrir VPN WireGuard'),
        ),
      );
    }

    final rxSamples = _metrics
        .where((s) => s.metricName == 'vpn_rx_bps')
        .toList(growable: false);
    final txSamples = _metrics
        .where((s) => s.metricName == 'vpn_tx_bps')
        .toList(growable: false);

    final rxPoints = <MetricSeriesPoint>[
      for (var i = 0; i < rxSamples.length; i++)
        MetricSeriesPoint(
          x: i.toDouble(),
          y: rxSamples[i].value,
          label: formatClockTime(rxSamples[i].recordedAt),
        ),
    ];
    final txPoints = <MetricSeriesPoint>[
      for (var i = 0; i < txSamples.length; i++)
        MetricSeriesPoint(
          x: i.toDouble(),
          y: txSamples[i].value,
          label: formatClockTime(txSamples[i].recordedAt),
        ),
    ];

    return SingleChildScrollView(
      padding: const EdgeInsets.symmetric(vertical: 8),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (peer.needsFirewallHint) ...<Widget>[
            NestedPanel(
              tone: SemanticTone.warning,
              child: Row(
                children: <Widget>[
                  Icon(
                    Icons.security_outlined,
                    color: context.semanticColors
                        .resolve(SemanticTone.warning, context.colorScheme)
                        .onContainer,
                  ),
                  const SizedBox(width: 10),
                  const Expanded(
                    child: Text(
                      'Túnel conectado, mas não responde a ping. Verifique o firewall do equipamento.',
                    ),
                  ),
                  TextButton(
                    onPressed: () => _openFirewallHints(peer),
                    child: const Text('Copiar regras'),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 12),
          ],
          SectionCard(
            title: 'Túnel WireGuard',
            subtitle: vpnProfileLabel(peer.deviceProfile, vpn.profiles),
            icon: Icons.vpn_lock_outlined,
            trailing: SemanticChip(
              label: peer.connectionStatus.label,
              tone: peer.connectionStatus.tone,
              icon: peer.connectionStatus.icon,
              compact: true,
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                KeyValueRow(
                  label: 'Perfil',
                  value: vpnProfileLabel(peer.deviceProfile, vpn.profiles),
                ),
                KeyValueRow(
                  label: 'IP na VPN',
                  value: peer.ipAddress,
                  monospace: true,
                  copyable: true,
                ),
                KeyValueRow(
                  label: 'Último handshake',
                  value: formatRelativeTime(peer.lastHandshakeAt),
                ),
                KeyValueRow(
                  label: 'Keepalive',
                  value: '${peer.persistentKeepalive}s',
                ),
                KeyValueRow(
                  label: 'Chave pública do peer',
                  value: peer.publicKey,
                  monospace: true,
                  copyable: true,
                ),
                if (vpn.serverState?.cidr != null)
                  KeyValueRow(
                    label: 'Faixa VPN',
                    value: vpn.serverState!.cidr!,
                    monospace: true,
                  ),
                KeyValueRow(
                  label: 'Situação',
                  value: peer.enabled ? 'Ativo' : 'Revogado',
                ),
                const Divider(height: 20),
                ResponsiveGrid(
                  xs: 2,
                  sm: 2,
                  md: 2,
                  lg: 2,
                  xl: 2,
                  children: <Widget>[
                    KpiCard(
                      label: 'Recebido (RX)',
                      value: formatBytes(peer.bytesRx),
                      icon: Icons.south_west,
                      tone: SemanticTone.success,
                    ),
                    KpiCard(
                      label: 'Enviado (TX)',
                      value: formatBytes(peer.bytesTx),
                      icon: Icons.north_east,
                      tone: SemanticTone.info,
                    ),
                  ],
                ),
                const SizedBox(height: 16),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: <Widget>[
                    FilledButton.tonalIcon(
                      onPressed: () => _openConfig(peer),
                      icon: const Icon(Icons.description_outlined, size: 18),
                      label: const Text('Ver configuração'),
                    ),
                    if (canWrite) ...<Widget>[
                      OutlinedButton.icon(
                        onPressed: () => _rotateKeys(peer),
                        icon: const Icon(Icons.key_outlined, size: 18),
                        label: const Text('Rotacionar chaves'),
                      ),
                      OutlinedButton.icon(
                        style: OutlinedButton.styleFrom(
                          foregroundColor: context.colorScheme.error,
                          side: BorderSide(color: context.colorScheme.error),
                        ),
                        onPressed: () => _revoke(peer),
                        icon: const Icon(Icons.delete_outline, size: 18),
                        label: const Text('Revogar acesso'),
                      ),
                    ],
                  ],
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          SectionCard(
            title: 'Tráfego do túnel',
            subtitle: 'Taxa recebida (RX) e enviada (TX)',
            icon: Icons.show_chart,
            child: _loadingMetrics
                ? const LoadingStateView(
                    message: 'Carregando histórico de tráfego...',
                  )
                : MetricLineChart(
                    height: 220,
                    emptyText: 'Ainda sem amostras de tráfego para este túnel.',
                    yFormatter: (v) => formatBps(v, fractionDigits: 1),
                    series: <MetricLineSeries>[
                      MetricLineSeries(
                        name: 'RX',
                        points: rxPoints,
                        tone: SemanticTone.success,
                        showArea: false,
                      ),
                      MetricLineSeries(
                        name: 'TX',
                        points: txPoints,
                        tone: SemanticTone.info,
                        showArea: false,
                      ),
                    ],
                  ),
          ),
        ],
      ),
    );
  }
}
