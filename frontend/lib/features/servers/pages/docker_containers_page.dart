import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/features/servers/widgets/docker_container_detail_sheet.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Aba "Containers" do Docker (`/docker/containers`).
class DockerContainersPage extends ConsumerStatefulWidget {
  const DockerContainersPage({super.key, this.initialStateFilter});

  final String? initialStateFilter;

  @override
  ConsumerState<DockerContainersPage> createState() =>
      _DockerContainersPageState();
}

class _DockerContainersPageState extends ConsumerState<DockerContainersPage> {
  final TextEditingController _searchController = TextEditingController();
  late ContainerStateFilter _filter;
  String _search = '';
  final Set<String> _collapsedGroups = <String>{};

  @override
  void initState() {
    super.initState();
    _filter = _parseFilter(widget.initialStateFilter);
  }

  @override
  void didUpdateWidget(covariant DockerContainersPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.initialStateFilter != widget.initialStateFilter &&
        widget.initialStateFilter != null) {
      setState(() => _filter = _parseFilter(widget.initialStateFilter));
    }
  }

  ContainerStateFilter _parseFilter(String? value) {
    return switch (value) {
      'running' => ContainerStateFilter.running,
      'stopped' => ContainerStateFilter.stopped,
      _ => ContainerStateFilter.all,
    };
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  Future<void> _executeAction(
    DockerContainerSummary container,
    DockerContainerActionMeta action,
  ) async {
    // Defesa em profundidade ADR 014: container protegido nunca para, reinicia, recria ou remove.
    if (container.isProtected && action.key != DockerContainerActionKey.start) {
      return;
    }

    if (action.requiresConfirm) {
      final confirmed = await showConfirmActionDialog(
        context: context,
        title: '${action.label} ${containerDisplayName(container)}?',
        message: action.hint,
        confirmLabel: action.label,
        tone: action.tone,
      );
      if (!confirmed || !mounted) return;
    }

    final notifier = ref.read(dockerProvider.notifier);
    final api = notifier.apiFor();
    final ok = await notifier.runAction(
      () async {
        switch (action.key) {
          case DockerContainerActionKey.start:
            await api.startContainer(container.id);
          case DockerContainerActionKey.stop:
            await api.stopContainer(container.id);
          case DockerContainerActionKey.restart:
            await api.restartContainer(container.id);
          case DockerContainerActionKey.update:
            await api.updateContainer(container.id);
          case DockerContainerActionKey.remove:
            await api.removeContainer(container.id);
        }
      },
      refresh: action.key == DockerContainerActionKey.update
          ? null
          : notifier.refreshContainers,
    );

    if (ok && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            action.key == DockerContainerActionKey.update
                ? 'Atualização de ${containerDisplayName(container)} iniciada (acompanhe no painel de operações).'
                : '${action.label} concluído para ${containerDisplayName(container)}.',
          ),
        ),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final docker = ref.watch(dockerProvider);
    final isAdmin = ref.watch(authProvider).isAdmin;
    final canLifecycle = isAdmin && docker.allows('lifecycle');
    final canUpdate = isAdmin && docker.allows('update');
    final allContainers = docker.current.containers;
    final counts = containerFilterCounts(allContainers);
    final filtered = filterDockerContainers(allContainers, _search, _filter);
    final groups = groupDockerContainers(filtered, docker.current.metricFor);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        SearchFilterHeader(
          searchController: _searchController,
          searchHint: 'Buscar container, imagem ou projeto compose...',
          onSearchChanged: (v) => setState(() => _search = v),
          filters: <Widget>[
            ChoiceChip(
              selected: _filter == ContainerStateFilter.all,
              label: Text('Todos (${counts[ContainerStateFilter.all]})'),
              onSelected: (_) =>
                  setState(() => _filter = ContainerStateFilter.all),
            ),
            ChoiceChip(
              selected: _filter == ContainerStateFilter.running,
              label: Text(
                'Em execução (${counts[ContainerStateFilter.running]})',
              ),
              onSelected: (_) =>
                  setState(() => _filter = ContainerStateFilter.running),
            ),
            ChoiceChip(
              selected: _filter == ContainerStateFilter.stopped,
              label: Text('Parados (${counts[ContainerStateFilter.stopped]})'),
              onSelected: (_) =>
                  setState(() => _filter = ContainerStateFilter.stopped),
            ),
          ],
        ),
        const SizedBox(height: 14),
        if (filtered.isEmpty)
          EmptyStateView(
            title: allContainers.isEmpty
                ? 'Nenhum container encontrado neste host'
                : 'Nenhum container corresponde ao filtro',
            icon: Icons.view_in_ar_outlined,
          )
        else
          ...groups.map((group) {
            final expanded = !_collapsedGroups.contains(group.key);
            return Padding(
              padding: const EdgeInsets.only(bottom: 12),
              child: Card(
                elevation: 2,
                shape: RoundedRectangleBorder(
                  borderRadius: AppTheme.borderRadius,
                ),
                clipBehavior: Clip.antiAlias,
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: <Widget>[
                    InkWell(
                      onTap: () => setState(() {
                        if (expanded) {
                          _collapsedGroups.add(group.key);
                        } else {
                          _collapsedGroups.remove(group.key);
                        }
                      }),
                      child: Padding(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 16,
                          vertical: 12,
                        ),
                        child: Row(
                          children: <Widget>[
                            Icon(
                              expanded ? Icons.expand_less : Icons.expand_more,
                              color: context.colorScheme.onSurfaceVariant,
                            ),
                            const SizedBox(width: 8),
                            Expanded(
                              child: Text(
                                group.label,
                                style: context.textTheme.titleSmall?.copyWith(
                                  fontWeight: FontWeight.w800,
                                ),
                              ),
                            ),
                            Wrap(
                              spacing: 6,
                              runSpacing: 4,
                              children: <Widget>[
                                SemanticChip(
                                  label:
                                      '${group.running}/${group.containers.length}',
                                  tone: group.running == group.containers.length
                                      ? SemanticTone.success
                                      : SemanticTone.warning,
                                  compact: true,
                                ),
                                SemanticChip(
                                  label:
                                      'CPU ${formatPercent(group.cpuPercent)}',
                                  tone: SemanticTone.primary,
                                  compact: true,
                                ),
                                SemanticChip(
                                  label: formatBinaryBytes(group.memoryBytes),
                                  tone: SemanticTone.info,
                                  compact: true,
                                ),
                              ],
                            ),
                          ],
                        ),
                      ),
                    ),
                    if (expanded) ...<Widget>[
                      const Divider(height: 1),
                      ...group.containers.map((container) {
                        final metric = docker.current.metricFor(container.id);
                        final actions = allowedContainerActions(
                          container: container,
                          canLifecycle: canLifecycle,
                          canUpdate: canUpdate,
                        );
                        return _ContainerRowTile(
                          container: container,
                          metric: metric,
                          actions: actions,
                          busy: docker.actionLoading,
                          onInspect: () => showDockerContainerDetailSheet(
                            context,
                            container: container,
                            onAction: (action) =>
                                _executeAction(container, action),
                          ),
                          onAction: (action) =>
                              _executeAction(container, action),
                        );
                      }),
                    ],
                  ],
                ),
              ),
            );
          }),
      ],
    );
  }
}

class _ContainerRowTile extends StatelessWidget {
  const _ContainerRowTile({
    required this.container,
    required this.metric,
    required this.actions,
    required this.busy,
    required this.onInspect,
    required this.onAction,
  });

  final DockerContainerSummary container;
  final DockerContainerMetrics? metric;
  final List<DockerContainerActionMeta> actions;
  final bool busy;
  final VoidCallback onInspect;
  final ValueChanged<DockerContainerActionMeta> onAction;

  @override
  Widget build(BuildContext context) {
    final stateMeta = containerStateMeta(container.state);
    final primaryAction = actions.isNotEmpty ? actions.first : null;
    final secondaryActions = actions.length > 1
        ? actions.sublist(1)
        : const <DockerContainerActionMeta>[];

    return InkWell(
      onTap: onInspect,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        decoration: BoxDecoration(
          border: Border(
            top: BorderSide(color: context.colorScheme.outlineVariant),
          ),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Wrap(
                        spacing: 8,
                        runSpacing: 4,
                        crossAxisAlignment: WrapCrossAlignment.center,
                        children: <Widget>[
                          Text(
                            containerDisplayName(container),
                            style: context.textTheme.titleSmall?.copyWith(
                              fontWeight: FontWeight.w700,
                            ),
                          ),
                          SemanticChip(
                            label: stateMeta.label,
                            tone: stateMeta.tone,
                            icon: stateMeta.icon,
                            compact: true,
                          ),
                          if (container.isProtected)
                            const Tooltip(
                              message: 'Protegido (com.netmonitor.protected=true): parar, recriar e remover bloqueados.',
                              child: SemanticChip(
                                label: 'Protegido',
                                tone: SemanticTone.info,
                                icon: Icons.shield,
                                compact: true,
                              ),
                            ),
                        ],
                      ),
                      const SizedBox(height: 4),
                      Text(
                        '${container.image} · ${shortDockerId(container.id)}',
                        style: context.textTheme.bodySmall?.copyWith(
                          color: context.colorScheme.onSurfaceVariant,
                          fontFamily: 'monospace',
                        ),
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 8),
                Row(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    if (primaryAction != null)
                      IconButton(
                        tooltip: primaryAction.label,
                        onPressed: busy ? null : () => onAction(primaryAction),
                        icon: Icon(primaryAction.icon),
                      ),
                    if (secondaryActions.isNotEmpty)
                      PopupMenuButton<DockerContainerActionMeta>(
                        tooltip: 'Ações do container',
                        enabled: !busy,
                        onSelected: onAction,
                        itemBuilder: (_) => secondaryActions
                            .map(
                              (a) => PopupMenuItem<DockerContainerActionMeta>(
                                value: a,
                                child: ListTile(
                                  dense: true,
                                  leading: Icon(a.icon),
                                  title: Text(a.label),
                                  subtitle: Text(a.hint, maxLines: 2),
                                ),
                              ),
                            )
                            .toList(growable: false),
                      ),
                  ],
                ),
              ],
            ),
            const SizedBox(height: 6),
            Wrap(
              spacing: 12,
              runSpacing: 4,
              children: <Widget>[
                if (metric != null) ...<Widget>[
                  Text(
                    'CPU: ${formatPercent(metric!.cpu.usagePercent)}',
                    style: context.textTheme.bodySmall?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  Text(
                    'RAM: ${formatBinaryBytes(metric!.memory.usageBytes)}',
                    style: context.textTheme.bodySmall,
                  ),
                  Text(
                    'Net: ↓${formatDecimalBytes(metric!.network.receivedBytes)} ↑${formatDecimalBytes(metric!.network.transmittedBytes)}',
                    style: context.textTheme.bodySmall,
                  ),
                ] else
                  Text(
                    container.status,
                    style: context.textTheme.bodySmall?.copyWith(
                      color: context.colorScheme.onSurfaceVariant,
                    ),
                  ),
                if (container.ports.isNotEmpty)
                  Text(
                    'Portas: ${container.ports.take(3).map((p) => p.formatted).join(', ')}',
                    style: context.textTheme.bodySmall?.copyWith(
                      color: context.colorScheme.onSurfaceVariant,
                    ),
                  ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
