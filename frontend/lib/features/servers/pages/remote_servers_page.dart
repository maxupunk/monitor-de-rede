import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/areas/area_contribution.dart';
import 'package:netmonitor_app/areas/composition_registries.dart';
import 'package:netmonitor_app/core/areas/areas_provider.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/agent_models.dart';
import 'package:netmonitor_app/features/servers/providers/agents_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/features/servers/widgets/agent_enrollment_dialog.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Tela de Servidores Remotos / Agentes (`/remote-servers`, area `core` por ADR 017/019).
class RemoteServersPage extends ConsumerStatefulWidget {
  const RemoteServersPage({super.key});

  @override
  ConsumerState<RemoteServersPage> createState() => _RemoteServersPageState();
}

class _RemoteServersPageState extends ConsumerState<RemoteServersPage> {
  final TextEditingController _searchController = TextEditingController();
  String _search = '';

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(
      () => ref.read(agentsProvider.notifier).fetchAgents(),
    );
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _openCreateWizard() async {
    final visibleAreas = ref.read(visibleAreasProvider);
    final paths = ServerConnectSlot.resolve(visibleAreas);
    if (paths.isEmpty) return;

    final nameController = TextEditingController();
    var selectedPath = paths.firstWhere(
      (p) => p.recommended,
      orElse: () => paths.first,
    );

    final result =
        await showAdaptiveFormSheet<
          ({String name, ServerConnectPathContribution path})
        >(
          context: context,
          title: 'Conectar servidor remoto',
          subtitle:
              'O agente só disca para fora — nenhuma porta é aberta no servidor.',
          maxWidth: 620,
          builder: (sheetCtx) {
            return StatefulBuilder(
              builder: (ctx, setModalState) {
                return Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    TextField(
                      controller: nameController,
                      autofocus: true,
                      decoration: const InputDecoration(
                        labelText: 'Nome do servidor',
                        hintText: 'Ex.: srv-producao-01',
                      ),
                    ),
                    const SizedBox(height: 16),
                    Text(
                      'Como este servidor chega à central?',
                      style: ctx.textTheme.titleSmall?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    const SizedBox(height: 10),
                    ...paths.map(
                      (path) => Padding(
                        padding: const EdgeInsets.only(bottom: 10),
                        child: ChoiceCard(
                          title: path.title,
                          subtitle: path.description,
                          icon: path.icon,
                          selected: selectedPath.id == path.id,
                          badge: path.recommended ? 'Recomendado' : null,
                          badgeTone: SemanticTone.success,
                          tone: path.recommended
                              ? SemanticTone.success
                              : SemanticTone.primary,
                          onTap: () => setModalState(() => selectedPath = path),
                        ),
                      ),
                    ),
                    const SizedBox(height: 12),
                    Align(
                      alignment: Alignment.centerRight,
                      child: FilledButton.icon(
                        onPressed: () {
                          final name = nameController.text.trim();
                          if (name.isEmpty) {
                            ScaffoldMessenger.of(ctx).showSnackBar(
                              const SnackBar(
                                content: Text('Informe o nome do servidor.'),
                              ),
                            );
                            return;
                          }
                          Navigator.of(
                            ctx,
                          ).pop((name: name, path: selectedPath));
                        },
                        icon: const Icon(Icons.arrow_forward),
                        label: const Text('Continuar'),
                      ),
                    ),
                  ],
                );
              },
            );
          },
        );

    nameController.dispose();
    if (!mounted || result == null) return;

    // Caminho com diálogo próprio (ex.: VpnServerConnect da área VPN).
    if (result.path.dialogBuilder != null) {
      await showDialog<void>(
        context: context,
        builder: (dlgCtx) => result.path.dialogBuilder!(
          dlgCtx,
          serverName: result.name,
          agentId: null,
          onDone: () => ref.read(agentsProvider.notifier).fetchAgents(),
        ),
      );
      return;
    }

    final created = await ref
        .read(agentsProvider.notifier)
        .createAgent(AgentCreateInput(name: result.name));
    if (!mounted || created == null) return;
    await showAgentEnrollmentDialog(
      context,
      agent: created.agent,
      enrollment: created.enrollment,
      preferredKinds: result.path.addressKinds,
    );
  }

  Future<void> _openMovePath(
    AgentView agent,
    ServerConnectPathContribution path,
  ) async {
    if (path.dialogBuilder == null) return;
    await showDialog<void>(
      context: context,
      builder: (dlgCtx) => path.dialogBuilder!(
        dlgCtx,
        serverName: agent.name,
        agentId: agent.id,
        onDone: () => ref.read(agentsProvider.notifier).fetchAgents(),
      ),
    );
  }

  Future<void> _openEditAgent(AgentView agent) async {
    final nameController = TextEditingController(text: agent.name);
    var enforceTunnelIp = agent.enforceTunnelIp;
    final saved = await showAdaptiveFormSheet<bool>(
      context: context,
      title: 'Editar ${agent.name}',
      builder: (ctx) {
        return StatefulBuilder(
          builder: (innerCtx, setModalState) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                TextField(
                  controller: nameController,
                  decoration: const InputDecoration(
                    labelText: 'Nome do servidor',
                  ),
                ),
                if (agent.deviceId != null) ...<Widget>[
                  const SizedBox(height: 12),
                  SwitchListTile(
                    contentPadding: EdgeInsets.zero,
                    value: enforceTunnelIp,
                    title: const Text('Exigir conexão pelo túnel VPN'),
                    subtitle: const Text(
                      'Recusa conexões que não venham do IP do túnel associado.',
                    ),
                    onChanged: (v) => setModalState(() => enforceTunnelIp = v),
                  ),
                ],
                const SizedBox(height: 16),
                Align(
                  alignment: Alignment.centerRight,
                  child: FilledButton(
                    onPressed: () async {
                      final ok = await ref
                          .read(agentsProvider.notifier)
                          .updateAgent(
                            agent.id,
                            AgentUpdateInput(
                              name: nameController.text.trim(),
                              deviceId: agent.deviceId,
                              enforceTunnelIp: enforceTunnelIp,
                            ),
                          );
                      if (innerCtx.mounted) Navigator.of(innerCtx).pop(ok);
                    },
                    child: const Text('Salvar alterações'),
                  ),
                ),
              ],
            );
          },
        );
      },
    );
    nameController.dispose();
    if (saved == true && mounted) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(const SnackBar(content: Text('Servidor atualizado.')));
    }
  }

  Future<void> _reissueEnrollment(AgentView agent) async {
    final enrollment = await ref
        .read(agentsProvider.notifier)
        .reissueEnrollment(agent.id);
    if (!mounted || enrollment == null) return;
    await showAgentEnrollmentDialog(
      context,
      agent: agent,
      enrollment: enrollment,
    );
  }

  Future<void> _revokeAgent(AgentView agent) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Revogar acesso de ${agent.name}?',
      message:
          'O token atual do agente será invalidado imediatamente. '
          'Você poderá gerar um novo código de instalação depois.',
      confirmLabel: 'Revogar acesso',
      tone: SemanticTone.warning,
    );
    if (!confirmed || !mounted) return;
    final ok = await ref.read(agentsProvider.notifier).revokeAgent(agent.id);
    if (ok && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Acesso de ${agent.name} revogado.')),
      );
    }
  }

  Future<void> _removeAgent(AgentView agent) async {
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: 'Remover ${agent.name}?',
      message:
          'Esta ação remove o cadastro do agente, seus monitores vinculados e o histórico '
          'de métricas do host. Os containers no servidor continuam rodando.',
      confirmLabel: 'Remover servidor',
      tone: SemanticTone.error,
    );
    if (!confirmed || !mounted) return;
    final ok = await ref.read(agentsProvider.notifier).removeAgent(agent.id);
    if (ok && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Servidor ${agent.name} removido.')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final agentsState = ref.watch(agentsProvider);
    final isAdmin = ref.watch(authProvider).isAdmin;
    final visibleAreas = ref.watch(visibleAreasProvider);
    final connectPaths = ServerConnectSlot.resolve(visibleAreas);
    final movePaths = connectPaths
        .where((p) => p.moveExistingLabel != null && p.dialogBuilder != null)
        .toList(growable: false);

    final term = _search.trim().toLowerCase();
    final filtered = agentsState.agents
        .where((a) {
          if (term.isEmpty) return true;
          final hay =
              '${a.name} ${a.host.hostname ?? ''} ${a.host.os ?? ''} ${a.deviceIp ?? ''}'
                  .toLowerCase();
          return hay.contains(term);
        })
        .toList(growable: false);

    final connectedCount = agentsState.agents.where((a) => a.connected).length;
    final pendingCount = agentsState.agents
        .where((a) => a.status == 'pending')
        .length;
    final isCompact = MediaQuery.sizeOf(context).width < 680;

    return RefreshIndicator(
      onRefresh: () => ref.read(agentsProvider.notifier).fetchAgents(),
      child: ListView(
        padding: isCompact
            ? const EdgeInsets.only(bottom: 16)
            : const EdgeInsets.all(16),
        children: <Widget>[
          if (isCompact) ...<Widget>[
            Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    'Servidores remotos',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: context.textTheme.titleLarge?.copyWith(
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                ),
                if (isAdmin) ...<Widget>[
                  const SizedBox(width: 8),
                  FilledButton.icon(
                    onPressed: _openCreateWizard,
                    icon: const Icon(Icons.add, size: 18),
                    label: const Text('Conectar'),
                  ),
                ],
                IconButton(
                  tooltip: 'Atualizar',
                  onPressed: agentsState.loading
                      ? null
                      : () => ref.read(agentsProvider.notifier).fetchAgents(),
                  icon: const Icon(Icons.refresh, size: 20),
                ),
              ],
            ),
            const SizedBox(height: 10),
            UnifiedFilterBar(
              searchController: _searchController,
              searchHint: 'Buscar por nome, hostname, OS ou IP...',
              onSearchChanged: (v) => setState(() => _search = v),
            ),
            const SizedBox(height: 10),
            // Faixa simétrica compacta de 3 contadores (evita card órfão em 2 colunas)
            Wrap(
              spacing: 8,
              runSpacing: 6,
              children: <Widget>[
                SemanticChip(
                  label: '${formatInteger(agentsState.agents.length)} total',
                  tone: SemanticTone.primary,
                  icon: Icons.dns_outlined,
                  compact: true,
                ),
                SemanticChip(
                  label: '${formatInteger(connectedCount)} conectados',
                  tone: SemanticTone.success,
                  icon: Icons.cloud_done_outlined,
                  compact: true,
                ),
                SemanticChip(
                  label: '${formatInteger(pendingCount)} pendentes',
                  tone: pendingCount > 0
                      ? SemanticTone.info
                      : SemanticTone.secondary,
                  icon: Icons.hourglass_top,
                  compact: true,
                ),
              ],
            ),
            const SizedBox(height: 14),
          ] else ...<Widget>[
            SearchFilterHeader(
              title: 'Servidores remotos',
              subtitle:
                  'Agentes conectados à central para Docker, métricas, bancos e deploys.',
              searchController: _searchController,
              searchHint: 'Buscar por nome, hostname, OS ou IP...',
              onSearchChanged: (v) => setState(() => _search = v),
              actions: <Widget>[
                OutlinedButton.icon(
                  onPressed: agentsState.loading
                      ? null
                      : () => ref.read(agentsProvider.notifier).fetchAgents(),
                  icon: const Icon(Icons.refresh, size: 18),
                  label: const Text('Atualizar'),
                ),
                if (isAdmin)
                  FilledButton.icon(
                    onPressed: _openCreateWizard,
                    icon: const Icon(Icons.add),
                    label: const Text('Conectar servidor'),
                  ),
              ],
            ),
            const SizedBox(height: 16),
            ResponsiveGrid(
              xs: 3,
              sm: 3,
              md: 3,
              lg: 3,
              xl: 3,
              children: <Widget>[
                KpiCard(
                  label: 'Servidores cadastrados',
                  value: formatInteger(agentsState.agents.length),
                  icon: Icons.dns_outlined,
                  tone: SemanticTone.primary,
                ),
                KpiCard(
                  label: 'Conectados agora',
                  value: formatInteger(connectedCount),
                  icon: Icons.cloud_done_outlined,
                  tone: SemanticTone.success,
                ),
                KpiCard(
                  label: 'Aguardando instalação',
                  value: formatInteger(pendingCount),
                  icon: Icons.hourglass_top,
                  tone: pendingCount > 0
                      ? SemanticTone.info
                      : SemanticTone.secondary,
                ),
              ],
            ),
            const SizedBox(height: 16),
          ],

          if (agentsState.error != null) ...<Widget>[
            NestedPanel(
              tone: SemanticTone.error,
              child: Text(agentsState.error!),
            ),
            const SizedBox(height: 12),
          ],

          if (agentsState.loading && !agentsState.loaded)
            const LoadingStateView(message: 'Carregando servidores remotos...')
          else if (filtered.isEmpty)
            EmptyStateView(
              title: agentsState.agents.isEmpty
                  ? 'Nenhum servidor remoto cadastrado'
                  : 'Nenhum servidor encontrado',
              description: agentsState.agents.isEmpty
                  ? 'Conecte servidores Linux ou hosts Docker para monitorar containers, métricas e bancos.'
                  : 'Ajuste o termo da busca para ver outros servidores.',
              icon: Icons.dns_outlined,
              action: isAdmin && agentsState.agents.isEmpty
                  ? FilledButton.icon(
                      onPressed: _openCreateWizard,
                      icon: const Icon(Icons.add),
                      label: const Text('Conectar primeiro servidor'),
                    )
                  : null,
            )
          else
            ResponsiveGrid(
              xs: 1,
              sm: 1,
              md: 2,
              lg: 2,
              xl: 2,
              children: filtered
                  .map(
                    (agent) => _AgentCard(
                      agent: agent,
                      isAdmin: isAdmin,
                      movePaths: movePaths,
                      onEdit: () => _openEditAgent(agent),
                      onReissue: () => _reissueEnrollment(agent),
                      onRevoke: () => _revokeAgent(agent),
                      onRemove: () => _removeAgent(agent),
                      onMove: (path) => _openMovePath(agent, path),
                    ),
                  )
                  .toList(growable: false),
            ),
        ],
      ),
    );
  }
}

class _AgentCard extends StatelessWidget {
  const _AgentCard({
    required this.agent,
    required this.isAdmin,
    required this.movePaths,
    required this.onEdit,
    required this.onReissue,
    required this.onRevoke,
    required this.onRemove,
    required this.onMove,
  });

  final AgentView agent;
  final bool isAdmin;
  final List<ServerConnectPathContribution> movePaths;
  final VoidCallback onEdit;
  final VoidCallback onReissue;
  final VoidCallback onRevoke;
  final VoidCallback onRemove;
  final ValueChanged<ServerConnectPathContribution> onMove;

  (String, SemanticTone, IconData) _statusVisual() {
    if (agent.connected || agent.status == 'online') {
      return ('Conectado', SemanticTone.success, Icons.check_circle_outline);
    }
    return switch (agent.status) {
      'pending' => (
        'Aguardando instalação',
        SemanticTone.info,
        Icons.hourglass_top,
      ),
      'revoked' => ('Revogado', SemanticTone.secondary, Icons.block),
      _ => ('Desconectado', SemanticTone.error, Icons.cloud_off),
    };
  }

  @override
  Widget build(BuildContext context) {
    final (statusLabel, statusTone, statusIcon) = _statusVisual();
    final hostSubtitle = <String>[
      if (agent.host.hostname != null && agent.host.hostname!.isNotEmpty)
        agent.host.hostname!,
      if (agent.host.os != null && agent.host.os!.isNotEmpty) agent.host.os!,
      if (agent.host.arch != null && agent.host.arch!.isNotEmpty)
        agent.host.arch!,
    ].join(' · ');

    final showOverflowMenu = isAdmin && !agent.local;
    final primaryActions = <Widget>[
      FilledButton.tonalIcon(
        onPressed: () => context.go(
          '/docker?host=${Uri.encodeQueryComponent(agent.hostKey)}',
        ),
        icon: const Icon(Icons.view_in_ar_outlined, size: 18),
        label: const Text('Docker'),
      ),
      OutlinedButton.icon(
        onPressed: () => context.go(
          '/docker/history?host=${Uri.encodeQueryComponent(agent.hostKey)}',
        ),
        icon: const Icon(Icons.show_chart, size: 18),
        label: const Text('Histórico'),
      ),
      if (isAdmin &&
          !agent.local &&
          agent.status != 'revoked' &&
          agent.deviceId == null)
        ...movePaths.map(
          (path) => OutlinedButton.icon(
            onPressed: () => onMove(path),
            icon: Icon(path.moveExistingIcon ?? path.icon, size: 18),
            label: Text(path.moveExistingLabel!),
          ),
        ),
    ];

    return SectionCard(
      title: agent.name,
      subtitle: hostSubtitle.isNotEmpty
          ? hostSubtitle
          : 'Ainda não se conectou',
      icon: agent.local ? Icons.hub_outlined : Icons.dns_outlined,
      iconTone: statusTone,
      trailing: SemanticChip(
        label: statusLabel,
        tone: statusTone,
        icon: statusIcon,
        compact: true,
      ),
      footer: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Expanded(
            child: SymmetricalActionBar(
              spacing: 8,
              runSpacing: 8,
              primaryFullWidthWhenOdd: true,
              children: primaryActions,
            ),
          ),
          if (showOverflowMenu) ...<Widget>[
            const SizedBox(width: 4),
            PopupMenuButton<String>(
              tooltip: 'Mais ações',
              onSelected: (action) {
                switch (action) {
                  case 'edit':
                    onEdit();
                  case 'reissue':
                    onReissue();
                  case 'revoke':
                    onRevoke();
                  case 'remove':
                    onRemove();
                }
              },
              itemBuilder: (_) => <PopupMenuEntry<String>>[
                const PopupMenuItem<String>(
                  value: 'edit',
                  child: ListTile(
                    dense: true,
                    leading: Icon(Icons.edit_outlined),
                    title: Text('Editar'),
                  ),
                ),
                if (agent.status != 'revoked')
                  const PopupMenuItem<String>(
                    value: 'reissue',
                    child: ListTile(
                      dense: true,
                      leading: Icon(Icons.key_outlined),
                      title: Text('Código de instalação'),
                    ),
                  ),
                if (agent.status != 'revoked')
                  const PopupMenuItem<String>(
                    value: 'revoke',
                    child: ListTile(
                      dense: true,
                      leading: Icon(Icons.block),
                      title: Text('Revogar acesso'),
                    ),
                  ),
                const PopupMenuDivider(),
                const PopupMenuItem<String>(
                  value: 'remove',
                  child: ListTile(
                    dense: true,
                    leading: Icon(Icons.delete_outline),
                    title: Text('Remover servidor'),
                  ),
                ),
              ],
            ),
          ],
        ],
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (agent.local) ...<Widget>[
            const Align(
              alignment: Alignment.centerLeft,
              child: Tooltip(
                message:
                    'Agente local gerenciado pelo docker-compose.yml (netmonitor-agent — ADR 014).',
                child: SemanticChip(
                  label: 'Esta central',
                  tone: SemanticTone.info,
                  icon: Icons.shield_outlined,
                  compact: true,
                ),
              ),
            ),
            const SizedBox(height: 8),
          ],
          KeyValueRow(
            label: 'Docker Engine',
            value: agent.host.docker.available
                ? 'Disponível${agent.host.docker.version != null ? ' (v${agent.host.docker.version})' : ''}'
                : (agent.host.docker.reason ?? 'Indisponível'),
          ),
          KeyValueRow(
            label: 'Docker Compose',
            value: agent.host.compose.available
                ? 'Disponível${agent.host.compose.version != null ? ' (${agent.host.compose.version})' : ''}'
                : (agent.host.compose.reason ?? 'Indisponível'),
          ),
          if (!agent.local && agent.deviceName != null)
            KeyValueRow(
              label: 'Dispositivo VPN',
              value:
                  '${agent.deviceName} (${agent.deviceIp ?? '—'} · ${agent.enforceTunnelIp ? 'exigido pelo túnel' : 'qualquer origem'})',
            ),
          KeyValueRow(
            label: 'Última atividade',
            value:
                '${formatRelativeTime(agent.lastSeenAt)}${agent.version != null ? ' · v${agent.version}' : ''}',
          ),
          if (agent.host.policy.isNotEmpty) ...<Widget>[
            const SizedBox(height: 10),
            Text(
              'Política local (AGENT_ALLOW)',
              style: context.textTheme.labelMedium?.copyWith(
                color: context.colorScheme.onSurfaceVariant,
                fontWeight: FontWeight.w700,
              ),
            ),
            const SizedBox(height: 6),
            Wrap(
              spacing: 6,
              runSpacing: 6,
              children: agent.host.policy
                  .map(
                    (perm) => SemanticChip(
                      label: permissionMeta(perm).label,
                      tone: permissionTone(perm),
                      compact: true,
                    ),
                  )
                  .toList(growable: false),
            ),
          ],
        ],
      ),
    );
  }
}
