import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/features/servers/widgets/database_restore_dialog.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:url_launcher/url_launcher.dart';

/// Diálogo de execução de deploy (`DeployRunDialog`) com etapas, diff de commits,
/// console ao vivo via SSE (`deploys:log`), aprovação/recusa/rollback e restauração
/// de backup pré-deploy via `showDatabaseRestoreDialog`.
Future<void> showDeployRunDialog(
  BuildContext context, {
  required int runId,
  required bool canAdmin,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Deploy #$runId',
    subtitle: 'Progresso e console ao vivo via SSE',
    maxWidth: 860,
    builder: (_) => _DeployRunBody(initialRunId: runId, canAdmin: canAdmin),
  );
}

class _DeployRunBody extends ConsumerStatefulWidget {
  const _DeployRunBody({required this.initialRunId, required this.canAdmin});
  final int initialRunId;
  final bool canAdmin;

  @override
  ConsumerState<_DeployRunBody> createState() => _DeployRunBodyState();
}

class _DeployRunBodyState extends ConsumerState<_DeployRunBody> {
  late int _runId;
  DeployChangesView? _changes;
  List<DeployBackupView> _backups = const <DeployBackupView>[];
  bool _loadingMeta = false;
  bool _acting = false;

  @override
  void initState() {
    super.initState();
    _runId = widget.initialRunId;
    Future<void>.microtask(_open);
  }

  @override
  void dispose() {
    ref.read(deploysProvider.notifier).closeRunConsole(_runId);
    super.dispose();
  }

  Future<void> _open() async {
    setState(() => _loadingMeta = true);
    final notifier = ref.read(deploysProvider.notifier);
    try {
      await notifier.openRunConsole(_runId);
      final meta = await Future.wait<dynamic>(<Future<dynamic>>[
        notifier
            .runChanges(_runId)
            .catchError(
              (_) => const DeployChangesView(commits: <DeployCommitView>[]),
            ),
        notifier
            .runBackups(_runId)
            .catchError((_) => const <DeployBackupView>[]),
      ]);
      if (!mounted) return;
      setState(() {
        _changes = meta[0] as DeployChangesView;
        _backups = meta[1] as List<DeployBackupView>;
        _loadingMeta = false;
      });
    } catch (_) {
      if (mounted) setState(() => _loadingMeta = false);
    }
  }

  Future<void> _approve(String when) async {
    setState(() => _acting = true);
    try {
      await ref.read(deploysProvider.notifier).approveRun(_runId, when: when);
    } finally {
      if (mounted) setState(() => _acting = false);
    }
  }

  Future<void> _reject() async {
    final reasonCtrl = TextEditingController();
    final reason = await showDialog<String>(
      context: context,
      builder: (dlgCtx) => AlertDialog(
        title: const Text('Recusar deploy'),
        content: TextField(
          controller: reasonCtrl,
          decoration: const InputDecoration(
            labelText: 'Motivo (opcional)',
            hintText: 'ex.: Aguardar fim do horário comercial',
          ),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(dlgCtx).pop(),
            child: const Text('Cancelar'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dlgCtx).pop(reasonCtrl.text.trim()),
            child: const Text('Recusar'),
          ),
        ],
      ),
    );
    reasonCtrl.dispose();
    if (reason == null || !mounted) return;
    setState(() => _acting = true);
    try {
      await ref
          .read(deploysProvider.notifier)
          .rejectRun(_runId, reason: reason);
    } finally {
      if (mounted) setState(() => _acting = false);
    }
  }

  Future<void> _cancel() async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Cancelar deploy #$_runId?',
      message: 'O deploy será removido da fila e não será publicado.',
      confirmLabel: 'Cancelar deploy',
      tone: SemanticTone.warning,
    );
    if (!ok || !mounted) return;
    setState(() => _acting = true);
    try {
      await ref.read(deploysProvider.notifier).cancelRun(_runId);
    } finally {
      if (mounted) setState(() => _acting = false);
    }
  }

  Future<void> _restoreVersion() async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Restaurar versão anterior?',
      message:
          'Publica novamente esta revisão no alvo. '
          'Se houver migrations incompatíveis de banco de dados, restaure o backup pré-deploy também.',
      confirmLabel: 'Restaurar agora',
      tone: SemanticTone.warning,
    );
    if (!ok || !mounted) return;
    setState(() => _acting = true);
    try {
      final newRun = await ref
          .read(deploysProvider.notifier)
          .restoreRun(_runId, when: 'now');
      if (!mounted) return;
      ref.read(deploysProvider.notifier).closeRunConsole(_runId);
      setState(() {
        _runId = newRun.id;
        _acting = false;
      });
      await _open();
    } catch (err) {
      if (!mounted) return;
      setState(() => _acting = false);
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('Erro ao restaurar versão: $err')));
    }
  }

  @override
  Widget build(BuildContext context) {
    final deploys = ref.watch(deploysProvider);
    final run = deploys.runById(_runId);
    final lines = deploys.runConsoles[_runId] ?? const <String>[];

    if (run == null) {
      return const LoadingStateView(
        message: 'Carregando detalhes do deploy...',
      );
    }

    final statusVisual = deployRunStatusMeta(run.status);
    final stages = computeDeployStageStates(run.stage, run.status);
    DeployTargetView? target;
    for (final app in deploys.applications) {
      for (final t in app.targets) {
        if (t.id == run.targetId) target = t;
      }
    }
    final isLive = target?.live?.id == run.id;
    final hasWindow =
        target?.windowStart != null &&
        target?.windowEnd != null &&
        target!.windowStart!.isNotEmpty;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Wrap(
          spacing: 8,
          runSpacing: 8,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: <Widget>[
            SemanticChip(
              label: statusVisual.label,
              tone: statusVisual.tone,
              icon: statusVisual.icon,
            ),
            SemanticChip(
              label: '${run.applicationName} → ${run.targetName}',
              tone: SemanticTone.primary,
            ),
            SemanticChip(
              label: run.shortSha,
              tone: SemanticTone.secondary,
              icon: Icons.commit,
            ),
            if (run.durationMs != null)
              SemanticChip(
                label: formatElapsedMs(run.durationMs),
                tone: SemanticTone.info,
              ),
          ],
        ),
        const SizedBox(height: 10),

        if (run.commitMessage != null && run.commitMessage!.isNotEmpty)
          Text(
            commitFirstLine(run.commitMessage),
            style: context.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.w700,
            ),
          ),
        Text(
          'Criado em ${formatDateTime(run.createdAt)}'
          '${run.triggeredBy != null ? ' · por ${run.triggeredBy}' : ''}'
          '${run.scheduledFor != null ? ' · agendado ${formatTimeUntil(run.scheduledFor)}' : ''}',
          style: context.textTheme.bodySmall?.copyWith(
            color: context.colorScheme.onSurfaceVariant,
          ),
        ),
        const SizedBox(height: 12),

        // Etapas
        Wrap(
          spacing: 6,
          runSpacing: 6,
          children: deployStagesOrder
              .where((s) {
                if (s.key == 'backup') {
                  return target?.backupConnectionId != null ||
                      run.backupId != null;
                }
                if (s.key == 'rollback') {
                  return run.status == 'rolled_back' || run.stage == 'rollback';
                }
                return true;
              })
              .map((stage) {
                final st = stages[stage.key] ?? 'todo';
                final tone = switch (st) {
                  'done' => SemanticTone.success,
                  'current' => SemanticTone.primary,
                  'failed' => SemanticTone.error,
                  _ => SemanticTone.secondary,
                };
                return SemanticChip(
                  label: stage.label,
                  tone: tone,
                  icon: stage.icon,
                  compact: true,
                );
              })
              .toList(growable: false),
        ),

        if (run.error != null && run.error!.isNotEmpty) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(tone: SemanticTone.error, child: Text(run.error!)),
        ],

        // Backups pré-deploy vinculados (integração direta com DatabaseRestoreDialog!)
        if (_backups.isNotEmpty) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            tone: SemanticTone.warning,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Text(
                  'Backup pré-deploy disponível',
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 6),
                ..._backups.map(
                  (b) => Padding(
                    padding: const EdgeInsets.only(bottom: 6),
                    child: Wrap(
                      spacing: 8,
                      runSpacing: 6,
                      alignment: WrapAlignment.spaceBetween,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: <Widget>[
                        Text(
                          '${b.databaseName} · ${formatDateTime(b.startedAt)} (${b.status})',
                          style: context.textTheme.bodySmall,
                        ),
                        if (widget.canAdmin && b.status == 'success')
                          FilledButton.tonalIcon(
                            onPressed: () => showDatabaseRestoreDialog(
                              context,
                              backupId: b.id,
                              sourceDatabaseName: b.databaseName,
                              sourceConnectionId: b.connectionId,
                            ),
                            icon: const Icon(
                              Icons.settings_backup_restore,
                              size: 16,
                            ),
                            label: const Text('Restaurar este banco'),
                          ),
                      ],
                    ),
                  ),
                ),
              ],
            ),
          ),
        ],

        // Diff de commits (O que muda)
        if (!_loadingMeta &&
            _changes != null &&
            _changes!.commits.isNotEmpty) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                Wrap(
                  spacing: 8,
                  runSpacing: 4,
                  alignment: WrapAlignment.spaceBetween,
                  crossAxisAlignment: WrapCrossAlignment.center,
                  children: <Widget>[
                    Text(
                      'O que muda (${countNoun(_changes!.total, 'commit')})',
                      style: context.textTheme.titleSmall?.copyWith(
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    if (_changes!.compareUrl != null)
                      TextButton.icon(
                        onPressed: () => launchUrl(
                          Uri.parse(_changes!.compareUrl!),
                          mode: LaunchMode.externalApplication,
                        ),
                        icon: const Icon(Icons.open_in_new, size: 14),
                        label: const Text('Comparar no GitHub'),
                      ),
                  ],
                ),
                const SizedBox(height: 6),
                ..._changes!.commits
                    .take(8)
                    .map(
                      (c) => Padding(
                        padding: const EdgeInsets.only(bottom: 4),
                        child: Text(
                          '${c.shortSha} · ${commitFirstLine(c.message)}',
                          style: context.textTheme.bodySmall?.copyWith(
                            fontFamily: 'monospace',
                          ),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ),
              ],
            ),
          ),
        ],

        // Ações de administrador
        if (widget.canAdmin) ...<Widget>[
          const SizedBox(height: 12),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: <Widget>[
              if (run.status == 'awaiting_approval') ...<Widget>[
                FilledButton.icon(
                  onPressed: _acting ? null : () => _approve('now'),
                  icon: const Icon(Icons.check),
                  label: const Text('Publicar agora'),
                ),
                if (hasWindow)
                  OutlinedButton.icon(
                    onPressed: _acting ? null : () => _approve('window'),
                    icon: const Icon(Icons.schedule),
                    label: const Text('Publicar na janela'),
                  ),
                TextButton.icon(
                  onPressed: _acting ? null : _reject,
                  icon: const Icon(Icons.close),
                  label: const Text('Recusar'),
                ),
              ],
              if (run.status == 'scheduled' || run.status == 'queued')
                OutlinedButton.icon(
                  onPressed: _acting ? null : _cancel,
                  icon: const Icon(Icons.cancel_outlined),
                  label: const Text('Cancelar deploy'),
                ),
              if (run.status == 'succeeded' && !isLive)
                FilledButton.tonalIcon(
                  onPressed: _acting ? null : _restoreVersion,
                  icon: const Icon(Icons.settings_backup_restore),
                  label: const Text('Restaurar esta versão'),
                ),
            ],
          ),
        ],

        const SizedBox(height: 14),
        CodeViewer(
          title: 'Console do deploy (${lines.length} linhas)',
          code: lines.isEmpty
              ? '(Aguardando saída do agente...)'
              : lines.join('\n'),
          maxHeight: 280,
        ),
      ],
    );
  }
}

/// Diálogo de compilação no servidor (`DeployBuildDialog` — ADR 023).
Future<void> showDeployBuildDialog(
  BuildContext context, {
  required int buildId,
  required bool canAdmin,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Compilação #$buildId',
    subtitle: 'Builder BuildKit isolado · progresso ao vivo via SSE',
    maxWidth: 860,
    builder: (_) =>
        _DeployBuildBody(initialBuildId: buildId, canAdmin: canAdmin),
  );
}

class _DeployBuildBody extends ConsumerStatefulWidget {
  const _DeployBuildBody({
    required this.initialBuildId,
    required this.canAdmin,
  });
  final int initialBuildId;
  final bool canAdmin;

  @override
  ConsumerState<_DeployBuildBody> createState() => _DeployBuildBodyState();
}

class _DeployBuildBodyState extends ConsumerState<_DeployBuildBody> {
  late int _buildId;
  bool _acting = false;

  @override
  void initState() {
    super.initState();
    _buildId = widget.initialBuildId;
    Future<void>.microtask(
      () => ref.read(deploysProvider.notifier).openBuildConsole(_buildId),
    );
  }

  @override
  void dispose() {
    ref.read(deploysProvider.notifier).closeBuildConsole(_buildId);
    super.dispose();
  }

  Future<void> _cancel() async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Cancelar compilação #$_buildId?',
      message: 'A compilação em andamento no builder será interrompida.',
      confirmLabel: 'Cancelar compilação',
      tone: SemanticTone.warning,
    );
    if (!ok || !mounted) return;
    setState(() => _acting = true);
    try {
      await ref.read(deploysProvider.notifier).cancelBuild(_buildId);
    } finally {
      if (mounted) setState(() => _acting = false);
    }
  }

  Future<void> _rebuild() async {
    setState(() => _acting = true);
    try {
      final next = await ref.read(deploysProvider.notifier).rebuild(_buildId);
      if (!mounted) return;
      ref.read(deploysProvider.notifier).closeBuildConsole(_buildId);
      setState(() {
        _buildId = next.id;
        _acting = false;
      });
      await ref.read(deploysProvider.notifier).openBuildConsole(_buildId);
    } catch (err) {
      if (!mounted) return;
      setState(() => _acting = false);
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text('Erro ao recompilar: $err')));
    }
  }

  @override
  Widget build(BuildContext context) {
    final deploys = ref.watch(deploysProvider);
    final build = deploys.builds[_buildId];
    final lines = deploys.buildConsoles[_buildId] ?? const <String>[];

    if (build == null) {
      return const LoadingStateView(message: 'Carregando compilação...');
    }

    final statusVisual = deployBuildStatusMeta(build.status);
    final stages = computeBuildStageStates(build.stage, build.status);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        Wrap(
          spacing: 8,
          runSpacing: 8,
          children: <Widget>[
            SemanticChip(
              label: statusVisual.label,
              tone: statusVisual.tone,
              icon: statusVisual.icon,
            ),
            SemanticChip(label: build.application, tone: SemanticTone.primary),
            SemanticChip(label: build.shortSha, tone: SemanticTone.secondary),
            if (build.currentImage != null && statusVisual.live)
              SemanticChip(label: build.currentImage!, tone: SemanticTone.info),
            if (build.durationMs != null)
              SemanticChip(
                label: formatElapsedMs(build.durationMs),
                tone: SemanticTone.info,
              ),
          ],
        ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 6,
          runSpacing: 6,
          children: buildStagesOrder
              .map((s) {
                final st = stages[s.key] ?? 'todo';
                final tone = switch (st) {
                  'done' => SemanticTone.success,
                  'current' => SemanticTone.primary,
                  'failed' => SemanticTone.error,
                  _ => SemanticTone.secondary,
                };
                return SemanticChip(
                  label: s.label,
                  tone: tone,
                  icon: s.icon,
                  compact: true,
                );
              })
              .toList(growable: false),
        ),
        if (build.error != null && build.error!.isNotEmpty) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(tone: SemanticTone.error, child: Text(build.error!)),
        ],
        if (build.images.isNotEmpty) ...<Widget>[
          const SizedBox(height: 12),
          NestedPanel(
            tone: SemanticTone.success,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  'Imagens armazenadas no Registry',
                  style: context.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w700,
                  ),
                ),
                const SizedBox(height: 6),
                ...build.images.map(
                  (img) => Text(
                    '${img.repository} · ${img.shortDigest} · ${formatDecimalBytes(img.bytes)}',
                    style: context.textTheme.bodySmall?.copyWith(
                      fontFamily: 'monospace',
                    ),
                  ),
                ),
              ],
            ),
          ),
        ],
        if (widget.canAdmin) ...<Widget>[
          const SizedBox(height: 12),
          Wrap(
            spacing: 8,
            children: <Widget>[
              if (statusVisual.live)
                OutlinedButton.icon(
                  onPressed: _acting ? null : _cancel,
                  icon: const Icon(Icons.cancel_outlined),
                  label: const Text('Cancelar compilação'),
                )
              else
                FilledButton.tonalIcon(
                  onPressed: _acting ? null : _rebuild,
                  icon: const Icon(Icons.refresh),
                  label: const Text('Recompilar sem cache'),
                ),
            ],
          ),
        ],
        const SizedBox(height: 14),
        CodeViewer(
          title: 'Console da compilação (${lines.length} linhas)',
          code: lines.isEmpty
              ? '(Aguardando logs do builder...)'
              : lines.join('\n'),
          maxHeight: 280,
        ),
      ],
    );
  }
}

/// Diálogo de segredos de build (BuildKit `--secret`, nunca `--build-arg` — ADR 023).
Future<void> showDeployBuildSecretsDialog(
  BuildContext context, {
  required DeployApplicationView application,
}) {
  return showAdaptiveFormSheet<void>(
    context: context,
    title: 'Segredos de build · ${application.name}',
    subtitle:
        'Injetados somente via BuildKit --secret (nunca expostos na imagem ou nos logs)',
    maxWidth: 640,
    builder: (_) => _BuildSecretsBody(applicationId: application.id),
  );
}

class _BuildSecretsBody extends ConsumerStatefulWidget {
  const _BuildSecretsBody({required this.applicationId});
  final int applicationId;

  @override
  ConsumerState<_BuildSecretsBody> createState() => _BuildSecretsBodyState();
}

class _BuildSecretsBodyState extends ConsumerState<_BuildSecretsBody> {
  final TextEditingController _nameCtrl = TextEditingController();
  final TextEditingController _valueCtrl = TextEditingController();
  List<DeployBuildSecretView> _secrets = const <DeployBuildSecretView>[];
  bool _loading = true;
  bool _saving = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    Future<void>.microtask(_load);
  }

  @override
  void dispose() {
    _nameCtrl.dispose();
    _valueCtrl.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    setState(() {
      _loading = true;
      _error = null;
    });
    try {
      final list = await ref
          .read(deploysProvider.notifier)
          .buildSecrets(widget.applicationId);
      if (!mounted) return;
      setState(() {
        _secrets = list;
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

  Future<void> _save() async {
    final name = _nameCtrl.text.trim();
    final value = _valueCtrl.text;
    if (!isValidEnvKey(name) || value.isEmpty) {
      setState(
        () => _error = 'Informe um nome válido (A-Z, 0-9, _) e um valor.',
      );
      return;
    }
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      final list = await ref
          .read(deploysProvider.notifier)
          .saveBuildSecret(widget.applicationId, name, value);
      if (!mounted) return;
      _nameCtrl.clear();
      _valueCtrl.clear();
      setState(() {
        _secrets = list;
        _saving = false;
      });
    } catch (err) {
      if (!mounted) return;
      setState(() {
        _error = err.toString();
        _saving = false;
      });
    }
  }

  Future<void> _remove(String name) async {
    final ok = await showConfirmActionDialog(
      context: context,
      title: 'Remover segredo $name?',
      message: 'As próximas compilações não receberão mais este segredo.',
      confirmLabel: 'Remover',
      tone: SemanticTone.error,
    );
    if (!ok || !mounted) return;
    await ref
        .read(deploysProvider.notifier)
        .removeBuildSecret(widget.applicationId, name);
    if (mounted) await _load();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        ResponsiveFieldRow(
          breakpoint: 480,
          spacing: 8,
          runSpacing: 10,
          children: <Widget>[
            TextField(
              controller: _nameCtrl,
              decoration: const InputDecoration(
                labelText: 'Nome do segredo',
                hintText: 'NPM_TOKEN',
              ),
            ),
            TextField(
              controller: _valueCtrl,
              obscureText: true,
              decoration: const InputDecoration(labelText: 'Valor'),
            ),
          ],
        ),
        const SizedBox(height: 10),
        Align(
          alignment: Alignment.centerRight,
          child: FilledButton(
            onPressed: _saving ? null : _save,
            child: const Text('Salvar'),
          ),
        ),
        if (_error != null) ...<Widget>[
          const SizedBox(height: 10),
          NestedPanel(tone: SemanticTone.error, child: Text(_error!)),
        ],
        const SizedBox(height: 14),
        if (_loading)
          const LoadingStateView(message: 'Carregando segredos...')
        else if (_secrets.isEmpty)
          const EmptyStateView(title: 'Nenhum segredo de build cadastrado')
        else
          ..._secrets.map(
            (s) => ListTile(
              contentPadding: EdgeInsets.zero,
              leading: const Icon(Icons.vpn_key_outlined),
              title: Text(
                s.name,
                style: const TextStyle(
                  fontFamily: 'monospace',
                  fontWeight: FontWeight.w700,
                ),
              ),
              subtitle: Text('Atualizado em ${formatDateTime(s.updatedAt)}'),
              trailing: IconButton(
                icon: const Icon(Icons.delete_outline),
                onPressed: () => _remove(s.name),
              ),
            ),
          ),
      ],
    );
  }
}
