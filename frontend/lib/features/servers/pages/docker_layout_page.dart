import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/realtime/sse_service.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Abas do módulo Docker (7 sub-páginas sob `/docker`).
enum DockerTab {
  overview('/docker', 'Visão geral', Icons.dashboard_outlined),
  containers('/docker/containers', 'Containers', Icons.view_in_ar_outlined),
  compose('/docker/compose', 'Projetos', Icons.description_outlined),
  volumes('/docker/volumes', 'Volumes', Icons.storage_outlined),
  networks('/docker/networks', 'Redes', Icons.hub_outlined),
  images('/docker/images', 'Imagens', Icons.layers_outlined),
  history('/docker/history', 'Histórico', Icons.show_chart);

  const DockerTab(this.path, this.label, this.icon);
  final String path;
  final String label;
  final IconData icon;
}

/// Layout compartilhado de toda a suíte Docker (`/docker/*`).
///
/// Adquire interesse SSE `docker` via `sseService.acquireInterest('docker')`
/// enquanto montado — ZERO polling! Todo host é identificado por `HostKey = agent-<id>`.
class DockerLayoutScaffold extends ConsumerStatefulWidget {
  const DockerLayoutScaffold({
    super.key,
    required this.activeTab,
    required this.child,
    this.hostQueryParam,
  });

  final DockerTab activeTab;
  final Widget child;
  final String? hostQueryParam;

  @override
  ConsumerState<DockerLayoutScaffold> createState() =>
      _DockerLayoutScaffoldState();
}

class _DockerLayoutScaffoldState extends ConsumerState<DockerLayoutScaffold> {
  VoidCallback? _releaseInterest;

  @override
  void initState() {
    super.initState();
    _releaseInterest = ref
        .read(sseServiceProvider.notifier)
        .acquireInterest('docker');
    Future<void>.microtask(_syncHosts);
  }

  @override
  void didUpdateWidget(covariant DockerLayoutScaffold oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.hostQueryParam != widget.hostQueryParam &&
        widget.hostQueryParam != null &&
        widget.hostQueryParam!.isNotEmpty) {
      Future<void>.microtask(
        () => ref
            .read(dockerProvider.notifier)
            .selectHost(widget.hostQueryParam!),
      );
    }
  }

  Future<void> _syncHosts() async {
    await ref
        .read(dockerProvider.notifier)
        .fetchHosts(preferredHostKey: widget.hostQueryParam);
  }

  @override
  void dispose() {
    _releaseInterest?.call();
    super.dispose();
  }

  void _navigateTab(DockerTab tab) {
    final suffix = ref.read(dockerProvider).hostQuerySuffix;
    context.go('${tab.path}$suffix');
  }

  void _onHostChanged(String? nextHostKey) {
    if (nextHostKey == null || nextHostKey.isEmpty) return;
    ref.read(dockerProvider.notifier).selectHost(nextHostKey);
    final suffix = ref.read(dockerProvider).hostQuerySuffix;
    context.go('${widget.activeTab.path}$suffix');
  }

  @override
  Widget build(BuildContext context) {
    final docker = ref.watch(dockerProvider);
    final selectedHost = docker.selectedHost;
    final current = docker.current;
    final ops = docker.selectedHostOperations;
    final isCompact = MediaQuery.sizeOf(context).width < 600;

    final summaryBadge = (selectedHost != null && !selectedHost.online)
        ? const SemanticChip(
            label: 'Desconectado',
            tone: SemanticTone.error,
            icon: Icons.cloud_off,
            compact: true,
          )
        : (current.status != null && !current.available)
        ? const SemanticChip(
            label: 'Engine indisponível',
            tone: SemanticTone.warning,
            icon: Icons.warning_amber_rounded,
            compact: true,
          )
        : current.available
        ? SemanticChip(
            label:
                '${current.runningContainers} ativos · ${current.stoppedContainers} parados',
            tone: current.runningContainers > 0
                ? SemanticTone.success
                : SemanticTone.secondary,
            compact: true,
          )
        : null;

    final desktopMetaChips = <Widget>[
      ?summaryBadge,
      if (selectedHost != null &&
          selectedHost.online &&
          current.status?.engineVersion != null)
        SemanticChip(
          label: 'Engine ${current.status!.engineVersion}',
          tone: SemanticTone.secondary,
          compact: true,
        ),
      if (current.lastUpdatedAt != null)
        SemanticChip(
          label: 'Ao vivo · ${formatRelativeTime(current.lastUpdatedAt)}',
          tone: SemanticTone.success,
          icon: Icons.sensors,
          compact: true,
        ),
    ];

    final hostDropdown = docker.hosts.isNotEmpty
        ? AppDropdownFormField<String>(
            initialValue:
                docker.hosts.any((h) => h.key == docker.selectedHostKey)
                ? docker.selectedHostKey
                : docker.hosts.first.key,
            isDense: true,
            decoration: const InputDecoration(
              labelText: 'Host Docker',
              contentPadding: EdgeInsets.symmetric(
                horizontal: 12,
                vertical: 10,
              ),
            ),
            items: docker.hosts
                .map(
                  (host) => DropdownMenuItem<String>(
                    value: host.key,
                    child: Row(
                      children: <Widget>[
                        Icon(
                          Icons.circle,
                          size: 10,
                          color: host.online
                              ? context.semanticColors.success
                              : context.semanticColors.error,
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: Text(
                            host.isLocal ? '${host.name} (local)' : host.name,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      ],
                    ),
                  ),
                )
                .toList(growable: false),
            onChanged: _onHostChanged,
          )
        : null;

    final canRefresh =
        selectedHost != null && selectedHost.online && !docker.loading;
    final refreshIconButton = IconButton.filledTonal(
      tooltip: 'Atualizar',
      onPressed: canRefresh
          ? () => ref.read(dockerProvider.notifier).refreshAll()
          : null,
      icon: const Icon(Icons.refresh, size: 20),
    );

    return CustomScrollView(
      slivers: <Widget>[
        // Cabeçalho compacto e simétrico (sem margens duplas sobre o AdaptiveAppShell)
        SliverToBoxAdapter(
          child: Padding(
            padding: EdgeInsets.fromLTRB(
              0,
              isCompact ? 2 : 4,
              0,
              isCompact ? 6 : 8,
            ),
            child: isCompact
                ? Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: <Widget>[
                      Row(
                        children: <Widget>[
                          Text(
                            'Docker',
                            style: context.textTheme.titleLarge?.copyWith(
                              fontWeight: FontWeight.w800,
                            ),
                          ),
                          const SizedBox(width: 10),
                          if (summaryBadge != null)
                            Expanded(
                              child: Align(
                                alignment: Alignment.centerRight,
                                child: summaryBadge,
                              ),
                            )
                          else
                            const Spacer(),
                          if (hostDropdown == null) ...<Widget>[
                            const SizedBox(width: 6),
                            refreshIconButton,
                          ],
                        ],
                      ),
                      if (hostDropdown != null) ...<Widget>[
                        const SizedBox(height: 8),
                        Row(
                          crossAxisAlignment: CrossAxisAlignment.center,
                          children: <Widget>[
                            Expanded(child: hostDropdown),
                            const SizedBox(width: 8),
                            refreshIconButton,
                          ],
                        ),
                      ],
                    ],
                  )
                : Row(
                    crossAxisAlignment: CrossAxisAlignment.center,
                    children: <Widget>[
                      Expanded(
                        child: Wrap(
                          spacing: 8,
                          runSpacing: 6,
                          crossAxisAlignment: WrapCrossAlignment.center,
                          children: <Widget>[
                            Text(
                              'Docker',
                              style: context.textTheme.headlineSmall?.copyWith(
                                fontWeight: FontWeight.w800,
                              ),
                            ),
                            ...desktopMetaChips,
                          ],
                        ),
                      ),
                      const SizedBox(width: 12),
                      Row(
                        mainAxisSize: MainAxisSize.min,
                        crossAxisAlignment: CrossAxisAlignment.center,
                        children: <Widget>[
                          if (hostDropdown != null) ...<Widget>[
                            ConstrainedBox(
                              constraints: const BoxConstraints(
                                minWidth: 200,
                                maxWidth: 260,
                              ),
                              child: hostDropdown,
                            ),
                            const SizedBox(width: 8),
                          ],
                          refreshIconButton,
                        ],
                      ),
                    ],
                  ),
          ),
        ),

        // Barra compacta fixa apenas para as 7 abas Docker
        SliverPersistentHeader(
          pinned: true,
          delegate: _DockerTabBarDelegate(
            height: 44,
            child: Material(
              color: context.colorScheme.surface,
              child: Column(
                mainAxisAlignment: MainAxisAlignment.end,
                children: <Widget>[
                  Expanded(
                    child: SingleChildScrollView(
                      scrollDirection: Axis.horizontal,
                      padding: EdgeInsets.zero,
                      child: Row(
                        children: DockerTab.values
                            .map((tab) {
                              final selected = tab == widget.activeTab;
                              final badgeCount = !current.available
                                  ? null
                                  : switch (tab) {
                                      DockerTab.containers =>
                                        current.containers.length,
                                      DockerTab.volumes =>
                                        current.volumes.length,
                                      DockerTab.networks =>
                                        current.networks.length,
                                      DockerTab.images => current.images.length,
                                      _ => null,
                                    };
                              return Padding(
                                padding: const EdgeInsets.only(right: 6),
                                child: ChoiceChip(
                                  selected: selected,
                                  showCheckmark: false,
                                  visualDensity: VisualDensity.compact,
                                  avatar: Icon(tab.icon, size: 16),
                                  label: Text(
                                    badgeCount != null
                                        ? '${tab.label} ($badgeCount)'
                                        : tab.label,
                                  ),
                                  onSelected: (_) => _navigateTab(tab),
                                ),
                              );
                            })
                            .toList(growable: false),
                      ),
                    ),
                  ),
                  const Divider(height: 1),
                ],
              ),
            ),
          ),
        ),

        // Conteúdo principal alinhado às bordas do cabeçalho (sem double-padding)
        SliverPadding(
          padding: EdgeInsets.only(top: isCompact ? 8 : 10, bottom: 16),
          sliver: SliverList(
            delegate: SliverChildListDelegate(<Widget>[
              if (ops.isNotEmpty) ...<Widget>[
                _DockerOperationsPanel(
                  operations: ops,
                  onDismiss: (id) =>
                      ref.read(dockerProvider.notifier).dismissOperation(id),
                ),
                const SizedBox(height: 12),
              ],
              if (docker.error != null) ...<Widget>[
                NestedPanel(
                  tone: SemanticTone.error,
                  child: Text(docker.error!),
                ),
                const SizedBox(height: 12),
              ],
              if (docker.hostsLoaded && docker.hosts.isEmpty)
                const EmptyStateView(
                  title: 'Nenhum host Docker disponível',
                  description:
                      'Conecte o serviço netmonitor-agent no docker-compose.yml ou um servidor remoto.',
                  icon: Icons.view_in_ar_outlined,
                )
              else ...<Widget>[
                if (selectedHost != null &&
                    !selectedHost.online &&
                    widget.activeTab != DockerTab.history) ...<Widget>[
                  NestedPanel(
                    tone: SemanticTone.warning,
                    child: Wrap(
                      spacing: 8,
                      runSpacing: 6,
                      alignment: WrapAlignment.spaceBetween,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: <Widget>[
                        Row(
                          mainAxisSize: MainAxisSize.min,
                          children: <Widget>[
                            Icon(
                              Icons.cloud_off,
                              color: context.semanticColors.warning,
                            ),
                            const SizedBox(width: 10),
                            Flexible(
                              child: Text(
                                'O agente ${selectedHost.name} está desconectado. '
                                'O histórico de métricas continua disponível na aba Histórico.',
                              ),
                            ),
                          ],
                        ),
                        TextButton(
                          onPressed: () => _navigateTab(DockerTab.history),
                          child: const Text('Ver histórico'),
                        ),
                      ],
                    ),
                  ),
                  const SizedBox(height: 12),
                ],
                if (current.status != null &&
                    !current.available &&
                    widget.activeTab != DockerTab.history)
                  _DockerUnavailableCard(
                    reason: current.status!.reason,
                    isLocal: selectedHost?.isLocal ?? false,
                  )
                else
                  widget.child,
              ],
            ]),
          ),
        ),
      ],
    );
  }
}

class _DockerTabBarDelegate extends SliverPersistentHeaderDelegate {
  const _DockerTabBarDelegate({required this.height, required this.child});

  final double height;
  final Widget child;

  @override
  double get minExtent => height;

  @override
  double get maxExtent => height;

  @override
  Widget build(
    BuildContext context,
    double shrinkOffset,
    bool overlapsContent,
  ) {
    return SizedBox.expand(child: child);
  }

  @override
  bool shouldRebuild(covariant _DockerTabBarDelegate oldDelegate) =>
      oldDelegate.height != height || oldDelegate.child != child;
}

class _DockerOperationsPanel extends StatelessWidget {
  const _DockerOperationsPanel({
    required this.operations,
    required this.onDismiss,
  });

  final List<DockerOperationState> operations;
  final ValueChanged<String> onDismiss;

  @override
  Widget build(BuildContext context) {
    return SectionCard(
      title: 'Operações em andamento',
      icon: Icons.sync,
      iconTone: SemanticTone.info,
      child: Column(
        children: operations
            .map((op) {
              final tone = switch (op.state) {
                'succeeded' => SemanticTone.success,
                'failed' => SemanticTone.error,
                _ => SemanticTone.info,
              };
              final lastLine =
                  op.message ??
                  (op.lines.isNotEmpty ? op.lines.last : 'Aguardando saída...');
              return Padding(
                padding: const EdgeInsets.only(bottom: 8),
                child: NestedPanel(
                  tone: tone,
                  child: Row(
                    children: <Widget>[
                      if (op.isRunning)
                        const SizedBox(
                          width: 18,
                          height: 18,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      else
                        Icon(
                          op.state == 'succeeded'
                              ? Icons.check_circle
                              : Icons.error_outline,
                          size: 20,
                        ),
                      const SizedBox(width: 10),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: <Widget>[
                            Text(
                              '${op.kind} · ${op.target}',
                              style: context.textTheme.labelLarge?.copyWith(
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                            const SizedBox(height: 2),
                            Text(
                              lastLine,
                              maxLines: 2,
                              overflow: TextOverflow.ellipsis,
                              style: context.textTheme.bodySmall,
                            ),
                          ],
                        ),
                      ),
                      if (!op.isRunning)
                        IconButton(
                          tooltip: 'Dispensar',
                          icon: const Icon(Icons.close, size: 18),
                          onPressed: () => onDismiss(op.operationId),
                        ),
                    ],
                  ),
                ),
              );
            })
            .toList(growable: false),
      ),
    );
  }
}

class _DockerUnavailableCard extends StatelessWidget {
  const _DockerUnavailableCard({this.reason, required this.isLocal});

  final String? reason;
  final bool isLocal;

  @override
  Widget build(BuildContext context) {
    return SectionCard(
      title: 'Docker Engine indisponível neste host',
      subtitle:
          reason ?? 'O agente não conseguiu acessar /var/run/docker.sock.',
      icon: Icons.warning_amber_rounded,
      iconTone: SemanticTone.warning,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          const Text(
            'Verifique se o serviço Docker está em execução e se o agente tem acesso ao socket:',
          ),
          const SizedBox(height: 12),
          CodeViewer(
            title: isLocal
                ? 'Checklist para o agente local (docker-compose.yml)'
                : 'Checklist para servidor remoto',
            code: isLocal
                ? 'DOCKER_ENABLED=true\n'
                      'volumes:\n'
                      '  - /var/run/docker.sock:/var/run/docker.sock\n'
                      '# Recrie o agente local:\n'
                      'docker compose up -d --force-recreate netmonitor-agent'
                : 'systemctl status docker\n'
                      'systemctl status netmonitor-agent\n'
                      'journalctl -u netmonitor-agent -n 50 --no-pager',
          ),
        ],
      ),
    );
  }
}
