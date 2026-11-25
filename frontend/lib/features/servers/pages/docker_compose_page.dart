import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/features/servers/utils/docker_presentation.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Aba "Projetos" (Docker Compose) (`/docker/compose`).
///
/// Os projetos saem dos containers ao vivo do host (o snapshot do SSE): os
/// cartões acompanham cada `up`/`stop` sem nova consulta, e a aba não depende
/// de uma requisição ao agente para abrir.
class DockerComposePage extends ConsumerStatefulWidget {
  const DockerComposePage({super.key});

  @override
  ConsumerState<DockerComposePage> createState() => _DockerComposePageState();
}

class _DockerComposePageState extends ConsumerState<DockerComposePage> {
  Future<void> _runComposeAction(
    ComposeProject project,
    String action, {
    String? service,
  }) async {
    if (project.protected && action != 'pull') return;

    final label = switch (action) {
      'pull' => 'Baixar imagens',
      'up' => service != null ? 'Iniciar serviço' : 'Aplicar stack (up)',
      'restart' => 'Reiniciar',
      'stop' => 'Parar',
      'down' => 'Derrubar projeto (down)',
      _ => action,
    };
    final targetLabel = service != null
        ? '${project.name} / $service'
        : project.name;
    final confirmed = await showConfirmActionDialog(
      context: context,
      title: '$label em $targetLabel?',
      message:
          'A operação será executada pelo agente Docker remoto e o progresso aparecerá no painel de operações.',
      confirmLabel: label,
      tone: (action == 'down' || action == 'stop')
          ? SemanticTone.warning
          : SemanticTone.primary,
    );
    if (!confirmed || !mounted) return;

    final ok = await ref
        .read(dockerProvider.notifier)
        .runComposeAction(project.name, action, service: service);
    if (ok && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Operação "$label" enviada para $targetLabel.')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final docker = ref.watch(dockerProvider);
    final auth = ref.watch(authProvider);
    final selectedHost = docker.selectedHost;
    final live = docker.current;
    final projects = composeProjectsOf(live.containers);

    final composeReady =
        (selectedHost?.composeAvailable == true) && docker.allows('compose');
    final canAct = composeReady && auth.isAdmin;
    final canAutomate = auth.roleIn(ProductArea.servers) == UserRole.admin;
    final probeId = docker.selectedHostKey.replaceFirst('agent-', '');

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Text(
          'Projetos Docker Compose',
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: context.textTheme.titleMedium?.copyWith(
            fontWeight: FontWeight.w800,
          ),
        ),
        const SizedBox(height: 10),

        if (!composeReady) ...<Widget>[
          NestedPanel(
            tone: SemanticTone.info,
            child: Text(
              selectedHost?.composeAvailable != true
                  ? 'O plugin docker compose não está disponível neste host.'
                  : 'A permissão "compose" não está liberada em AGENT_ALLOW / LOCAL_AGENT_ALLOW neste agente.',
            ),
          ),
          const SizedBox(height: 12),
        ],

        if (live.status == null)
          const LoadingStateView(
            message: 'Aguardando os containers deste host...',
          )
        else if (!live.available)
          NestedPanel(
            tone: SemanticTone.warning,
            child: Text(
              live.status?.reason ??
                  'A Docker Engine deste host não está acessível agora.',
            ),
          )
        else if (projects.isEmpty)
          const EmptyStateView(
            title: 'Nenhum projeto Compose detectado',
            description:
                'Stacks iniciadas com docker compose aparecerão aqui automaticamente.',
            icon: Icons.description_outlined,
          )
        else
          ResponsiveGrid(
            xs: 1,
            sm: 1,
            md: 2,
            lg: 2,
            xl: 2,
            children: projects
                .map((project) {
                  return SectionCard(
                    title: project.name,
                    subtitle: project.services.isNotEmpty
                        ? '${project.services.length} serviço(s)'
                        : null,
                    icon: Icons.layers_outlined,
                    iconTone: project.running == project.containers
                        ? SemanticTone.success
                        : SemanticTone.warning,
                    trailing: SemanticChip(
                      label: '${project.running}/${project.containers}',
                      tone: project.running == project.containers
                          ? SemanticTone.success
                          : SemanticTone.warning,
                      compact: true,
                    ),
                    footer: SymmetricalActionBar(
                      spacing: 8,
                      runSpacing: 8,
                      primaryFullWidthWhenOdd: true,
                      children: <Widget>[
                        if (canAutomate && !project.protected)
                          FilledButton.tonalIcon(
                            onPressed: () => context.go(
                              '/deploys?setup=1&probe=${Uri.encodeQueryComponent(probeId)}&project=${Uri.encodeQueryComponent(project.name)}',
                            ),
                            icon: const Icon(
                              Icons.rocket_launch_outlined,
                              size: 16,
                            ),
                            label: const Text('Automatizar deploy'),
                          ),
                        if (canAct) ...<Widget>[
                          OutlinedButton.icon(
                            onPressed: () => _runComposeAction(project, 'pull'),
                            icon: const Icon(Icons.download_outlined, size: 16),
                            label: const Text('Baixar imagens'),
                          ),
                          if (!project.protected) ...<Widget>[
                            OutlinedButton.icon(
                              onPressed: () => _runComposeAction(project, 'up'),
                              icon: const Icon(Icons.play_arrow, size: 16),
                              label: const Text('Aplicar'),
                            ),
                            OutlinedButton.icon(
                              onPressed: () =>
                                  _runComposeAction(project, 'restart'),
                              icon: const Icon(Icons.restart_alt, size: 16),
                              label: const Text('Reiniciar'),
                            ),
                            OutlinedButton.icon(
                              onPressed: () =>
                                  _runComposeAction(project, 'stop'),
                              icon: const Icon(Icons.stop_outlined, size: 16),
                              label: const Text('Parar'),
                            ),
                            OutlinedButton.icon(
                              onPressed: () =>
                                  _runComposeAction(project, 'down'),
                              icon: Icon(
                                Icons.delete_sweep_outlined,
                                size: 16,
                                color: context.colorScheme.error,
                              ),
                              label: Text(
                                'Derrubar',
                                style: TextStyle(
                                  color: context.colorScheme.error,
                                ),
                              ),
                            ),
                          ],
                        ],
                      ],
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: <Widget>[
                        if (project.protected) ...<Widget>[
                          const Align(
                            alignment: Alignment.centerLeft,
                            child: SemanticChip(
                              label: 'Instalação NetMonitor',
                              tone: SemanticTone.info,
                              icon: Icons.shield,
                              compact: true,
                            ),
                          ),
                          const SizedBox(height: 8),
                        ],
                        if (project.workingDir != null &&
                            project.workingDir!.isNotEmpty) ...<Widget>[
                          Tooltip(
                            message: project.workingDir!,
                            child: Row(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: <Widget>[
                                Padding(
                                  padding: const EdgeInsets.only(top: 2),
                                  child: Icon(
                                    Icons.folder_outlined,
                                    size: 15,
                                    color: context.colorScheme.onSurfaceVariant,
                                  ),
                                ),
                                const SizedBox(width: 6),
                                Expanded(
                                  child: Text(
                                    project.workingDir!,
                                    maxLines: 2,
                                    overflow: TextOverflow.ellipsis,
                                    style: context.textTheme.bodySmall
                                        ?.copyWith(
                                          fontFamily: 'monospace',
                                          color: context
                                              .colorScheme
                                              .onSurfaceVariant,
                                        ),
                                  ),
                                ),
                              ],
                            ),
                          ),
                          const SizedBox(height: 10),
                        ],
                        ...project.services.map((svc) {
                          return Padding(
                            padding: const EdgeInsets.only(bottom: 8),
                            child: NestedPanel(
                              padding: const EdgeInsets.symmetric(
                                horizontal: 12,
                                vertical: 8,
                              ),
                              child: Row(
                                children: <Widget>[
                                  Expanded(
                                    child: Column(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      children: <Widget>[
                                        Text(
                                          svc.name,
                                          maxLines: 1,
                                          overflow: TextOverflow.ellipsis,
                                          style: context.textTheme.titleSmall
                                              ?.copyWith(
                                                fontWeight: FontWeight.w700,
                                              ),
                                        ),
                                        Text(
                                          '${svc.running}/${svc.containers} em execução',
                                          style: context.textTheme.bodySmall
                                              ?.copyWith(
                                                color: context
                                                    .colorScheme
                                                    .onSurfaceVariant,
                                              ),
                                        ),
                                      ],
                                    ),
                                  ),
                                  if (canAct)
                                    Wrap(
                                      spacing: 4,
                                      children: <Widget>[
                                        IconButton(
                                          tooltip:
                                              'Baixar imagem de ${svc.name}',
                                          icon: const Icon(
                                            Icons.download_outlined,
                                            size: 18,
                                          ),
                                          onPressed: () => _runComposeAction(
                                            project,
                                            'pull',
                                            service: svc.name,
                                          ),
                                        ),
                                        if (!project.protected) ...<Widget>[
                                          IconButton(
                                            tooltip: svc.running > 0
                                                ? 'Parar'
                                                : 'Iniciar',
                                            icon: Icon(
                                              svc.running > 0
                                                  ? Icons.stop
                                                  : Icons.play_arrow,
                                              size: 18,
                                            ),
                                            onPressed: () => _runComposeAction(
                                              project,
                                              svc.running > 0 ? 'stop' : 'up',
                                              service: svc.name,
                                            ),
                                          ),
                                          IconButton(
                                            tooltip: 'Reiniciar ${svc.name}',
                                            icon: const Icon(
                                              Icons.restart_alt,
                                              size: 18,
                                            ),
                                            onPressed: () => _runComposeAction(
                                              project,
                                              'restart',
                                              service: svc.name,
                                            ),
                                          ),
                                        ],
                                      ],
                                    ),
                                ],
                              ),
                            ),
                          );
                        }),
                      ],
                    ),
                  );
                })
                .toList(growable: false),
          ),
      ],
    );
  }
}
