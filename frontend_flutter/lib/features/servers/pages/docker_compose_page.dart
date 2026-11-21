import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/features/servers/models/docker_models.dart';
import 'package:netmonitor_app/features/servers/providers/docker_provider.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';

/// Aba "Projetos" (Docker Compose) (`/docker/compose`).
class DockerComposePage extends ConsumerStatefulWidget {
  const DockerComposePage({super.key});

  @override
  ConsumerState<DockerComposePage> createState() => _DockerComposePageState();
}

class _DockerComposePageState extends ConsumerState<DockerComposePage> {
  List<ComposeProject> _projects = const <ComposeProject>[];
  bool _loading = false;
  String? _error;
  String _loadedHostKey = '';

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
  }

  Future<void> _load() async {
    final hostKey = ref.read(dockerProvider).selectedHostKey;
    if (hostKey.isEmpty) return;
    setState(() {
      _loading = true;
      _error = null;
      _loadedHostKey = hostKey;
    });
    try {
      final list = await ref
          .read(dockerProvider.notifier)
          .apiFor(hostKey)
          .composeProjects();
      if (!mounted) return;
      setState(() {
        _projects = list;
        _loading = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = err.toString();
        _loading = false;
      });
    }
  }

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
      message: 'A operação será executada pelo agente Docker remoto e o progresso aparecerá no painel de operações.',
      confirmLabel: label,
      tone: (action == 'down' || action == 'stop')
          ? SemanticTone.warning
          : SemanticTone.primary,
    );
    if (!confirmed || !mounted) return;

    final ok = await ref
        .read(dockerProvider.notifier)
        .runAction(
          () => ref
              .read(dockerProvider.notifier)
              .apiFor()
              .composeAction(project.name, action, service: service),
        );
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

    if (docker.selectedHostKey.isNotEmpty &&
        docker.selectedHostKey != _loadedHostKey &&
        !_loading) {
      WidgetsBinding.instance.addPostFrameCallback((_) => _load());
    }

    final composeReady =
        (selectedHost?.composeAvailable == true) && docker.allows('compose');
    final canAct = composeReady && auth.isAdmin;
    final canAutomate = auth.roleIn(ProductArea.servers) == UserRole.admin;
    final probeId = docker.selectedHostKey.replaceFirst('agent-', '');

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Wrap(
          spacing: 8,
          runSpacing: 8,
          alignment: WrapAlignment.spaceBetween,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            Text(
              'Projetos Docker Compose',
              style: context.textTheme.titleMedium?.copyWith(
                fontWeight: FontWeight.w800,
              ),
            ),
            OutlinedButton.icon(
              onPressed: _loading ? null : _load,
              icon: const Icon(Icons.refresh, size: 18),
              label: const Text('Recarregar projetos'),
            ),
          ],
        ),
        const SizedBox(height: 12),

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

        if (_error != null)
          ErrorStateView(message: _error!, onRetry: _load)
        else if (_loading && _projects.isEmpty)
          const LoadingStateView(message: 'Listando projetos compose...')
        else if (_projects.isEmpty)
          const EmptyStateView(
            title: 'Nenhum projeto Compose detectado',
            description: 'Stacks iniciadas com docker compose aparecerão aqui automaticamente.',
            icon: Icons.description_outlined,
          )
        else
          ResponsiveGrid(
            xs: 1,
            sm: 1,
            md: 2,
            lg: 2,
            xl: 2,
            children: _projects
                .map((project) {
                  return SectionCard(
                    title: project.name,
                    subtitle:
                        project.workingDir ??
                        'Diretório de trabalho não informado',
                    icon: Icons.layers_outlined,
                    iconTone: project.running == project.containers
                        ? SemanticTone.success
                        : SemanticTone.warning,
                    trailing: Wrap(
                      spacing: 6,
                      runSpacing: 4,
                      children: <Widget>[
                        if (project.protected)
                          const SemanticChip(
                            label: 'Instalação NetMonitor',
                            tone: SemanticTone.info,
                            icon: Icons.shield,
                            compact: true,
                          ),
                        SemanticChip(
                          label: '${project.running}/${project.containers}',
                          tone: project.running == project.containers
                              ? SemanticTone.success
                              : SemanticTone.warning,
                          compact: true,
                        ),
                      ],
                    ),
                    footer: Wrap(
                      spacing: 8,
                      runSpacing: 8,
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
                            TextButton(
                              onPressed: () =>
                                  _runComposeAction(project, 'stop'),
                              child: const Text('Parar'),
                            ),
                            TextButton(
                              onPressed: () =>
                                  _runComposeAction(project, 'down'),
                              child: const Text('Derrubar'),
                            ),
                          ],
                        ],
                      ],
                    ),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: project.services
                          .map((svc) {
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
                                              onPressed: () =>
                                                  _runComposeAction(
                                                    project,
                                                    svc.running > 0
                                                        ? 'stop'
                                                        : 'up',
                                                    service: svc.name,
                                                  ),
                                            ),
                                            IconButton(
                                              tooltip: 'Reiniciar ${svc.name}',
                                              icon: const Icon(
                                                Icons.restart_alt,
                                                size: 18,
                                              ),
                                              onPressed: () =>
                                                  _runComposeAction(
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
                          })
                          .toList(growable: false),
                    ),
                  );
                })
                .toList(growable: false),
          ),
      ],
    );
  }
}
