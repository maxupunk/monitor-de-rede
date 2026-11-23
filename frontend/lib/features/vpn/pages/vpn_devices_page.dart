import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/vpn/models/vpn_models.dart';
import 'package:netmonitor_app/features/vpn/pages/vpn_layout.dart';
import 'package:netmonitor_app/features/vpn/providers/vpn_provider.dart';
import 'package:netmonitor_app/features/vpn/widgets/vpn_firewall_hints_dialog.dart';
import 'package:netmonitor_app/features/vpn/widgets/vpn_peer_wizard.dart';
import 'package:netmonitor_app/features/vpn/widgets/vpn_script_viewer.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Página `/vpn/devices` — Lista de peers WireGuard atualizada ao vivo via SSE (`vpn:peers_updated`).
class VpnDevicesPage extends ConsumerStatefulWidget {
  const VpnDevicesPage({super.key});

  @override
  ConsumerState<VpnDevicesPage> createState() => _VpnDevicesPageState();
}

class _VpnDevicesPageState extends ConsumerState<VpnDevicesPage> {
  final _searchCtrl = TextEditingController();
  String _query = '';
  VpnConnectionStatus? _statusFilter;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted) return;
      final notifier = ref.read(vpnProvider.notifier);
      await Future.wait<void>(<Future<void>>[
        notifier.fetchServer(),
        notifier.fetchPeers(),
      ]);
    });
  }

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  Future<void> _addPeer() async {
    final notifier = ref.read(vpnProvider.notifier);
    final artifact = await showVpnPeerWizard(context: context);
    if (artifact == null) return;
    if (!mounted) {
      notifier.clearLastArtifact();
      return;
    }
    await showVpnScriptViewer(context: context, ref: ref, artifact: artifact);
  }

  Future<void> _renamePeer(VpnPeer peer) async {
    final newName = await showDialog<String>(
      context: context,
      builder: (_) => _RenamePeerDialog(initialName: peer.displayName),
    );
    if (!mounted ||
        newName == null ||
        newName.isEmpty ||
        newName == peer.displayName) {
      return;
    }
    final ok = await ref
        .read(vpnProvider.notifier)
        .renamePeer(peer.id, newName);
    if (ok && mounted) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(const SnackBar(content: Text('Dispositivo renomeado.')));
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

  Future<void> _rotatePeer(VpnPeer peer) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Rotacionar chaves de ${peer.displayName}?',
      message:
          'A chave anterior será revogada imediatamente e um novo script/QR será gerado.',
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

  Future<void> _revokePeer(VpnPeer peer) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Revogar ${peer.displayName}?',
      message:
          'O túnel será encerrado imediatamente e o IP ${peer.ipAddress} voltará para a faixa disponível.',
      confirmLabel: 'Revogar acesso',
      tone: SemanticTone.error,
      icon: Icons.delete_outline,
    );
    if (!confirmed || !mounted) return;
    final ok = await ref.read(vpnProvider.notifier).revokePeer(peer.id);
    if (ok && mounted) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(const SnackBar(content: Text('Acesso VPN revogado.')));
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
    final scheme = context.colorScheme;

    final filtered = vpn.peers
        .where((peer) {
          if (_statusFilter != null && peer.connectionStatus != _statusFilter) {
            return false;
          }
          if (_query.isNotEmpty) {
            final q = _query.toLowerCase();
            final match =
                peer.displayName.toLowerCase().contains(q) ||
                peer.ipAddress.toLowerCase().contains(q) ||
                peer.deviceProfile.toLowerCase().contains(q);
            if (!match) return false;
          }
          return true;
        })
        .toList(growable: false);

    return VpnLayout(
      activeTab: 'devices',
      slivers: <Widget>[
        SliverToBoxAdapter(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              if (!vpn.isConfigured && !vpn.loading) ...<Widget>[
                NestedPanel(
                  tone: SemanticTone.info,
                  child: Wrap(
                    spacing: 8,
                    runSpacing: 6,
                    alignment: WrapAlignment.spaceBetween,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: <Widget>[
                      const Text(
                        'Configure o servidor VPN antes de adicionar dispositivos.',
                      ),
                      TextButton(
                        onPressed: () => context.go('/vpn'),
                        child: const Text('Configurar servidor'),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 12),
              ],
              UnifiedFilterBar(
                searchController: _searchCtrl,
                searchHint: 'Buscar por nome, IP ou perfil...',
                onSearchChanged: (value) =>
                    setState(() => _query = value.trim()),
                trailingActions: <Widget>[
                  if (canWrite)
                    MediaQuery.sizeOf(context).width < 520
                        ? IconButton.filled(
                            tooltip: 'Adicionar dispositivo',
                            onPressed: vpn.isConfigured ? _addPeer : null,
                            icon: const Icon(Icons.add),
                          )
                        : FilledButton.icon(
                            onPressed: vpn.isConfigured ? _addPeer : null,
                            icon: const Icon(Icons.add),
                            label: const Text('Adicionar dispositivo'),
                          ),
                ],
                quickChips: <Widget>[
                  FilterChip(
                    label: const Text('Todos'),
                    visualDensity: VisualDensity.compact,
                    selected: _statusFilter == null,
                    onSelected: (_) => setState(() => _statusFilter = null),
                  ),
                  for (final status in VpnConnectionStatus.values)
                    FilterChip(
                      label: Text(status.label),
                      visualDensity: VisualDensity.compact,
                      selected: _statusFilter == status,
                      onSelected: (selected) => setState(
                        () => _statusFilter = selected ? status : null,
                      ),
                    ),
                ],
              ),
              const SizedBox(height: 12),
            ],
          ),
        ),
        if (vpn.loading && vpn.peers.isEmpty)
          const SliverFillRemaining(
            hasScrollBody: false,
            child: LoadingStateView(message: 'Carregando dispositivos VPN...'),
          )
        else if (filtered.isEmpty)
          SliverFillRemaining(
            hasScrollBody: false,
            child: EmptyStateView(
              title: vpn.peers.isEmpty
                  ? 'Nenhum dispositivo na VPN'
                  : 'Nenhum dispositivo encontrado',
              description: vpn.peers.isEmpty
                  ? 'Adicione equipamentos, servidores remotos ou celulares para conectar pelo WireGuard.'
                  : 'Ajuste os filtros de busca para ver outros peers.',
              icon: Icons.vpn_key_off_outlined,
              action: canWrite && vpn.isConfigured && vpn.peers.isEmpty
                  ? FilledButton.icon(
                      onPressed: _addPeer,
                      icon: const Icon(Icons.add),
                      label: const Text('Adicionar dispositivo'),
                    )
                  : null,
            ),
          )
        else
          SliverList.separated(
            itemCount: filtered.length,
            separatorBuilder: (_, _) => const SizedBox(height: 10),
            itemBuilder: (context, index) {
              final peer = filtered[index];
              final supportsQr = vpnProfileSupportsQrCode(
                peer.deviceProfile,
                vpn.profiles,
              );
              final lastActivity = peer.lastSeenAt ?? peer.lastHandshakeAt;
              return SectionCard(
                padding: const EdgeInsets.all(14),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Container(
                          width: 40,
                          height: 40,
                          decoration: BoxDecoration(
                            color: scheme.primaryContainer,
                            borderRadius: BorderRadius.circular(10),
                          ),
                          child: Icon(
                            vpnProfileIconData(
                              peer.deviceProfile,
                              vpn.profiles,
                            ),
                            color: scheme.onPrimaryContainer,
                          ),
                        ),
                        const SizedBox(width: 12),
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: <Widget>[
                              Wrap(
                                spacing: 8,
                                runSpacing: 4,
                                crossAxisAlignment: WrapCrossAlignment.center,
                                children: <Widget>[
                                  InkWell(
                                    onTap: () => context.go(
                                      '/devices/${peer.deviceId}?tab=vpn',
                                    ),
                                    child: Text(
                                      peer.displayName,
                                      style: context.textTheme.titleSmall
                                          ?.copyWith(
                                            fontWeight: FontWeight.w700,
                                            color: scheme.primary,
                                          ),
                                    ),
                                  ),
                                  SemanticChip(
                                    label: peer.connectionStatus.label,
                                    tone: peer.connectionStatus.tone,
                                    icon: peer.connectionStatus.icon,
                                    compact: true,
                                  ),
                                ],
                              ),
                              const SizedBox(height: 4),
                              Wrap(
                                spacing: 12,
                                runSpacing: 4,
                                children: <Widget>[
                                  Text(
                                    vpnProfileLabel(
                                      peer.deviceProfile,
                                      vpn.profiles,
                                    ),
                                    style: context.textTheme.bodySmall
                                        ?.copyWith(
                                          color: scheme.onSurfaceVariant,
                                        ),
                                  ),
                                  Text(
                                    peer.ipAddress,
                                    style: context.textTheme.bodySmall
                                        ?.copyWith(
                                          fontFamily: 'monospace',
                                          fontWeight: FontWeight.w600,
                                        ),
                                  ),
                                  Tooltip(
                                    message:
                                        'Handshake: ${formatDateTime(peer.lastHandshakeAt)}',
                                    child: Text(
                                      'Atividade: ${formatRelativeTime(lastActivity)}',
                                      style: context.textTheme.bodySmall
                                          ?.copyWith(
                                            color: scheme.onSurfaceVariant,
                                          ),
                                    ),
                                  ),
                                  Text(
                                    'RX ${formatBytes(peer.bytesRx)} · TX ${formatBytes(peer.bytesTx)}',
                                    style: context.textTheme.bodySmall
                                        ?.copyWith(fontWeight: FontWeight.w600),
                                  ),
                                ],
                              ),
                            ],
                          ),
                        ),
                      ],
                    ),
                    if (peer.needsFirewallHint) ...<Widget>[
                      const SizedBox(height: 10),
                      NestedPanel(
                        tone: SemanticTone.warning,
                        padding: const EdgeInsets.symmetric(
                          horizontal: 12,
                          vertical: 8,
                        ),
                        child: Wrap(
                          spacing: 8,
                          runSpacing: 6,
                          alignment: WrapAlignment.spaceBetween,
                          crossAxisAlignment: WrapCrossAlignment.center,
                          children: <Widget>[
                            const Text(
                              'Túnel conectado, mas não responde a ping',
                            ),
                            TextButton(
                              onPressed: () => _openFirewallHints(peer),
                              child: const Text('Copiar regras de firewall'),
                            ),
                          ],
                        ),
                      ),
                    ] else if (peer.pingOutsideTunnel) ...<Widget>[
                      const SizedBox(height: 10),
                      const NestedPanel(
                        tone: SemanticTone.info,
                        padding: EdgeInsets.symmetric(
                          horizontal: 12,
                          vertical: 8,
                        ),
                        child: Text(
                          'O ping não sai pelo túnel — registre o vpn-probe.',
                        ),
                      ),
                    ],
                    const SizedBox(height: 10),
                    SymmetricalActionBar(
                      spacing: 8,
                      runSpacing: 8,
                      primaryFullWidthWhenOdd: true,
                      children: <Widget>[
                        FilledButton.tonalIcon(
                          style: FilledButton.styleFrom(
                            padding: const EdgeInsets.symmetric(
                              horizontal: 10,
                              vertical: 10,
                            ),
                          ),
                          onPressed: () => _openConfig(peer),
                          icon: Icon(
                            supportsQr
                                ? Icons.qr_code_2
                                : Icons.description_outlined,
                            size: 17,
                          ),
                          label: Text(
                            supportsQr ? 'QR / Config' : 'Config',
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                        if (peer.pingMonitorId != null)
                          OutlinedButton.icon(
                            style: OutlinedButton.styleFrom(
                              padding: const EdgeInsets.symmetric(
                                horizontal: 10,
                                vertical: 10,
                              ),
                            ),
                            onPressed: () =>
                                context.go('/monitors/${peer.pingMonitorId}'),
                            icon: const Icon(
                              Icons.monitor_heart_outlined,
                              size: 17,
                            ),
                            label: const Text(
                              'Histórico ping',
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                        if (canWrite) ...<Widget>[
                          OutlinedButton.icon(
                            style: OutlinedButton.styleFrom(
                              padding: const EdgeInsets.symmetric(
                                horizontal: 10,
                                vertical: 10,
                              ),
                            ),
                            onPressed: () => _renamePeer(peer),
                            icon: const Icon(Icons.edit_outlined, size: 17),
                            label: const Text(
                              'Renomear',
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                          OutlinedButton.icon(
                            style: OutlinedButton.styleFrom(
                              padding: const EdgeInsets.symmetric(
                                horizontal: 10,
                                vertical: 10,
                              ),
                            ),
                            onPressed: () => _rotatePeer(peer),
                            icon: const Icon(Icons.key_outlined, size: 17),
                            label: const Text(
                              'Rotacionar',
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                          OutlinedButton.icon(
                            style: OutlinedButton.styleFrom(
                              padding: const EdgeInsets.symmetric(
                                horizontal: 10,
                                vertical: 10,
                              ),
                            ),
                            onPressed: () => _revokePeer(peer),
                            icon: Icon(
                              Icons.delete_outline,
                              size: 17,
                              color: scheme.error,
                            ),
                            label: Text(
                              'Revogar',
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                              style: TextStyle(color: scheme.error),
                            ),
                          ),
                        ],
                      ],
                    ),
                  ],
                ),
              );
            },
          ),
      ],
    );
  }
}

class _RenamePeerDialog extends StatefulWidget {
  const _RenamePeerDialog({required this.initialName});

  final String initialName;

  @override
  State<_RenamePeerDialog> createState() => _RenamePeerDialogState();
}

class _RenamePeerDialogState extends State<_RenamePeerDialog> {
  late final TextEditingController _ctrl;

  @override
  void initState() {
    super.initState();
    _ctrl = TextEditingController(text: widget.initialName);
  }

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Renomear dispositivo VPN'),
      content: TextField(
        controller: _ctrl,
        autofocus: true,
        decoration: const InputDecoration(labelText: 'Nome'),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancelar'),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(_ctrl.text.trim()),
          child: const Text('Salvar'),
        ),
      ],
    );
  }
}
