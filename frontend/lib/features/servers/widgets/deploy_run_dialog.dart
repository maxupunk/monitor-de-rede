import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:netmonitor_app/core/utils/error_utils.dart';
import 'package:netmonitor_app/core/utils/formatters.dart';
import 'package:netmonitor_app/features/servers/models/deploy_models.dart';
import 'package:netmonitor_app/features/servers/providers/deploys_provider.dart';
import 'package:netmonitor_app/features/servers/utils/server_utils.dart';
import 'package:netmonitor_app/features/servers/widgets/database_restore_dialog.dart';
import 'package:netmonitor_app/shared/widgets/widgets.dart';
import 'package:netmonitor_app/theme/app_theme.dart';
import 'package:url_launcher/url_launcher.dart';

export 'deploy/deploy_build_dialogs.dart'
    show showDeployBuildDialog, showDeployBuildSecretsDialog;

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
    final reason = await showDialog<String>(
      context: context,
      builder: (_) => const _RejectDeployDialog(),
    );
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
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('Erro ao restaurar versão: ${describeError(err)}'),
        ),
      );
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
              label: '${run.applicationName} -> ${run.targetName}',
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

class _RejectDeployDialog extends StatefulWidget {
  const _RejectDeployDialog();

  @override
  State<_RejectDeployDialog> createState() => _RejectDeployDialogState();
}

class _RejectDeployDialogState extends State<_RejectDeployDialog> {
  final TextEditingController _reasonCtrl = TextEditingController();

  @override
  void dispose() {
    _reasonCtrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Recusar deploy'),
      content: TextField(
        controller: _reasonCtrl,
        autofocus: true,
        decoration: const InputDecoration(
          labelText: 'Motivo (opcional)',
          hintText: 'ex.: Aguardar fim do horário comercial',
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancelar'),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(_reasonCtrl.text.trim()),
          child: const Text('Recusar'),
        ),
      ],
    );
  }
}
