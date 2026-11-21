import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/network/models/site_network_models.dart';
import 'package:netmonitor_app/features/network/providers/sites_networks_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

class ProbesPage extends ConsumerStatefulWidget {
  const ProbesPage({super.key});

  @override
  ConsumerState<ProbesPage> createState() => _ProbesPageState();
}

class _ProbesPageState extends ConsumerState<ProbesPage> {
  final TextEditingController _searchCtrl = TextEditingController();
  String _statusFilter = 'all';

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(
      () => ref.read(probesProvider.notifier).fetchProbes(),
    );
  }

  @override
  void dispose() {
    _searchCtrl.dispose();
    super.dispose();
  }

  Future<void> _testProbe(ProbeModel probe) async {
    final ok = await ref.read(probesProvider.notifier).testProbe(probe.id);
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          ok
              ? 'Sonda "${probe.name}" respondeu com sucesso.'
              : (ref.read(probesProvider).error ?? 'Falha ao testar sonda.'),
        ),
      ),
    );
  }

  Future<void> _revokeProbe(ProbeModel probe) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Revogar sonda',
      message:
          'Revogar o token de acesso da sonda "${probe.name}"? Ela não poderá mais reportar medições até ser registrada novamente.',
      confirmLabel: 'Revogar',
    );
    if (!confirmed) return;
    final ok = await ref.read(probesProvider.notifier).revokeProbe(probe.id);
    if (ok && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Sonda "${probe.name}" revogada.')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(probesProvider);
    final canWrite = ref.watch(authProvider).canWrite;
    final query = _searchCtrl.text.trim().toLowerCase();

    final filtered = state.items
        .where((p) {
          if (_statusFilter != 'all' && p.status != _statusFilter) return false;
          if (query.isEmpty) return true;
          return '${p.name} ${p.location ?? ''} ${p.ipAddress ?? ''} ${p.probeType ?? ''}'
              .toLowerCase()
              .contains(query);
        })
        .toList(growable: false);

    return RefreshIndicator(
      onRefresh: () => ref.read(probesProvider.notifier).fetchProbes(),
      child: ListView(
        children: <Widget>[
          SearchFilterHeader(
            title: 'Sondas de Rede',
            subtitle: 'Coletores remotos e sonda dedicada WireGuard (vpn-probe) atualizados via SSE',
            searchController: _searchCtrl,
            searchHint: 'Buscar sonda por nome, IP ou localização...',
            onSearchChanged: (_) => setState(() {}),
            actions: <Widget>[
              OutlinedButton.icon(
                onPressed: () => context.go('/remote-servers'),
                icon: const Icon(Icons.dns_outlined, size: 18),
                label: const Text('Servidores Remotos'),
              ),
            ],
            filters: <Widget>[
              for (final opt in const <(String, String)>[
                ('all', 'Todas'),
                ('online', 'Online'),
                ('offline', 'Offline'),
                ('revoked', 'Revogadas'),
              ])
                ChoiceChip(
                  label: Text(opt.$2),
                  selected: _statusFilter == opt.$1,
                  onSelected: (_) => setState(() => _statusFilter = opt.$1),
                ),
            ],
          ),
          const SizedBox(height: 14),
          ResponsiveGrid(
            xs: 2,
            sm: 2,
            md: 4,
            lg: 4,
            xl: 4,
            children: <Widget>[
              KpiCard(
                label: 'Total',
                value: formatInteger(state.items.length),
                icon: Icons.sensors,
                tone: SemanticTone.primary,
              ),
              KpiCard(
                label: 'Online',
                value: formatInteger(state.onlineCount),
                icon: Icons.check_circle_outline,
                tone: SemanticTone.success,
              ),
              KpiCard(
                label: 'Offline',
                value: formatInteger(state.offlineCount),
                icon: Icons.cloud_off_outlined,
                tone: SemanticTone.warning,
              ),
              KpiCard(
                label: 'Revogadas',
                value: formatInteger(state.revokedCount),
                icon: Icons.block_outlined,
                tone: SemanticTone.error,
              ),
            ],
          ),
          const SizedBox(height: 12),
          NestedPanel(
            tone: SemanticTone.info,
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                const Icon(Icons.info_outline, size: 20),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    'A sonda vpn-probe mede ICMP/SNMP na faixa 10.8.0.x dentro do namespace WireGuard. '
                    'Na topologia padrão o namespace já reside no container da API; com VPN_PROBE_EXTERNAL=true ela aparece aqui.',
                    style: context.textTheme.bodySmall,
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 14),
          if (state.loading && state.items.isEmpty)
            const LoadingStateView(message: 'Carregando sondas de rede...')
          else if (state.error != null && state.items.isEmpty)
            ErrorStateView(
              message: state.error!,
              onRetry: () => ref.read(probesProvider.notifier).fetchProbes(),
            )
          else if (filtered.isEmpty)
            const EmptyStateView(
              title: 'Nenhuma sonda registrada',
              description: 'Registre uma sonda remota via CLI (backend-cli task vpn_probe_register) ou conecte um agente em Servidores Remotos.',
              icon: Icons.sensors_off,
            )
          else
            ResponsiveGrid(
              xs: 1,
              sm: 1,
              md: 2,
              lg: 3,
              xl: 3,
              children: filtered
                  .map((probe) {
                    final testing = state.testingId == probe.id;
                    return SectionCard(
                      title: probe.name,
                      subtitle:
                          probe.location ??
                          probe.ipAddress ??
                          'Sem localização',
                      icon: probe.isVpnProbe
                          ? Icons.vpn_lock_outlined
                          : Icons.sensors,
                      iconTone: probe.status == 'online'
                          ? SemanticTone.success
                          : probe.status == 'revoked'
                          ? SemanticTone.error
                          : SemanticTone.warning,
                      trailing: StatusIndicator.fromStatus(
                        probe.status,
                        compact: true,
                      ),
                      footer: canWrite
                          ? Row(
                              mainAxisAlignment: MainAxisAlignment.end,
                              children: <Widget>[
                                OutlinedButton.icon(
                                  onPressed:
                                      testing || probe.status == 'revoked'
                                      ? null
                                      : () => _testProbe(probe),
                                  icon: testing
                                      ? const SizedBox(
                                          width: 14,
                                          height: 14,
                                          child: CircularProgressIndicator(
                                            strokeWidth: 2,
                                          ),
                                        )
                                      : const Icon(
                                          Icons.network_check,
                                          size: 16,
                                        ),
                                  label: const Text('Testar'),
                                ),
                                if (probe.status != 'revoked') ...<Widget>[
                                  const SizedBox(width: 8),
                                  TextButton.icon(
                                    onPressed: () => _revokeProbe(probe),
                                    icon: Icon(
                                      Icons.block,
                                      size: 16,
                                      color: context.colorScheme.error,
                                    ),
                                    label: Text(
                                      'Revogar',
                                      style: TextStyle(
                                        color: context.colorScheme.error,
                                      ),
                                    ),
                                  ),
                                ],
                              ],
                            )
                          : null,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: <Widget>[
                          Wrap(
                            spacing: 6,
                            runSpacing: 6,
                            children: <Widget>[
                              if (probe.isVpnProbe)
                                const SemanticChip(
                                  label: 'vpn-probe',
                                  tone: SemanticTone.primary,
                                  compact: true,
                                ),
                              if (probe.version != null)
                                SemanticChip(
                                  label: 'v${probe.version}',
                                  tone: SemanticTone.info,
                                  compact: true,
                                ),
                            ],
                          ),
                          const SizedBox(height: 8),
                          KeyValueRow(
                            label: 'IP',
                            value: probe.ipAddress ?? '—',
                            monospace: true,
                          ),
                          KeyValueRow(
                            label: 'Último heartbeat',
                            value: formatRelativeTime(probe.lastHeartbeatAt),
                          ),
                        ],
                      ),
                    );
                  })
                  .toList(growable: false),
            ),
          const SizedBox(height: 24),
        ],
      ),
    );
  }
}
