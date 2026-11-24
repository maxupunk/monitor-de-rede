import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:go_router/go_router.dart';
import 'package:netmonitor_app/core/areas/area_models.dart';
import 'package:netmonitor_app/core/auth/area_access.dart';
import 'package:netmonitor_app/core/auth/auth_state.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/features/servers/widgets/deploy_dialogs.dart';
import 'package:netmonitor_app/features/servers/widgets/deploy_env_dialog.dart';
import 'package:netmonitor_app/features/servers/widgets/deploy_run_dialog.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:url_launcher/url_launcher.dart';

/// Página de Deploy Contínuo & Compilações no Servidor (`/deploys` — ADR 021 / ADR 023).
/// Recebe também callbacks do GitHub App redirecionados de `/deploys/github`.
class DeploysPage extends ConsumerStatefulWidget {
  const DeploysPage({super.key, this.queryParams = const <String, String>{}});

  final Map<String, String> queryParams;

  @override
  ConsumerState<DeploysPage> createState() => _DeploysPageState();
}

class _DeploysPageState extends ConsumerState<DeploysPage> {
  bool _guideExpanded = false;
  bool _processingGithubCallback = false;
  bool _handledInitialQuery = false;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_bootstrap);
  }

  @override
  void didUpdateWidget(covariant DeploysPage oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (_handledInitialQuery &&
        widget.queryParams.isNotEmpty &&
        !mapEquals(oldWidget.queryParams, widget.queryParams)) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) _consumeQueryParams();
      });
    }
  }

  Future<void> _bootstrap() async {
    await Future.wait<void>(<Future<void>>[
      ref.read(deploysProvider.notifier).fetchOverview(),
      ref.read(deploysProvider.notifier).fetchRecentBuilds(),
    ]);
    if (!mounted || _handledInitialQuery) return;
    _handledInitialQuery = true;
    await _consumeQueryParams();
  }

  /// Fim do fluxo do GitHub: diz que deu certo e leva à conexão.
  void _announceGithubConnected(String message) {
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(message),
        action: SnackBarAction(
          label: 'Ver conexão',
          onPressed: () {
            if (mounted) showDeployConnectionsDialog(context);
          },
        ),
      ),
    );
  }

  Future<void> _consumeQueryParams() async {
    final q = widget.queryParams;
    if (q.isEmpty) return;
    final canAdmin =
        ref.read(authProvider).roleIn(ProductArea.servers) == UserRole.admin;

    // 1. Callback GitHub App (`?code=&state=` ou `?installation_id=`)
    final code = q['code'];
    final state = q['state'];
    final installationId = int.tryParse(q['installation_id'] ?? '');
    final isGithubCallback =
        (code != null && code.isNotEmpty) ||
        (installationId != null && installationId > 0);
    if (isGithubCallback && !canAdmin) {
      GoRouter.maybeOf(context)?.replace('/deploys');
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text(
            'O retorno do GitHub só é concluído por um administrador da área '
            'Servidores. Peça a ele para conectar o GitHub App.',
          ),
        ),
      );
      return;
    }
    if (canAdmin &&
        code != null &&
        code.isNotEmpty &&
        state != null &&
        state.isNotEmpty) {
      setState(() => _processingGithubCallback = true);
      try {
        final conn = await ref
            .read(deploysProvider.notifier)
            .connectGithub(code, state);
        if (!mounted) return;
        GoRouter.maybeOf(context)?.replace('/deploys');
        if (!conn.installed && conn.installUrl.isNotEmpty) {
          // Passo 2: instalar o app e escolher os repositórios; o GitHub
          // volta com `?installation_id=`.
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text(
                'GitHub App de ${conn.account} criado. Agora instale-o nos '
                'repositórios…',
              ),
            ),
          );
          await launchUrl(
            Uri.parse(conn.installUrl),
            mode: LaunchMode.externalApplication,
            webOnlyWindowName: '_self',
          );
        } else {
          _announceGithubConnected('GitHub App de ${conn.account} conectado.');
        }
      } catch (err) {
        if (mounted) {
          // O código é de uso único: recarregar a página não pode reenviá-lo.
          GoRouter.maybeOf(context)?.replace('/deploys');
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text(
                'Erro ao conectar GitHub App: ${describeError(err)}',
              ),
            ),
          );
        }
      } finally {
        if (mounted) setState(() => _processingGithubCallback = false);
      }
      return;
    }

    if (canAdmin && installationId != null && installationId > 0) {
      setState(() => _processingGithubCallback = true);
      try {
        final conn = await ref
            .read(deploysProvider.notifier)
            .claimGithubInstallation(installationId);
        if (mounted) {
          GoRouter.maybeOf(context)?.replace('/deploys');
          _announceGithubConnected(
            'GitHub App instalado em ${conn.account}: conexão pronta.',
          );
        }
      } catch (err) {
        if (mounted) {
          GoRouter.maybeOf(context)?.replace('/deploys');
          ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(
              content: Text(
                'Erro ao vincular instalação GitHub: ${describeError(err)}',
              ),
            ),
          );
        }
      } finally {
        if (mounted) setState(() => _processingGithubCallback = false);
      }
      return;
    }

    // 2. Deep links `?run=`, `?build=`, `?setup=1&probe=&project=`
    final runId = int.tryParse(q['run'] ?? '');
    if (runId != null && runId > 0) {
      if (!mounted) return;
      GoRouter.maybeOf(context)?.replace('/deploys');
      await showDeployRunDialog(context, runId: runId, canAdmin: canAdmin);
      return;
    }

    final buildId = int.tryParse(q['build'] ?? '');
    if (buildId != null && buildId > 0) {
      if (!mounted) return;
      GoRouter.maybeOf(context)?.replace('/deploys');
      await showDeployBuildDialog(
        context,
        buildId: buildId,
        canAdmin: canAdmin,
      );
      return;
    }

    if (canAdmin && q['setup'] == '1') {
      final probeId = int.tryParse(q['probe'] ?? '');
      final project = q['project'];
      if (!mounted) return;
      GoRouter.maybeOf(context)?.replace('/deploys');
      await showDeploySetupDialog(
        context,
        presetProbeId: probeId,
        presetProject: project,
      );
    }
  }

  Future<void> _removeApplication(DeployApplicationView app) async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Remover aplicação ${app.name}?',
      message:
          'Remove a automação de deploy do NetMonitor. Os containers que já estão rodando no servidor continuam no ar.',
      confirmLabel: 'Remover aplicação',
      tone: SemanticTone.error,
    );
    if (!ok || !mounted) return;
    try {
      await ref.read(deploysProvider.notifier).removeApplication(app.id);
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Erro ao remover aplicação: ${describeError(err)}'),
        ),
      );
    }
  }

  Future<void> _removeTarget(DeployTargetView target) async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Remover alvo ${target.name}?',
      message:
          'O alvo será desvinculado dos próximos deploys. Os containers atuais continuam rodando.',
      confirmLabel: 'Remover alvo',
      tone: SemanticTone.error,
    );
    if (!ok || !mounted) return;
    try {
      await ref.read(deploysProvider.notifier).removeTarget(target.id);
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Erro ao remover alvo: ${describeError(err)}')),
      );
    }
  }

  void _handleGuideAction(DeployGuideAction action, bool canAdmin) {
    switch (action.kind) {
      case 'connect_github':
      case 'install_github':
        if (action.url != null && action.url!.isNotEmpty) {
          launchUrl(
            Uri.parse(action.url!),
            mode: LaunchMode.externalApplication,
          );
        } else {
          showDeployConnectionsDialog(context);
        }
      case 'open_agents':
        context.go('/remote-servers');
      case 'setup':
      case 'add_target':
        showDeploySetupDialog(
          context,
          presetApplicationId: action.applicationId,
        );
      case 'edit_env':
        if (action.targetId != null) {
          for (final app in ref.read(deploysProvider).applications) {
            for (final t in app.targets) {
              if (t.id == action.targetId) {
                showDeployEnvDialog(context, target: t, canAdmin: canAdmin);
                return;
              }
            }
          }
        }
      case 'build_now':
        if (action.applicationId != null) {
          for (final app in ref.read(deploysProvider).applications) {
            if (app.id == action.applicationId) {
              showDeployBuildStartDialog(context, application: app);
              return;
            }
          }
        }
      case 'open_run':
        if (action.runId != null) {
          showDeployRunDialog(
            context,
            runId: action.runId!,
            canAdmin: canAdmin,
          );
        }
    }
  }

  @override
  Widget build(BuildContext context) {
    final deploys = ref.watch(deploysProvider);
    final canAdmin =
        ref.watch(authProvider).roleIn(ProductArea.servers) == UserRole.admin;
    final overview = deploys.overview;
    final counts = overview?.counts ?? const DeployCounts();
    final awaiting = deploys.awaitingApproval;
    final isCompact = MediaQuery.sizeOf(context).width < 680;

    return RefreshIndicator(
      onRefresh: () async {
        await Future.wait<void>(<Future<void>>[
          ref.read(deploysProvider.notifier).fetchOverview(),
          ref.read(deploysProvider.notifier).fetchRecentBuilds(),
        ]);
      },
      child: ListView(
        padding: isCompact
            ? const EdgeInsets.only(bottom: 16)
            : const EdgeInsets.all(16),
        children: <Widget>[
          if (isCompact)
            Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    'Deploy contínuo',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: context.textTheme.titleLarge?.copyWith(
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                ),
                if (canAdmin)
                  FilledButton.icon(
                    style: FilledButton.styleFrom(
                      visualDensity: VisualDensity.compact,
                      padding: const EdgeInsets.symmetric(
                        horizontal: 12,
                        vertical: 8,
                      ),
                    ),
                    onPressed: () => showDeploySetupDialog(context),
                    icon: const Icon(Icons.rocket_launch_outlined, size: 16),
                    label: const Text('Automatizar'),
                  ),
                PopupMenuButton<String>(
                  tooltip: 'Mais ações',
                  onSelected: (cmd) {
                    if (cmd == 'refresh' && !deploys.loading) {
                      ref.read(deploysProvider.notifier).fetchOverview();
                    } else if (cmd == 'github') {
                      showDeployConnectionsDialog(context);
                    }
                  },
                  itemBuilder: (_) => <PopupMenuEntry<String>>[
                    PopupMenuItem<String>(
                      value: 'refresh',
                      enabled: !deploys.loading,
                      child: const ListTile(
                        dense: true,
                        leading: Icon(Icons.refresh),
                        title: Text('Atualizar'),
                      ),
                    ),
                    if (canAdmin)
                      const PopupMenuItem<String>(
                        value: 'github',
                        child: ListTile(
                          dense: true,
                          leading: Icon(Icons.link),
                          title: Text('Conexões GitHub'),
                        ),
                      ),
                  ],
                ),
              ],
            )
          else
            SearchFilterHeader(
              title: 'Deploy contínuo',
              subtitle:
                  'Automação de stacks Docker Compose com compilação isolada, aprovação e rollback.',
              actions: <Widget>[
                OutlinedButton.icon(
                  onPressed: deploys.loading
                      ? null
                      : () =>
                            ref.read(deploysProvider.notifier).fetchOverview(),
                  icon: const Icon(Icons.refresh, size: 18),
                  label: const Text('Atualizar'),
                ),
                if (canAdmin) ...<Widget>[
                  OutlinedButton.icon(
                    onPressed: () => showDeployConnectionsDialog(context),
                    icon: const Icon(Icons.link, size: 18),
                    label: const Text('GitHub'),
                  ),
                  FilledButton.icon(
                    onPressed: () => showDeploySetupDialog(context),
                    icon: const Icon(Icons.rocket_launch_outlined, size: 18),
                    label: const Text('Automatizar deploy'),
                  ),
                ],
              ],
            ),
          const SizedBox(height: 14),

          if (_processingGithubCallback) ...<Widget>[
            const NestedPanel(
              tone: SemanticTone.info,
              child: Row(
                children: <Widget>[
                  SizedBox(
                    width: 18,
                    height: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                  SizedBox(width: 10),
                  Text('Concluindo conexão com o GitHub App...'),
                ],
              ),
            ),
            const SizedBox(height: 12),
          ],

          if (awaiting.isNotEmpty) ...<Widget>[
            NestedPanel(
              tone: SemanticTone.warning,
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text(
                    countNoun(
                      awaiting.length,
                      'deploy aguardando aprovação',
                      'deploys aguardando aprovação',
                    ),
                    style: context.textTheme.titleSmall?.copyWith(
                      fontWeight: FontWeight.w800,
                    ),
                  ),
                  const SizedBox(height: 8),
                  Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    children: awaiting
                        .take(4)
                        .map((run) {
                          return FilledButton.tonalIcon(
                            onPressed: () => showDeployRunDialog(
                              context,
                              runId: run.id,
                              canAdmin: canAdmin,
                            ),
                            icon: const Icon(Icons.pending_actions, size: 16),
                            label: Text(
                              '${run.applicationName} -> ${run.targetName} (${run.shortSha})',
                            ),
                          );
                        })
                        .toList(growable: false),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 12),
          ],

          // Guia interativo do primeiro deploy
          if (overview != null && overview.guide.steps.isNotEmpty) ...<Widget>[
            _buildGuideCard(overview.guide, canAdmin),
            const SizedBox(height: 14),
          ],

          // 4 KPI tiles
          ResponsiveGrid(
            xs: 2,
            sm: 2,
            md: 4,
            lg: 4,
            xl: 4,
            children: <Widget>[
              KpiCard(
                label: 'Aplicações / Alvos',
                value: '${counts.applications} / ${counts.targets}',
                icon: Icons.apps_outlined,
                tone: SemanticTone.primary,
              ),
              KpiCard(
                label: 'Publicados (24h)',
                value: formatInteger(counts.succeeded24h),
                icon: Icons.check_circle_outline,
                tone: SemanticTone.success,
              ),
              KpiCard(
                label: 'Falhas / Rollbacks (24h)',
                value: formatInteger(counts.failed24h),
                icon: Icons.warning_amber_rounded,
                tone: counts.failed24h > 0
                    ? SemanticTone.error
                    : SemanticTone.secondary,
              ),
              KpiCard(
                label: 'Aguardando aprovação',
                value: formatInteger(awaiting.length),
                icon: Icons.pending_actions,
                tone: awaiting.isNotEmpty
                    ? SemanticTone.warning
                    : SemanticTone.info,
              ),
            ],
          ),
          const SizedBox(height: 16),

          if (deploys.error != null) ...<Widget>[
            NestedPanel(tone: SemanticTone.error, child: Text(deploys.error!)),
            const SizedBox(height: 12),
          ],

          if (deploys.loading && overview == null)
            const LoadingStateView(message: 'Carregando deploy contínuo...')
          else if (deploys.applications.isEmpty)
            EmptyStateView(
              title: 'Nenhuma aplicação automatizada',
              description:
                  'Conecte um repositório GitHub e um agente com permissão deploy para automatizar atualizações.',
              icon: Icons.rocket_launch_outlined,
              action: canAdmin
                  ? FilledButton.icon(
                      onPressed: () => showDeploySetupDialog(context),
                      icon: const Icon(Icons.add),
                      label: const Text('Configurar primeiro deploy'),
                    )
                  : null,
            )
          else
            ResponsiveGrid(
              xs: 1,
              sm: 1,
              md: 1,
              lg: 2,
              xl: 2,
              children: deploys.applications
                  .map((app) => _buildApplicationCard(app, deploys, canAdmin))
                  .toList(growable: false),
            ),

          const SizedBox(height: 16),

          // Compilações recentes + Deploys recentes
          ResponsiveGrid(
            xs: 1,
            sm: 1,
            md: 2,
            lg: 2,
            xl: 2,
            children: <Widget>[
              SectionCard(
                title: 'Compilações recentes',
                icon: Icons.build_outlined,
                iconTone: SemanticTone.info,
                child: deploys.recentBuilds.isEmpty
                    ? const EmptyStateView(title: 'Nenhuma compilação recente')
                    : Column(
                        children: deploys.recentBuilds
                            .take(10)
                            .map((b) {
                              final vis = deployBuildStatusMeta(b.status);
                              return Padding(
                                padding: const EdgeInsets.only(bottom: 8),
                                child: ChoiceCard(
                                  title: '${b.application} · ${b.shortSha}',
                                  subtitle:
                                      '${refShortName(b.reference)} · ${formatRelativeTime(b.createdAt)}'
                                      '${b.durationMs != null ? ' · ${formatElapsedMs(b.durationMs)}' : ''}',
                                  badge: vis.label,
                                  badgeTone: vis.tone,
                                  tone: vis.tone,
                                  icon: vis.icon,
                                  onTap: () => showDeployBuildDialog(
                                    context,
                                    buildId: b.id,
                                    canAdmin: canAdmin,
                                  ),
                                ),
                              );
                            })
                            .toList(growable: false),
                      ),
              ),
              SectionCard(
                title: 'Publicações recentes',
                icon: Icons.history,
                iconTone: SemanticTone.primary,
                child: deploys.recentRuns.isEmpty
                    ? const EmptyStateView(title: 'Nenhuma publicação recente')
                    : Column(
                        children: deploys.recentRuns
                            .take(15)
                            .map((run) {
                              final vis = deployRunStatusMeta(run.status);
                              return Padding(
                                padding: const EdgeInsets.only(bottom: 8),
                                child: ChoiceCard(
                                  title:
                                      '${run.applicationName} -> ${run.targetName} (${run.shortSha})',
                                  subtitle:
                                      '${formatRelativeTime(run.createdAt)} · ${commitFirstLine(run.commitMessage)}',
                                  badge: vis.label,
                                  badgeTone: vis.tone,
                                  tone: vis.tone,
                                  icon: vis.icon,
                                  onTap: () => showDeployRunDialog(
                                    context,
                                    runId: run.id,
                                    canAdmin: canAdmin,
                                  ),
                                ),
                              );
                            })
                            .toList(growable: false),
                      ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildGuideCard(DeployGuideView guide, bool canAdmin) {
    final required = guide.steps
        .where((s) => s.key != 'env')
        .toList(growable: false);
    final doneCount = required.where((s) => s.status == 'done').length;
    final total = required.length;

    return Card(
      elevation: 2,
      shape: RoundedRectangleBorder(borderRadius: AppTheme.borderRadius),
      child: Column(
        children: <Widget>[
          ListTile(
            leading: Icon(
              guide.complete ? Icons.check_circle : Icons.route_outlined,
              color: guide.complete
                  ? context.semanticColors.success
                  : context.colorScheme.primary,
            ),
            title: Text(
              guide.complete
                  ? 'Guia de deploy concluído ($doneCount/$total)'
                  : 'Guia do primeiro deploy ($doneCount/$total etapas)',
              style: const TextStyle(fontWeight: FontWeight.w700),
            ),
            trailing: Icon(
              _guideExpanded ? Icons.expand_less : Icons.expand_more,
            ),
            onTap: () => setState(() => _guideExpanded = !_guideExpanded),
          ),
          if (_guideExpanded) ...<Widget>[
            const Divider(height: 1),
            Padding(
              padding: const EdgeInsets.all(14),
              child: Column(
                children: guide.steps
                    .map((step) {
                      final tone = switch (step.status) {
                        'done' => SemanticTone.success,
                        'current' => SemanticTone.primary,
                        'optional' => SemanticTone.info,
                        _ => SemanticTone.secondary,
                      };
                      final action = step.action;
                      final actionAllowed =
                          action != null &&
                          (canAdmin ||
                              action.kind == 'open_run' ||
                              action.kind == 'open_agents');
                      return Padding(
                        padding: const EdgeInsets.only(bottom: 8),
                        child: NestedPanel(
                          tone: tone,
                          child: Wrap(
                            spacing: 8,
                            runSpacing: 6,
                            alignment: WrapAlignment.spaceBetween,
                            crossAxisAlignment: WrapCrossAlignment.center,
                            children: <Widget>[
                              Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: <Widget>[
                                  Text(
                                    step.title,
                                    style: const TextStyle(
                                      fontWeight: FontWeight.w700,
                                    ),
                                  ),
                                  const SizedBox(height: 2),
                                  Text(
                                    step.detail,
                                    style: context.textTheme.bodySmall,
                                  ),
                                ],
                              ),
                              if (actionAllowed)
                                FilledButton.tonal(
                                  onPressed: () =>
                                      _handleGuideAction(action, canAdmin),
                                  child: const Text('Abrir'),
                                ),
                            ],
                          ),
                        ),
                      );
                    })
                    .toList(growable: false),
              ),
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildApplicationCard(
    DeployApplicationView app,
    DeploysState deploys,
    bool canAdmin,
  ) {
    final latestBuild = deploys.latestBuildOf(app.id);
    final isNetmonitorBuilder = app.builder == 'netmonitor';
    final isCompact = MediaQuery.sizeOf(context).width < 520;
    final builderChip = SemanticChip(
      label: isNetmonitorBuilder ? 'Compila no NetMonitor' : 'GitHub Actions',
      tone: isNetmonitorBuilder ? SemanticTone.info : SemanticTone.secondary,
      compact: true,
    );

    final Widget? menuButton = canAdmin
        ? PopupMenuButton<String>(
            onSelected: (action) {
              switch (action) {
                case 'build':
                  showDeployBuildStartDialog(context, application: app);
                case 'secrets':
                  showDeployBuildSecretsDialog(context, application: app);
                case 'workflow':
                  showDeployWorkflowDialog(context, application: app);
                case 'remove':
                  _removeApplication(app);
              }
            },
            itemBuilder: (_) => <PopupMenuEntry<String>>[
              if (isNetmonitorBuilder)
                const PopupMenuItem(
                  value: 'build',
                  child: ListTile(
                    dense: true,
                    leading: Icon(Icons.build_outlined),
                    title: Text('Compilar agora'),
                  ),
                ),
              if (isNetmonitorBuilder)
                const PopupMenuItem(
                  value: 'secrets',
                  child: ListTile(
                    dense: true,
                    leading: Icon(Icons.vpn_key_outlined),
                    title: Text('Segredos de build'),
                  ),
                ),
              const PopupMenuItem(
                value: 'workflow',
                child: ListTile(
                  dense: true,
                  leading: Icon(Icons.code),
                  title: Text('Workflow GitHub Actions'),
                ),
              ),
              const PopupMenuDivider(),
              const PopupMenuItem(
                value: 'remove',
                child: ListTile(
                  dense: true,
                  leading: Icon(Icons.delete_outline),
                  title: Text('Remover aplicação'),
                ),
              ),
            ],
          )
        : null;

    return SectionCard(
      title: app.name,
      subtitle:
          '${app.repository} · ${app.rollout == 'parallel' ? 'todos juntos' : 'um alvo por vez'}',
      icon: Icons.rocket_launch_outlined,
      iconTone: SemanticTone.primary,
      trailing: isCompact
          ? menuButton
          : (menuButton == null
                ? builderChip
                : Wrap(
                    spacing: 6,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: <Widget>[builderChip, menuButton],
                  )),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: <Widget>[
          if (isCompact) ...<Widget>[
            Align(alignment: Alignment.centerLeft, child: builderChip),
            const SizedBox(height: 10),
          ],
          if (canAdmin) ...<Widget>[
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: <Widget>[
                if (isNetmonitorBuilder)
                  FilledButton.tonalIcon(
                    onPressed: (app.buildProbeId == null && app.targets.isEmpty)
                        ? null
                        : () => showDeployBuildStartDialog(
                            context,
                            application: app,
                          ),
                    icon: const Icon(Icons.build_outlined, size: 16),
                    label: const Text('Compilar agora'),
                  )
                else
                  FilledButton.tonalIcon(
                    onPressed: () =>
                        showDeployWorkflowDialog(context, application: app),
                    icon: const Icon(Icons.code, size: 16),
                    label: const Text('Ligar o GitHub'),
                  ),
              ],
            ),
            const SizedBox(height: 10),
          ],

          if (isNetmonitorBuilder && latestBuild != null) ...<Widget>[
            Builder(
              builder: (_) {
                final vis = deployBuildStatusMeta(latestBuild.status);
                return ChoiceCard(
                  title: 'Última compilação · ${latestBuild.shortSha}',
                  subtitle: formatRelativeTime(latestBuild.createdAt),
                  badge: vis.label,
                  badgeTone: vis.tone,
                  tone: vis.tone,
                  icon: vis.icon,
                  onTap: () => showDeployBuildDialog(
                    context,
                    buildId: latestBuild.id,
                    canAdmin: canAdmin,
                  ),
                );
              },
            ),
            const SizedBox(height: 10),
          ],

          ...app.targets.map((target) => _buildTargetRow(target, canAdmin)),
        ],
      ),
    );
  }

  Widget _buildTargetRow(DeployTargetView target, bool canAdmin) {
    final agent = target.agent;
    final agentTone = agent == null || !agent.connected
        ? SemanticTone.error
        : (agent.allowsDeploy ? SemanticTone.success : SemanticTone.warning);
    final window = deployWindowLabel(target.windowStart, target.windowEnd);
    final pending = target.pending;

    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: NestedPanel(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              children: <Widget>[
                Expanded(
                  child: Text(
                    '${target.name} (${target.project})',
                    style: context.textTheme.titleSmall?.copyWith(
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                ),
                if (!target.enabled)
                  const SemanticChip(
                    label: 'Desativado',
                    tone: SemanticTone.secondary,
                    compact: true,
                  ),
                if (canAdmin)
                  IconButton(
                    tooltip: 'Remover alvo',
                    visualDensity: VisualDensity.compact,
                    icon: const Icon(Icons.delete_outline, size: 18),
                    onPressed: () => _removeTarget(target),
                  ),
              ],
            ),
            const SizedBox(height: 6),
            Wrap(
              spacing: 6,
              runSpacing: 6,
              children: <Widget>[
                SemanticChip(
                  label: agent?.name ?? 'Agente removido',
                  tone: agentTone,
                  icon: Icons.dns_outlined,
                  compact: true,
                ),
                SemanticChip(
                  label: refShortName(target.reference),
                  tone: SemanticTone.info,
                  icon: Icons.account_tree_outlined,
                  compact: true,
                ),
                if (target.requiresApproval)
                  const SemanticChip(
                    label: 'Com aprovação',
                    tone: SemanticTone.warning,
                    compact: true,
                  ),
                if (window.isNotEmpty)
                  SemanticChip(
                    label: window,
                    tone: SemanticTone.secondary,
                    icon: Icons.schedule,
                    compact: true,
                  ),
                if (target.live != null)
                  SemanticChip(
                    label: 'No ar: ${target.live!.shortSha}',
                    tone: SemanticTone.success,
                    compact: true,
                  ),
              ],
            ),
            const SizedBox(height: 10),
            if (pending != null) ...<Widget>[
              SizedBox(
                width: double.infinity,
                child: FilledButton.icon(
                  onPressed: () => showDeployRunDialog(
                    context,
                    runId: pending.id,
                    canAdmin: canAdmin,
                  ),
                  icon: const Icon(Icons.pending_actions, size: 16),
                  label: Text(
                    pending.status == 'awaiting_approval'
                        ? 'Revisar e publicar ${pending.shortSha}'
                        : '${deployRunStatusMeta(pending.status).label} (${pending.shortSha})',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
              ),
              const SizedBox(height: 8),
            ],
            SymmetricalActionBar(
              spacing: 8,
              runSpacing: 8,
              primaryFullWidthWhenOdd: true,
              children: <Widget>[
                if (canAdmin) ...<Widget>[
                  FilledButton.tonalIcon(
                    style: FilledButton.styleFrom(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 10,
                        vertical: 10,
                      ),
                    ),
                    onPressed: () =>
                        showDeployPublishDialog(context, target: target),
                    icon: const Icon(Icons.publish, size: 16),
                    label: const Text(
                      'Publicar',
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
                    onPressed: () => showDeployEnvDialog(
                      context,
                      target: target,
                      canAdmin: canAdmin,
                    ),
                    icon: const Icon(Icons.vpn_key_outlined, size: 16),
                    label: const Text(
                      'Variáveis',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ),
                ],
                OutlinedButton.icon(
                  style: OutlinedButton.styleFrom(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 10,
                      vertical: 10,
                    ),
                  ),
                  onPressed: () => showDeployHistoryDialog(
                    context,
                    target: target,
                    canAdmin: canAdmin,
                  ),
                  icon: const Icon(Icons.history, size: 16),
                  label: const Text(
                    'Histórico',
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
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
